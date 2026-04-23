use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use queryfabric_ir::{
    BinaryOperator, LiteralValue, ParameterRef, ParameterSummary, ParameterValue, ParsedQuery,
    QueryFabricError, QueryParameters, Result, SyntaxExpr, SyntaxExprKind, SyntaxProjectionItem,
    SyntaxQuery, SyntaxRelation, SyntaxSelect, SyntaxSetExpr, SyntaxTableWithJoins,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedQuerySummary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_relation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projected_columns: Option<Vec<String>>,
    pub predicate_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_limit: Option<u64>,
    pub scope: String,
    pub output_format: String,
}

pub fn inspect_parameters(parsed: &ParsedQuery) -> ParameterSummary {
    let mut summary = ParameterSummary::default();
    visit_query(parsed.syntax(), &mut summary);
    summary
}

pub fn inspect_query(
    parsed: &ParsedQuery,
    parameters: Option<&QueryParameters>,
) -> ParsedQuerySummary {
    let select = first_select(parsed.syntax());
    ParsedQuerySummary {
        primary_relation: select.and_then(summary_table),
        projected_columns: select.and_then(summary_columns),
        predicate_count: select
            .and_then(|select| select.selection.as_ref())
            .map(count_predicates)
            .unwrap_or(0),
        row_limit: parsed
            .syntax()
            .limit
            .as_ref()
            .and_then(|expr| extract_limit(expr, parameters)),
        scope: parsed
            .dialect_metadata()
            .get("syql.scope")
            .unwrap_or("local")
            .to_owned(),
        output_format: parsed
            .dialect_metadata()
            .get("syql.download")
            .unwrap_or("arrow")
            .to_owned(),
    }
}

pub fn parameter_value_from_json(value: &Value) -> Result<ParameterValue> {
    Ok(match value {
        Value::Null => ParameterValue::Null,
        Value::Bool(value) => ParameterValue::Boolean(*value),
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                ParameterValue::Int64(value)
            } else if let Some(value) = number.as_u64() {
                ParameterValue::Int64(i64::try_from(value).map_err(|_| {
                    QueryFabricError::Emission(format!(
                        "unsupported numeric parameter value `{number}`"
                    ))
                })?)
            } else if let Some(value) = number.as_f64() {
                ParameterValue::Float64(value.to_string())
            } else {
                return Err(QueryFabricError::Emission(format!(
                    "unsupported numeric parameter value `{number}`"
                )));
            }
        }
        Value::String(value) => {
            if looks_like_uuid(value) {
                ParameterValue::Uuid(value.clone())
            } else {
                ParameterValue::Utf8(value.clone())
            }
        }
        Value::Array(values) => ParameterValue::List(
            values
                .iter()
                .map(parameter_value_from_json)
                .collect::<Result<Vec<_>>>()?,
        ),
        Value::Object(_) => ParameterValue::Json(value.to_string()),
    })
}

pub fn build_query_parameters(
    positional: Option<&[Value]>,
    named: Option<&HashMap<String, Value>>,
) -> Result<QueryParameters> {
    if positional.is_some() && named.is_some() {
        return Err(QueryFabricError::Emission(
            "mixed positional and named parameters are not supported".into(),
        ));
    }

    let mut parameters = QueryParameters::default();
    if let Some(positional) = positional {
        for (index, value) in positional.iter().enumerate() {
            parameters.insert_positional((index + 1) as u32, parameter_value_from_json(value)?);
        }
    }
    if let Some(named) = named {
        for (name, value) in named {
            parameters.insert_named(name.clone(), parameter_value_from_json(value)?);
        }
    }

    Ok(parameters)
}

fn visit_query(query: &SyntaxQuery, summary: &mut ParameterSummary) {
    for cte in &query.ctes {
        visit_query(&cte.query, summary);
    }
    visit_set_expr(&query.body, summary);
    for order_by in &query.order_by {
        visit_expr(&order_by.expr, summary);
    }
    if let Some(limit) = &query.limit {
        visit_expr(limit, summary);
    }
    if let Some(offset) = &query.offset {
        visit_expr(offset, summary);
    }
}

fn visit_set_expr(expr: &SyntaxSetExpr, summary: &mut ParameterSummary) {
    match expr {
        SyntaxSetExpr::Select(select) => visit_select(select, summary),
        SyntaxSetExpr::UnionAll { left, right, .. } => {
            visit_set_expr(left, summary);
            visit_set_expr(right, summary);
        }
        SyntaxSetExpr::Unsupported { .. } => {}
    }
}

fn visit_select(select: &SyntaxSelect, summary: &mut ParameterSummary) {
    for item in &select.projection {
        match item {
            SyntaxProjectionItem::Expr(details) => visit_expr(&details.expr, summary),
            SyntaxProjectionItem::Wildcard { .. } | SyntaxProjectionItem::Unsupported { .. } => {}
        }
    }
    for table in &select.from {
        visit_table_with_joins(table, summary);
    }
    if let Some(selection) = &select.selection {
        visit_expr(selection, summary);
    }
    for group_by in &select.group_by {
        visit_expr(group_by, summary);
    }
    if let Some(having) = &select.having {
        visit_expr(having, summary);
    }
}

