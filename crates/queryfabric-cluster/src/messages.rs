use chrono::{DateTime, Utc};
use piying::Reply;
use queryfabric_contract::{NodeId, ResourceRef};
use serde::{Deserialize, Serialize};

/// Generic endpoint discovery request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetEndpoint;

/// Generic endpoint discovery response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Reply)]
pub struct EndpointReply {
    /// Endpoint string, typically `host:port`.
    pub endpoint: String,
    /// Whether the endpoint expects TLS.
    pub tls: bool,
}

/// Generic catalog request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogRequest {
    /// Return only catalog entries updated since this instant.
    pub since: Option<DateTime<Utc>>,
}

/// Generic catalog response.
#[derive(Debug, Clone, Serialize, Deserialize, Reply)]
pub struct CatalogResponse<T: Send + 'static> {
    /// Resource entries matching the request.
    pub resources: Vec<T>,
    /// Time at which the catalog snapshot was generated.
    pub as_of: DateTime<Utc>,
}

/// Generic resource catalog announcement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogAnnouncement<R = ResourceRef, C = NodeId, T = ()> {
    /// Cluster announcing the catalog change.
    pub cluster_id: C,
    /// Resource identifier affected by the change.
    pub resource_id: R,
    /// Kind of catalog update.
    pub action: CatalogAction,
    /// Additional resource facets, such as table names.
    pub facets: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Optional full catalog entry to accompany the announcement.
    pub catalog_entry: Option<T>,
}

/// Change type reported in a catalog announcement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CatalogAction {
    /// A resource was added.
    Added,
    /// A resource was removed.
    Removed,
    /// A resource was updated in place.
    Updated,
}
