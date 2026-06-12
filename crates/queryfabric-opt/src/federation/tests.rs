use queryfabric_adapter_clickhouse::ClickHouseAdapter;
use queryfabric_catalog::{
    BackendAdapter, ColumnSchema, MemoryCatalog, PlanFeatures, RelationKind, RelationSchema,
    SqlArtifact, bind_and_validate, inspect_plan,
};
use queryfabric_dialect_sql::GenericSqlDialect;
use queryfabric_ir::{BoundQuery, DataType, Dialect, QueryParameters};

use super::*;

fn samples_catalog() -> MemoryCatalog {
    let mut catalog = MemoryCatalog::default();
    catalog.register_relation(RelationSchema {
        namespace: None,
        name: "samples".into(),
        aliases: Vec::new(),
        kind: RelationKind::Table,
        columns: vec![
            ColumnSchema {
                name: "cell".into(),
                data_type: DataType::Utf8,
                nullable: false,
                metadata: Default::default(),
            },
            ColumnSchema {
                name: "x".into(),
                data_type: DataType::Float64,
                nullable: true,
                metadata: Default::default(),
            },
        ],
        metadata: Default::default(),
    });
    catalog
}

fn bind_query(sql: &str, catalog: &MemoryCatalog) -> (BoundQuery, PlanFeatures, SqlArtifact) {
    let parsed = GenericSqlDialect.parse(sql).expect("parse");
    let features = inspect_plan(&parsed).expect("features");
    let bound = bind_and_validate(&parsed, catalog, &QueryParameters::default()).expect("bind");
    let artifact = ClickHouseAdapter
        .emit(&bound, catalog)
        .expect("emit")
        .as_sql()
        .cloned()
        .expect("sql artifact");
    (bound, features, artifact)
}

#[test]
fn aggregate_two_stage_decomposes_avg_and_count() {
    let catalog = samples_catalog();
    let (bound, features, artifact) = bind_query(
        "SELECT cell, AVG(x) AS avg_x, COUNT(*) AS n FROM samples GROUP BY cell",
        &catalog,
    );
    let plan =
        build_scatter_gather_plan(&ClickHouseAdapter, &catalog, &bound, &features, &artifact)
            .expect("scatter-gather plan");

    assert_eq!(plan.from_target, "samples");
    // Scatter stage: AVG decomposes into SUM + COUNT partials; COUNT stays a
    // node-local COUNT. GROUP BY is preserved on every node.
    assert_eq!(
        plan.scatter_sql,
        "SELECT samples.cell, sum(samples.x) AS __fed_avg_x_sum_1, \
         count(samples.x) AS __fed_avg_x_cnt_1, count(*) AS __fed_n_2 \
         FROM samples GROUP BY samples.cell"
    );
    // Merge stage: AVG must be SUM(sum)/SUM(count) — never AVG(AVG) — and the
    // partial COUNT merges with SUM.
    assert_eq!(
        plan.gather_sql,
        "SELECT cell, (sum(__fed_avg_x_sum_1) / sum(__fed_avg_x_cnt_1)) AS avg_x, \
         sum(__fed_n_2) AS n FROM ({partials}) AS samples GROUP BY cell"
    );
    assert_eq!(plan.result_schema, artifact.result_schema);
}

#[test]
fn aggregate_two_stage_merges_sum_count_min_max() {
    let catalog = samples_catalog();
    let (bound, features, artifact) = bind_query(
        "SELECT SUM(x) AS total, COUNT(x) AS cnt, MIN(x) AS lo, MAX(x) AS hi FROM samples",
        &catalog,
    );
    let plan =
        build_scatter_gather_plan(&ClickHouseAdapter, &catalog, &bound, &features, &artifact)
            .expect("scatter-gather plan");

    assert_eq!(
        plan.scatter_sql,
        "SELECT sum(samples.x) AS __fed_total_0, count(samples.x) AS __fed_cnt_1, \
         min(samples.x) AS __fed_lo_2, max(samples.x) AS __fed_hi_3 FROM samples"
    );
    assert_eq!(
        plan.gather_sql,
        "SELECT sum(__fed_total_0) AS total, sum(__fed_cnt_1) AS cnt, \
         min(__fed_lo_2) AS lo, max(__fed_hi_3) AS hi FROM ({partials}) AS samples"
    );
}

#[test]
fn passthrough_strips_order_and_limit_from_scatter() {
    let catalog = samples_catalog();
    let (bound, features, artifact) =
        bind_query("SELECT cell FROM samples ORDER BY cell LIMIT 5", &catalog);
    let plan =
        build_scatter_gather_plan(&ClickHouseAdapter, &catalog, &bound, &features, &artifact)
            .expect("scatter-gather plan");

    assert!(!plan.scatter_sql.contains("ORDER BY"));
    assert!(!plan.scatter_sql.contains("LIMIT"));
    assert!(plan.gather_sql.contains("FROM ({partials}) AS samples"));
    assert!(plan.gather_sql.contains("ORDER BY"));
    assert!(plan.gather_sql.contains("LIMIT 5"));
}

#[test]
fn distinct_aggregates_are_rejected() {
    let catalog = samples_catalog();
    let (bound, features, artifact) =
        bind_query("SELECT COUNT(DISTINCT cell) AS n FROM samples", &catalog);
    let error =
        build_scatter_gather_plan(&ClickHouseAdapter, &catalog, &bound, &features, &artifact)
            .expect_err("DISTINCT aggregates cannot be decomposed");
    assert!(matches!(error, FederationError::Unsupported(_)));
}

#[test]
fn passthrough_gather_sql_selects_partials() {
    assert_eq!(passthrough_gather_sql(), "SELECT * FROM ({partials})");
}
