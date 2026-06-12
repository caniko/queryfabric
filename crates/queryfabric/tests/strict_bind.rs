use queryfabric::{
    Catalog, ClickHouseAdapter, ColumnSchema, DataType, GenericSqlDialect, PostgresAdapter,
    QueryCompiler, QueryParameters, RelationKind, RelationSchema, bind_and_validate_query,
    inspect_parameters,
};

fn catalog() -> impl Catalog {
    let mut catalog = queryfabric::MemoryCatalog::default();
    catalog.set_snapshot_id("snapshot-2026-04-18");
    catalog.register_relation(RelationSchema {
        namespace: None,
        name: "records".into(),
        aliases: vec!["n".into()],
        kind: RelationKind::Table,
        columns: vec![
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
        metadata: Default::default(),
    });
    catalog
}

#[test]
fn strict_bind_unknown_relation_returns_diagnostics() {
    let compiler = QueryCompiler::default();
    let parsed = compiler
        .parse(&GenericSqlDialect, "SELECT record_id FROM missing_table")
        .expect("parse");

    let error = bind_and_validate_query(&parsed, &catalog(), &QueryParameters::default())
        .expect_err("bind should fail");
    let details = error.as_bind().expect("expected bind error");

    assert_eq!(
        details.source_sql.as_deref(),
        Some("SELECT record_id FROM missing_table")
    );
    assert_eq!(details.dialect.as_deref(), Some("sql"));
    assert!(details.diagnostics.iter().any(|diag| diag.code == "QF0005"));
    assert_eq!(
        details
            .provenance
            .as_ref()
            .and_then(|receipt| receipt.catalog_snapshot.as_ref())
            .map(|snapshot| snapshot.0.as_str()),
        Some("snapshot-2026-04-18")
    );
}

#[test]
fn strict_bind_rejects_unresolved_parameter_contracts() {
    let compiler = QueryCompiler::default();
    let parsed = compiler
        .parse(&GenericSqlDialect, "SELECT $1 FROM records")
        .expect("parse");

    let error = bind_and_validate_query(&parsed, &catalog(), &QueryParameters::default())
        .expect_err("bind should fail");
    let details = error.as_bind().expect("expected bind error");
    assert!(details.diagnostics.iter().any(|diag| diag.code == "QF0018"));
    assert!(details.diagnostics.iter().any(|diag| diag.code == "QF0019"));
}

#[test]
fn strict_bind_rejects_parameter_value_type_mismatch() {
    let compiler = QueryCompiler::default();
    let parsed = compiler
        .parse(
            &GenericSqlDialect,
            "SELECT record_id FROM records WHERE score > $1",
        )
        .expect("parse");

    let mut parameters = QueryParameters::default();
    parameters.insert_positional(1, queryfabric::ParameterValue::Utf8("bad".into()));

    let error =
        bind_and_validate_query(&parsed, &catalog(), &parameters).expect_err("bind should fail");
    let details = error.as_bind().expect("expected bind error");
    assert!(details.diagnostics.iter().any(|diag| diag.code == "QF0020"));
}

#[test]
fn strict_bind_unknown_column_includes_suggestion_remediation() {
    let compiler = QueryCompiler::default();
    let parsed = compiler
        .parse(&GenericSqlDialect, "SELECT scroe FROM records")
        .expect("parse");

    let error = bind_and_validate_query(&parsed, &catalog(), &QueryParameters::default())
        .expect_err("bind should fail");
    let details = error.as_bind().expect("expected bind error");
    let diagnostic = details
        .diagnostics
        .iter()
        .find(|diag| diag.code == "QF0015")
        .expect("unknown-column diagnostic");
    let remediation = diagnostic
        .remediation
        .as_deref()
        .expect("suggestion remediation");
    assert!(remediation.contains("score"));
}

#[test]
fn warning_only_backend_specific_function_still_binds() {
    let compiler = QueryCompiler::default();
    let parsed = compiler
        .parse(
            &GenericSqlDialect,
            "SELECT ch.avg_merge(score) FROM records",
        )
        .expect("parse");

    let bound = bind_and_validate_query(&parsed, &catalog(), &QueryParameters::default())
        .expect("bind should succeed with warning");
    assert!(bound.diagnostics().iter().any(|diag| diag.code == "QF0104"));
}

#[test]
fn emission_uses_backend_native_placeholders() {
    let compiler = QueryCompiler::default();
    let parsed = compiler
        .parse(
            &GenericSqlDialect,
            "SELECT record_id FROM records WHERE score > $1",
        )
        .expect("parse");

    let mut parameters = QueryParameters::default();
    parameters.insert_positional(1, queryfabric::ParameterValue::Float64("100.0".into()));

    let bound = bind_and_validate_query(&parsed, &catalog(), &parameters).expect("bind");

    let pg = compiler
        .emit(&bound, &PostgresAdapter, &catalog())
        .expect("postgres emit");
    let pg_sql = pg.as_sql().expect("postgres sql");
    assert!(pg_sql.text.contains("$1"));
    assert_eq!(pg_sql.parameters.len(), 1);

    let ch = compiler
        .emit(&bound, &ClickHouseAdapter, &catalog())
        .expect("clickhouse emit");
    let ch_sql = ch.as_sql().expect("clickhouse sql");
    assert!(ch_sql.text.contains("{p1:Float64}"));
    assert_eq!(ch_sql.parameters.len(), 1);
}

#[test]
fn named_parameter_binds_and_emits() {
    let compiler = QueryCompiler::default();
    let parsed = compiler
        .parse(
            &GenericSqlDialect,
            "SELECT record_id FROM records WHERE score > :min_len",
        )
        .expect("parse");

    let mut parameters = QueryParameters::default();
    parameters.insert_named(
        "min_len",
        queryfabric::ParameterValue::Float64("50.0".into()),
    );

    let bound = bind_and_validate_query(&parsed, &catalog(), &parameters).expect("bind");
    assert_eq!(bound.parameters().len(), 1);

    let pg = compiler
        .emit(&bound, &PostgresAdapter, &catalog())
        .expect("postgres emit");
    let pg_sql = pg.as_sql().expect("postgres sql");
    assert!(pg_sql.text.contains("$1"));
}

#[test]
fn list_parameter_emits_backend_specific_in_form() {
    let compiler = QueryCompiler::default();
    let parsed = compiler
        .parse(
            &GenericSqlDialect,
            "SELECT record_id FROM records WHERE record_id IN ($1)",
        )
        .expect("parse");

    let mut parameters = QueryParameters::default();
    parameters.insert_positional(
        1,
        queryfabric::ParameterValue::List(vec![
            queryfabric::ParameterValue::Uuid("11111111-1111-1111-1111-111111111111".into()),
            queryfabric::ParameterValue::Uuid("22222222-2222-2222-2222-222222222222".into()),
        ]),
    );

    let bound = bind_and_validate_query(&parsed, &catalog(), &parameters).expect("bind");

    let pg = compiler
        .emit(&bound, &PostgresAdapter, &catalog())
        .expect("postgres emit");
    let pg_sql = pg.as_sql().expect("postgres sql");
    assert!(pg_sql.text.contains("= ANY($1)"));

    let ch = compiler
        .emit(&bound, &ClickHouseAdapter, &catalog())
        .expect("clickhouse emit");
    let ch_sql = ch.as_sql().expect("clickhouse sql");
    assert!(ch_sql.text.contains("IN {p1:Array(UUID)}"));
}

#[test]
fn inspect_parameters_reports_query_contract() {
    let compiler = QueryCompiler::default();
    let parsed = compiler
        .parse(
            &GenericSqlDialect,
            "SELECT record_id FROM records WHERE score > $2 AND record_id IN ($1)",
        )
        .expect("parse");
    let summary = inspect_parameters(&parsed);
    assert_eq!(summary.positional_count, 2);
    assert!(summary.named_params.is_empty());
}

#[test]
fn strict_bind_rejects_multi_column_scalar_subqueries() {
    let compiler = QueryCompiler::default();
    let parsed = compiler
        .parse(
            &GenericSqlDialect,
            "SELECT (SELECT record_id, score FROM records LIMIT 1) AS bad_scalar",
        )
        .expect("parse");

    let error = bind_and_validate_query(&parsed, &catalog(), &QueryParameters::default())
        .expect_err("bind should fail");
    let details = error.as_bind().expect("expected bind error");
    assert!(details.diagnostics.iter().any(|diag| diag.code == "QF0023"));
}

#[test]
fn strict_bind_rejects_multi_column_in_subqueries() {
    let compiler = QueryCompiler::default();
    let parsed = compiler
        .parse(
            &GenericSqlDialect,
            "SELECT record_id FROM records WHERE record_id IN (SELECT source_record_id, target_record_id FROM links)",
        )
        .expect("parse");

    let error = bind_and_validate_query(&parsed, &catalog(), &QueryParameters::default())
        .expect_err("bind should fail");
    let details = error.as_bind().expect("expected bind error");
    assert!(details.diagnostics.iter().any(|diag| diag.code == "QF0024"));
}
