use std::fmt;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Coarse health of a federation node.
///
/// Serialized as snake_case unit variants; this is the wire vocabulary the
/// health protocol (`queryfabric-cluster`) carries in health-probe replies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    /// Node is healthy.
    Healthy,
    /// Node is reachable but degraded.
    Degraded,
    /// Node is currently unreachable.
    Unreachable,
    /// Node health has not yet been established.
    Unknown,
}

impl Health {
    /// Returns true when a node is healthy enough for request delegation.
    pub const fn is_delegatable(self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }
}

impl fmt::Display for Health {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Unreachable => "unreachable",
            Self::Unknown => "unknown",
        };
        f.write_str(value)
    }
}

/// Result of probing a single node.
#[derive(Debug, Clone)]
pub struct ProbeResult<T = ()> {
    /// Health status derived from the probe.
    pub health: Health,
    /// Host-specific probe output carried forward on success.
    pub output: T,
}

/// Host-implemented probe adapter used by the generic health monitor.
///
/// `C` is the node identifier (bound to `NodeId` by the federation crates)
/// and `H` is the registry handle type carrying the node's name and
/// endpoints. The health monitor in `queryfabric-cluster` drives this trait;
/// the host implements it with whatever storage or data-plane checks its
/// domain requires.
#[async_trait]
pub trait ClusterProbe<C: Send + 'static, H: Send + 'static>:
    Clone + Send + Sync + 'static
{
    /// Host-specific data emitted by a successful probe.
    type Output: Default + Send + 'static;

    /// Probe one node and return the resulting health plus probe output.
    async fn probe(&self, node: C, handle: H) -> ProbeResult<Self::Output>;

    /// Hook invoked after a full sweep with the outputs from successful probes.
    async fn on_successful_sweep(&self, _outputs: Vec<Self::Output>) {}
}
