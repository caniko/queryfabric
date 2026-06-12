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
    Restored,
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

impl Activity {
    /// Stable low-cardinality tag identifying the activity kind.
    ///
    /// Matches the serde `activity` tag; persistent stores index on this
    /// (e.g. a `LowCardinality(String)` column) so it must never change for
    /// an existing variant.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Deleted { .. } => "deleted",
            Self::Restored => "restored",
            Self::Accessed { .. } => "accessed",
            Self::Modified { .. } => "modified",
            Self::OwnershipTransferred { .. } => "ownership_transferred",
            Self::ContentHashRecorded { .. } => "content_hash_recorded",
            Self::FederationFlow { .. } => "federation_flow",
            Self::BackupAnchor { .. } => "backup_anchor",
        }
    }
}

/// Extension hook for host-specific provenance activities.
///
/// QueryFabric serializes these opaquely alongside [`Activity`] entries; it
/// never inspects the payload beyond the stable kind identifier.
pub trait DomainActivity: Serialize + Send + Sync {
    /// Stable machine-readable identifier for this activity kind.
    fn activity_kind(&self) -> &str;
}
