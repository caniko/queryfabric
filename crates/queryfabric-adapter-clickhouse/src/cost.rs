use std::collections::BTreeSet;

use queryfabric_catalog::{Catalog, CostEstimateError, EstimatedCost};
use queryfabric_ir::{
    BinaryOperator, BoundExpr, BoundExprKind, BoundFunctionCall, BoundQuery, BoundQueryPlan,
    BoundRelation, BoundSelect, BoundSetExpr, BoundTableWithJoins, DataType, LiteralValue,
};

pub(super) struct RelationCostContext {
    pub(super) namespace: Option<String>,
    pub(super) name: String,
    pub(super) binding_name: String,
    pub(super) estimated_rows: u64,
    pub(super) average_row_bytes: u64,
    pub(super) partition_columns: BTreeSet<String>,
    pub(super) partition_count: u32,
}

pub(super) fn estimate_clickhouse_cost(
    query: &BoundQuery,
    catalog: &dyn Catalog,
) -> std::result::Result<EstimatedCost, CostEstimateError> {
    let mut relations = Vec::new();
    collect_relation_cost_contexts(query.plan(), catalog, &mut relations)?;
    if relations.is_empty() {
        return Err(CostEstimateError::MissingStatistics(
            "query does not reference a catalog relation".into(),
        ));
    }

    let rows_scanned = relations
        .iter()
        .map(|relation| relation.estimated_rows)
        .sum::<u64>()
        .max(1);
    let bytes_scanned = relations
        .iter()
        .map(|relation| {
            relation
                .estimated_rows
                .saturating_mul(relation.average_row_bytes)
        })
        .sum::<u64>()
        .max(1);
    let partitions_touched = estimate_partitions_touched(query.plan(), &relations).max(1);
    let memory_bytes = estimate_memory_bytes(query.plan(), rows_scanned, bytes_scanned).max(1);
    let wallclock_estimate_ms = rows_scanned.div_ceil(5_000_000).max(1);

    Ok(EstimatedCost {
        memory_bytes,
        rows_scanned,
        partitions_touched,
        wallclock_estimate_ms,
    })
}

fn collect_relation_cost_contexts(
    plan: &BoundQueryPlan,
    catalog: &dyn Catalog,
    relations: &mut Vec<RelationCostContext>,
) -> std::result::Result<(), CostEstimateError> {
    for cte in &plan.ctes {
        collect_relation_cost_contexts(&cte.query, catalog, relations)?;
    }
    collect_set_expr_relations(&plan.body, catalog, relations)
}

fn collect_set_expr_relations(
    expr: &BoundSetExpr,
    catalog: &dyn Catalog,
    relations: &mut Vec<RelationCostContext>,
) -> std::result::Result<(), CostEstimateError> {
    match expr {
        BoundSetExpr::Select(select) => {
            for table in &select.from {
                collect_table_relations(table, catalog, relations)?;
            }
            Ok(())
        }
        BoundSetExpr::UnionAll { left, right, .. } => {
            collect_set_expr_relations(left, catalog, relations)?;
            collect_set_expr_relations(right, catalog, relations)
        }
        BoundSetExpr::Unsupported { description, .. } => Err(CostEstimateError::Backend(format!(
            "unsupported bound set expression: {description}"
        ))),
    }
}

fn collect_table_relations(
    table: &BoundTableWithJoins,
    catalog: &dyn Catalog,
    relations: &mut Vec<RelationCostContext>,
) -> std::result::Result<(), CostEstimateError> {
    collect_relation(&table.relation, catalog, relations)?;
    for join in &table.joins {
        collect_relation(&join.relation, catalog, relations)?;
    }
    Ok(())
}

