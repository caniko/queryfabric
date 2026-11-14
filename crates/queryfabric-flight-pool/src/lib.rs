//! Lock-free Arrow Flight client connection pool.
//!
//! [`FlightChannelPool<K>`] caches `tonic::transport::Channel`s keyed by `K`.
//! Reads are lock-free via [`papaya::HashMap`]. Channel creation is not
//! serialized — concurrent callers for the same key may race and create
//! duplicate connections, but the last writer wins harmlessly (tonic
//! channels are cheap to clone/drop).

#![warn(missing_docs)]

use std::fmt;
use std::hash::Hash;
use std::time::{Duration, Instant};

use arrow_flight::flight_service_client::FlightServiceClient;
use papaya::HashMap as PapayaMap;
use secrecy::{ExposeSecret, SecretString};
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Identity};
use tracing::{debug, warn};

/// TLS material used for a mutually authenticated Flight connection.
#[derive(Clone)]
pub struct FlightMtlsConfig {
    domain_name: String,
    ca_certificate_pem: String,
    identity_certificate_pem: String,
    identity_private_key_pem: SecretString,
    fingerprint: [u8; 32],
}

impl fmt::Debug for FlightMtlsConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlightMtlsConfig")
            .field("domain_name", &self.domain_name)
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

impl FlightMtlsConfig {
    /// Build mTLS client material from PEM-encoded certificates and key.
    #[must_use]
    pub fn new(
        domain_name: impl Into<String>,
        ca_certificate_pem: impl Into<String>,
        identity_certificate_pem: impl Into<String>,
        identity_private_key_pem: SecretString,
    ) -> Self {
        let domain_name = domain_name.into();
        let ca_certificate_pem = ca_certificate_pem.into();
        let identity_certificate_pem = identity_certificate_pem.into();
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain_name.as_bytes());
        hasher.update(ca_certificate_pem.as_bytes());
        hasher.update(identity_certificate_pem.as_bytes());
        hasher.update(identity_private_key_pem.expose_secret().as_bytes());

        Self {
            domain_name,
            ca_certificate_pem,
            identity_certificate_pem,
            identity_private_key_pem,
            fingerprint: *hasher.finalize().as_bytes(),
        }
    }

    fn client_config(&self) -> ClientTlsConfig {
        ClientTlsConfig::new()
            .domain_name(&self.domain_name)
            .ca_certificate(Certificate::from_pem(&self.ca_certificate_pem))
            .identity(Identity::from_pem(
                &self.identity_certificate_pem,
                self.identity_private_key_pem.expose_secret(),
            ))
    }
}

/// Transport security for a Flight connection.
#[derive(Debug, Clone)]
pub enum FlightTransport {
    /// Unencrypted transport. Suitable only for isolated development networks.
    Plaintext,
    /// Server-authenticated TLS using the platform trust roots.
    Tls,
    /// Mutually authenticated TLS using explicit trust and identity material.
    MutualTls(FlightMtlsConfig),
}

impl FlightTransport {
    fn scheme(&self) -> &'static str {
        match self {
            Self::Plaintext => "http",
            Self::Tls | Self::MutualTls(_) => "https",
        }
    }

    fn cache_identity(&self) -> [u8; 32] {
        match self {
            Self::Plaintext => *blake3::hash(b"plaintext").as_bytes(),
            Self::Tls => *blake3::hash(b"tls-system-roots").as_bytes(),
            Self::MutualTls(config) => config.fingerprint,
        }
    }
}

