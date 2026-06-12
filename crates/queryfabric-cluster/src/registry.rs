use std::hash::Hash;
use std::sync::Arc;

use papaya::HashMap as PapayaMap;
use queryfabric_contract::{NodeId, ResourceRef};
use thespis::Actor;
use thespis::actor::{ActorRef, Spawn};
use thespis::error::RegistryError;
use thespis::remote::RemoteActor;

use crate::routing::ResourceLocation;

/// A lightweight handle storing a cluster's name, DHT key, and optional Flight endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterRemoteHandle<C = NodeId> {
    /// Stable cluster identifier.
    pub cluster_id: C,
    /// Human-readable cluster name.
    pub cluster_name: String,
    /// DHT actor name used for remote registration.
    pub dht_name: String,
    /// Internal endpoint for data-plane delegation.
    pub flight_endpoint: Option<String>,
    /// Whether the internal data-plane endpoint uses TLS.
    pub flight_tls: bool,
}

/// Shared map of cluster_id to remote handle.
pub type ClusterRefs<C = NodeId> = Arc<PapayaMap<C, ClusterRemoteHandle<C>>>;

/// Shared cluster/resource registry bookkeeping.
#[derive(Clone)]
pub struct HubRegistryState<R = ResourceRef, C = NodeId> {
    cluster_refs: ClusterRefs<C>,
    resource_index: Arc<PapayaMap<R, ResourceLocation<C>>>,
}

impl<R, C> HubRegistryState<R, C> {
    /// Construct registry state from the shared cluster and resource maps.
    #[must_use]
    pub fn new(
        cluster_refs: ClusterRefs<C>,
        resource_index: Arc<PapayaMap<R, ResourceLocation<C>>>,
    ) -> Self {
        Self {
            cluster_refs,
            resource_index,
        }
    }

    /// Return the shared cluster-reference map.
    pub fn cluster_refs(&self) -> &ClusterRefs<C> {
        &self.cluster_refs
    }

    /// Return the shared resource-location index.
    pub fn resource_index(&self) -> &Arc<PapayaMap<R, ResourceLocation<C>>> {
        &self.resource_index
    }
}

impl<R, C> HubRegistryState<R, C>
where
    R: Eq + Hash,
    C: Clone + Eq + Hash,
{
    /// Insert or replace a cluster handle.
    pub fn upsert_cluster(&self, handle: ClusterRemoteHandle<C>) {
        let guard = self.cluster_refs.guard();
        self.cluster_refs
            .insert(handle.cluster_id.clone(), handle, &guard);
    }

    /// Insert or replace a resource-location entry.
    pub fn upsert_resource(&self, resource_id: R, location: ResourceLocation<C>) {
        let guard = self.resource_index.guard();
        self.resource_index.insert(resource_id, location, &guard);
    }

    /// Remove a resource-location entry if it exists.
    pub fn remove_resource(&self, resource_id: &R) {
        let guard = self.resource_index.guard();
        self.resource_index.remove(resource_id, &guard);
    }
}

/// DHT naming configuration for a hub plus one named actor per cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DhtNaming {
    /// Prefix used to derive per-cluster DHT actor names.
    pub cluster_prefix: &'static str,
    /// Stable DHT actor name for the hub actor.
    pub hub_name: &'static str,
}

impl DhtNaming {
    /// Construct a naming scheme for hub and cluster actor names.
    pub const fn new(cluster_prefix: &'static str, hub_name: &'static str) -> Self {
        Self {
            cluster_prefix,
            hub_name,
        }
    }

    /// Build the cluster DHT name for `cluster_name`.
    pub fn cluster_name(self, cluster_name: &str) -> String {
        dht_name(self.cluster_prefix, cluster_name)
    }

    /// Return the stable hub actor name.
    pub const fn hub_name(self) -> &'static str {
        self.hub_name
    }
}

/// Build a stable DHT actor name from a prefix and cluster name.
pub fn dht_name(prefix: &str, cluster_name: &str) -> String {
    format!("{prefix}:{cluster_name}")
}

/// Spawn a remote actor and register it under a DHT name.
///
/// # Errors
/// Returns any remote-registry error raised while registering the actor.
pub async fn spawn_and_register<A>(
    args: A::Args,
    dht_name: impl Into<Arc<str>>,
) -> Result<ActorRef<A>, RegistryError>
where
    A: Actor + Spawn + RemoteActor + 'static,
{
    let actor_ref = A::spawn(args);
    actor_ref.register(dht_name).await?;
    Ok(actor_ref)
}

pub(crate) fn get_handle<C>(
    cluster_refs: &ClusterRefs<C>,
    cluster_id: &C,
) -> Option<ClusterRemoteHandle<C>>
where
    C: Clone + Eq + Hash,
{
    let guard = cluster_refs.guard();
    cluster_refs.get(cluster_id, &guard).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node_id() -> NodeId {
        NodeId::from(uuid::Uuid::now_v7())
    }

    #[test]
    fn registry_state_updates_cluster_refs() {
        let refs: ClusterRefs = Arc::new(PapayaMap::new());
        let resources = Arc::new(PapayaMap::new());
        let registry: HubRegistryState = HubRegistryState::new(Arc::clone(&refs), resources);
        let cluster_id = node_id();

        registry.upsert_cluster(ClusterRemoteHandle {
            cluster_id,
            cluster_name: "cluster-a".to_owned(),
            dht_name: "cluster:cluster-a".to_owned(),
            flight_endpoint: None,
            flight_tls: false,
        });

        let guard = refs.guard();
        assert!(refs.get(&cluster_id, &guard).is_some());
    }

    #[test]
    fn dht_naming_builds_cluster_and_hub_names() {
        let naming = DhtNaming::new("cluster", "hub");

        assert_eq!(naming.cluster_name("alpha"), "cluster:alpha");
        assert_eq!(naming.cluster_name(""), "cluster:");
        assert_eq!(naming.hub_name(), "hub");
    }

    #[test]
    fn registry_state_updates_resource_index() {
        let refs = Arc::new(PapayaMap::new());
        let resources = Arc::new(PapayaMap::new());
        let registry: HubRegistryState = HubRegistryState::new(refs, Arc::clone(&resources));
        let cluster_id = node_id();
        let resource_id = ResourceRef::new(uuid::Uuid::now_v7(), uuid::Uuid::now_v7());

        registry.upsert_resource(
            resource_id,
            ResourceLocation {
                cluster_id,
                tables: vec!["table".to_owned()],
            },
        );
        let guard = resources.guard();
        assert_eq!(
            resources
                .get(&resource_id, &guard)
                .map(|loc| loc.cluster_id),
            Some(cluster_id)
        );
        drop(guard);

        registry.remove_resource(&resource_id);
        let guard = resources.guard();
        assert!(resources.get(&resource_id, &guard).is_none());
    }
}