fn collect_relation(
    relation: &BoundRelation,
    catalog: &dyn Catalog,
    relations: &mut Vec<RelationCostContext>,
) -> std::result::Result<(), CostEstimateError> {
    match relation {
        BoundRelation::Table { binding, .. } => {
            let relation_name = binding.relation_name.as_ref().ok_or_else(|| {
                CostEstimateError::MissingStatistics(format!(
                    "table binding {} has no relation name",
                    binding.binding_name
                ))
            })?;
            let stats = catalog
                .relation_statistics(relation_name.namespace.as_deref(), &relation_name.name)
                .ok_or_else(|| {
                    CostEstimateError::MissingStatistics(relation_name.display_name())
                })?;
            let schema = catalog
                .resolve_relation(relation_name.namespace.as_deref(), &relation_name.name)
                .ok_or_else(|| {
                    CostEstimateError::MissingStatistics(relation_name.display_name())
                })?;
            relations.push(RelationCostContext {
                namespace: relation_name.namespace.clone(),
                name: relation_name.name.clone(),
                binding_name: binding.binding_name.clone(),
                estimated_rows: stats.estimated_rows,
                average_row_bytes: stats.average_row_bytes,
                partition_columns: partition_columns(&schema),
                partition_count: relation_partition_count(&schema),
            });
            Ok(())
        }
        BoundRelation::Derived { query, .. } => {
            collect_relation_cost_contexts(query, catalog, relations)
        }
        BoundRelation::NestedJoin {
            table_with_joins, ..
        } => collect_table_relations(table_with_joins, catalog, relations),
        BoundRelation::Unsupported { description, .. } => Err(CostEstimateError::Backend(format!(
            "unsupported bound relation: {description}"
        ))),
    }
}

fn partition_columns(relation: &queryfabric_catalog::RelationSchema) -> BTreeSet<String> {
    relation
        .metadata
        .get("partition_columns")
        .or_else(|| relation.metadata.get("partition_column"))
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(|name| name.to_ascii_lowercase())
                .collect()
        })
        .unwrap_or_default()
}

fn relation_partition_count(relation: &queryfabric_catalog::RelationSchema) -> u32 {
    relation
        .metadata
        .get("partition_count")
        .and_then(|value| value.parse().ok())
        .unwrap_or(1)
        .max(1)
}

fn estimate_partitions_touched(plan: &BoundQueryPlan, relations: &[RelationCostContext]) -> u32 {
    let mut touched = 0_u32;
    for relation in relations {
        let pruned = relation
            .partition_columns
            .iter()
            .filter_map(|column| partition_predicate_span(&plan.body, relation, column))
            .min();
        touched = touched.saturating_add(pruned.unwrap_or(relation.partition_count));
    }
    touched
}

fn partition_predicate_span(
    expr: &BoundSetExpr,
    relation: &RelationCostContext,
    column: &str,
) -> Option<u32> {
    match expr {
        BoundSetExpr::Select(select) => select
            .selection
            .as_ref()
            .and_then(|selection| partition_expr_span(selection, relation, column)),
        BoundSetExpr::UnionAll { left, right, .. } => {
            match (
                partition_predicate_span(left, relation, column),
                partition_predicate_span(right, relation, column),
            ) {
                (Some(left), Some(right)) => Some(left.saturating_add(right)),
                (Some(count), None) | (None, Some(count)) => Some(count),
                (None, None) => None,
            }
        }
        BoundSetExpr::Unsupported { .. } => None,
    }
}

