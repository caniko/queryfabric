use serde::{Deserialize, Serialize};

use crate::identity::ResourceRef;

/// Size statistics for one relation, used by the catalog cost model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationStats {
    pub estimated_rows: u64,
    pub average_row_bytes: u64,
}

/// Host-implemented source of relation statistics.
///
/// The catalog cost model consumes this (wired in Phase 04) so the host can
/// inject live statistics without QueryFabric knowing where they come from.
pub trait StatisticsSource: Send + Sync {
    /// Statistics for the relation backing `resource`, or `None` when unknown.
    fn stats_for(&self, resource: ResourceRef) -> Option<RelationStats>;
}
