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
use tonic::transport::Channel;
use tracing::{debug, warn};

/// A pool of Flight client connections, keyed by `K`.
#[derive(Debug)]
pub struct FlightChannelPool<K>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
{
    channels: PapayaMap<K, (Channel, Instant)>,
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
    /// `endpoint` should be `host:port`. When `tls` is true, the connection
    /// uses `https://`; otherwise `http://`.
    ///
    /// # Errors
    /// Returns any connection-construction or connection-establishment error
    /// reported by `tonic::transport::Channel`.
    pub async fn get_client(
        &self,
        key: K,
        endpoint: &str,
        tls: bool,
    ) -> Result<FlightServiceClient<Channel>, tonic::transport::Error> {
        // Fast path: lock-free read.
        {
            let guard = self.channels.guard();
            if let Some((channel, _)) = self.channels.get(&key, &guard) {
                let channel = channel.clone();
                self.channels
                    .insert(key.clone(), (channel.clone(), Instant::now()), &guard);
                return Ok(FlightServiceClient::new(channel));
            }
        }

        let scheme = if tls { "https" } else { "http" };
        let uri = format!("{scheme}://{endpoint}");

        debug!(?key, uri = %uri, "Creating new Flight channel");

        let channel = Channel::from_shared(uri)
            .expect("endpoint must form a valid URI (scheme://host:port)")
            .connect_timeout(self.connect_timeout)
            .timeout(self.request_timeout)
            .connect()
            .await?;

        let guard = self.channels.guard();
        self.channels
            .insert(key, (channel.clone(), Instant::now()), &guard);
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
            .filter(|(_, (_, last_used))| last_used.elapsed() > max_idle)
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
}