fn partition_expr_span(
    expr: &BoundExpr,
    relation: &RelationCostContext,
    column: &str,
) -> Option<u32> {
    match &expr.kind {
        BoundExprKind::Binary { op, left, right } => match op {
            BinaryOperator::And => match (
                partition_expr_span(left, relation, column),
                partition_expr_span(right, relation, column),
            ) {
                (Some(left), Some(right)) => Some(left.min(right)),
                (Some(count), None) | (None, Some(count)) => Some(count),
                (None, None) => None,
            },
            BinaryOperator::Or => match (
                partition_expr_span(left, relation, column),
                partition_expr_span(right, relation, column),
            ) {
                (Some(left), Some(right)) => Some(left.saturating_add(right)),
                _ => None,
            },
            BinaryOperator::Eq
            | BinaryOperator::Lt
            | BinaryOperator::LtEq
            | BinaryOperator::Gt
            | BinaryOperator::GtEq
                if compares_partition_to_literal(left, right, relation, column)
                    || compares_partition_to_literal(right, left, relation, column) =>
            {
                Some(1)
            }
            _ => None,
        },
        BoundExprKind::Between {
            expr,
            low,
            high,
            negated: false,
        } if is_partition_column(expr, relation, column)
            && is_literal_expr(low)
            && is_literal_expr(high) =>
        {
            Some(2)
        }
        BoundExprKind::InList {
            expr,
            list,
            negated: false,
        } if is_partition_column(expr, relation, column) => {
            let literal_count = list.iter().filter(|item| is_literal_expr(item)).count();
            (literal_count > 0).then_some(u32::try_from(literal_count).unwrap_or(u32::MAX))
        }
        _ => None,
    }
}

fn compares_partition_to_literal(
    maybe_column: &BoundExpr,
    maybe_literal: &BoundExpr,
    relation: &RelationCostContext,
    column: &str,
) -> bool {
    is_partition_column(maybe_column, relation, column) && is_literal_expr(maybe_literal)
}

fn is_partition_column(expr: &BoundExpr, relation: &RelationCostContext, column: &str) -> bool {
    let BoundExprKind::Column(column_ref) = &expr.kind else {
        return false;
    };
    if !column_ref.name.eq_ignore_ascii_case(column) {
        return false;
    }
    column_ref.relation.as_ref().is_none_or(|qualifier| {
        qualifier.eq_ignore_ascii_case(&relation.binding_name)
            || qualifier.eq_ignore_ascii_case(&relation.name)
            || relation
                .namespace
                .as_ref()
                .is_some_and(|namespace| qualifier.eq_ignore_ascii_case(namespace))
    })
}

fn is_literal_expr(expr: &BoundExpr) -> bool {
    matches!(
        expr.kind,
        BoundExprKind::Literal(
            LiteralValue::Boolean(_)
                | LiteralValue::Int64(_)
                | LiteralValue::Float64(_)
                | LiteralValue::Utf8(_)
        )
    )
}

fn estimate_memory_bytes(plan: &BoundQueryPlan, rows_scanned: u64, bytes_scanned: u64) -> u64 {
    estimate_set_expr_memory(&plan.body, rows_scanned, bytes_scanned)
}

fn estimate_set_expr_memory(expr: &BoundSetExpr, rows_scanned: u64, bytes_scanned: u64) -> u64 {
    match expr {
        BoundSetExpr::Select(select) => estimate_select_memory(select, rows_scanned, bytes_scanned),
        BoundSetExpr::UnionAll { left, right, .. } => {
            estimate_set_expr_memory(left, rows_scanned, bytes_scanned)
                .saturating_add(estimate_set_expr_memory(right, rows_scanned, bytes_scanned))
        }
        BoundSetExpr::Unsupported { .. } => 1,
    }
}

fn estimate_select_memory(select: &BoundSelect, rows_scanned: u64, bytes_scanned: u64) -> u64 {
    let aggregate_count = select_aggregate_count(select);
    if select.group_by.is_empty() {
        return if aggregate_count > 0 {
            u64::from(aggregate_count)
                .saturating_mul(64)
                .saturating_add(128)
        } else {
            bytes_scanned
        };
    }

    let group_key_bytes = select
        .group_by
        .iter()
        .map(|expr| data_type_size(&expr.data_type))
        .sum::<u64>()
        .max(8);
    let groups = select
        .group_by
        .iter()
        .fold(1_u64, |acc, _| acc.saturating_mul(1_000))
        .min(rows_scanned)
        .max(1);
    let aggregate_state_bytes = u64::from(aggregate_count.max(1)).saturating_mul(64);

    groups.saturating_mul(
        group_key_bytes
            .saturating_add(aggregate_state_bytes)
            .saturating_add(32),
    )
}

