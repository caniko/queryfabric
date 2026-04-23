use std::collections::BTreeSet;

use queryfabric_ir::{
    BackendClause, BoundExpr, BoundExprKind, BoundQueryPlan, BoundRelation, BoundSelect,
    BoundSetExpr, FunctionRef, ParsedQuery, SyntaxExpr, SyntaxExprKind, SyntaxQuery,
    SyntaxRelation, SyntaxSelect, SyntaxSetExpr,
};
use serde::{Deserialize, Serialize};

use crate::model::{Catalog, FunctionKind};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanFeatures {
    pub has_ctes: bool,
    pub has_recursive_ctes: bool,
    pub has_derived_tables: bool,
    pub has_joins: bool,
    pub has_windows: bool,
    pub has_set_operations: bool,
    pub has_aggregates: bool,
    pub has_distinct_aggregates: bool,
    pub has_scalar_subqueries: bool,
    pub has_in_subqueries: bool,
    pub has_clickhouse_settings: bool,
    pub has_clickhouse_format: bool,
    pub has_limit_offset: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unsupported_set_ops: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub functions: BTreeSet<FunctionRef>,
}

pub fn inspect_plan(query: &ParsedQuery) -> queryfabric_ir::Result<PlanFeatures> {
    Ok(plan_features_from_syntax(query.syntax()))
}

pub fn plan_features_from_syntax(query: &SyntaxQuery) -> PlanFeatures {
    let mut features = PlanFeatures {
        has_ctes: !query.ctes.is_empty(),
        has_recursive_ctes: query.with_recursive,
        has_clickhouse_settings: query
            .backend_clauses
            .iter()
            .any(|clause| matches!(clause, BackendClause::ClickHouseSettings { .. })),
        has_clickhouse_format: query
            .backend_clauses
            .iter()
            .any(|clause| matches!(clause, BackendClause::ClickHouseFormat { .. })),
        has_limit_offset: query.limit.is_some() || query.offset.is_some(),
        ..PlanFeatures::default()
    };
    visit_syntax_set_expr(&query.body, &mut features);
    for cte in &query.ctes {
        merge_plan_features(&mut features, plan_features_from_syntax(&cte.query));
    }
    features
}

pub fn plan_features_from_bound(query: &BoundQueryPlan, catalog: &dyn Catalog) -> PlanFeatures {
    let mut features = PlanFeatures {
        has_ctes: !query.ctes.is_empty(),
        has_clickhouse_settings: query
            .backend_clauses
            .iter()
            .any(|clause| matches!(clause, BackendClause::ClickHouseSettings { .. })),
        has_clickhouse_format: query
            .backend_clauses
            .iter()
            .any(|clause| matches!(clause, BackendClause::ClickHouseFormat { .. })),
        has_limit_offset: query.limit.is_some() || query.offset.is_some(),
        ..PlanFeatures::default()
    };
    visit_bound_set_expr(&query.body, &mut features, catalog);
    for cte in &query.ctes {
        merge_plan_features(&mut features, plan_features_from_bound(&cte.query, catalog));
    }
    features
}

fn merge_plan_features(target: &mut PlanFeatures, other: PlanFeatures) {
    target.has_ctes |= other.has_ctes;
    target.has_recursive_ctes |= other.has_recursive_ctes;
    target.has_derived_tables |= other.has_derived_tables;
    target.has_joins |= other.has_joins;
    target.has_windows |= other.has_windows;
    target.has_set_operations |= other.has_set_operations;
    target.has_aggregates |= other.has_aggregates;
    target.has_distinct_aggregates |= other.has_distinct_aggregates;
    target.has_scalar_subqueries |= other.has_scalar_subqueries;
    target.has_in_subqueries |= other.has_in_subqueries;
    target.has_clickhouse_settings |= other.has_clickhouse_settings;
    target.has_clickhouse_format |= other.has_clickhouse_format;
    target.has_limit_offset |= other.has_limit_offset;
    target.unsupported_set_ops.extend(other.unsupported_set_ops);
    target.functions.extend(other.functions);
}

fn visit_syntax_set_expr(expr: &SyntaxSetExpr, features: &mut PlanFeatures) {
    match expr {
        SyntaxSetExpr::Select(select) => visit_syntax_select(select, features),
        SyntaxSetExpr::UnionAll { left, right, .. } => {
            features.has_set_operations = true;
            visit_syntax_set_expr(left, features);
            visit_syntax_set_expr(right, features);
        }
        SyntaxSetExpr::Unsupported { description, .. } => {
            features.unsupported_set_ops.push(description.clone());
        }
    }
}

