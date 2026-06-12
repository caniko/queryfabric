//! Generic actor + libp2p cluster federation substrate.
//!
//! This crate contains the reusable building blocks for a federation of
//! nodes: swarm bootstrap ([`swarm`]), Fernet key rotation ([`fernet`]),
//! the DHT-backed cluster registry ([`registry`]), cached health state and
//! the circuit-breaker health monitor ([`health`], [`health_monitor`]), and
//! resource-locality routing ([`routing`]).
//!
//! Everything is generic over the node identifier `C` and resource
//! identifier `R`, defaulting to [`queryfabric_contract::NodeId`] and
//! [`queryfabric_contract::ResourceRef`]. Domain-specific behaviour enters
//! only through the [`ClusterProbe`] trait from `queryfabric-contract`;
//! hosts compose these primitives while keeping database, catalog, and
//! data-plane logic in their own crates.

#![warn(missing_docs)]

/// Fernet key-rotation and encryption helpers.
pub mod fernet;
/// Health-cache types and helpers.
pub mod health;
/// Periodic health-monitor actor and circuit-breaker primitives.
pub mod health_monitor;
/// Message types shared across cluster actors.
pub mod messages;
/// Cluster registry state and DHT naming helpers.
pub mod registry;
/// Resource-locality routing primitives.
pub mod routing;
/// Swarm bootstrap configuration and entry point.
pub mod swarm;

pub use health::{HealthCache, HealthPing, HealthPong, is_delegatable};
pub use health_monitor::{
    CheckAllClusters, CircuitConfig, CircuitPhase, CircuitState, GetHealth, HealthMonitorActor,
    HealthMonitorArgs, ResetCircuitBreaker,
};
pub use messages::{
    CatalogAction, CatalogAnnouncement, CatalogRequest, CatalogResponse, EndpointReply, GetEndpoint,
};
pub use queryfabric_contract::{ClusterProbe, Health, NodeId, ProbeResult, ResourceRef};
pub use registry::{
    ClusterRefs, ClusterRemoteHandle, DhtNaming, HubRegistryState, dht_name, spawn_and_register,
};
pub use routing::{
    InMemoryResourceLocator, RemoteGroup, ResourceLocation, ResourceLocator, RoutingDecision,
    get_healthy_endpoint, resolve_locality,
};
pub use swarm::{SwarmConfig, bootstrap_swarm};