fn select_aggregate_count(select: &BoundSelect) -> u32 {
    let mut count = 0_u32;
    for item in &select.projection {
        if let Some(expr) = item.as_expr() {
            count = count.saturating_add(expr_aggregate_count(&expr.expr));
        }
    }
    if let Some(having) = &select.having {
        count = count.saturating_add(expr_aggregate_count(having));
    }
    count
}

fn expr_aggregate_count(expr: &BoundExpr) -> u32 {
    match &expr.kind {
        BoundExprKind::Function(function) => {
            let nested = function
                .args
                .iter()
                .map(expr_aggregate_count)
                .sum::<u32>()
                .saturating_add(
                    function
                        .filter
                        .as_ref()
                        .map(|filter| expr_aggregate_count(filter))
                        .unwrap_or(0),
                );
            if is_aggregate_function(function) {
                nested.saturating_add(1)
            } else {
                nested
            }
        }
        BoundExprKind::Unary { expr, .. } | BoundExprKind::Cast { expr, .. } => {
            expr_aggregate_count(expr)
        }
        BoundExprKind::Binary { left, right, .. } => {
            expr_aggregate_count(left).saturating_add(expr_aggregate_count(right))
        }
        BoundExprKind::Case {
            operand,
            when_then,
            else_result,
        } => operand
            .as_deref()
            .map(expr_aggregate_count)
            .unwrap_or(0)
            .saturating_add(
                when_then
                    .iter()
                    .map(|branch| {
                        expr_aggregate_count(&branch.condition)
                            .saturating_add(expr_aggregate_count(&branch.result))
                    })
                    .sum(),
            )
            .saturating_add(
                else_result
                    .as_deref()
                    .map(expr_aggregate_count)
                    .unwrap_or(0),
            ),
        BoundExprKind::Between {
            expr, low, high, ..
        } => expr_aggregate_count(expr)
            .saturating_add(expr_aggregate_count(low))
            .saturating_add(expr_aggregate_count(high)),
        BoundExprKind::InList { expr, list, .. } => expr_aggregate_count(expr)
            .saturating_add(list.iter().map(expr_aggregate_count).sum::<u32>()),
        BoundExprKind::Tuple(items) | BoundExprKind::Array(items) => {
            items.iter().map(expr_aggregate_count).sum()
        }
        BoundExprKind::Like { expr, pattern, .. } => {
            expr_aggregate_count(expr).saturating_add(expr_aggregate_count(pattern))
        }
        BoundExprKind::IsNull { expr, .. } => expr_aggregate_count(expr),
        BoundExprKind::InSubquery { expr, .. } => expr_aggregate_count(expr),
        BoundExprKind::Column(_)
        | BoundExprKind::Literal(_)
        | BoundExprKind::Parameter(_)
        | BoundExprKind::ScalarSubquery(_)
        | BoundExprKind::Exists(_)
        | BoundExprKind::Unsupported { .. } => 0,
    }
}

fn is_aggregate_function(function: &BoundFunctionCall) -> bool {
    matches!(
        function.function.name.to_ascii_lowercase().as_str(),
        "avg"
            | "count"
            | "countmerge"
            | "count_merge"
            | "max"
            | "min"
            | "sum"
            | "summerge"
            | "sum_merge"
            | "uniq"
            | "uniqexact"
            | "uniq_exact"
    ) || function.distinct
}

fn data_type_size(data_type: &DataType) -> u64 {
    match data_type {
        DataType::Boolean => 1,
        DataType::Int32 => 4,
        DataType::Int64 => 8,
        DataType::Float64 => 8,
        DataType::Utf8 => 32,
        DataType::Uuid => 16,
        DataType::Json => 64,
        DataType::Date => 4,
        DataType::Decimal { .. } => 16,
        DataType::Timestamp { .. } => 8,
        DataType::List(inner) => 8 + data_type_size(inner),
        DataType::Struct(fields) => fields.iter().map(|f| data_type_size(&f.data_type)).sum(),
        DataType::Unknown => 8,
        _ => 8,
    }
}