fn visit_table_with_joins(table: &SyntaxTableWithJoins, summary: &mut ParameterSummary) {
    visit_relation(&table.relation, summary);
    for join in &table.joins {
        visit_relation(&join.relation, summary);
        if let Some(on) = &join.on {
            visit_expr(on, summary);
        }
    }
}

fn visit_relation(relation: &SyntaxRelation, summary: &mut ParameterSummary) {
    match relation {
        SyntaxRelation::Derived { query, .. } => visit_query(query, summary),
        SyntaxRelation::NestedJoin {
            table_with_joins, ..
        } => visit_table_with_joins(table_with_joins, summary),
        SyntaxRelation::Table { .. } | SyntaxRelation::Unsupported { .. } => {}
    }
}

fn visit_expr(expr: &SyntaxExpr, summary: &mut ParameterSummary) {
    match &expr.kind {
        SyntaxExprKind::Parameter(reference) => record_parameter(reference, summary),
        SyntaxExprKind::Unary { expr, .. }
        | SyntaxExprKind::Cast { expr, .. }
        | SyntaxExprKind::IsNull { expr, .. } => visit_expr(expr, summary),
        SyntaxExprKind::Binary { left, right, .. } => {
            visit_expr(left, summary);
            visit_expr(right, summary);
        }
        SyntaxExprKind::Function(function) => {
            for arg in &function.args {
                visit_expr(arg, summary);
            }
            if let Some(filter) = &function.filter {
                visit_expr(filter, summary);
            }
            if let Some(over) = &function.over {
                for partition_by in &over.partition_by {
                    visit_expr(partition_by, summary);
                }
                for order_by in &over.order_by {
                    visit_expr(&order_by.expr, summary);
                }
            }
        }
        SyntaxExprKind::Case {
            operand,
            when_then,
            else_result,
        } => {
            if let Some(operand) = operand {
                visit_expr(operand, summary);
            }
            for pair in when_then {
                visit_expr(&pair.condition, summary);
                visit_expr(&pair.result, summary);
            }
            if let Some(else_result) = else_result {
                visit_expr(else_result, summary);
            }
        }
        SyntaxExprKind::Between {
            expr, low, high, ..
        } => {
            visit_expr(expr, summary);
            visit_expr(low, summary);
            visit_expr(high, summary);
        }
        SyntaxExprKind::InList { expr, list, .. } => {
            visit_expr(expr, summary);
            for item in list {
                visit_expr(item, summary);
            }
        }
        SyntaxExprKind::InSubquery { expr, subquery, .. } => {
            visit_expr(expr, summary);
            visit_query(subquery, summary);
        }
        SyntaxExprKind::ScalarSubquery(subquery) | SyntaxExprKind::Exists(subquery) => {
            visit_query(subquery, summary);
        }
        SyntaxExprKind::Like { expr, pattern, .. } => {
            visit_expr(expr, summary);
            visit_expr(pattern, summary);
        }
        SyntaxExprKind::Tuple(items) | SyntaxExprKind::Array(items) => {
            for item in items {
                visit_expr(item, summary);
            }
        }
        SyntaxExprKind::Column { .. }
        | SyntaxExprKind::Literal(_)
        | SyntaxExprKind::Unsupported { .. } => {}
    }
}

fn record_parameter(reference: &ParameterRef, summary: &mut ParameterSummary) {
    match reference {
        ParameterRef::Positional(position) => {
            summary.positional_count = summary.positional_count.max(*position);
        }
        ParameterRef::Named(name) => {
            let mut names = summary
                .named_params
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            names.insert(name.clone());
            summary.named_params = names.into_iter().collect();
        }
        _ => {}
    }
}

fn first_select(query: &SyntaxQuery) -> Option<&SyntaxSelect> {
    first_select_in_set_expr(&query.body)
}

fn first_select_in_set_expr(set_expr: &SyntaxSetExpr) -> Option<&SyntaxSelect> {
    match set_expr {
        SyntaxSetExpr::Select(select) => Some(select.as_ref()),
        SyntaxSetExpr::UnionAll { left, right, .. } => {
            first_select_in_set_expr(left).or_else(|| first_select_in_set_expr(right))
        }
        SyntaxSetExpr::Unsupported { .. } => None,
    }
}

fn summary_table(select: &SyntaxSelect) -> Option<String> {
    select.from.first().map(|table| match &table.relation {
        SyntaxRelation::Table { name, .. } => name.display_name(),
        SyntaxRelation::Derived { .. } => "<subquery>".to_owned(),
        SyntaxRelation::NestedJoin { .. } => "<join>".to_owned(),
        SyntaxRelation::Unsupported { .. } => "<unsupported>".to_owned(),
    })
}

