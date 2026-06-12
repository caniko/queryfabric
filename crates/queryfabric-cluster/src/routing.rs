use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use papaya::HashMap as PapayaMap;
use queryfabric_contract::{NodeId, ResourceRef};

use crate::health::{HealthCache, is_delegatable};
use crate::registry::{ClusterRefs, get_handle};

/// Where a resource lives in the federation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLocation<C = NodeId> {
    /// Cluster that currently owns or serves the resource.
    pub cluster_id: C,
    /// Optional domain-specific resource facets such as table names.
    pub tables: Vec<String>,
}

/// Result of partitioning resources into local vs remote groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingDecision<R = ResourceRef, C = NodeId> {
    /// Resources assumed to be local because no remote location is known.
    pub local_ids: Vec<R>,
    /// Resources grouped by remote cluster.
    pub remote: Vec<RemoteGroup<R, C>>,
}

/// A group of resources that all live on the same remote cluster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteGroup<R = ResourceRef, C = NodeId> {
    /// Remote cluster id.
    pub cluster_id: C,
    /// Resources routed to that cluster.
    pub resource_ids: Vec<R>,
}

/// Locates resources across a cluster federation.
pub trait ResourceLocator {
    /// Resource identifier type.
    type ResourceId: Clone + Eq + Hash;
    /// Cluster identifier type.
    type ClusterId: Clone + Eq + Hash;
    /// Location record type stored for each resource.
    type Location: Clone;

    /// Insert or replace the location for one resource id.
    fn upsert(&self, resource_id: Self::ResourceId, location: Self::Location);
    /// Remove one resource id from the locator.
    fn remove(&self, resource_id: &Self::ResourceId);
    /// Resolve many resource ids into local and remote routing groups.
    fn locate_many(
        &self,
        resource_ids: &[Self::ResourceId],
    ) -> RoutingDecision<Self::ResourceId, Self::ClusterId>;

    /// Return the total number of indexed resources.
    fn resource_count(&self) -> usize {
        0
    }
}

/// Lock-free in-memory resource locator backed by papaya.
#[derive(Clone)]
pub struct InMemoryResourceLocator<R = ResourceRef, C = NodeId> {
    index: Arc<PapayaMap<R, ResourceLocation<C>>>,
}

impl<R, C> Default for InMemoryResourceLocator<R, C> {
    fn default() -> Self {
        Self {
            index: Arc::new(PapayaMap::new()),
        }
    }
}

impl<R, C> InMemoryResourceLocator<R, C> {
    /// Construct an empty in-memory locator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Wrap an existing shared index.
    #[must_use]
    pub fn from_index(index: Arc<PapayaMap<R, ResourceLocation<C>>>) -> Self {
        Self { index }
    }

    /// Borrow the shared backing index.
    pub fn index(&self) -> &Arc<PapayaMap<R, ResourceLocation<C>>> {
        &self.index
    }

    /// Consume the locator and return the shared backing index.
    pub fn into_index(self) -> Arc<PapayaMap<R, ResourceLocation<C>>> {
        self.index
    }
}

impl<R, C> ResourceLocator for InMemoryResourceLocator<R, C>
where
    R: Clone + Eq + Hash,
    C: Clone + Eq + Hash,
{
    type ResourceId = R;
    type ClusterId = C;
    type Location = ResourceLocation<C>;

    fn upsert(&self, resource_id: Self::ResourceId, location: Self::Location) {
        let guard = self.index.guard();
        self.index.insert(resource_id, location, &guard);
    }

    fn remove(&self, resource_id: &Self::ResourceId) {
        let guard = self.index.guard();
        self.index.remove(resource_id, &guard);
    }

    fn locate_many(
        &self,
        resource_ids: &[Self::ResourceId],
    ) -> RoutingDecision<Self::ResourceId, Self::ClusterId> {
        resolve_locality(&self.index, resource_ids)
    }

    fn resource_count(&self) -> usize {
        self.index.len()
    }
}

