use std::sync::Arc;

use papaya::HashMap as PapayaMap;
use queryfabric_contract::{NodeId, ResourceRef};

/// Maps `ResourceRef → ResourceLocation` for O(1) routing on the hub.
///
/// Populated from catalog replies during health sweeps and
/// [`ResourceAnnouncement`](crate::messages::ResourceAnnouncement) events.
/// Uses [`papaya`] for lock-free concurrent reads from request handlers.
pub type ResourceLocalityIndex = Arc<PapayaMap<ResourceRef, ResourceLocation>>;

/// Where a resource lives in the federation.
pub type ResourceLocation = queryfabric_cluster::ResourceLocation<NodeId>;

/// Result of partitioning a set of resources into local vs. remote groups.
pub type RoutingDecision = queryfabric_cluster::RoutingDecision<ResourceRef, NodeId>;

/// A group of resources that all live on the same remote cluster.
pub type RemoteGroup = queryfabric_cluster::RemoteGroup<ResourceRef, NodeId>;

/// Cached cluster health state, keyed by node.
pub type HealthCache = queryfabric_cluster::HealthCache<NodeId>;

/// Shared map of node id to remote handle.
pub type ClusterRefs = queryfabric_cluster::ClusterRefs<NodeId>;

/// A lightweight handle storing a cluster's name, DHT key, and optional Flight endpoint.
pub type ClusterRemoteHandle = queryfabric_cluster::ClusterRemoteHandle<NodeId>;

/// Given a set of resources, partition them into local vs. remote groups.
///
/// Any resource not found in the index is assumed to be hub-local.
pub fn resolve_locality(
    index: &ResourceLocalityIndex,
    resource_ids: &[ResourceRef],
) -> RoutingDecision {
    queryfabric_cluster::resolve_locality(index, resource_ids)
}

/// Check if a cluster is healthy enough for request delegation.
///
/// Returns `true` for `Healthy` and `Degraded` nodes,
/// `false` for `Unreachable` or unknown nodes.
pub fn is_delegatable(health_cache: &HealthCache, node: NodeId) -> bool {
    queryfabric_cluster::is_delegatable(health_cache, node)
}

/// Get the Flight endpoint for a cluster, if the cluster is healthy and has one.
///
/// Returns `Some((endpoint, tls))` when the cluster is delegatable and has a
/// Flight endpoint.
pub fn get_healthy_flight_endpoint(
    cluster_refs: &ClusterRefs,
    health_cache: &HealthCache,
    node: NodeId,
) -> Option<(String, bool)> {
    queryfabric_cluster::get_healthy_endpoint(cluster_refs, health_cache, node)
}
