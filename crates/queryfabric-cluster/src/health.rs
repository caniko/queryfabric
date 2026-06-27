use std::hash::Hash;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use papaya::HashMap as PapayaMap;
use queryfabric_contract::{Health, NodeId};
use serde::{Deserialize, Serialize};
use piying::Reply;

/// Cached cluster health state.
pub type HealthCache<C = NodeId> = Arc<PapayaMap<C, Health>>;

/// Health probe request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthPing {
    /// Timestamp from the probing node.
    pub timestamp: DateTime<Utc>,
}

/// Generic health probe response.
#[derive(Debug, Clone, Serialize, Deserialize, Reply)]
pub struct HealthPong {
    /// Current coarse-grained cluster health status.
    pub status: Health,
    /// Schema version served by the cluster.
    pub schema_version: i32,
    /// Number of resources currently advertised by the cluster.
    pub resource_count: u64,
    /// Process uptime in seconds.
    pub uptime_secs: u64,
    /// Whether backing storage passed the cluster's own health checks.
    pub storage_ok: bool,
    /// Host-defined auxiliary revision (e.g. a domain vocabulary version)
    /// echoed back so the hub can detect stale nodes.
    #[serde(default)]
    pub host_revision: i64,
}

/// Check if a cached cluster is healthy enough for request delegation.
pub fn is_delegatable<C>(health_cache: &HealthCache<C>, cluster_id: C) -> bool
where
    C: Eq + Hash,
{
    let guard = health_cache.guard();
    health_cache
        .get(&cluster_id, &guard)
        .is_some_and(|health| health.is_delegatable())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_delegatability_by_variant() {
        assert!(Health::Healthy.is_delegatable());
        assert!(Health::Degraded.is_delegatable());
        assert!(!Health::Unreachable.is_delegatable());
        assert!(!Health::Unknown.is_delegatable());
    }

    #[test]
    fn cached_delegatability_handles_missing_cluster() {
        let cache: HealthCache = Arc::new(PapayaMap::new());
        let cluster_id = NodeId::from(uuid::Uuid::now_v7());
        assert!(!is_delegatable(&cache, cluster_id));

        let guard = cache.guard();
        cache.insert(cluster_id, Health::Healthy, &guard);
        assert!(is_delegatable(&cache, cluster_id));
    }
}
