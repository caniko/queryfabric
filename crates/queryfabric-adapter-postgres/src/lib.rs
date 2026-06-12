use queryfabric_catalog::{
    BackendAdapter, BackendAnalysis, BackendExecutionLimits, BackendFeature, CapabilitySet,
    Catalog, CostEstimateError, EmitArtifact, EstimatedCost, PlanCostEstimator,
    ResultDeliveryFormat, SqlBackend, analyze_backend_support, emit_sql_artifact, unsupported,
};
use queryfabric_ir::{BoundQuery, QueryDiagnostic, Result};

#[derive(Debug, Default, Clone, Copy)]
pub struct PostgresAdapter;

impl BackendAdapter for PostgresAdapter {
    fn name(&self) -> &'static str {
        "postgres"
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
            BackendFeature::Explain,
            BackendFeature::LimitOffset,
        ])
        .with_limits(BackendExecutionLimits {
            max_rows: None,
            max_bytes_scanned: None,
            max_result_bytes: None,
            max_concurrent_queries: None,
            interactive_byte_limit: 128 * 1024 * 1024,
            batch_byte_limit: 4 * 1024 * 1024 * 1024,
        })
        .with_result_formats([ResultDeliveryFormat::Csv, ResultDeliveryFormat::Json])
        .with_async_export(false)
        .with_federated_execution(false)
    }

    fn analyze(&self, query: &BoundQuery, _catalog: &dyn Catalog) -> BackendAnalysis {
        analyze_backend_support(query, _catalog, self.name(), self.capabilities(), false)
    }

    fn emit(&self, query: &BoundQuery, catalog: &dyn Catalog) -> Result<EmitArtifact> {
        let analysis = self.analyze(query, catalog);
        if !analysis.supported {
            return Err(unsupported(
                "postgres-emission",
                diagnostic_summary(&analysis.diagnostics),
            ));
        }

        Ok(EmitArtifact::Sql(emit_sql_artifact(
            query,
            catalog,
            SqlBackend::Postgres,
        )?))
    }
}

impl PlanCostEstimator for PostgresAdapter {
    fn estimate(
        &self,
        _: &BoundQuery,
        _: &dyn Catalog,
    ) -> std::result::Result<EstimatedCost, CostEstimateError> {
        Err(CostEstimateError::Unsupported)
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
        BackendAdapter, BackendFeature, Catalog, ColumnSchema, CostEstimateError, MemoryCatalog,
        PlanCostEstimator, RelationKind, RelationSchema, bind_and_validate,
    };
    use queryfabric_dialect_sql::GenericSqlDialect;
    use queryfabric_ir::{DataType, Dialect, QueryParameters};

    use super::PostgresAdapter;

    fn catalog() -> impl Catalog {
        let mut catalog = MemoryCatalog::default();
        catalog.register_relation(RelationSchema {
            namespace: None,
            name: "records".into(),
            aliases: Vec::new(),
            kind: RelationKind::Table,
            columns: vec![ColumnSchema {
                name: "record_id".into(),
                data_type: DataType::Uuid,
                nullable: false,
                metadata: Default::default(),
            }],
            metadata: Default::default(),
        });
        catalog
    }

    #[test]
    fn emits_postgres_sql_for_portable_subset() {
        let parsed = GenericSqlDialect
            .parse("SELECT record_id FROM records LIMIT 5")
            .expect("parse");
        let bound =
            bind_and_validate(&parsed, &catalog(), &QueryParameters::default()).expect("bind");
        let artifact = <PostgresAdapter as queryfabric_catalog::BackendAdapter>::emit(
            &PostgresAdapter,
            &bound,
            &catalog(),
        )
        .expect("emit");
        let queryfabric_catalog::EmitArtifact::Sql(sql) = artifact else {
            panic!("expected SQL artifact");
        };
        assert_eq!(sql.dialect, "postgres");
        assert_eq!(sql.text, "SELECT records.record_id FROM records LIMIT 5");
    }

    #[test]
    fn does_not_advertise_isolated_execution_capability() {
        assert!(
            !PostgresAdapter
                .capabilities()
                .supports(BackendFeature::IsolatedExecution)
        );
    }

    #[test]
    fn cost_estimator_reports_unsupported() {
        let parsed = GenericSqlDialect
            .parse("SELECT record_id FROM records LIMIT 5")
            .expect("parse");
        let catalog = catalog();
        let bound =
            bind_and_validate(&parsed, &catalog, &QueryParameters::default()).expect("bind");
        let error = PostgresAdapter
            .estimate(&bound, &catalog)
            .expect_err("postgres cost estimation should be unsupported");
        assert!(matches!(error, CostEstimateError::Unsupported));
    }
}
