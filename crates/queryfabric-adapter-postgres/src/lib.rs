use queryfabric_catalog::{
    BackendAdapter, BackendAnalysis, BackendFeature, CapabilitySet, Catalog, EmitArtifact,
    SqlBackend, analyze_backend_support, emit_sql_artifact, unsupported,
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
        Catalog, ColumnSchema, MemoryCatalog, RelationKind, RelationSchema, bind_and_validate,
    };
    use queryfabric_dialect_sql::GenericSqlDialect;
    use queryfabric_ir::{DataType, Dialect, QueryParameters};

    use super::PostgresAdapter;

    fn catalog() -> impl Catalog {
        let mut catalog = MemoryCatalog::default();
        catalog.register_relation(RelationSchema {
            namespace: None,
            name: "neurons".into(),
            aliases: Vec::new(),
            kind: RelationKind::Table,
            columns: vec![ColumnSchema {
                name: "neuron_id".into(),
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
            .parse("SELECT neuron_id FROM neurons LIMIT 5")
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
        assert_eq!(sql.text, "SELECT neurons.neuron_id FROM neurons LIMIT 5");
    }
}