/// Errors returned while constructing or connecting a pooled Flight client.
#[derive(Debug, thiserror::Error)]
pub enum FlightPoolError {
    /// The supplied endpoint is not a valid URI authority.
    #[error("invalid Flight endpoint {endpoint:?}: {reason}")]
    InvalidEndpoint {
        /// Endpoint supplied by the caller.
        endpoint: String,
        /// URI parser diagnostic.
        reason: String,
    },
    /// Tonic could not configure or connect the transport.
    #[error(transparent)]
    Transport(#[from] tonic::transport::Error),
}

#[derive(Clone, Debug)]
struct CachedChannel {
    channel: Channel,
    uri: String,
    transport_identity: [u8; 32],
    last_used: Instant,
}

/// A pool of Flight client connections, keyed by `K`.
#[derive(Debug)]
pub struct FlightChannelPool<K>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
{
    channels: PapayaMap<K, CachedChannel>,
    connect_timeout: Duration,
    request_timeout: Duration,
}

impl<K> FlightChannelPool<K>
where
    K: Eq + Hash + Clone + Send + Sync + fmt::Debug + 'static,
{
    /// Create a pool with the given connect timeout and a 5-minute request timeout.
    #[must_use]
    pub fn new(connect_timeout: Duration) -> Self {
        Self {
            channels: PapayaMap::new(),
            connect_timeout,
            request_timeout: Duration::from_secs(300),
        }
    }

    /// Create a pool with custom connect and per-request timeouts.
    #[must_use]
    pub fn with_request_timeout(connect_timeout: Duration, request_timeout: Duration) -> Self {
        Self {
            channels: PapayaMap::new(),
            connect_timeout,
            request_timeout,
        }
    }

    /// True if the pool currently caches no connections.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    /// Get or create a Flight client for `key`.
    ///
    /// `endpoint` should be `host:port`.
    ///
    /// # Errors
    /// Returns any connection-construction or connection-establishment error
    /// reported by `tonic::transport::Channel`.
    pub async fn get_client(
        &self,
        key: K,
        endpoint: &str,
        transport: &FlightTransport,
    ) -> Result<FlightServiceClient<Channel>, FlightPoolError> {
        let uri = format!("{}://{endpoint}", transport.scheme());
        let transport_identity = transport.cache_identity();

        // Fast path: lock-free read.
        {
            let guard = self.channels.guard();
            if let Some(cached) = self.channels.get(&key, &guard)
                && cached.uri == uri
                && cached.transport_identity == transport_identity
            {
                let mut cached = cached.clone();
                cached.last_used = Instant::now();
                self.channels.insert(key.clone(), cached.clone(), &guard);
                return Ok(FlightServiceClient::new(cached.channel));
            }
        }

        debug!(?key, uri = %uri, "Creating new Flight channel");

        let mut endpoint_builder = Channel::from_shared(uri.clone()).map_err(|error| {
            FlightPoolError::InvalidEndpoint {
                endpoint: endpoint.to_owned(),
                reason: error.to_string(),
            }
        })?;
        if let FlightTransport::MutualTls(config) = transport {
            endpoint_builder = endpoint_builder.tls_config(config.client_config())?;
        }
        let channel = endpoint_builder
            .connect_timeout(self.connect_timeout)
            .timeout(self.request_timeout)
            .connect()
            .await?;

        let guard = self.channels.guard();
        self.channels.insert(
            key,
            CachedChannel {
                channel: channel.clone(),
                uri,
                transport_identity,
                last_used: Instant::now(),
            },
            &guard,
        );
        Ok(FlightServiceClient::new(channel))
    }

    /// Evict a cached connection.
    pub async fn remove(&self, key: &K) {
        let guard = self.channels.guard();
        if self.channels.remove(key, &guard).is_some() {
            warn!(?key, "Evicted Flight channel from pool");
        }
    }

    /// Drop connections idle for longer than `max_idle`. Returns the count reaped.
    #[must_use]
    pub fn reap_stale_connections(&self, max_idle: Duration) -> usize {
        let guard = self.channels.guard();
        let stale: Vec<K> = self
            .channels
            .iter(&guard)
            .filter(|(_, cached)| cached.last_used.elapsed() > max_idle)
            .map(|(k, _)| k.clone())
            .collect();

        let count = stale.len();
        for k in &stale {
            self.channels.remove(k, &guard);
            debug!(key = ?k, "Reaped stale Flight channel");
        }
        if count > 0 {
            warn!(reaped = count, "Reaped stale Flight channels from pool");
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn pool_initially_empty() {
        let pool: FlightChannelPool<Uuid> = FlightChannelPool::new(Duration::from_secs(5));
        assert!(pool.is_empty());
    }

    #[tokio::test]
    async fn remove_empty_is_noop() {
        let pool: FlightChannelPool<Uuid> = FlightChannelPool::new(Duration::from_secs(5));
        pool.remove(&Uuid::now_v7()).await;
        assert!(pool.is_empty());
    }

    #[test]
    fn custom_request_timeout() {
        let pool: FlightChannelPool<Uuid> = FlightChannelPool::with_request_timeout(
            Duration::from_secs(5),
            Duration::from_secs(60),
        );
        assert_eq!(pool.connect_timeout, Duration::from_secs(5));
        assert_eq!(pool.request_timeout, Duration::from_secs(60));
    }

    #[test]
    fn mtls_debug_redacts_private_key() {
        let config = FlightMtlsConfig::new(
            "node.internal",
            "ca",
            "cert",
            SecretString::from("private-key"),
        );
        assert!(!format!("{config:?}").contains("private-key"));
    }

    #[tokio::test]
    async fn invalid_endpoint_is_an_error() {
        let pool: FlightChannelPool<Uuid> = FlightChannelPool::new(Duration::from_secs(1));
        let error = pool
            .get_client(Uuid::now_v7(), "not a host", &FlightTransport::Plaintext)
            .await
            .unwrap_err();
        assert!(matches!(error, FlightPoolError::InvalidEndpoint { .. }));
    }
}
