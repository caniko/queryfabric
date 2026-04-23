use queryfabric::{
    ClickHouseAdapter, ColumnSchema, DataType, GenericSqlDialect, MemoryCatalog, PostgresAdapter,
    QueryCompiler, QueryParameters, RelationKind, RelationSchema, bind_and_validate_query,
};

fn main() -> Result<(), queryfabric::QueryFabricError> {
    let compiler = QueryCompiler::default();
    let parsed = compiler.parse(
        &GenericSqlDialect,
        "SELECT region, AVG(signal) AS avg_signal \
         FROM observations \
         WHERE signal > $1 \
         GROUP BY region \
         HAVING AVG(signal) > $2 \
         ORDER BY avg_signal DESC \
         LIMIT 3",
    )?;

    let mut catalog = MemoryCatalog::default();
    catalog.set_snapshot_id("multi-backend-catalog");
    catalog.register_relation(RelationSchema {
        namespace: None,
        name: "observations".into(),
        aliases: vec!["o".into()],
        kind: RelationKind::Table,
        columns: vec![
            ColumnSchema {
                name: "region".into(),
                data_type: DataType::Utf8,
                nullable: false,
                metadata: Default::default(),
            },
            ColumnSchema {
                name: "signal".into(),
                data_type: DataType::Float64,
                nullable: false,
                metadata: Default::default(),
            },
        ],
        metadata: Default::default(),
    });

    let mut parameters = QueryParameters::default();
    parameters.insert_positional(1, queryfabric::ParameterValue::Float64("0.5".into()));
    parameters.insert_positional(2, queryfabric::ParameterValue::Float64("1.0".into()));

    let bound = bind_and_validate_query(&parsed, &catalog, &parameters)?;

    for (label, adapter) in [
        (
            "ClickHouse",
            &ClickHouseAdapter as &dyn queryfabric::BackendAdapter,
        ),
        (
            "PostgreSQL",
            &PostgresAdapter as &dyn queryfabric::BackendAdapter,
        ),
    ] {
        let analysis = compiler.analyze(&bound, adapter, &catalog);
        println!("{label}");
        println!("  supported: {}", analysis.supported);
        println!("  estimated cost: {:?}", analysis.estimated_cost_class);
        println!("  result schema: {:?}", analysis.result_schema);
        println!("  analysis provenance: {:?}", analysis.provenance);

        if !analysis.diagnostics.is_empty() {
            println!("  diagnostics:");
            for diagnostic in &analysis.diagnostics {
                println!("    - [{}] {}", diagnostic.code, diagnostic.message);
            }
        }

        let artifact = compiler.emit(&bound, adapter, &catalog)?;
        let sql = artifact.as_sql().expect("SQL artifact");
        println!("  emitted dialect: {}", sql.dialect);
        println!("  emitted schema: {:?}", sql.result_schema);
        println!("  emitted provenance: {:?}", sql.provenance);
        println!("  sql:\n{}\n", sql.text);
    }

    Ok(())
}
