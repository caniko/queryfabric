use std::collections::HashMap;

use queryfabric::{
    GenericSqlDialect, ParameterValue, QueryCompiler, QueryParameters, SyqlDialect,
    build_query_parameters, inspect_query, parameter_value_from_json,
};
use serde_json::json;

#[test]
fn inspect_query_reports_portable_shape_and_parameterized_limit() {
    let parsed = QueryCompiler::default()
        .parse(
            &GenericSqlDialect,
            "SELECT neuron_id FROM neurons WHERE cable_length > 100 LIMIT $1",
        )
        .expect("parse");
    let mut parameters = QueryParameters::default();
    parameters.insert_positional(1, ParameterValue::Int64(9));

    let summary = inspect_query(&parsed, Some(&parameters));

    assert_eq!(summary.primary_relation.as_deref(), Some("neurons"));
    assert_eq!(
        summary.projected_columns,
        Some(vec!["neuron_id".to_owned()])
    );
    assert_eq!(summary.predicate_count, 1);
    assert_eq!(summary.row_limit, Some(9));
    assert_eq!(summary.scope, "local");
    assert_eq!(summary.output_format, "arrow");
}

#[test]
fn inspect_query_preserves_syql_scope_and_download_metadata() {
    let parsed = QueryCompiler::default()
        .parse(
            &SyqlDialect,
            "FROM neurons WHERE cable_length > 100 SCOPE remote DOWNLOAD csv",
        )
        .expect("parse");

    let summary = inspect_query(&parsed, None);

    assert_eq!(summary.scope, "remote");
    assert_eq!(summary.output_format, "csv");
    assert_eq!(summary.row_limit, None);
}

#[test]
fn parameter_helpers_convert_json_values() {
    assert_eq!(
        parameter_value_from_json(&json!(null)).expect("null"),
        ParameterValue::Null
    );
    assert_eq!(
        parameter_value_from_json(&json!(true)).expect("bool"),
        ParameterValue::Boolean(true)
    );
    assert_eq!(
        parameter_value_from_json(&json!(42)).expect("int"),
        ParameterValue::Int64(42)
    );
    assert_eq!(
        parameter_value_from_json(&json!(42.5)).expect("float"),
        ParameterValue::Float64("42.5".into())
    );
    assert_eq!(
        parameter_value_from_json(&json!("123e4567-e89b-12d3-a456-426614174000")).expect("uuid"),
        ParameterValue::Uuid("123e4567-e89b-12d3-a456-426614174000".into())
    );
    assert_eq!(
        parameter_value_from_json(&json!("mouse")).expect("text"),
        ParameterValue::Utf8("mouse".into())
    );
    assert_eq!(
        parameter_value_from_json(&json!([1, "mouse"])).expect("array"),
        ParameterValue::List(vec![
            ParameterValue::Int64(1),
            ParameterValue::Utf8("mouse".into()),
        ])
    );
    assert_eq!(
        parameter_value_from_json(&json!({"species": "mouse", "count": 2})).expect("object"),
        ParameterValue::Json(r#"{"count":2,"species":"mouse"}"#.into())
    );
}

#[test]
fn build_query_parameters_rejects_mixed_positional_and_named_modes() {
    let positional = vec![json!(1)];
    let named = HashMap::from([(String::from("species"), json!("mouse"))]);

    let error = build_query_parameters(Some(&positional), Some(&named)).expect_err("mixed");

    assert!(
        error
            .to_string()
            .contains("mixed positional and named parameters are not supported")
    );
}
