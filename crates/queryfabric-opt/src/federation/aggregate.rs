use queryfabric_ir::{
    BinaryOperator, BoundColumnRef, BoundExpr, BoundExprKind, BoundFunctionCall,
    BoundProjectionExpr, BoundProjectionItem, DataType, FunctionRef, ResultField, SyntaxNode,
};

use super::{FederationError, Result};

pub(super) struct AggregateRewrite {
    pub(super) scatter_projection: Vec<BoundProjectionItem>,
    pub(super) gather_projection: BoundProjectionItem,
}

pub(super) fn classify_supported_aggregate(
    details: &BoundProjectionExpr,
    index: usize,
) -> Result<Option<AggregateRewrite>> {
    let BoundExprKind::Function(function) = &details.expr.kind else {
        return Ok(None);
    };
    if function.over.is_some() || function.filter.is_some() || function.distinct {
        return Err(FederationError::Unsupported(
            "federation aggregate planning does not support DISTINCT, FILTER, or windowed aggregates"
                .into(),
        ));
    }

    let function_name = function.function.name.to_ascii_lowercase();
    let user_alias = details
        .alias
        .clone()
        .unwrap_or_else(|| auto_aggregate_alias(function, index));
    let node = details.node.clone();

    let rewrite = match function_name.as_str() {
        "sum" | "min" | "max" => {
            let scatter_alias = format!("__fed_{user_alias}_{index}");
            let scatter_field = ResultField::new(
                &scatter_alias,
                details.field.data_type.clone(),
                details.field.nullable,
            );
            let scatter_projection = vec![BoundProjectionItem::expr(
                details.expr.clone(),
                Some(scatter_alias.clone()),
                scatter_field.clone(),
                node.clone(),
            )];
            let gather_function = BoundExpr {
                kind: BoundExprKind::function(BoundFunctionCall {
                    function: FunctionRef {
                        namespace: None,
                        name: function_name.clone(),
                    },
                    resolved_backend_name: None,
                    args: vec![column_expr(
                        &scatter_alias,
                        scatter_field.data_type.clone(),
                        scatter_field.nullable,
                        node.clone(),
                    )],
                    distinct: false,
                    filter: None,
                    over: None,
                    resolved_signature_name: None,
                }),
                data_type: details.field.data_type.clone(),
                nullable: details.field.nullable,
                node: node.clone(),
            };
            AggregateRewrite {
                scatter_projection,
                gather_projection: BoundProjectionItem::expr(
                    gather_function,
                    Some(user_alias.clone()),
                    details.field.clone(),
                    node.clone(),
                ),
            }
        }
        "count" => {
            let scatter_alias = format!("__fed_{user_alias}_{index}");
            let scatter_field = ResultField::new(&scatter_alias, DataType::Int64, false);
            let scatter_projection = vec![BoundProjectionItem::expr(
                details.expr.clone(),
                Some(scatter_alias.clone()),
                scatter_field.clone(),
                node.clone(),
            )];
            let gather_function = BoundExpr {
                kind: BoundExprKind::function(BoundFunctionCall {
                    function: FunctionRef {
                        namespace: None,
                        name: "sum".into(),
                    },
                    resolved_backend_name: None,
                    args: vec![column_expr(
                        &scatter_alias,
                        scatter_field.data_type.clone(),
                        scatter_field.nullable,
                        node.clone(),
                    )],
                    distinct: false,
                    filter: None,
                    over: None,
                    resolved_signature_name: None,
                }),
                data_type: details.field.data_type.clone(),
                nullable: details.field.nullable,
                node: node.clone(),
            };
            AggregateRewrite {
                scatter_projection,
                gather_projection: BoundProjectionItem::expr(
                    gather_function,
                    Some(user_alias.clone()),
                    details.field.clone(),
                    node.clone(),
                ),
            }
        }
        "avg" => {
            let sum_alias = format!("__fed_{user_alias}_sum_{index}");
            let cnt_alias = format!("__fed_{user_alias}_cnt_{index}");
            let arg = function.args.first().cloned().ok_or_else(|| {
                FederationError::Unsupported(
                    "AVG federation decomposition requires a single argument".into(),
                )
            })?;
            let sum_field = ResultField::new(&sum_alias, DataType::Float64, true);
            let cnt_field = ResultField::new(&cnt_alias, DataType::Int64, false);
            let scatter_projection = vec![
                BoundProjectionItem::expr(
                    BoundExpr {
                        kind: BoundExprKind::function(BoundFunctionCall {
                            function: FunctionRef {
                                namespace: None,
                                name: "sum".into(),
                            },
                            resolved_backend_name: None,
                            args: vec![arg.clone()],
                            distinct: false,
                            filter: None,
                            over: None,
                            resolved_signature_name: None,
                        }),
                        data_type: DataType::Float64,
                        nullable: true,
                        node: node.clone(),
                    },
                    Some(sum_alias.clone()),
                    sum_field.clone(),
                    node.clone(),
                ),
                BoundProjectionItem::expr(
                    BoundExpr {
                        kind: BoundExprKind::function(BoundFunctionCall {
                            function: FunctionRef {
                                namespace: None,
                                name: "count".into(),
                            },
                            resolved_backend_name: None,
                            args: vec![arg],
                            distinct: false,
                            filter: None,
                            over: None,
                            resolved_signature_name: None,
                        }),
                        data_type: DataType::Int64,
                        nullable: false,
                        node: node.clone(),
                    },
                    Some(cnt_alias.clone()),
                    cnt_field.clone(),
                    node.clone(),
                ),
            ];
            let sum_expr = BoundExpr {
                kind: BoundExprKind::function(BoundFunctionCall {
                    function: FunctionRef {
                        namespace: None,
                        name: "sum".into(),
                    },
                    resolved_backend_name: None,
                    args: vec![column_expr(
                        &sum_alias,
                        sum_field.data_type.clone(),
                        sum_field.nullable,
                        node.clone(),
                    )],
                    distinct: false,
                    filter: None,
                    over: None,
                    resolved_signature_name: None,
                }),
                data_type: DataType::Float64,
                nullable: true,
                node: node.clone(),
            };
            let cnt_expr = BoundExpr {
                kind: BoundExprKind::function(BoundFunctionCall {
                    function: FunctionRef {
                        namespace: None,
                        name: "sum".into(),
                    },
                    resolved_backend_name: None,
                    args: vec![column_expr(
                        &cnt_alias,
                        cnt_field.data_type.clone(),
                        cnt_field.nullable,
                        node.clone(),
                    )],
                    distinct: false,
                    filter: None,
                    over: None,
                    resolved_signature_name: None,
                }),
                data_type: DataType::Float64,
                nullable: true,
                node: node.clone(),
            };
            let gather_expr = BoundExpr {
                kind: BoundExprKind::Binary {
                    op: BinaryOperator::Divide,
                    left: Box::new(sum_expr),
                    right: Box::new(cnt_expr),
                },
                data_type: details.field.data_type.clone(),
                nullable: details.field.nullable,
                node: node.clone(),
            };
            AggregateRewrite {
                scatter_projection,
                gather_projection: BoundProjectionItem::expr(
                    gather_expr,
                    Some(user_alias.clone()),
                    details.field.clone(),
                    node.clone(),
                ),
            }
        }
        _ => {
            return Err(FederationError::Unsupported(format!(
                "federation aggregate planning does not support `{}`",
                function.function.display_name()
            )));
        }
    };

    Ok(Some(rewrite))
}

