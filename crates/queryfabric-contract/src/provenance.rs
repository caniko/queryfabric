use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::identity::NodeId;

/// Core provenance activities QueryFabric records against a resource.
///
/// Implemented (persisted and queried) in Phase 05. Hosts attach
/// domain-specific activities via [`DomainActivity`] instead of extending
/// this enum.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "activity", rename_all = "snake_case")]
pub enum Activity {
    Created,
    Deleted {
        reason: String,
    },
    Accessed {
        rows: u64,
    },
    Modified {
        field: String,
    },
    OwnershipTransferred {
        from: Uuid,
        to: Uuid,
    },
    ContentHashRecorded {
        algo: String,
        hash: String,
    },
    FederationFlow {
        nodes: Vec<NodeId>,
        latencies_ms: Vec<u32>,
    },
    BackupAnchor {
        location: String,
        content_hash: String,
    },
}

/// Extension hook for host-specific provenance activities.
///
/// QueryFabric serializes these opaquely alongside [`Activity`] entries; it
/// never inspects the payload beyond the stable kind identifier.
pub trait DomainActivity: Serialize + Send + Sync {
    /// Stable machine-readable identifier for this activity kind.
    fn activity_kind(&self) -> &str;
}
