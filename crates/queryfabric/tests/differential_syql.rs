use queryfabric::{
    Catalog, ClickHouseAdapter, GenericSqlDialect, PostgresAdapter, QueryCompiler, QueryParameters,
    SyqlDialect, bind_and_validate_query,
};

fn catalog() -> impl Catalog {
    queryfabric::portable_catalog("differential-syql")
}

#[test]
fn sql_and_syql_portable_queries_bind_equivalently() {
    let compiler = QueryCompiler::default();
    let cases = [
        (
            "SELECT * FROM neurons WHERE cable_length > 100 LIMIT 5",
            "FROM neurons WHERE cable_length > 100 LIMIT 5",
        ),
        (
            "SELECT neuron_id, RANK() OVER (ORDER BY cable_length DESC) AS rk FROM neurons",
            "SELECT neuron_id, RANK() OVER (ORDER BY cable_length DESC) AS rk FROM neurons",
        ),
        (
            "SELECT neuron_id FROM neurons ORDER BY neuron_id LIMIT 3 OFFSET 1",
            "SELECT neuron_id FROM neurons ORDER BY neuron_id LIMIT 3 OFFSET 1 SCOPE remote DOWNLOAD csv",
        ),
    ];

    for (sql_text, syql_text) in cases {
        let sql = compiler
            .parse(&GenericSqlDialect, sql_text)
            .expect("sql parse");
        let syql = compiler.parse(&SyqlDialect, syql_text).expect("syql parse");

        let sql_bound = bind_and_validate_query(&sql, &catalog(), &QueryParameters::default())
            .expect("sql bind");
        let syql_bound = bind_and_validate_query(&syql, &catalog(), &QueryParameters::default())
            .expect("syql bind");

        assert_eq!(sql.canonical_sql(), syql.canonical_sql(), "{sql_text}");
        assert_eq!(
            sql_bound.result_schema(),
            syql_bound.result_schema(),
            "{sql_text}"
        );

        let sql_pg = compiler
            .emit(&sql_bound, &PostgresAdapter, &catalog())
            .expect("sql postgres emit");
        let syql_pg = compiler
            .emit(&syql_bound, &PostgresAdapter, &catalog())
            .expect("syql postgres emit");
        assert_eq!(
            sql_pg.as_sql().expect("sql pg").text,
            syql_pg.as_sql().expect("syql pg").text,
            "{sql_text}"
        );

        let sql_ch = compiler
            .emit(&sql_bound, &ClickHouseAdapter, &catalog())
            .expect("sql clickhouse emit");
        let syql_ch = compiler
            .emit(&syql_bound, &ClickHouseAdapter, &catalog())
            .expect("syql clickhouse emit");
        assert_eq!(
            sql_ch.as_sql().expect("sql ch").text,
            syql_ch.as_sql().expect("syql ch").text,
            "{sql_text}"
        );
    }
}
