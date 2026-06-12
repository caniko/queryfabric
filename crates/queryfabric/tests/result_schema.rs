use queryfabric::{
    Catalog, DataType, GenericSqlDialect, QueryCompiler, QueryParameters, ResultField,
    bind_and_validate_query,
};

fn catalog() -> impl Catalog {
    queryfabric::portable_catalog("result-schema")
}

fn bind(query: &str) -> queryfabric::BoundQuery {
    let compiler = QueryCompiler::default();
    let parsed = compiler
        .parse(&GenericSqlDialect, query)
        .expect("parse should succeed");
    bind_and_validate_query(&parsed, &catalog(), &QueryParameters::default()).expect("bind")
}

fn field(name: &str, data_type: DataType, nullable: bool) -> ResultField {
    ResultField {
        name: name.into(),
        data_type,
        nullable,
        metadata: Default::default(),
    }
}

#[test]
fn simple_projection_schema_is_precise() {
    let bound = bind("SELECT record_id, score FROM records");
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("record_id", DataType::Uuid, false),
            field("score", DataType::Float64, true),
        ]
    );
}

#[test]
fn coalesce_with_non_nullable_fallback_is_non_nullable() {
    let bound = bind("SELECT COALESCE(score, 0.0) AS stabilized_score FROM records");
    assert_eq!(
        bound.result_schema().fields(),
        &[field("stabilized_score", DataType::Float64, false)]
    );
}

#[test]
fn select_star_schema_expands_relation_fields() {
    let bound = bind("SELECT * FROM records");
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("record_id", DataType::Uuid, false),
            field("score", DataType::Float64, true),
        ]
    );
}

#[test]
fn aggregate_schema_preserves_empty_input_nullability() {
    let bound = bind(
        "SELECT COUNT(weight) AS ct, SUM(weight) AS total_weight, AVG(weight) AS mean_weight, MIN(weight) AS min_weight, MAX(weight) AS max_weight FROM links",
    );
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("ct", DataType::Int64, false),
            field("total_weight", DataType::Float64, true),
            field("mean_weight", DataType::Float64, true),
            field("min_weight", DataType::Float64, true),
            field("max_weight", DataType::Float64, true),
        ]
    );
}

#[test]
fn qualified_wildcard_schema_stays_scoped() {
    let bound = bind(
        "SELECT n.* FROM records AS n LEFT JOIN links AS s ON n.record_id = s.target_record_id",
    );
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("record_id", DataType::Uuid, false),
            field("score", DataType::Float64, true),
        ]
    );
}

#[test]
fn join_projection_schema_tracks_all_fields() {
    let bound = bind(
        "SELECT * FROM records AS n INNER JOIN links AS s ON n.record_id = s.target_record_id",
    );
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("record_id", DataType::Uuid, false),
            field("score", DataType::Float64, true),
            field("source_record_id", DataType::Uuid, false),
            field("target_record_id", DataType::Uuid, false),
            field("weight", DataType::Float64, false),
        ]
    );
}

#[test]
fn lag_over_non_nullable_input_is_nullable() {
    let bound = bind("SELECT LAG(weight) OVER (ORDER BY weight) AS previous_weight FROM links");
    assert_eq!(
        bound.result_schema().fields(),
        &[field("previous_weight", DataType::Float64, true)]
    );
}

#[test]
fn like_over_nullable_input_is_nullable() {
    let bound = bind("SELECT CAST(score AS Utf8) LIKE '1%' AS matches_prefix FROM records");
    assert_eq!(
        bound.result_schema().fields(),
        &[field("matches_prefix", DataType::Boolean, true)]
    );
}

#[test]
fn between_propagates_nullable_bounds() {
    let bound =
        bind("SELECT weight BETWEEN score AND 10.0 AS within_range FROM records CROSS JOIN links");
    assert_eq!(
        bound.result_schema().fields(),
        &[field("within_range", DataType::Boolean, true)]
    );
}

#[test]
fn in_list_propagates_nullable_items() {
    let bound = bind("SELECT weight IN (score, 1.0) AS matches_any FROM records CROSS JOIN links");
    assert_eq!(
        bound.result_schema().fields(),
        &[field("matches_any", DataType::Boolean, true)]
    );
}

#[test]
fn left_join_projection_makes_right_side_nullable() {
    let bound = bind(
        "SELECT n.record_id, s.weight FROM records AS n LEFT JOIN links AS s ON n.record_id = s.target_record_id",
    );
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("record_id", DataType::Uuid, false),
            field("weight", DataType::Float64, true),
        ]
    );
}

#[test]
fn right_join_projection_makes_left_side_nullable() {
    let bound = bind(
        "SELECT n.record_id, s.weight FROM records AS n RIGHT JOIN links AS s ON n.record_id = s.target_record_id",
    );
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("record_id", DataType::Uuid, true),
            field("weight", DataType::Float64, false),
        ]
    );
}

#[test]
fn full_join_projection_makes_both_sides_nullable() {
    let bound = bind(
        "SELECT n.record_id, s.weight FROM records AS n FULL JOIN links AS s ON n.record_id = s.target_record_id",
    );
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("record_id", DataType::Uuid, true),
            field("weight", DataType::Float64, true),
        ]
    );
}

#[test]
fn derived_subquery_schema_flows_through_alias() {
    let bound = bind("SELECT derived.record_id FROM (SELECT record_id FROM records) AS derived");
    assert_eq!(
        bound.result_schema().fields(),
        &[field("record_id", DataType::Uuid, false)]
    );
}

#[test]
fn cte_schema_flows_to_consumers() {
    let bound = bind("WITH recent AS (SELECT record_id FROM records) SELECT record_id FROM recent");
    assert_eq!(
        bound.result_schema().fields(),
        &[field("record_id", DataType::Uuid, false)]
    );
}

#[test]
fn scalar_subquery_projection_schema_is_typed_and_conservatively_nullable() {
    let bound =
        bind("SELECT record_id, (SELECT COUNT(weight) FROM links) AS link_count FROM records");
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("record_id", DataType::Uuid, false),
            field("link_count", DataType::Int64, true),
        ]
    );
}

#[test]
fn in_subquery_is_nullable_when_inner_values_are_nullable() {
    let bound = bind("SELECT score IN (SELECT score FROM records) AS maybe_match FROM records");
    assert_eq!(
        bound.result_schema().fields(),
        &[field("maybe_match", DataType::Boolean, true)]
    );
}

#[test]
fn union_all_schema_uses_left_names_and_common_types() {
    let bound = bind("SELECT record_id FROM records UNION ALL SELECT source_record_id FROM links");
    assert_eq!(
        bound.result_schema().fields(),
        &[field("record_id", DataType::Uuid, false)]
    );
}

#[test]
fn union_all_schema_widens_nullability_across_branches() {
    let bound = bind("SELECT weight FROM links UNION ALL SELECT score FROM records");
    assert_eq!(
        bound.result_schema().fields(),
        &[field("weight", DataType::Float64, true)]
    );
}

#[test]
fn window_function_schema_is_inferred() {
    let bound = bind("SELECT record_id, RANK() OVER (ORDER BY score DESC) AS rk FROM records");
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("record_id", DataType::Uuid, false),
            field("rk", DataType::Int64, false),
        ]
    );
}