fn visit_syntax_select(select: &SyntaxSelect, features: &mut PlanFeatures) {
    for item in &select.projection {
        if let Some(details) = item.as_expr() {
            visit_syntax_expr(&details.expr, features);
        }
    }
    for table in &select.from {
        if !table.joins.is_empty() {
            features.has_joins = true;
        }
        visit_syntax_relation(&table.relation, features);
        for join in &table.joins {
            visit_syntax_relation(&join.relation, features);
            if let Some(on) = &join.on {
                visit_syntax_expr(on, features);
            }
        }
    }
    if let Some(selection) = &select.selection {
        visit_syntax_expr(selection, features);
    }
    for expr in &select.group_by {
        visit_syntax_expr(expr, features);
    }
    if let Some(having) = &select.having {
        visit_syntax_expr(having, features);
    }
}

fn visit_syntax_relation(relation: &SyntaxRelation, features: &mut PlanFeatures) {
    match relation {
        SyntaxRelation::Derived { query, .. } => {
            features.has_derived_tables = true;
            merge_plan_features(features, plan_features_from_syntax(query));
        }
        SyntaxRelation::NestedJoin {
            table_with_joins, ..
        } => {
            features.has_derived_tables = true;
            visit_syntax_relation(&table_with_joins.relation, features);
            for join in &table_with_joins.joins {
                visit_syntax_relation(&join.relation, features);
            }
        }
        SyntaxRelation::Table { .. } | SyntaxRelation::Unsupported { .. } => {}
    }
}

fn visit_syntax_expr(expr: &SyntaxExpr, features: &mut PlanFeatures) {
    match &expr.kind {
        SyntaxExprKind::Binary { left, right, .. } => {
            visit_syntax_expr(left, features);
            visit_syntax_expr(right, features);
        }
        SyntaxExprKind::Unary { expr, .. }
        | SyntaxExprKind::Cast { expr, .. }
        | SyntaxExprKind::IsNull { expr, .. } => visit_syntax_expr(expr, features),
        SyntaxExprKind::Function(function) => {
            features.functions.insert(function.function.clone());
            if function.distinct {
                features.has_distinct_aggregates = true;
            }
            if function.over.is_some() {
                features.has_windows = true;
            }
            match function.function.name.as_str() {
                "count" | "sum" | "avg" | "min" | "max" | "quantile" | "avg_merge"
                | "count_merge" | "sum_merge" => features.has_aggregates = true,
                _ => {}
            }
            for arg in &function.args {
                visit_syntax_expr(arg, features);
            }
        }
        SyntaxExprKind::Case {
            operand,
            when_then,
            else_result,
        } => {
            if let Some(operand) = operand {
                visit_syntax_expr(operand, features);
            }
            for pair in when_then {
                visit_syntax_expr(&pair.condition, features);
                visit_syntax_expr(&pair.result, features);
            }
            if let Some(else_result) = else_result {
                visit_syntax_expr(else_result, features);
            }
        }
        SyntaxExprKind::Between {
            expr, low, high, ..
        } => {
            visit_syntax_expr(expr, features);
            visit_syntax_expr(low, features);
            visit_syntax_expr(high, features);
        }
        SyntaxExprKind::InList { expr, list, .. } => {
            visit_syntax_expr(expr, features);
            for item in list {
                visit_syntax_expr(item, features);
            }
        }
        SyntaxExprKind::InSubquery { expr, subquery, .. } => {
            features.has_in_subqueries = true;
            visit_syntax_expr(expr, features);
            merge_plan_features(features, plan_features_from_syntax(subquery));
        }
        SyntaxExprKind::ScalarSubquery(subquery) | SyntaxExprKind::Exists(subquery) => {
            features.has_scalar_subqueries = true;
            merge_plan_features(features, plan_features_from_syntax(subquery));
        }
        SyntaxExprKind::Like { expr, pattern, .. } => {
            visit_syntax_expr(expr, features);
            visit_syntax_expr(pattern, features);
        }
        SyntaxExprKind::Tuple(items) | SyntaxExprKind::Array(items) => {
            for item in items {
                visit_syntax_expr(item, features);
            }
        }
        SyntaxExprKind::Column { .. }
        | SyntaxExprKind::Literal(_)
        | SyntaxExprKind::Parameter(_)
        | SyntaxExprKind::Unsupported { .. } => {}
    }
}

fn visit_bound_set_expr(expr: &BoundSetExpr, features: &mut PlanFeatures, catalog: &dyn Catalog) {
    match expr {
        BoundSetExpr::Select(select) => visit_bound_select(select, features, catalog),
        BoundSetExpr::UnionAll { left, right, .. } => {
            features.has_set_operations = true;
            visit_bound_set_expr(left, features, catalog);
            visit_bound_set_expr(right, features, catalog);
        }
        BoundSetExpr::Unsupported { .. } => {}
    }
}

