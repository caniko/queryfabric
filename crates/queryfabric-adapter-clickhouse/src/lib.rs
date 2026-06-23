use std::collections::BTreeSet;

use queryfabric_catalog::{
    BackendAdapter, BackendAnalysis, BackendExecutionLimits, BackendFeature, CapabilitySet,
    Catalog, CostEstimateError, EmitArtifact, EstimatedCost, PlanCostEstimator, RelationKind,
    RelationSchema, ResultDeliveryFormat, SqlBackend, analyze_backend_support, emit_sql_artifact,
    unsupported,
};
use queryfabric_ir::{
    BinaryOperator, BoundColumnRef, BoundExpr, BoundExprKind, BoundFunctionCall, BoundOrderByExpr,
    BoundProjectionItem, BoundQuery, BoundQueryPlan, BoundRelation, BoundSelect, BoundSetExpr,
    BoundTableWithJoins, DataType, FunctionRef, LiteralValue, QueryDiagnostic, Result, ResultField,
    ResultSchema, SyntaxNode,
};

mod cost;
mod rewrite;

use cost::estimate_clickhouse_cost;
use rewrite::rewrite_query_for_clickhouse;

pub mod arrow;
pub mod driver;
mod runtime;
pub mod types;

pub use arrow::{clickhouse_arrow_safe_artifact_sql, clickhouse_arrow_safe_sql};
pub use driver::{ClickHouseConfig, ClickHouseError, DynamicClient, downcast_view_types};
pub use runtime::{ClickHouseArrowTransport, ClickHouseRuntime};
pub use types::{ChType, SimpleColumnType};

#[derive(Debug, Default, Clone, Copy)]
pub struct ClickHouseAdapter;

impl BackendAdapter for ClickHouseAdapter {
    fn name(&self) -> &'static str {
        "clickhouse"
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::from_features([
            BackendFeature::CommonTableExpressions,
            BackendFeature::DerivedTables,
            BackendFeature::Joins,
            BackendFeature::Windows,
            BackendFeature::SetOperations,
            BackendFeature::Aggregates,
            BackendFeature::DistinctAggregates,
            BackendFeature::ScalarSubqueries,
            BackendFeature::InSubqueries,
            BackendFeature::NamespacedFunctions,
            BackendFeature::ApproximateAggregates,
            BackendFeature::Explain,
            BackendFeature::LimitOffset,
            BackendFeature::IsolatedExecution,
            BackendFeature::UuidToStringInArrowOutput,
        ])
        .with_limits(BackendExecutionLimits {
            max_rows: None,
            max_bytes_scanned: None,
            max_result_bytes: None,
            max_concurrent_queries: None,
            interactive_byte_limit: 512 * 1024 * 1024,
            batch_byte_limit: 4 * 1024 * 1024 * 1024,
        })
        .with_result_formats([
            ResultDeliveryFormat::ArrowIpc,
            ResultDeliveryFormat::Parquet,
            ResultDeliveryFormat::Csv,
            ResultDeliveryFormat::Json,
        ])
        .with_async_export(true)
        .with_federated_execution(true)
    }

    fn analyze(&self, query: &BoundQuery, catalog: &dyn Catalog) -> BackendAnalysis {
        let mut analysis =
            analyze_backend_support(query, catalog, self.name(), self.capabilities(), true);
        let (_, mv_summary) =
            rewrite_query_for_clickhouse(query, catalog, self.uuid_arrow_workaround_enabled());
        analysis
            .diagnostics
            .extend(mv_summary.analysis_diagnostics(self.name()));
        analysis.supported = !analysis.diagnostics.iter().any(QueryDiagnostic::is_error);
        analysis
    }

    fn emit(&self, query: &BoundQuery, catalog: &dyn Catalog) -> Result<EmitArtifact> {
        let analysis = self.analyze(query, catalog);
        if !analysis.supported {
            return Err(unsupported(
                "clickhouse-emission",
                diagnostic_summary(&analysis.diagnostics),
            ));
        }

        let (rewritten_query, mv_summary) =
            rewrite_query_for_clickhouse(query, catalog, self.uuid_arrow_workaround_enabled());
        let mut artifact = emit_sql_artifact(&rewritten_query, catalog, SqlBackend::ClickHouse)?;
        if let Some(rewritten_to) = mv_summary.rewritten_to_metadata() {
            artifact
                .metadata
                .insert("clickhouse.rewritten_to".into(), rewritten_to);
        }
        Ok(EmitArtifact::Sql(artifact))
    }
}