fn summary_columns(select: &SyntaxSelect) -> Option<Vec<String>> {
    if select.projection.is_empty() {
        return None;
    }

    let mut columns = Vec::with_capacity(select.projection.len());
    for item in &select.projection {
        match item {
            SyntaxProjectionItem::Wildcard { .. } => return None,
            SyntaxProjectionItem::Expr(details) => {
                let label = details
                    .alias
                    .clone()
                    .unwrap_or_else(|| expression_label(&details.expr));
                columns.push(label);
            }
            SyntaxProjectionItem::Unsupported { .. } => columns.push("<expr>".to_owned()),
        }
    }

    (!columns.is_empty()).then_some(columns)
}

fn expression_label(expr: &SyntaxExpr) -> String {
    match &expr.kind {
        SyntaxExprKind::Column { name, .. } => name.clone(),
        SyntaxExprKind::Function(call) => call.function.display_name(),
        _ => "<expr>".to_owned(),
    }
}

fn count_predicates(expr: &SyntaxExpr) -> usize {
    match &expr.kind {
        SyntaxExprKind::Binary {
            op: BinaryOperator::And,
            left,
            right,
        } => count_predicates(left) + count_predicates(right),
        _ => 1,
    }
}

fn extract_limit(expr: &SyntaxExpr, parameters: Option<&QueryParameters>) -> Option<u64> {
    match &expr.kind {
        SyntaxExprKind::Literal(LiteralValue::Int64(value)) => u64::try_from(*value).ok(),
        SyntaxExprKind::Parameter(reference) => match parameters?.lookup(reference)? {
            ParameterValue::Int64(value) => u64::try_from(*value).ok(),
            _ => None,
        },
        _ => None,
    }
}

fn looks_like_uuid(value: &str) -> bool {
    if value.len() != 36 {
        return false;
    }

    value.bytes().enumerate().all(|(index, byte)| match index {
        8 | 13 | 18 | 23 => byte == b'-',
        _ => byte.is_ascii_hexdigit(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use queryfabric_dialect_sql::GenericSqlDialect;
    use queryfabric_dialect_syql::SyqlDialect;
    use queryfabric_ir::{Dialect, ParameterValue, QueryParameters};
    use serde_json::json;

    use super::{
        build_query_parameters, inspect_parameters, inspect_query, parameter_value_from_json,
    };

    #[test]
    fn collects_positional_and_named_placeholders() {
        let parsed = GenericSqlDialect
            .parse(
                "SELECT neuron_id FROM neurons \
                 WHERE cable_length > $2 \
                 AND dataset_id IN (:ids) \
                 ORDER BY greatest(:lo, $1) DESC",
            )
            .expect("parse");
        let summary = inspect_parameters(&parsed);
        assert_eq!(summary.positional_count, 2);
        assert_eq!(
            summary.named_params,
            vec!["ids".to_owned(), "lo".to_owned()]
        );
    }

    #[test]
    fn inspect_query_resolves_parameterized_limit() {
        let parsed = GenericSqlDialect
            .parse("SELECT neuron_id FROM neurons WHERE cable_length > 100 LIMIT $1")
            .expect("parse");
        let mut parameters = QueryParameters::default();
        parameters.insert_positional(1, ParameterValue::Int64(7));

        let summary = inspect_query(&parsed, Some(&parameters));

        assert_eq!(summary.primary_relation.as_deref(), Some("neurons"));
        assert_eq!(
            summary.projected_columns,
            Some(vec!["neuron_id".to_owned()])
        );
        assert_eq!(summary.predicate_count, 1);
        assert_eq!(summary.row_limit, Some(7));
        assert_eq!(summary.scope, "local");
        assert_eq!(summary.output_format, "arrow");
    }

    #[test]
    fn inspect_query_preserves_syql_metadata_defaults() {
        let parsed = SyqlDialect
            .parse("FROM neurons WHERE cable_length > 100 SCOPE remote DOWNLOAD csv")
            .expect("parse");

        let summary = inspect_query(&parsed, None);

        assert_eq!(summary.scope, "remote");
        assert_eq!(summary.output_format, "csv");
        assert_eq!(summary.row_limit, None);
    }

    #[test]
    fn json_parameter_conversion_handles_scalars_lists_and_objects() {
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
            parameter_value_from_json(&json!("123e4567-e89b-12d3-a456-426614174000"))
                .expect("uuid"),
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
    fn build_query_parameters_rejects_mixed_modes() {
        let positional = vec![json!(1)];
        let named = HashMap::from([(String::from("species"), json!("mouse"))]);

        let error = build_query_parameters(Some(&positional), Some(&named)).expect_err("mixed");

        assert!(
            error
                .to_string()
                .contains("mixed positional and named parameters are not supported")
        );
    }
}
