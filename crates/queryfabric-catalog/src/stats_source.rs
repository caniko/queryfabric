use std::collections::BTreeMap;

use queryfabric_contract::{ResourceRef, StatisticsSource};
use queryfabric_ir::CatalogSnapshotId;

use crate::model::{
    Catalog, FunctionRegistry, FunctionSignature, RelationSchema, RelationStatistics,
};

/// Bridges a host [`StatisticsSource`] into catalog [`RelationStatistics`].
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

/// Catalog wrapper that answers statistics queries from a host
/// [`StatisticsSource`] instead of schema metadata.
///
/// The host binds relation names to the opaque [`ResourceRef`]s its source
/// understands; everything else delegates to the wrapped catalog. Relations
/// without a binding (or where the source has no answer) fall back to the
/// inner catalog's statistics.
pub struct StatisticsOverlayCatalog<'a> {
    inner: &'a dyn Catalog,
    source: &'a dyn StatisticsSource,
    bindings: BTreeMap<(Option<String>, String), ResourceRef>,
}

impl<'a> StatisticsOverlayCatalog<'a> {
    pub fn new(inner: &'a dyn Catalog, source: &'a dyn StatisticsSource) -> Self {
        Self {
            inner,
            source,
            bindings: BTreeMap::new(),
        }
    }

    /// Bind a relation name to the resource the statistics source keys on.
    pub fn bind_resource(&mut self, namespace: Option<&str>, name: &str, resource: ResourceRef) {
        self.bindings.insert(
            (namespace.map(str::to_owned), name.to_ascii_lowercase()),
            resource,
        );
    }
}

impl FunctionRegistry for StatisticsOverlayCatalog<'_> {
    fn resolve_function(&self, namespace: Option<&str>, name: &str) -> Option<FunctionSignature> {
        self.inner.resolve_function(namespace, name)
    }

    fn functions(&self) -> Vec<FunctionSignature> {
        self.inner.functions()
    }
}

impl Catalog for StatisticsOverlayCatalog<'_> {
    fn snapshot_id(&self) -> CatalogSnapshotId {
        self.inner.snapshot_id()
    }

    fn resolve_relation(&self, namespace: Option<&str>, name: &str) -> Option<RelationSchema> {
        self.inner.resolve_relation(namespace, name)
    }

    fn relations(&self) -> Vec<RelationSchema> {
        self.inner.relations()
    }

    fn relation_statistics(
        &self,
        namespace: Option<&str>,
        name: &str,
    ) -> Option<RelationStatistics> {
        let key = (namespace.map(str::to_owned), name.to_ascii_lowercase());
        self.bindings
            .get(&key)
            .and_then(|resource| relation_statistics_from_source(self.source, *resource, name))
            .or_else(|| self.inner.relation_statistics(namespace, name))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap as Map;

    use queryfabric_contract::RelationStats;

    use super::*;
    use crate::model::{
        BackendExecutionLimits, CapabilitySet, DefaultQueryCostModel, MemoryCatalog,
        QueryCostInput, QueryCostModel, QueryTimeoutClass, RelationKind,
    };

    struct MapSource(Map<ResourceRef, RelationStats>);

    impl StatisticsSource for MapSource {
        fn stats_for(&self, resource: ResourceRef) -> Option<RelationStats> {
            self.0.get(&resource).copied()
        }
    }

    fn resource(id: u128) -> ResourceRef {
        ResourceRef::new(uuid::Uuid::from_u128(7), uuid::Uuid::from_u128(id))
    }

    fn catalog_with_relation(name: &str) -> MemoryCatalog {
        let mut catalog = MemoryCatalog::default();
        catalog.register_relation(RelationSchema {
            namespace: None,
            name: name.into(),
            aliases: Vec::new(),
            kind: RelationKind::Table,
            columns: Vec::new(),
            metadata: Default::default(),
        });
        catalog
    }

    #[test]
    fn overlay_answers_statistics_from_the_injected_source() {
        let catalog = catalog_with_relation("samples");
        let source = MapSource(Map::from([(
            resource(1),
            RelationStats {
                estimated_rows: 42_000,
                average_row_bytes: 96,
            },
        )]));
        let mut overlay = StatisticsOverlayCatalog::new(&catalog, &source);
        overlay.bind_resource(None, "samples", resource(1));

        let stats = overlay
            .relation_statistics(None, "samples")
            .expect("statistics from source");
        assert_eq!(stats.estimated_rows, 42_000);
        assert_eq!(stats.average_row_bytes, 96);

        // Unbound relations fall back to the inner catalog (no metadata → None).
        assert!(overlay.relation_statistics(None, "absent").is_none());
    }

    #[test]
    fn injected_memory_catalog_statistics_override_schema_metadata() {
        let mut catalog = catalog_with_relation("samples");
        assert!(catalog.relation_statistics(None, "samples").is_none());
        catalog.set_relation_statistics(
            None,
            "samples",
            RelationStatistics {
                relation: "samples".into(),
                estimated_rows: 10,
                average_row_bytes: 64,
                shard_count: Some(2),
            },
        );
        let stats = catalog
            .relation_statistics(None, "samples")
            .expect("injected statistics");
        assert_eq!(stats.estimated_rows, 10);
        assert_eq!(stats.shard_count, Some(2));
    }

    #[test]
    fn cost_model_selects_mode_from_injected_statistics_at_byte_thresholds() {
        let limits = BackendExecutionLimits {
            interactive_byte_limit: 1_000_000,
            batch_byte_limit: 100_000_000,
            ..BackendExecutionLimits::default()
        };
        let capabilities = CapabilitySet::default().with_limits(limits);
        let catalog = catalog_with_relation("samples");
        // One column, selectivity 100%: estimated bytes = rows * row_bytes.
        let cases = [
            (10_000u64, QueryTimeoutClass::Interactive), // 1 MB, at the interactive limit
            (50_000, QueryTimeoutClass::Batch),          // 5 MB
            (2_000_000_000, QueryTimeoutClass::Export),  // 200 GB, above the batch limit
        ];
        for (rows, expected) in cases {
            let source = MapSource(Map::from([(
                resource(1),
                RelationStats {
                    estimated_rows: rows,
                    average_row_bytes: 100,
                },
            )]));
            let mut overlay = StatisticsOverlayCatalog::new(&catalog, &source);
            overlay.bind_resource(None, "samples", resource(1));

            let input = QueryCostInput {
                relations: overlay
                    .relation_statistics(None, "samples")
                    .into_iter()
                    .collect(),
                selected_columns: 1,
                row_limit: Some(rows),
                estimated_filter_selectivity_ppm: 1_000_000,
                backend_capabilities: capabilities.clone(),
                ..QueryCostInput::default()
            };
            let estimate = DefaultQueryCostModel.estimate(&input);
            assert_eq!(
                estimate.timeout_class, expected,
                "rows={rows} bytes={}",
                estimate.estimated_bytes
            );
        }
    }
}