fn visit_bound_select(select: &BoundSelect, features: &mut PlanFeatures, catalog: &dyn Catalog) {
    for item in &select.projection {
        if let Some(details) = item.as_expr() {
            visit_bound_expr(&details.expr, features, catalog);
        }
    }
    for table in &select.from {
        if !table.joins.is_empty() {
            features.has_joins = true;
        }
        visit_bound_relation(&table.relation, features, catalog);
        for join in &table.joins {
            visit_bound_relation(&join.relation, features, catalog);
            if let Some(on) = &join.on {
                visit_bound_expr(on, features, catalog);
            }
        }
    }
    if let Some(selection) = &select.selection {
        visit_bound_expr(selection, features, catalog);
    }
    for expr in &select.group_by {
        visit_bound_expr(expr, features, catalog);
    }
    if let Some(having) = &select.having {
        visit_bound_expr(having, features, catalog);
    }
}

fn visit_bound_relation(
    relation: &BoundRelation,
    features: &mut PlanFeatures,
    catalog: &dyn Catalog,
) {
    match relation {
        BoundRelation::Derived { query, .. } => {
            features.has_derived_tables = true;
            merge_plan_features(features, plan_features_from_bound(query, catalog));
        }
        BoundRelation::NestedJoin {
            table_with_joins, ..
        } => {
            features.has_derived_tables = true;
            visit_bound_relation(&table_with_joins.relation, features, catalog);
            for join in &table_with_joins.joins {
                visit_bound_relation(&join.relation, features, catalog);
            }
        }
        BoundRelation::Table { .. } | BoundRelation::Unsupported { .. } => {}
    }
}

fn visit_bound_expr(expr: &BoundExpr, features: &mut PlanFeatures, catalog: &dyn Catalog) {
    match &expr.kind {
        BoundExprKind::Binary { left, right, .. } => {
            visit_bound_expr(left, features, catalog);
            visit_bound_expr(right, features, catalog);
        }
        BoundExprKind::Unary { expr, .. }
        | BoundExprKind::Cast { expr, .. }
        | BoundExprKind::IsNull { expr, .. } => visit_bound_expr(expr, features, catalog),
        BoundExprKind::Function(function) => {
            features.functions.insert(function.function.clone());
            if let Some(signature) = catalog.resolve_function(
                function.function.namespace.as_deref(),
                &function.function.name,
            ) {
                match signature.kind {
                    FunctionKind::Aggregate => features.has_aggregates = true,
                    FunctionKind::Window => features.has_windows = true,
                    FunctionKind::Scalar => {}
                }
                if signature.metadata_flag("approximate") {
                    features.has_aggregates = true;
                }
            }
            if function.distinct {
                features.has_distinct_aggregates = true;
            }
            if function.over.is_some() {
                features.has_windows = true;
            }
            for arg in &function.args {
                visit_bound_expr(arg, features, catalog);
            }
        }
        BoundExprKind::Case {
            operand,
            when_then,
            else_result,
        } => {
            if let Some(operand) = operand {
                visit_bound_expr(operand, features, catalog);
            }
            for pair in when_then {
                visit_bound_expr(&pair.condition, features, catalog);
                visit_bound_expr(&pair.result, features, catalog);
            }
            if let Some(else_result) = else_result {
                visit_bound_expr(else_result, features, catalog);
            }
        }
        BoundExprKind::Between {
            expr, low, high, ..
        } => {
            visit_bound_expr(expr, features, catalog);
            visit_bound_expr(low, features, catalog);
            visit_bound_expr(high, features, catalog);
        }
        BoundExprKind::InList { expr, list, .. } => {
            visit_bound_expr(expr, features, catalog);
            for item in list {
                visit_bound_expr(item, features, catalog);
            }
        }
        BoundExprKind::InSubquery { expr, subquery, .. } => {
            features.has_in_subqueries = true;
            visit_bound_expr(expr, features, catalog);
            merge_plan_features(features, plan_features_from_bound(subquery, catalog));
        }
        BoundExprKind::ScalarSubquery(subquery) | BoundExprKind::Exists(subquery) => {
            features.has_scalar_subqueries = true;
            merge_plan_features(features, plan_features_from_bound(subquery, catalog));
        }
        BoundExprKind::Like { expr, pattern, .. } => {
            visit_bound_expr(expr, features, catalog);
            visit_bound_expr(pattern, features, catalog);
        }
        BoundExprKind::Tuple(items) | BoundExprKind::Array(items) => {
            for item in items {
                visit_bound_expr(item, features, catalog);
            }
        }
        BoundExprKind::Column(_)
        | BoundExprKind::Literal(_)
        | BoundExprKind::Parameter(_)
        | BoundExprKind::Unsupported { .. } => {}
    }
}