fn auto_aggregate_alias(function: &BoundFunctionCall, index: usize) -> String {
    let func = function.function.name.to_ascii_lowercase();
    let arg = function
        .args
        .first()
        .and_then(simple_expr_name)
        .unwrap_or_else(|| "value".into());
    format!("{func}_{arg}_{index}")
}

fn simple_expr_name(expr: &BoundExpr) -> Option<String> {
    match &expr.kind {
        BoundExprKind::Column(column) => Some(column.name.clone()),
        BoundExprKind::Literal(_) => Some("literal".into()),
        _ => None,
    }
}

pub(super) fn projection_output_name(details: &BoundProjectionExpr) -> Option<String> {
    details
        .alias
        .clone()
        .or_else(|| simple_expr_name(&details.expr))
}

pub(super) fn column_expr(
    name: &str,
    data_type: DataType,
    nullable: bool,
    node: SyntaxNode,
) -> BoundExpr {
    BoundExpr {
        kind: BoundExprKind::Column(BoundColumnRef {
            relation: None,
            name: name.into(),
        }),
        data_type,
        nullable,
        node,
    }
}

pub(super) fn column_projection(
    name: &str,
    field: ResultField,
    node: SyntaxNode,
) -> BoundProjectionItem {
    BoundProjectionItem::expr(
        column_expr(name, field.data_type.clone(), field.nullable, node.clone()),
        None,
        field,
        node,
    )
}

pub(super) fn field_for_projection(item: &BoundProjectionItem) -> Result<ResultField> {
    match item {
        BoundProjectionItem::Expr(details) => Ok(details.field.clone()),
        _ => Err(FederationError::Unsupported(
            "federation aggregate planning requires expression projections".into(),
        )),
    }
}

pub(super) fn expr_contains_aggregate(expr: &BoundExpr) -> bool {
    match &expr.kind {
        BoundExprKind::Function(function) => {
            matches!(
                function.function.name.to_ascii_lowercase().as_str(),
                "count" | "sum" | "avg" | "min" | "max" | "count_distinct"
            ) || function.args.iter().any(expr_contains_aggregate)
                || function
                    .filter
                    .as_deref()
                    .is_some_and(expr_contains_aggregate)
        }
        BoundExprKind::Unary { expr, .. }
        | BoundExprKind::Cast { expr, .. }
        | BoundExprKind::IsNull { expr, .. } => expr_contains_aggregate(expr),
        BoundExprKind::Binary { left, right, .. } => {
            expr_contains_aggregate(left) || expr_contains_aggregate(right)
        }
        BoundExprKind::Case {
            operand,
            when_then,
            else_result,
        } => {
            operand.as_deref().is_some_and(expr_contains_aggregate)
                || when_then.iter().any(|branch| {
                    expr_contains_aggregate(&branch.condition)
                        || expr_contains_aggregate(&branch.result)
                })
                || else_result.as_deref().is_some_and(expr_contains_aggregate)
        }
        BoundExprKind::Between {
            expr, low, high, ..
        } => {
            expr_contains_aggregate(expr)
                || expr_contains_aggregate(low)
                || expr_contains_aggregate(high)
        }
        BoundExprKind::InList { expr, list, .. } => {
            expr_contains_aggregate(expr) || list.iter().any(expr_contains_aggregate)
        }
        BoundExprKind::Like { expr, pattern, .. } => {
            expr_contains_aggregate(expr) || expr_contains_aggregate(pattern)
        }
        BoundExprKind::Tuple(values) | BoundExprKind::Array(values) => {
            values.iter().any(expr_contains_aggregate)
        }
        BoundExprKind::InSubquery { .. }
        | BoundExprKind::ScalarSubquery(_)
        | BoundExprKind::Exists(_)
        | BoundExprKind::Column(_)
        | BoundExprKind::Literal(_)
        | BoundExprKind::Parameter(_)
        | BoundExprKind::Unsupported { .. } => false,
    }
}
