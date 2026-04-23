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
    let bound = bind("SELECT neuron_id, cable_length FROM neurons");
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("neuron_id", DataType::Uuid, false),
            field("cable_length", DataType::Float64, true),
        ]
    );
}

#[test]
fn coalesce_with_non_nullable_fallback_is_non_nullable() {
    let bound = bind("SELECT COALESCE(cable_length, 0.0) AS stabilized_length FROM neurons");
    assert_eq!(
        bound.result_schema().fields(),
        &[field("stabilized_length", DataType::Float64, false)]
    );
}

#[test]
fn select_star_schema_expands_relation_fields() {
    let bound = bind("SELECT * FROM neurons");
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("neuron_id", DataType::Uuid, false),
            field("cable_length", DataType::Float64, true),
        ]
    );
}

#[test]
fn aggregate_schema_preserves_empty_input_nullability() {
    let bound = bind(
        "SELECT COUNT(weight) AS ct, SUM(weight) AS total_weight, AVG(weight) AS mean_weight, MIN(weight) AS min_weight, MAX(weight) AS max_weight FROM synapses",
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
        "SELECT n.* FROM neurons AS n LEFT JOIN synapses AS s ON n.neuron_id = s.target_neuron_id",
    );
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("neuron_id", DataType::Uuid, false),
            field("cable_length", DataType::Float64, true),
        ]
    );
}

#[test]
fn join_projection_schema_tracks_all_fields() {
    let bound = bind(
        "SELECT * FROM neurons AS n INNER JOIN synapses AS s ON n.neuron_id = s.target_neuron_id",
    );
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("neuron_id", DataType::Uuid, false),
            field("cable_length", DataType::Float64, true),
            field("source_neuron_id", DataType::Uuid, false),
            field("target_neuron_id", DataType::Uuid, false),
            field("weight", DataType::Float64, false),
        ]
    );
}

#[test]
fn lag_over_non_nullable_input_is_nullable() {
    let bound = bind("SELECT LAG(weight) OVER (ORDER BY weight) AS previous_weight FROM synapses");
    assert_eq!(
        bound.result_schema().fields(),
        &[field("previous_weight", DataType::Float64, true)]
    );
}

#[test]
fn like_over_nullable_input_is_nullable() {
    let bound = bind("SELECT CAST(cable_length AS Utf8) LIKE '1%' AS matches_prefix FROM neurons");
    assert_eq!(
        bound.result_schema().fields(),
        &[field("matches_prefix", DataType::Boolean, true)]
    );
}

#[test]
fn between_propagates_nullable_bounds() {
    let bound = bind(
        "SELECT weight BETWEEN cable_length AND 10.0 AS within_range FROM neurons CROSS JOIN synapses",
    );
    assert_eq!(
        bound.result_schema().fields(),
        &[field("within_range", DataType::Boolean, true)]
    );
}

#[test]
fn in_list_propagates_nullable_items() {
    let bound = bind(
        "SELECT weight IN (cable_length, 1.0) AS matches_any FROM neurons CROSS JOIN synapses",
    );
    assert_eq!(
        bound.result_schema().fields(),
        &[field("matches_any", DataType::Boolean, true)]
    );
}

#[test]
fn left_join_projection_makes_right_side_nullable() {
    let bound = bind(
        "SELECT n.neuron_id, s.weight FROM neurons AS n LEFT JOIN synapses AS s ON n.neuron_id = s.target_neuron_id",
    );
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("neuron_id", DataType::Uuid, false),
            field("weight", DataType::Float64, true),
        ]
    );
}

#[test]
fn right_join_projection_makes_left_side_nullable() {
    let bound = bind(
        "SELECT n.neuron_id, s.weight FROM neurons AS n RIGHT JOIN synapses AS s ON n.neuron_id = s.target_neuron_id",
    );
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("neuron_id", DataType::Uuid, true),
            field("weight", DataType::Float64, false),
        ]
    );
}

#[test]
fn full_join_projection_makes_both_sides_nullable() {
    let bound = bind(
        "SELECT n.neuron_id, s.weight FROM neurons AS n FULL JOIN synapses AS s ON n.neuron_id = s.target_neuron_id",
    );
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("neuron_id", DataType::Uuid, true),
            field("weight", DataType::Float64, true),
        ]
    );
}

#[test]
fn derived_subquery_schema_flows_through_alias() {
    let bound = bind("SELECT derived.neuron_id FROM (SELECT neuron_id FROM neurons) AS derived");
    assert_eq!(
        bound.result_schema().fields(),
        &[field("neuron_id", DataType::Uuid, false)]
    );
}

#[test]
fn cte_schema_flows_to_consumers() {
    let bound = bind("WITH recent AS (SELECT neuron_id FROM neurons) SELECT neuron_id FROM recent");
    assert_eq!(
        bound.result_schema().fields(),
        &[field("neuron_id", DataType::Uuid, false)]
    );
}

#[test]
fn scalar_subquery_projection_schema_is_typed_and_conservatively_nullable() {
    let bound = bind(
        "SELECT neuron_id, (SELECT COUNT(weight) FROM synapses) AS synapse_count FROM neurons",
    );
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("neuron_id", DataType::Uuid, false),
            field("synapse_count", DataType::Int64, true),
        ]
    );
}

#[test]
fn in_subquery_is_nullable_when_inner_values_are_nullable() {
    let bound = bind(
        "SELECT cable_length IN (SELECT cable_length FROM neurons) AS maybe_match FROM neurons",
    );
    assert_eq!(
        bound.result_schema().fields(),
        &[field("maybe_match", DataType::Boolean, true)]
    );
}

#[test]
fn union_all_schema_uses_left_names_and_common_types() {
    let bound =
        bind("SELECT neuron_id FROM neurons UNION ALL SELECT source_neuron_id FROM synapses");
    assert_eq!(
        bound.result_schema().fields(),
        &[field("neuron_id", DataType::Uuid, false)]
    );
}

#[test]
fn union_all_schema_widens_nullability_across_branches() {
    let bound = bind("SELECT weight FROM synapses UNION ALL SELECT cable_length FROM neurons");
    assert_eq!(
        bound.result_schema().fields(),
        &[field("weight", DataType::Float64, true)]
    );
}

#[test]
fn window_function_schema_is_inferred() {
    let bound =
        bind("SELECT neuron_id, RANK() OVER (ORDER BY cable_length DESC) AS rk FROM neurons");
    assert_eq!(
        bound.result_schema().fields(),
        &[
            field("neuron_id", DataType::Uuid, false),
            field("rk", DataType::Int64, false),
        ]
    );
}
