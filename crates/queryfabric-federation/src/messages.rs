use queryfabric_contract::{NodeId, ResourceRef};
use serde::{Deserialize, Serialize};
use piying::Reply;

use crate::schema::SchemaMigration;

pub use queryfabric_cluster::messages::{
    CatalogAction as ResourceAction, CatalogRequest, CatalogResponse,
    EndpointReply as FlightEndpointReply, GetEndpoint as GetFlightEndpoint,
};
pub use queryfabric_cluster::{HealthPing, HealthPong};

/// Identity a cluster presents during registration. All fields are opaque to
/// the federation layer; validation is the host's job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterIdentity {
    /// Human-readable cluster name (unique within the federation).
    pub name: String,
    /// Public control-plane endpoint of the cluster.
    pub endpoint: String,
    /// Control-plane port.
    pub port: i32,
    /// Optional CA certificate (PEM) for TLS to the cluster.
    pub ca_certificate_pem: Option<String>,
    /// Free-text description.
    pub description: Option<String>,
    /// Operating institution.
    pub institution: Option<String>,
    /// Operator contact email.
    pub contact_email: Option<String>,
}

/// Registration request (cluster → hub).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterCluster {
    /// Identity of the registering cluster.
    pub identity: ClusterIdentity,
    // SECURITY: Stays as String (not a secret wrapper) because remote actor
    // messages require Serialize + Deserialize. Do not log RegisterCluster
    // messages at debug level.
    /// Shared federation password authorising the registration.
    pub federation_password: String,
}

/// Registration reply (hub → cluster).
#[derive(Debug, Clone, Serialize, Deserialize, Reply)]
pub struct RegisterClusterReply {
    /// Assigned node id (serialises as a bare UUID).
    pub cluster_id: NodeId,
    // SECURITY: api_key is the raw secret returned only once at registration.
    // Do not log this struct at info/debug level.
    /// API key for subsequent control-plane calls.
    pub api_key: String,
    /// Current federation schema version.
    pub schema_version: i32,
    /// Migrations needed to reach `schema_version` from scratch.
    pub schema_ddl: Vec<SchemaMigration>,
    /// Whether the registration was accepted.
    pub accepted: bool,
    /// Human-readable status message.
    pub message: String,
}

/// Resource announcement (cluster → hub, fire-and-forget).
///
/// The generalisation of the originating domain's dataset announcement:
/// `resource_id` was `dataset_id`, `facets` was `tables`.
pub type ResourceAnnouncement<T = serde_json::Value> =
    queryfabric_cluster::CatalogAnnouncement<ResourceRef, NodeId, T>;

/// Trigger a schema-sync broadcast to every registered cluster
/// (local admin message, not remote).
#[derive(Debug, Clone, Copy)]
pub struct SyncAllSchemas;
