use serde::{Deserialize, Serialize};

use crate::identity::NodeId;

/// Health of a probed federation node.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProbeStatus {
    Healthy,
    Degraded { reason: String },
    Unreachable,
}

/// Health monitor over federation cluster nodes.
///
/// Trait shape defined here; implemented in Phase 03 (`queryfabric-cluster`).
#[async_trait::async_trait]
pub trait ClusterProbe: Send + Sync {
    async fn probe_node(&self, node: NodeId) -> ProbeStatus;
}
