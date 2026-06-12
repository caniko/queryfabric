use queryfabric_contract::{ResourceRef, StatisticsSource};

use crate::model::RelationStatistics;

/// Bridges a host [`StatisticsSource`] into catalog [`RelationStatistics`].
///
/// Wiring into the cost model is completed in Phase 04; until then this
/// adapter is the only contact point between the catalog and
/// `queryfabric-contract`.
pub fn relation_statistics_from_source(
    source: &dyn StatisticsSource,
    resource: ResourceRef,
    relation: &str,
) -> Option<RelationStatistics> {
    let stats = source.stats_for(resource)?;
    Some(RelationStatistics {
        relation: relation.to_owned(),
        estimated_rows: stats.estimated_rows,
        average_row_bytes: stats.average_row_bytes,
        shard_count: None,
    })
}
