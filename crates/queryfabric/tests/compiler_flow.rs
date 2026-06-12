use queryfabric::{
    BackendAdapter, Catalog, ClickHouseAdapter, GenericSqlDialect, PostgresAdapter, QueryCompiler,
    QueryParameters, SyqlDialect, bind_and_validate_query, builtin_capability_manifest,
};

fn catalog() -> impl Catalog {
    queryfabric::portable_catalog("snapshot-2026-04-18")
}

#[test]
fn sql_and_syql_bind_to_equivalent_canonical_sql() {
    let compiler = QueryCompiler::default();
    let sql = compiler
        .parse(
            &GenericSqlDialect,
            "SELECT * FROM records WHERE score > 100",
        )
        .expect("sql parse");
    let syql = compiler
        .parse(&SyqlDialect, "FROM records WHERE score > 100")
        .expect("syql parse");
    assert_eq!(sql.canonical_sql(), syql.canonical_sql());
}

#[test]
fn compiler_emits_postgres_sql_and_keeps_provenance() {
    let compiler = QueryCompiler::default();
    let parsed = compiler
        .parse(&GenericSqlDialect, "SELECT record_id FROM records LIMIT 5")
        .expect("parse");
    let bound =
        bind_and_validate_query(&parsed, &catalog(), &QueryParameters::default()).expect("bind");
    let analysis = compiler.analyze(&bound, &PostgresAdapter, &catalog());
    assert!(analysis.supported);
    let artifact = compiler
        .emit(&bound, &PostgresAdapter, &catalog())
        .expect("emit");
    let sql = artifact.as_sql().expect("sql artifact");
    assert_eq!(sql.dialect, "postgres");
    assert_eq!(
        sql.provenance
            .catalog_snapshot
            .as_ref()
            .map(|id| id.0.as_str()),
        Some("snapshot-2026-04-18")
    );
}

#[test]
fn compiler_supports_count_star() {
    let compiler = QueryCompiler::default();
    let parsed = compiler
        .parse(&GenericSqlDialect, "SELECT COUNT(*) AS n FROM records")
        .expect("parse");
    let bound =
        bind_and_validate_query(&parsed, &catalog(), &QueryParameters::default()).expect("bind");
    let clickhouse = compiler
        .emit(&bound, &ClickHouseAdapter, &catalog())
        .expect("emit");
    let clickhouse_sql = clickhouse.as_sql().expect("sql artifact");
    assert_eq!(clickhouse_sql.text, "SELECT count(*) AS n FROM records");

    let postgres = compiler
        .emit(&bound, &PostgresAdapter, &catalog())
        .expect("emit");
    let postgres_sql = postgres.as_sql().expect("sql artifact");
    assert_eq!(postgres_sql.text, "SELECT count(*) AS n FROM records");
}

#[test]
fn capability_manifest_covers_built_in_backends() {
    let manifest = builtin_capability_manifest();
    assert_eq!(manifest.len(), 2);
    assert!(manifest.iter().any(|entry| entry.backend == "clickhouse"));
    assert!(manifest.iter().any(|entry| entry.backend == "postgres"));
    let clickhouse = manifest
        .iter()
        .find(|entry| entry.backend == "clickhouse")
        .expect("clickhouse");
    assert!(
        clickhouse.capabilities.features.len() >= ClickHouseAdapter.capabilities().features.len()
    );
}
