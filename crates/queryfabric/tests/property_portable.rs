use proptest::prelude::*;
use queryfabric::{
    BackendAdapter, ClickHouseAdapter, ColumnSchema, DataType, DiagnosticSeverity,
    GenericSqlDialect, MemoryCatalog, ParameterValue, PostgresAdapter, QueryCompiler,
    QueryParameters, RelationKind, RelationSchema, SyqlDialect, bind_and_validate_query,
};

#[derive(Debug, Clone, Copy)]
enum ComparisonOp {
    Gt,
    GtEq,
    Lt,
    LtEq,
}

impl ComparisonOp {
    fn render(self) -> &'static str {
        match self {
            Self::Gt => ">",
            Self::GtEq => ">=",
            Self::Lt => "<",
            Self::LtEq => "<=",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    fn render(self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }
}

#[derive(Debug, Clone)]
enum FilterSpec {
    None,
    Literal { op: ComparisonOp, threshold: i64 },
    Parameter { op: ComparisonOp, threshold: i64 },
    Between { low: i64, high: i64 },
}

#[derive(Debug, Clone)]
enum ScanProjection {
    Star,
    Columns(Vec<&'static str>),
    CaseBucket { threshold: i64 },
}

#[derive(Debug, Clone)]
struct ScanCase {
    projection: ScanProjection,
    filter: FilterSpec,
    order_by: Option<(&'static str, SortDirection)>,
    distinct: bool,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
enum JoinKindSpec {
    Inner,
    Left,
    Right,
    Full,
    Cross,
}

impl JoinKindSpec {
    fn render(self) -> &'static str {
        match self {
            Self::Inner => "INNER JOIN",
            Self::Left => "LEFT JOIN",
            Self::Right => "RIGHT JOIN",
            Self::Full => "FULL JOIN",
            Self::Cross => "CROSS JOIN",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum AggregateKind {
    Count { distinct: bool },
    Sum,
    Avg,
    Min,
    Max,
}

#[derive(Debug, Clone, Copy)]
enum WindowKind {
    Rank,
    Lag,
    Lead,
    FirstValue,
    LastValue,
}

#[derive(Debug, Clone, Copy)]
enum SubqueryKind {
    ScalarMaxWeight,
    InTargetRecordIds,
}

#[derive(Debug, Clone)]
enum PortableCase {
    Scan(ScanCase),
    Join {
        kind: JoinKindSpec,
        threshold: i64,
        limit: Option<u32>,
    },
    Aggregate {
        kind: AggregateKind,
        threshold: i64,
    },
    Derived {
        threshold: i64,
    },
    Cte {
        threshold: i64,
        limit: Option<u32>,
    },
    Window {
        kind: WindowKind,
        limit: Option<u32>,
    },
    UnionAll {
        left_threshold: i64,
        right_threshold: i64,
    },
    Subquery {
        kind: SubqueryKind,
    },
}

fn catalog() -> MemoryCatalog {
    let mut catalog = MemoryCatalog::default();
    catalog.set_snapshot_id("property-portable");
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
            ColumnSchema {
                name: "species".into(),
                data_type: DataType::Utf8,
                nullable: true,
                metadata: Default::default(),
            },
        ],
        metadata: Default::default(),
    });
    catalog.register_relation(RelationSchema {
        namespace: None,
        name: "links".into(),
        aliases: vec!["s".into()],
        kind: RelationKind::Table,
        columns: vec![
            ColumnSchema {
                name: "source_record_id".into(),
                data_type: DataType::Uuid,
                nullable: false,
                metadata: Default::default(),
            },
            ColumnSchema {
                name: "target_record_id".into(),
                data_type: DataType::Uuid,
                nullable: false,
                metadata: Default::default(),
            },
            ColumnSchema {
                name: "weight".into(),
                data_type: DataType::Float64,
                nullable: false,
                metadata: Default::default(),
            },
        ],
        metadata: Default::default(),
    });
    catalog
}

fn comparison_strategy() -> impl Strategy<Value = ComparisonOp> {
    prop_oneof![
        Just(ComparisonOp::Gt),
        Just(ComparisonOp::GtEq),
        Just(ComparisonOp::Lt),
        Just(ComparisonOp::LtEq),
    ]
}

fn sort_direction_strategy() -> impl Strategy<Value = SortDirection> {
    prop_oneof![Just(SortDirection::Asc), Just(SortDirection::Desc)]
}

fn scan_case_strategy() -> impl Strategy<Value = ScanCase> {
    let projection = prop_oneof![
        Just(ScanProjection::Star),
        Just(ScanProjection::Columns(vec!["record_id"])),
        Just(ScanProjection::Columns(vec!["score"])),
        Just(ScanProjection::Columns(vec!["record_id", "score"])),
        (0i64..200).prop_map(|threshold| ScanProjection::CaseBucket { threshold }),
    ];
    let filter = prop_oneof![
        Just(FilterSpec::None),
        ((0i64..200), comparison_strategy())
            .prop_map(|(threshold, op)| FilterSpec::Literal { op, threshold }),
        ((0i64..200), comparison_strategy())
            .prop_map(|(threshold, op)| FilterSpec::Parameter { op, threshold }),
        ((0i64..100), (100i64..300)).prop_map(|(low, high)| FilterSpec::Between { low, high }),
    ];
    (
        projection,
        filter,
        prop_oneof![
            Just(None),
            (Just("record_id"), sort_direction_strategy())
                .prop_map(|(column, direction)| Some((column, direction))),
            (Just("score"), sort_direction_strategy())
                .prop_map(|(column, direction)| Some((column, direction))),
        ],
        any::<bool>(),
        prop_oneof![Just(None), (0u32..8).prop_map(Some)],
        prop_oneof![Just(None), (0u32..4).prop_map(Some)],
    )
        .prop_map(|(projection, filter, order_by, distinct, limit, offset)| {
            let (distinct, order_by) = match &projection {
                ScanProjection::CaseBucket { .. } => (false, None),
                ScanProjection::Star => (distinct, order_by),
                ScanProjection::Columns(columns) => {
                    let order_by = match order_by {
                        Some((column, direction)) if columns.contains(&column) => {
                            Some((column, direction))
                        }
                        _ => None,
                    };
                    (distinct, order_by)
                }
            };
            ScanCase {
                projection,
                filter,
                order_by,
                distinct,
                limit,
                offset,
            }
        })
}

fn join_case_strategy() -> impl Strategy<Value = (JoinKindSpec, i64, Option<u32>)> {
    (
        prop_oneof![
            Just(JoinKindSpec::Inner),
            Just(JoinKindSpec::Left),
            Just(JoinKindSpec::Right),
            Just(JoinKindSpec::Full),
            Just(JoinKindSpec::Cross),
        ],
        0i64..200,
        prop_oneof![Just(None), (0u32..6).prop_map(Some)],
    )
}

fn aggregate_case_strategy() -> impl Strategy<Value = (AggregateKind, i64)> {
    (
        prop_oneof![
            Just(AggregateKind::Count { distinct: false }),
            Just(AggregateKind::Count { distinct: true }),
            Just(AggregateKind::Sum),
            Just(AggregateKind::Avg),
            Just(AggregateKind::Min),
            Just(AggregateKind::Max),
        ],
        0i64..200,
    )
}

fn window_case_strategy() -> impl Strategy<Value = (WindowKind, Option<u32>)> {
    (
        prop_oneof![
            Just(WindowKind::Rank),
            Just(WindowKind::Lag),
            Just(WindowKind::Lead),
            Just(WindowKind::FirstValue),
            Just(WindowKind::LastValue),
        ],
        prop_oneof![Just(None), (0u32..8).prop_map(Some)],
    )
}

fn portable_case_strategy() -> impl Strategy<Value = PortableCase> {
    prop_oneof![
        3 => scan_case_strategy().prop_map(PortableCase::Scan),
        2 => join_case_strategy().prop_map(|(kind, threshold, limit)| PortableCase::Join {
            kind,
            threshold,
            limit,
        }),
        2 => aggregate_case_strategy().prop_map(|(kind, threshold)| PortableCase::Aggregate {
            kind,
            threshold,
        }),
        1 => (0i64..200).prop_map(|threshold| PortableCase::Derived { threshold }),
        1 => ((0i64..200), prop_oneof![Just(None), (0u32..6).prop_map(Some)])
            .prop_map(|(threshold, limit)| PortableCase::Cte { threshold, limit }),
        1 => window_case_strategy().prop_map(|(kind, limit)| PortableCase::Window { kind, limit }),
        1 => ((0i64..200), (0i64..200)).prop_map(|(left_threshold, right_threshold)| {
            PortableCase::UnionAll {
                left_threshold,
                right_threshold,
            }
        }),
        1 => prop_oneof![
            Just(SubqueryKind::ScalarMaxWeight),
            Just(SubqueryKind::InTargetRecordIds),
        ]
        .prop_map(|kind| PortableCase::Subquery { kind }),
    ]
}

fn render_parameters(filter: &FilterSpec) -> QueryParameters {
    let mut parameters = QueryParameters::default();
    if let FilterSpec::Parameter { threshold, .. } = filter {
        parameters.insert_positional(1, ParameterValue::Float64(format!("{threshold}.0")));
    }
    parameters
}

fn render_scan(case: &ScanCase, syql: bool) -> (String, QueryParameters) {
    let mut sql = String::new();
    let use_shorthand = syql && matches!(case.projection, ScanProjection::Star) && !case.distinct;

    if use_shorthand {
        sql.push_str("FROM records");
    } else {
        sql.push_str("SELECT ");
        if case.distinct {
            sql.push_str("DISTINCT ");
        }
        sql.push_str(&render_scan_projection(&case.projection));
        sql.push_str(" FROM records");
    }
    if !matches!(case.filter, FilterSpec::None) {
        sql.push_str(" WHERE ");
        sql.push_str(&render_filter(&case.filter));
    }
    if let Some((column, direction)) = case.order_by {
        sql.push_str(" ORDER BY ");
        sql.push_str(column);
        sql.push(' ');
        sql.push_str(direction.render());
    }
    if let Some(limit) = case.limit {
        sql.push_str(" LIMIT ");
        sql.push_str(&limit.to_string());
    }
    if let Some(offset) = case.offset {
        sql.push_str(" OFFSET ");
        sql.push_str(&offset.to_string());
    }
    (sql, render_parameters(&case.filter))
}

fn render_scan_projection(projection: &ScanProjection) -> String {
    match projection {
        ScanProjection::Star => "*".into(),
        ScanProjection::Columns(columns) => columns.join(", "),
        ScanProjection::CaseBucket { threshold } => {
            format!("CASE WHEN score > {threshold} THEN 'high' ELSE 'low' END AS score_bucket")
        }
    }
}

fn render_filter(filter: &FilterSpec) -> String {
    match filter {
        FilterSpec::None => unreachable!("render_filter is only called for active filters"),
        FilterSpec::Literal { op, threshold } => {
            format!("score {} {threshold}", op.render())
        }
        FilterSpec::Parameter { op, .. } => format!("score {} $1", op.render()),
        FilterSpec::Between { low, high } => format!("score BETWEEN {low} AND {high}"),
    }
}

fn render_join(
    kind: JoinKindSpec,
    threshold: i64,
    limit: Option<u32>,
) -> (String, QueryParameters) {
    let mut sql = String::from("SELECT n.record_id, s.weight FROM records AS n ");
    sql.push_str(kind.render());
    sql.push_str(" links AS s");
    if !matches!(kind, JoinKindSpec::Cross) {
        sql.push_str(" ON n.record_id = s.target_record_id");
    }
    sql.push_str(" WHERE ");
    if matches!(kind, JoinKindSpec::Cross) {
        sql.push_str("n.record_id = s.target_record_id AND ");
    }
    sql.push_str(&format!("s.weight > {threshold}"));
    if let Some(limit) = limit {
        sql.push_str(&format!(" LIMIT {limit}"));
    }
    (sql, QueryParameters::default())
}

fn render_aggregate(kind: AggregateKind, threshold: i64) -> (String, QueryParameters) {
    let aggregate = match kind {
        AggregateKind::Count { distinct: true } => "COUNT(DISTINCT s.target_record_id)".to_string(),
        AggregateKind::Count { distinct: false } => "COUNT(s.target_record_id)".to_string(),
        AggregateKind::Sum => "SUM(s.weight)".to_string(),
        AggregateKind::Avg => "AVG(s.weight)".to_string(),
        AggregateKind::Min => "MIN(s.weight)".to_string(),
        AggregateKind::Max => "MAX(s.weight)".to_string(),
    };
    let sql = format!(
        "SELECT n.record_id, {aggregate} AS aggregate_value FROM records AS n INNER JOIN links AS s ON n.record_id = s.target_record_id GROUP BY n.record_id HAVING {aggregate} > {threshold}"
    );
    (sql, QueryParameters::default())
}

fn render_derived(threshold: i64) -> (String, QueryParameters) {
    (
        format!(
            "SELECT derived.record_id FROM (SELECT record_id, score FROM records WHERE score > {threshold}) AS derived"
        ),
        QueryParameters::default(),
    )
}

fn render_cte(threshold: i64, limit: Option<u32>) -> (String, QueryParameters) {
    let mut sql = format!(
        "WITH recent AS (SELECT record_id, score FROM records WHERE score > {threshold}) SELECT record_id FROM recent"
    );
    if let Some(limit) = limit {
        sql.push_str(&format!(" LIMIT {limit}"));
    }
    (sql, QueryParameters::default())
}

fn render_window(kind: WindowKind, limit: Option<u32>) -> (String, QueryParameters) {
    let window_expr = match kind {
        WindowKind::Rank => "RANK() OVER (ORDER BY score DESC) AS rank_pos".to_string(),
        WindowKind::Lag => "LAG(score) OVER (ORDER BY score) AS prev_score".to_string(),
        WindowKind::Lead => "LEAD(score) OVER (ORDER BY score) AS next_score".to_string(),
        WindowKind::FirstValue => {
            "FIRST_VALUE(score) OVER (ORDER BY score) AS first_score".to_string()
        }
        WindowKind::LastValue => {
            "LAST_VALUE(score) OVER (ORDER BY score) AS last_score".to_string()
        }
    };
    let mut sql = format!("SELECT record_id, {window_expr} FROM records");
    if let Some(limit) = limit {
        sql.push_str(&format!(" LIMIT {limit}"));
    }
    (sql, QueryParameters::default())
}

fn render_union_all(left_threshold: i64, right_threshold: i64) -> (String, QueryParameters) {
    (
        format!(
            "SELECT record_id FROM records WHERE score > {left_threshold} UNION ALL SELECT target_record_id FROM links WHERE weight > {right_threshold}"
        ),
        QueryParameters::default(),
    )
}

fn render_subquery(kind: SubqueryKind) -> (String, QueryParameters) {
    match kind {
        SubqueryKind::ScalarMaxWeight => (
            "SELECT record_id, (SELECT MAX(weight) FROM links) AS max_weight FROM records".into(),
            QueryParameters::default(),
        ),
        SubqueryKind::InTargetRecordIds => (
            "SELECT record_id FROM records WHERE record_id IN (SELECT target_record_id FROM links)"
                .into(),
            QueryParameters::default(),
        ),
    }
}

fn render_case(case: &PortableCase, syql: bool) -> (String, QueryParameters) {
    match case {
        PortableCase::Scan(scan) => render_scan(scan, syql),
        PortableCase::Join {
            kind,
            threshold,
            limit,
        } => render_join(*kind, *threshold, *limit),
        PortableCase::Aggregate { kind, threshold } => render_aggregate(*kind, *threshold),
        PortableCase::Derived { threshold } => render_derived(*threshold),
        PortableCase::Cte { threshold, limit } => render_cte(*threshold, *limit),
        PortableCase::Window { kind, limit } => render_window(*kind, *limit),
        PortableCase::UnionAll {
            left_threshold,
            right_threshold,
        } => render_union_all(*left_threshold, *right_threshold),
        PortableCase::Subquery { kind } => render_subquery(*kind),
    }
}

fn snapshot_id(value: Option<&queryfabric::CatalogSnapshotId>) -> Option<&str> {
    value.map(|id| id.0.as_str())
}

fn verify_case(case: PortableCase) {
    let compiler = QueryCompiler::default();
    let catalog = catalog();
    let (sql_text, sql_params) = render_case(&case, false);
    let (syql_text, syql_params) = render_case(&case, true);

    let sql_parsed = compiler.parse(&GenericSqlDialect, &sql_text).unwrap();
    let syql_parsed = compiler.parse(&SyqlDialect, &syql_text).unwrap();
    let sql_bound = bind_and_validate_query(&sql_parsed, &catalog, &sql_params).unwrap();
    let syql_bound = bind_and_validate_query(&syql_parsed, &catalog, &syql_params).unwrap();

    assert_eq!(
        sql_bound.parsed().canonical_sql(),
        syql_bound.parsed().canonical_sql()
    );
    assert_eq!(sql_bound.result_schema(), syql_bound.result_schema());
    assert_eq!(
        sql_bound.capability_requirements(),
        syql_bound.capability_requirements()
    );
    assert_eq!(sql_bound.parameters(), syql_bound.parameters());
    assert_eq!(
        sql_bound.provenance().query_hash,
        syql_bound.provenance().query_hash
    );
    assert_eq!(
        snapshot_id(sql_bound.catalog_snapshot()),
        snapshot_id(syql_bound.catalog_snapshot())
    );
    assert!(
        sql_bound
            .diagnostics()
            .iter()
            .all(|diag| diag.severity != DiagnosticSeverity::Error)
    );
    assert!(
        syql_bound
            .diagnostics()
            .iter()
            .all(|diag| diag.severity != DiagnosticSeverity::Error)
    );

    let clickhouse = ClickHouseAdapter;
    let postgres = PostgresAdapter;
    let backends: [(&str, &dyn BackendAdapter); 2] =
        [("clickhouse", &clickhouse), ("postgres", &postgres)];

    for (backend_name, adapter) in backends {
        let sql_analysis = compiler.analyze(&sql_bound, adapter, &catalog);
        let syql_analysis = compiler.analyze(&syql_bound, adapter, &catalog);
        assert!(
            sql_analysis.supported,
            "sql should be supported on {backend_name}"
        );
        assert!(
            syql_analysis.supported,
            "syql should be supported on {backend_name}"
        );
        assert_eq!(
            sql_analysis.estimated_cost_class,
            syql_analysis.estimated_cost_class
        );
        assert_eq!(sql_analysis.result_schema, syql_analysis.result_schema);
        assert_eq!(sql_analysis.diagnostics, syql_analysis.diagnostics);

        let sql_artifact = compiler.emit(&sql_bound, adapter, &catalog).unwrap();
        let syql_artifact = compiler.emit(&syql_bound, adapter, &catalog).unwrap();
        let sql_artifact = sql_artifact.as_sql().expect("portable query emits SQL");
        let syql_artifact = syql_artifact.as_sql().expect("portable query emits SQL");

        assert_eq!(sql_artifact.text, syql_artifact.text, "{case:?}");
        assert_eq!(
            sql_artifact.parameters, syql_artifact.parameters,
            "{case:?}"
        );
        assert_eq!(
            sql_artifact.result_schema, syql_artifact.result_schema,
            "{case:?}"
        );
        assert_eq!(
            sql_artifact.provenance.query_hash, syql_artifact.provenance.query_hash,
            "{case:?}"
        );
        assert_eq!(
            sql_artifact.provenance.backend.as_deref(),
            Some(backend_name)
        );
        assert_eq!(
            syql_artifact.provenance.backend.as_deref(),
            Some(backend_name)
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 48,
        max_shrink_iters: 32,
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    #[test]
    fn portable_sql_and_syql_queries_compile_equivalently(case in portable_case_strategy()) {
        verify_case(case);
    }
}