/// Given a set of resource IDs, partition them into local vs remote groups.
///
/// Any resource not found in the index is assumed to be local.
pub fn resolve_locality<R, C>(
    index: &Arc<PapayaMap<R, ResourceLocation<C>>>,
    resource_ids: &[R],
) -> RoutingDecision<R, C>
where
    R: Clone + Eq + Hash,
    C: Clone + Eq + Hash,
{
    let mut local_ids = Vec::new();
    let mut remote_map: HashMap<C, Vec<R>> = HashMap::new();

    let guard = index.guard();
    for resource_id in resource_ids {
        match index.get(resource_id, &guard) {
            Some(loc) => {
                remote_map
                    .entry(loc.cluster_id.clone())
                    .or_default()
                    .push(resource_id.clone());
            }
            None => local_ids.push(resource_id.clone()),
        }
    }

    let remote = remote_map
        .into_iter()
        .map(|(cluster_id, resource_ids)| RemoteGroup {
            cluster_id,
            resource_ids,
        })
        .collect();

    RoutingDecision { local_ids, remote }
}

/// Get the Flight endpoint for a cluster, if the cluster is delegatable and has one.
pub fn get_healthy_endpoint<C>(
    cluster_refs: &ClusterRefs<C>,
    health_cache: &HealthCache<C>,
    cluster_id: C,
) -> Option<(String, bool)>
where
    C: Clone + Eq + Hash,
{
    if !is_delegatable(health_cache, cluster_id.clone()) {
        return None;
    }

    get_handle(cluster_refs, &cluster_id).and_then(|handle| {
        handle
            .flight_endpoint
            .map(|endpoint| (endpoint, handle.flight_tls))
    })
}

#[cfg(test)]
mod tests {
    use queryfabric_contract::Health;

    use super::*;
    use crate::registry::ClusterRemoteHandle;

    fn node_id() -> NodeId {
        NodeId::from(uuid::Uuid::now_v7())
    }

    fn resource_ref() -> ResourceRef {
        ResourceRef::new(uuid::Uuid::now_v7(), uuid::Uuid::now_v7())
    }

    #[test]
    fn in_memory_locator_routes_unknown_resources_as_local() {
        let locator: InMemoryResourceLocator = InMemoryResourceLocator::new();
        let id = resource_ref();

        let decision = locator.locate_many(&[id]);

        assert_eq!(decision.local_ids, vec![id]);
        assert!(decision.remote.is_empty());
    }

    #[test]
    fn in_memory_locator_groups_known_resources_by_cluster() {
        let locator: InMemoryResourceLocator = InMemoryResourceLocator::new();
        let cluster = node_id();
        let a = resource_ref();
        let b = resource_ref();

        locator.upsert(
            a,
            ResourceLocation {
                cluster_id: cluster,
                tables: vec!["a".to_owned()],
            },
        );
        locator.upsert(
            b,
            ResourceLocation {
                cluster_id: cluster,
                tables: vec!["b".to_owned()],
            },
        );

        let decision = locator.locate_many(&[a, b]);

        assert!(decision.local_ids.is_empty());
        assert_eq!(decision.remote.len(), 1);
        assert_eq!(decision.remote[0].cluster_id, cluster);
        assert_eq!(decision.remote[0].resource_ids, vec![a, b]);
    }

    #[test]
    fn duplicate_resource_ids_preserve_current_behavior() {
        let locator: InMemoryResourceLocator = InMemoryResourceLocator::new();
        let cluster = node_id();
        let id = resource_ref();
        locator.upsert(
            id,
            ResourceLocation {
                cluster_id: cluster,
                tables: Vec::new(),
            },
        );

        let decision = locator.locate_many(&[id, id]);

        assert!(decision.local_ids.is_empty());
        assert_eq!(decision.remote.len(), 1);
        assert_eq!(decision.remote[0].resource_ids, vec![id, id]);
    }

    #[test]
    fn healthy_endpoint_requires_delegatable_health_and_endpoint() {
        let refs: ClusterRefs = Arc::new(PapayaMap::new());
        let cache: HealthCache = Arc::new(PapayaMap::new());
        let id = node_id();

        let guard = refs.guard();
        refs.insert(
            id,
            ClusterRemoteHandle {
                cluster_id: id,
                cluster_name: "cluster-a".to_owned(),
                dht_name: "cluster:cluster-a".to_owned(),
                flight_endpoint: Some("cluster-a:50052".to_owned()),
                flight_tls: true,
            },
            &guard,
        );
        drop(guard);

        let health_guard = cache.guard();
        cache.insert(id, Health::Unreachable, &health_guard);
        drop(health_guard);
        assert_eq!(get_healthy_endpoint(&refs, &cache, id), None);

        let health_guard = cache.guard();
        cache.insert(id, Health::Degraded, &health_guard);
        drop(health_guard);
        assert_eq!(
            get_healthy_endpoint(&refs, &cache, id),
            Some(("cluster-a:50052".to_owned(), true))
        );
    }
}
