use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use papaya::HashMap as PapayaMap;
use piying::actor::ActorRef;
use queryfabric_contract::{ClusterProbe, Health, NodeId, ProbeResult};
use thiserror::Error;

use crate::host::FederationHost;
use crate::locality::{ClusterRefs, ClusterRemoteHandle};
use crate::messages::{FlightEndpointReply, GetFlightEndpoint, HealthPing, HealthPong};
use crate::node_actor::ClusterNodeActor;
use crate::schema::{SchemaSync, SchemaSyncReply};

/// Errors raised while delivering hub → cluster requests.
#[derive(Debug, Error)]
pub enum TransportError {
    /// No node is registered under the requested DHT name.
    #[error("node '{0}' not found")]
    NotFound(String),
    /// The request reached the node but failed.
    #[error("request to '{0}' failed: {1}")]
    Request(String, String),
}

/// Hub-side transport over the cluster control-plane protocol.
///
/// The production implementation resolves `handle.dht_name` through the
/// libp2p DHT (host-side, where the concrete `RemoteActor` lives); the
/// [`InMemoryTransport`] resolves to in-process actors for tests and
/// single-process deployments.
#[async_trait]
pub trait FederationTransport: Send + Sync + 'static {
    /// Push a schema sync to one cluster.
    async fn schema_sync(
        &self,
        handle: &ClusterRemoteHandle,
        sync: SchemaSync,
    ) -> Result<SchemaSyncReply, TransportError>;

    /// Health-probe one cluster.
    async fn health_ping(
        &self,
        handle: &ClusterRemoteHandle,
        ping: HealthPing,
    ) -> Result<HealthPong, TransportError>;

    /// Ask one cluster for its data-plane endpoint.
    async fn get_flight_endpoint(
        &self,
        handle: &ClusterRemoteHandle,
    ) -> Result<FlightEndpointReply, TransportError>;
}

/// In-memory transport: resolves DHT names to in-process
/// [`ClusterNodeActor`]s. No networking; suitable for CI.
pub struct InMemoryTransport<H: FederationHost> {
    nodes: Arc<PapayaMap<String, ActorRef<ClusterNodeActor<H>>>>,
}

impl<H: FederationHost> Clone for InMemoryTransport<H> {
    fn clone(&self) -> Self {
        Self {
            nodes: Arc::clone(&self.nodes),
        }
    }
}

impl<H: FederationHost> Default for InMemoryTransport<H> {
    fn default() -> Self {
        Self {
            nodes: Arc::new(PapayaMap::new()),
        }
    }
}

impl<H: FederationHost> InMemoryTransport<H> {
    /// Construct an empty transport.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a node actor under its DHT name.
    pub fn register(&self, dht_name: impl Into<String>, node: ActorRef<ClusterNodeActor<H>>) {
        let guard = self.nodes.guard();
        self.nodes.insert(dht_name.into(), node, &guard);
    }

    fn lookup(&self, dht_name: &str) -> Result<ActorRef<ClusterNodeActor<H>>, TransportError> {
        let guard = self.nodes.guard();
        self.nodes
            .get(dht_name, &guard)
            .cloned()
            .ok_or_else(|| TransportError::NotFound(dht_name.to_owned()))
    }
}

#[async_trait]
impl<H: FederationHost> FederationTransport for InMemoryTransport<H> {
    async fn schema_sync(
        &self,
        handle: &ClusterRemoteHandle,
        sync: SchemaSync,
    ) -> Result<SchemaSyncReply, TransportError> {
        let node = self.lookup(&handle.dht_name)?;
        node.ask(sync)
            .send()
            .await
            .map_err(|e| TransportError::Request(handle.dht_name.clone(), e.to_string()))
    }

    async fn health_ping(
        &self,
        handle: &ClusterRemoteHandle,
        ping: HealthPing,
    ) -> Result<HealthPong, TransportError> {
        let node = self.lookup(&handle.dht_name)?;
        node.ask(ping)
            .send()
            .await
            .map_err(|e| TransportError::Request(handle.dht_name.clone(), e.to_string()))
    }

    async fn get_flight_endpoint(
        &self,
        handle: &ClusterRemoteHandle,
    ) -> Result<FlightEndpointReply, TransportError> {
        let node = self.lookup(&handle.dht_name)?;
        node.ask(GetFlightEndpoint)
            .send()
            .await
            .map_err(|e| TransportError::Request(handle.dht_name.clone(), e.to_string()))
    }
}

/// Generic transport-backed health probe.
///
/// Pings each cluster through the [`FederationTransport`] and, on success,
/// refreshes the cluster's Flight endpoint in the shared registry — the
/// same flow the originating domain probe performed, minus its catalog and
/// search side effects (hosts layer those on via their own
/// [`ClusterProbe`] impl wrapping this one, or a custom probe).
pub struct TransportProbe<T> {
    transport: Arc<T>,
    cluster_refs: ClusterRefs,
}

impl<T> Clone for TransportProbe<T> {
    fn clone(&self) -> Self {
        Self {
            transport: Arc::clone(&self.transport),
            cluster_refs: Arc::clone(&self.cluster_refs),
        }
    }
}

impl<T> TransportProbe<T> {
    /// Construct a probe over `transport` that refreshes endpoints in
    /// `cluster_refs`.
    pub fn new(transport: Arc<T>, cluster_refs: ClusterRefs) -> Self {
        Self {
            transport,
            cluster_refs,
        }
    }
}

#[async_trait]
impl<T: FederationTransport> ClusterProbe<NodeId, ClusterRemoteHandle> for TransportProbe<T> {
    type Output = Option<HealthPong>;

    async fn probe(&self, node: NodeId, handle: ClusterRemoteHandle) -> ProbeResult<Self::Output> {
        let ping = HealthPing {
            timestamp: Utc::now(),
        };
        match self.transport.health_ping(&handle, ping).await {
            Ok(pong) => {
                if let Ok(reply) = self.transport.get_flight_endpoint(&handle).await
                    && !reply.endpoint.is_empty()
                {
                    let guard = self.cluster_refs.guard();
                    let refreshed = ClusterRemoteHandle {
                        flight_endpoint: Some(reply.endpoint),
                        flight_tls: reply.tls,
                        ..handle
                    };
                    self.cluster_refs.insert(node, refreshed, &guard);
                }
                ProbeResult {
                    health: pong.status,
                    output: Some(pong),
                }
            }
            Err(_) => ProbeResult {
                health: Health::Unreachable,
                output: None,
            },
        }
    }
}