impl ClickHouseAdapter {
    fn uuid_arrow_workaround_enabled(&self) -> bool {
        self.capabilities()
            .supports(BackendFeature::UuidToStringInArrowOutput)
    }
}

impl PlanCostEstimator for ClickHouseAdapter {
    fn estimate(
        &self,
        plan: &BoundQuery,
        catalog: &dyn Catalog,
    ) -> std::result::Result<EstimatedCost, CostEstimateError> {
        estimate_clickhouse_cost(plan, catalog)
    }
}

fn diagnostic_summary(diagnostics: &[QueryDiagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}


#[cfg(test)]
mod tests {
    use queryfabric_catalog::{
        BackendAdapter, BackendFeature, Catalog, ColumnSchema, EmitArtifact, MemoryCatalog,
        PlanCostEstimator, RelationKind, RelationSchema, bind_and_validate,
    };
    use queryfabric_dialect_sql::GenericSqlDialect;
    use queryfabric_ir::{DataType, Dialect, FieldMetadata, QueryParameters};

    use super::ClickHouseAdapter;

    fn catalog() -> impl Catalog {
        let mut catalog = MemoryCatalog::default();
        catalog.register_relation(RelationSchema {
            namespace: None,
            name: "records".into(),
            aliases: Vec::new(),
            kind: RelationKind::Table,
            columns: vec![
                ColumnSchema {
                    name: "dataset_id".into(),
                    data_type: DataType::Utf8,
                    nullable: false,
                    metadata: Default::default(),
                },
                ColumnSchema {
                    name: "record_id".into(),
                    data_type: DataType::Uuid,
                    nullable: false,
                    metadata: Default::default(),
                },
                ColumnSchema {
                    name: "score".into(),
                    data_type: DataType::Float64,
                    nullable: true,
                    metadata: Default::default(),
                },
            ],
            metadata: [
                ("estimated_rows".into(), "1000000".into()),
                ("average_row_bytes".into(), "96".into()),
                ("partition_column".into(), "dataset_id".into()),
                ("partition_count".into(), "32".into()),
            ]
            .into_iter()
            .collect(),
        });
        catalog.register_relation(RelationSchema {
            namespace: None,
            name: "mv_dataset_summary".into(),
            aliases: Vec::new(),
            kind: RelationKind::MaterializedView,
            columns: vec![
                ColumnSchema {
                    name: "dataset_id".into(),
                    data_type: DataType::Uuid,
                    nullable: false,
                    metadata: Default::default(),
                },
                ColumnSchema {
                    name: "table_name".into(),
                    data_type: DataType::Utf8,
                    nullable: false,
                    metadata: Default::default(),
                },
                mv_column("row_count", DataType::Int64, "countmerge"),
                mv_column(
                    "last_updated",
                    DataType::Timestamp { timezone: None },
                    "max",
                ),
            ],
            metadata: [
                ("estimated_rows".into(), "128".into()),
                ("average_row_bytes".into(), "128".into()),
                ("partition_column".into(), "dataset_id".into()),
                ("partition_count".into(), "8".into()),
            ]
            .into_iter()
            .collect(),
        });
        catalog
    }

    fn mv_column(name: &str, data_type: DataType, merge_fn: &str) -> ColumnSchema {
        let mut metadata = FieldMetadata::default();
        metadata
            .extensions
            .insert("clickhouse.mv.merge_fn".into(), merge_fn.into());
        ColumnSchema {
            name: name.into(),
            data_type,
            nullable: true,
            metadata,
        }
    }

    fn bind(sql: &str) -> queryfabric_ir::BoundQuery {
        let catalog = catalog();
        let parsed = GenericSqlDialect.parse(sql).expect("parse");
        bind_and_validate(&parsed, &catalog, &QueryParameters::default()).expect("bind")
    }

    #[test]
    fn emits_sql_for_plain_select() {
        let catalog = catalog();
        let bound = bind("SELECT record_id, score FROM records LIMIT 10");
        let artifact = ClickHouseAdapter.emit(&bound, &catalog).expect("emit");
        let EmitArtifact::Sql(sql) = artifact else {
            panic!("expected SQL artifact");
        };
        assert_eq!(
            sql.text,
            "SELECT toString(records.record_id) AS record_id, records.score FROM records LIMIT 10"
        );
        assert_eq!(sql.result_schema.fields().len(), 2);
        assert_eq!(sql.result_schema.fields()[0].data_type, DataType::Utf8);
        assert!(sql.metadata.is_empty());
    }

    #[test]
    fn advertises_isolated_execution_capability() {
        assert!(
            ClickHouseAdapter
                .capabilities()
                .supports(BackendFeature::IsolatedExecution)
        );
    }

    #[test]
    fn advertises_uuid_arrow_workaround_capability() {
        assert!(
            ClickHouseAdapter
                .capabilities()
                .supports(BackendFeature::UuidToStringInArrowOutput)
        );
    }

    #[test]
    fn rewrites_uuid_projection_and_group_by_for_arrow_output() {
        let catalog = catalog();
        let bound = bind("SELECT record_id, count() AS n FROM records GROUP BY record_id");
        let artifact = ClickHouseAdapter.emit(&bound, &catalog).expect("emit");
        let EmitArtifact::Sql(sql) = artifact else {
            panic!("expected SQL artifact");
        };
        assert_eq!(
            sql.text,
            "SELECT toString(records.record_id) AS record_id, count(*) AS n FROM records GROUP BY toString(records.record_id)"
        );
        assert_eq!(sql.result_schema.fields()[0].data_type, DataType::Utf8);
        assert_eq!(sql.result_schema.fields()[0].name, "record_id");
    }

    #[test]
    fn rewrites_uuid_wildcard_projection_for_arrow_output() {
        let catalog = catalog();
        let bound = bind("SELECT * FROM records");
        let artifact = ClickHouseAdapter.emit(&bound, &catalog).expect("emit");
        let EmitArtifact::Sql(sql) = artifact else {
            panic!("expected SQL artifact");
        };
        assert_eq!(
            sql.text,
            "SELECT records.dataset_id AS dataset_id, toString(records.record_id) AS record_id, records.score AS score FROM records"
        );
        assert_eq!(sql.result_schema.fields()[1].data_type, DataType::Utf8);
    }

    #[test]
    fn leaves_non_uuid_projection_unchanged_for_arrow_output() {
        let catalog = catalog();
        let bound = bind("SELECT count() AS n FROM records");
        let artifact = ClickHouseAdapter.emit(&bound, &catalog).expect("emit");
        let EmitArtifact::Sql(sql) = artifact else {
            panic!("expected SQL artifact");
        };
        assert_eq!(sql.text, "SELECT count(*) AS n FROM records");
        assert_eq!(sql.result_schema.fields()[0].data_type, DataType::Int64);
    }

    #[test]
    fn preserves_existing_uuid_to_string_cast_without_double_wrap() {
        let catalog = catalog();
        let bound = bind("SELECT toString(record_id) AS record_id FROM records");
        let artifact = ClickHouseAdapter.emit(&bound, &catalog).expect("emit");
        let EmitArtifact::Sql(sql) = artifact else {
            panic!("expected SQL artifact");
        };
        assert_eq!(
            sql.text,
            "SELECT toString(records.record_id) AS record_id FROM records"
        );
        assert!(!sql.text.contains("toString(toString"), "{}", sql.text);
        assert_eq!(sql.result_schema.fields()[0].data_type, DataType::Utf8);
    }

    #[test]
    fn estimates_cost_for_grouped_query() {
        let catalog = catalog();
        let parsed = GenericSqlDialect
            .parse(
                "SELECT dataset_id, count(record_id) \
                 FROM records \
                 WHERE dataset_id = 'fafb' \
                 GROUP BY dataset_id",
            )
            .expect("parse");
        let bound =
            bind_and_validate(&parsed, &catalog, &QueryParameters::default()).expect("bind");

        let estimate = ClickHouseAdapter
            .estimate(&bound, &catalog)
            .expect("estimate");

        assert!(estimate.rows_scanned > 0, "{estimate:#?}");
        assert!(estimate.memory_bytes > 0, "{estimate:#?}");
        assert_eq!(estimate.partitions_touched, 1);
        assert!(estimate.wallclock_estimate_ms > 0, "{estimate:#?}");
    }

    #[test]
    fn rewrites_mv_projection_and_sets_metadata() {
        let catalog = catalog();
        let bound = bind(
            "SELECT ds.table_name, ds.row_count, ds.last_updated \
             FROM mv_dataset_summary AS ds \
             GROUP BY ds.table_name",
        );

        let analysis = ClickHouseAdapter.analyze(&bound, &catalog);
        assert!(
            analysis
                .diagnostics
                .iter()
                .any(|diag| diag.code == "QFCH201" && diag.message.contains("ds.row_count")),
            "{:#?}",
            analysis.diagnostics
        );

        let artifact = ClickHouseAdapter.emit(&bound, &catalog).expect("emit");
        let EmitArtifact::Sql(sql) = artifact else {
            panic!("expected SQL artifact");
        };
        assert_eq!(
            sql.metadata
                .get("clickhouse.rewritten_to")
                .map(String::as_str),
            Some("mv_dataset_summary")
        );
        assert!(
            sql.text.contains("countMerge(ds.row_count)"),
            "{}",
            sql.text
        );
        assert!(sql.text.contains("max(ds.last_updated)"), "{}", sql.text);
    }

    #[test]
    fn preserves_existing_wrapper_without_double_wrap() {
        let catalog = catalog();
        let bound = bind("SELECT ch.count_merge(row_count) FROM mv_dataset_summary");
        let artifact = ClickHouseAdapter.emit(&bound, &catalog).expect("emit");
        let EmitArtifact::Sql(sql) = artifact else {
            panic!("expected SQL artifact");
        };
        assert_eq!(
            sql.text,
            "SELECT countMerge(mv_dataset_summary.row_count) FROM mv_dataset_summary"
        );
        assert!(!sql.text.contains("countMerge(countMerge"), "{}", sql.text);
        assert!(!sql.metadata.contains_key("clickhouse.rewritten_to"));
    }

    #[test]
    fn surfaces_wrapper_near_miss_diagnostic() {
        let catalog = catalog();
        let bound = bind("SELECT ch.sum_merge(row_count) FROM mv_dataset_summary");
        let analysis = ClickHouseAdapter.analyze(&bound, &catalog);
        assert!(
            analysis.diagnostics.iter().any(|diag| {
                diag.code == "QFCH202"
                    && diag.message.contains("count_merge")
                    && diag.message.contains("sum_merge")
            }),
            "{:#?}",
            analysis.diagnostics
        );

        let artifact = ClickHouseAdapter.emit(&bound, &catalog).expect("emit");
        let EmitArtifact::Sql(sql) = artifact else {
            panic!("expected SQL artifact");
        };
        assert!(
            sql.text
                .contains("sumMerge(countMerge(mv_dataset_summary.row_count))"),
            "{}",
            sql.text
        );
    }
}
