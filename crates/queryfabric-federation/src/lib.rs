//! Generic federation layer: resource locality, routing, the schema-sync
//! protocol, and host-parametrised hub/cluster actor skeletons.
//!
//! Everything is expressed in [`queryfabric_contract::ResourceRef`] and
//! [`queryfabric_contract::NodeId`]; domain behaviour (catalog persistence,
//! DDL application, search indexing, vocabulary sync) enters only through
//! the [`FederationHost`] trait that the host application implements.
//!
//! # Actor protocol
//!
//! The wire-visible message set, kept stable for hosts migrating from a
//! domain-specific protocol (field-shape mapping from the originating
//! QueryFabric protocol in parentheses):
//!
//! | Message | Direction | Reply | Notes |
//! |---|---|---|---|
//! | [`RegisterCluster`] | cluster → hub | [`RegisterClusterReply`] | `identity` + `federation_password`; reply carries `cluster_id: NodeId` (serialises as a bare UUID), `api_key`, `schema_version`, `schema_ddl`, `accepted`, `message` |
//! | [`HealthPing`] | hub → cluster | [`HealthPong`] | pong fields: `status` (snake_case [`Health`]), `schema_version`, `resource_count` (was `dataset_count`), `uptime_secs`, `storage_ok` (was a backend-named flag), `host_revision` (was a vocabulary version) |
//! | [`SchemaSync`] | hub → cluster | [`SchemaSyncReply`] | migrations carry opaque DDL strings; `target_version`/`applied_version` semantics unchanged |
//! | [`ResourceAnnouncement`] | cluster → hub | `()` | was `DatasetAnnouncement`: `resource_id` (was `dataset_id`), `cluster_id`, `action`, `facets` (was `tables`), optional `catalog_entry` |
//! | [`CatalogRequest`] | hub → cluster | [`CatalogResponse`] | `since` filter; response entries are the host's `CatalogEntry` type |
//! | [`GetFlightEndpoint`] | hub → cluster | [`FlightEndpointReply`] | `endpoint` (`host:port`) + `tls` |
//! | [`SyncAllSchemas`] | local (admin) → hub | `Vec<(String, bool)>` | broadcast schema sync to every registered cluster |
//!
//! The actor skeletons ([`HubActor`], [`ClusterNodeActor`]) are local
//! `piying` actors generic over the host; remote (libp2p DHT) registration
//! stays in the host because `RemoteActor` derivation requires a concrete
//! type. Tests and single-process deployments use [`InMemoryTransport`].

#![warn(missing_docs)]

/// Host seam: the [`FederationHost`] trait and registration result.
pub mod host;
/// Hub-side actor skeleton.
pub mod hub_actor;
/// Resource-locality index, routing decision, and endpoint helpers.
pub mod locality;
/// Federation message types (registration, announcements, endpoints).
pub mod messages;
/// Cluster-node-side actor skeleton.
pub mod node_actor;
/// Schema-sync protocol with opaque DDL bodies.
pub mod schema;
/// Transport abstraction over hub → cluster requests, plus the in-memory
/// implementation and a transport-backed health probe.
pub mod transport;

pub use host::{ClusterRegistration, FederationHost};
pub use hub_actor::{CircuitResetHook, HubActor, HubActorArgs};
pub use locality::{
    ClusterRefs, ClusterRemoteHandle, HealthCache, RemoteGroup, ResourceLocalityIndex,
    ResourceLocation, RoutingDecision, get_healthy_flight_endpoint, is_delegatable,
    resolve_locality,
};
pub use messages::{
    CatalogRequest, CatalogResponse, ClusterIdentity, FlightEndpointReply, GetFlightEndpoint,
    HealthPing, HealthPong, RegisterCluster, RegisterClusterReply, ResourceAction,
    ResourceAnnouncement, SyncAllSchemas,
};
pub use node_actor::{ClusterNodeActor, ClusterNodeArgs};
pub use queryfabric_contract::{Health, NodeId, ResourceRef};
pub use schema::{SchemaMigration, SchemaSync, SchemaSyncReply, apply_schema_sync, ddl_allowed};
pub use transport::{FederationTransport, InMemoryTransport, TransportError, TransportProbe};
