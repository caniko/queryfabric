mod scope;
use scope::{ClickHouseMvSummary, ResolvedWrapper, SelectScope, WrapperNearMiss, WrapperSpec};

use std::collections::BTreeSet;

use queryfabric_catalog::{Catalog, RelationKind, RelationSchema};
use queryfabric_ir::{
    BinaryOperator, BoundColumnRef, BoundExpr, BoundExprKind, BoundFunctionCall, BoundOrderByExpr,
    BoundProjectionExpr, BoundProjectionItem, BoundQuery, BoundQueryPlan, BoundRelation,
    BoundSelect, BoundSetExpr, BoundTableWithJoins, DataType, FunctionRef, LiteralValue,
    QueryDiagnostic, Result, ResultField, ResultSchema, SyntaxNode,
};

pub(super) fn rewrite_query_for_clickhouse(
    query: &BoundQuery,
    catalog: &dyn Catalog,
    uuid_arrow_workaround_enabled: bool,
) -> (BoundQuery, ClickHouseMvSummary) {
    let mut summary = ClickHouseMvSummary::default();
    let plan = rewrite_query_plan(
        query.plan(),
        catalog,
        &mut summary,
        uuid_arrow_workaround_enabled,
    );
    (query.clone().with_plan(plan), summary)
}

fn rewrite_query_plan(
    plan: &BoundQueryPlan,
    catalog: &dyn Catalog,
    summary: &mut ClickHouseMvSummary,
    uuid_arrow_workaround_enabled: bool,
) -> BoundQueryPlan {
    let body = rewrite_set_expr(&plan.body, catalog, summary, uuid_arrow_workaround_enabled);
    let result_schema = result_schema_for_set_expr(&body);
    BoundQueryPlan {
        node: plan.node.clone(),
        ctes: plan
            .ctes
            .iter()
            .map(|cte| {
                let query =
                    rewrite_query_plan(&cte.query, catalog, summary, uuid_arrow_workaround_enabled);
                queryfabric_ir::BoundCte {
                    name: cte.name.clone(),
                    columns: cte.columns.clone(),
                    result_schema: query.result_schema.clone(),
                    query: Box::new(query),
                    node: cte.node.clone(),
                }
            })
            .collect(),
        body,
        order_by: plan
            .order_by
            .iter()
            .map(|expr| {
                rewrite_order_by_expr(
                    expr,
                    None,
                    false,
                    catalog,
                    summary,
                    uuid_arrow_workaround_enabled,
                )
            })
            .collect(),
        limit: plan.limit.as_ref().map(|expr| {
            rewrite_expr(
                expr,
                None,
                false,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )
        }),
        offset: plan.offset.as_ref().map(|expr| {
            rewrite_expr(
                expr,
                None,
                false,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )
        }),
        backend_clauses: plan.backend_clauses.clone(),
        result_schema,
    }
}

fn rewrite_set_expr(
    expr: &BoundSetExpr,
    catalog: &dyn Catalog,
    summary: &mut ClickHouseMvSummary,
    uuid_arrow_workaround_enabled: bool,
) -> BoundSetExpr {
    match expr {
        BoundSetExpr::Select(select) => BoundSetExpr::Select(Box::new(rewrite_select(
            select,
            catalog,
            summary,
            uuid_arrow_workaround_enabled,
        ))),
        BoundSetExpr::UnionAll {
            left,
            right,
            node,
            result_schema: _,
        } => {
            let left = rewrite_set_expr(left, catalog, summary, uuid_arrow_workaround_enabled);
            let right = rewrite_set_expr(right, catalog, summary, uuid_arrow_workaround_enabled);
            BoundSetExpr::UnionAll {
                result_schema: result_schema_for_set_expr(&left),
                left: Box::new(left),
                right: Box::new(right),
                node: node.clone(),
            }
        }
        BoundSetExpr::Unsupported {
            description,
            node,
            result_schema,
        } => BoundSetExpr::Unsupported {
            description: description.clone(),
            node: node.clone(),
            result_schema: result_schema.clone(),
        },
    }
}

fn rewrite_select(
    select: &BoundSelect,
    catalog: &dyn Catalog,
    summary: &mut ClickHouseMvSummary,
    uuid_arrow_workaround_enabled: bool,
) -> BoundSelect {
    let from: Vec<_> = select
        .from
        .iter()
        .map(|table| {
            rewrite_table_with_joins(table, catalog, summary, uuid_arrow_workaround_enabled)
        })
        .collect();
    let scope = scope_from_from_clause(&from, catalog);

    let projection = select
        .projection
        .iter()
        .flat_map(|item| match item {
            BoundProjectionItem::Expr(details) => {
                let rewritten = rewrite_expr(
                    &details.expr,
                    Some(&scope),
                    true,
                    catalog,
                    summary,
                    uuid_arrow_workaround_enabled,
                );
                let (expr, wrapped_for_arrow) =
                    rewrite_uuid_arrow_expr(rewritten, uuid_arrow_workaround_enabled);
                let field = if wrapped_for_arrow {
                    string_result_field(&details.field)
                } else {
                    details.field.clone()
                };
                vec![BoundProjectionItem::expr(
                    expr,
                    details
                        .alias
                        .clone()
                        .or_else(|| wrapped_for_arrow.then(|| details.field.name.clone())),
                    field,
                    details.node.clone(),
                )]
            }
            BoundProjectionItem::Wildcard {
                qualifier,
                fields,
                node,
            } => expand_wildcard_projection(
                qualifier.as_deref(),
                fields,
                node,
                &scope,
                uuid_arrow_workaround_enabled,
            )
            .unwrap_or_else(|| {
                vec![BoundProjectionItem::Wildcard {
                    qualifier: qualifier.clone(),
                    fields: fields.clone(),
                    node: node.clone(),
                }]
            }),
            BoundProjectionItem::Unsupported { description, node } => {
                vec![BoundProjectionItem::Unsupported {
                    description: description.clone(),
                    node: node.clone(),
                }]
            }
        })
        .collect::<Vec<_>>();

    let result_schema = ResultSchema {
        fields: projection
            .iter()
            .flat_map(|item| match item {
                BoundProjectionItem::Expr(details) => vec![details.field.clone()],
                BoundProjectionItem::Wildcard { fields, .. } => fields.clone(),
                BoundProjectionItem::Unsupported { .. } => Vec::new(),
            })
            .collect(),
        metadata: select.result_schema.metadata.clone(),
    };

    BoundSelect {
        distinct: select.distinct,
        projection,
        from,
        selection: select.selection.as_ref().map(|expr| {
            rewrite_expr(
                expr,
                Some(&scope),
                false,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )
        }),
        group_by: select
            .group_by
            .iter()
            .map(|expr| {
                let rewritten = rewrite_expr(
                    expr,
                    Some(&scope),
                    false,
                    catalog,
                    summary,
                    uuid_arrow_workaround_enabled,
                );
                rewrite_uuid_arrow_expr(rewritten, uuid_arrow_workaround_enabled).0
            })
            .collect(),
        having: select.having.as_ref().map(|expr| {
            rewrite_expr(
                expr,
                Some(&scope),
                true,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )
        }),
        result_schema,
        node: select.node.clone(),
    }
}

fn rewrite_table_with_joins(
    table: &BoundTableWithJoins,
    catalog: &dyn Catalog,
    summary: &mut ClickHouseMvSummary,
    uuid_arrow_workaround_enabled: bool,
) -> BoundTableWithJoins {
    BoundTableWithJoins {
        relation: rewrite_relation(
            &table.relation,
            catalog,
            summary,
            uuid_arrow_workaround_enabled,
        ),
        joins: table
            .joins
            .iter()
            .map(|join| queryfabric_ir::BoundJoin {
                kind: join.kind,
                relation: rewrite_relation(
                    &join.relation,
                    catalog,
                    summary,
                    uuid_arrow_workaround_enabled,
                ),
                on: join.on.as_ref().map(|expr| {
                    rewrite_expr(
                        expr,
                        None,
                        false,
                        catalog,
                        summary,
                        uuid_arrow_workaround_enabled,
                    )
                }),
                node: join.node.clone(),
            })
            .collect(),
        node: table.node.clone(),
    }
}

fn rewrite_relation(
    relation: &BoundRelation,
    catalog: &dyn Catalog,
    summary: &mut ClickHouseMvSummary,
    uuid_arrow_workaround_enabled: bool,
) -> BoundRelation {
    match relation {
        BoundRelation::Table { binding, node } => BoundRelation::Table {
            binding: binding.clone(),
            node: node.clone(),
        },
        BoundRelation::Derived {
            binding,
            query,
            node,
        } => BoundRelation::Derived {
            binding: binding.clone(),
            query: Box::new(rewrite_query_plan(
                query,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )),
            node: node.clone(),
        },
        BoundRelation::NestedJoin {
            binding,
            table_with_joins,
            node,
        } => BoundRelation::NestedJoin {
            binding: binding.clone(),
            table_with_joins: Box::new(rewrite_table_with_joins(
                table_with_joins,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )),
            node: node.clone(),
        },
        BoundRelation::Unsupported {
            description,
            binding_name,
            node,
        } => BoundRelation::Unsupported {
            description: description.clone(),
            binding_name: binding_name.clone(),
            node: node.clone(),
        },
    }
}

fn scope_from_from_clause(from: &[BoundTableWithJoins], catalog: &dyn Catalog) -> SelectScope {
    let mut scope = SelectScope::default();
    for table in from {
        collect_relation_bindings(&table.relation, catalog, &mut scope);
        for join in &table.joins {
            collect_relation_bindings(&join.relation, catalog, &mut scope);
        }
    }
    scope
}

fn collect_relation_bindings(
    relation: &BoundRelation,
    catalog: &dyn Catalog,
    scope: &mut SelectScope,
) {
    match relation {
        BoundRelation::Table { binding, .. } => {
            let Some(name) = binding.relation_name.as_ref() else {
                return;
            };
            let Some(schema) = catalog.resolve_relation(name.namespace.as_deref(), &name.name)
            else {
                return;
            };
            scope.push(binding.binding_name.clone(), name.display_name(), schema);
        }
        BoundRelation::NestedJoin {
            table_with_joins, ..
        } => {
            collect_relation_bindings(&table_with_joins.relation, catalog, scope);
            for join in &table_with_joins.joins {
                collect_relation_bindings(&join.relation, catalog, scope);
            }
        }
        BoundRelation::Derived { .. } | BoundRelation::Unsupported { .. } => {}
    }
}

fn rewrite_uuid_arrow_expr(
    expr: BoundExpr,
    uuid_arrow_workaround_enabled: bool,
) -> (BoundExpr, bool) {
    if !uuid_arrow_workaround_enabled
        || expr.data_type != DataType::Uuid
        || is_to_string_call(&expr)
    {
        return (expr, false);
    }

    let node = expr.node.clone();
    let nullable = expr.nullable;
    (
        BoundExpr {
            kind: BoundExprKind::function(BoundFunctionCall {
                function: FunctionRef {
                    namespace: None,
                    name: "toString".into(),
                },
                resolved_backend_name: None,
                args: vec![expr],
                distinct: false,
                filter: None,
                over: None,
                resolved_signature_name: Some("toString".into()),
            }),
            data_type: DataType::Utf8,
            nullable,
            node,
        },
        true,
    )
}

fn expand_wildcard_projection(
    qualifier: Option<&str>,
    fields: &[ResultField],
    node: &SyntaxNode,
    scope: &SelectScope,
    uuid_arrow_workaround_enabled: bool,
) -> Option<Vec<BoundProjectionItem>> {
    let binding_name = scope.target_binding_name(qualifier)?;
    Some(
        fields
            .iter()
            .map(|field| {
                let expr = BoundExpr {
                    kind: BoundExprKind::Column(BoundColumnRef {
                        relation: Some(binding_name.to_owned()),
                        name: field.name.clone(),
                    }),
                    data_type: field.data_type.clone(),
                    nullable: field.nullable,
                    node: node.clone(),
                };
                let (expr, wrapped_for_arrow) =
                    rewrite_uuid_arrow_expr(expr, uuid_arrow_workaround_enabled);
                BoundProjectionItem::expr(
                    expr,
                    Some(field.name.clone()),
                    if wrapped_for_arrow {
                        string_result_field(field)
                    } else {
                        field.clone()
                    },
                    node.clone(),
                )
            })
            .collect(),
    )
}

fn is_to_string_call(expr: &BoundExpr) -> bool {
    let BoundExprKind::Function(function) = &expr.kind else {
        return false;
    };
    function.function.namespace.is_none()
        && function.function.name.eq_ignore_ascii_case("toString")
        && function.args.len() == 1
        && !function.distinct
        && function.filter.is_none()
        && function.over.is_none()
}

fn string_result_field(field: &ResultField) -> ResultField {
    ResultField {
        name: field.name.clone(),
        data_type: DataType::Utf8,
        nullable: field.nullable,
        metadata: field.metadata.clone(),
    }
}

fn result_schema_for_set_expr(expr: &BoundSetExpr) -> ResultSchema {
    match expr {
        BoundSetExpr::Select(select) => select.result_schema.clone(),
        BoundSetExpr::UnionAll { result_schema, .. }
        | BoundSetExpr::Unsupported { result_schema, .. } => result_schema.clone(),
    }
}

fn rewrite_order_by_expr(
    expr: &BoundOrderByExpr,
    scope: Option<&SelectScope>,
    allow_column_wrap: bool,
    catalog: &dyn Catalog,
    summary: &mut ClickHouseMvSummary,
    uuid_arrow_workaround_enabled: bool,
) -> BoundOrderByExpr {
    BoundOrderByExpr {
        expr: rewrite_expr(
            &expr.expr,
            scope,
            allow_column_wrap,
            catalog,
            summary,
            uuid_arrow_workaround_enabled,
        ),
        asc: expr.asc,
        nulls_first: expr.nulls_first,
        node: expr.node.clone(),
    }
}

fn rewrite_expr(
    expr: &BoundExpr,
    scope: Option<&SelectScope>,
    allow_column_wrap: bool,
    catalog: &dyn Catalog,
    summary: &mut ClickHouseMvSummary,
    uuid_arrow_workaround_enabled: bool,
) -> BoundExpr {
    let kind = match &expr.kind {
        BoundExprKind::Column(column) => {
            if allow_column_wrap {
                scope
                    .and_then(|scope| {
                        scope.resolve_wrapper(column.relation.as_deref(), &column.name)
                    })
                    .map(|resolved| {
                        summary.record_wrap(&resolved, &expr.node);
                        BoundExprKind::function(BoundFunctionCall {
                            function: resolved.wrapper.function_ref(),
                            resolved_backend_name: None,
                            args: vec![expr.clone()],
                            distinct: false,
                            filter: None,
                            over: None,
                            resolved_signature_name: Some(resolved.wrapper.name.to_owned()),
                        })
                    })
                    .unwrap_or_else(|| expr.kind.clone())
            } else {
                expr.kind.clone()
            }
        }
        BoundExprKind::Literal(_)
        | BoundExprKind::Parameter(_)
        | BoundExprKind::Unsupported { .. } => expr.kind.clone(),
        BoundExprKind::Unary { op, expr: inner } => BoundExprKind::Unary {
            op: *op,
            expr: Box::new(rewrite_expr(
                inner,
                scope,
                allow_column_wrap,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )),
        },
        BoundExprKind::Binary { op, left, right } => BoundExprKind::Binary {
            op: *op,
            left: Box::new(rewrite_expr(
                left,
                scope,
                allow_column_wrap,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )),
            right: Box::new(rewrite_expr(
                right,
                scope,
                allow_column_wrap,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )),
        },
        BoundExprKind::Function(function) => {
            let keep_args = allow_column_wrap
                && scope.is_some_and(|scope| is_existing_wrapper(function, scope));
            if allow_column_wrap
                && !keep_args
                && let Some(scope) = scope
                && let Some(mismatch) = scope.detect_wrapper_near_miss(function)
            {
                summary.record_near_miss(mismatch, &expr.node);
            }
            let args = if keep_args {
                function.args.clone()
            } else {
                function
                    .args
                    .iter()
                    .map(|arg| {
                        rewrite_expr(
                            arg,
                            scope,
                            allow_column_wrap,
                            catalog,
                            summary,
                            uuid_arrow_workaround_enabled,
                        )
                    })
                    .collect()
            };
            BoundExprKind::function(BoundFunctionCall {
                function: function.function.clone(),
                resolved_backend_name: function.resolved_backend_name.clone(),
                args,
                distinct: function.distinct,
                filter: function.filter.as_ref().map(|expr| {
                    Box::new(rewrite_expr(
                        expr,
                        scope,
                        false,
                        catalog,
                        summary,
                        uuid_arrow_workaround_enabled,
                    ))
                }),
                over: function
                    .over
                    .as_ref()
                    .map(|window| queryfabric_ir::BoundWindowSpec {
                        partition_by: window
                            .partition_by
                            .iter()
                            .map(|expr| {
                                rewrite_expr(
                                    expr,
                                    scope,
                                    false,
                                    catalog,
                                    summary,
                                    uuid_arrow_workaround_enabled,
                                )
                            })
                            .collect(),
                        order_by: window
                            .order_by
                            .iter()
                            .map(|expr| {
                                rewrite_order_by_expr(
                                    expr,
                                    scope,
                                    false,
                                    catalog,
                                    summary,
                                    uuid_arrow_workaround_enabled,
                                )
                            })
                            .collect(),
                        node: window.node.clone(),
                    }),
                resolved_signature_name: function.resolved_signature_name.clone(),
            })
        }
        BoundExprKind::Case {
            operand,
            when_then,
            else_result,
        } => BoundExprKind::Case {
            operand: operand.as_ref().map(|expr| {
                Box::new(rewrite_expr(
                    expr,
                    scope,
                    allow_column_wrap,
                    catalog,
                    summary,
                    uuid_arrow_workaround_enabled,
                ))
            }),
            when_then: when_then
                .iter()
                .map(|branch| queryfabric_ir::BoundWhenThen {
                    condition: rewrite_expr(
                        &branch.condition,
                        scope,
                        allow_column_wrap,
                        catalog,
                        summary,
                        uuid_arrow_workaround_enabled,
                    ),
                    result: rewrite_expr(
                        &branch.result,
                        scope,
                        allow_column_wrap,
                        catalog,
                        summary,
                        uuid_arrow_workaround_enabled,
                    ),
                })
                .collect(),
            else_result: else_result.as_ref().map(|expr| {
                Box::new(rewrite_expr(
                    expr,
                    scope,
                    allow_column_wrap,
                    catalog,
                    summary,
                    uuid_arrow_workaround_enabled,
                ))
            }),
        },
        BoundExprKind::Cast {
            expr: inner,
            data_type,
        } => BoundExprKind::Cast {
            expr: Box::new(rewrite_expr(
                inner,
                scope,
                allow_column_wrap,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )),
            data_type: data_type.clone(),
        },
        BoundExprKind::Between {
            expr: input,
            low,
            high,
            negated,
        } => BoundExprKind::Between {
            expr: Box::new(rewrite_expr(
                input,
                scope,
                allow_column_wrap,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )),
            low: Box::new(rewrite_expr(
                low,
                scope,
                allow_column_wrap,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )),
            high: Box::new(rewrite_expr(
                high,
                scope,
                allow_column_wrap,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )),
            negated: *negated,
        },
        BoundExprKind::InList {
            expr: input,
            list,
            negated,
        } => BoundExprKind::InList {
            expr: Box::new(rewrite_expr(
                input,
                scope,
                allow_column_wrap,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )),
            list: list
                .iter()
                .map(|item| {
                    rewrite_expr(
                        item,
                        scope,
                        allow_column_wrap,
                        catalog,
                        summary,
                        uuid_arrow_workaround_enabled,
                    )
                })
                .collect(),
            negated: *negated,
        },
        BoundExprKind::InSubquery {
            expr: input,
            subquery,
            negated,
        } => BoundExprKind::InSubquery {
            expr: Box::new(rewrite_expr(
                input,
                scope,
                allow_column_wrap,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )),
            subquery: Box::new(rewrite_query_plan(
                subquery,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )),
            negated: *negated,
        },
        BoundExprKind::ScalarSubquery(subquery) => BoundExprKind::ScalarSubquery(Box::new(
            rewrite_query_plan(subquery, catalog, summary, uuid_arrow_workaround_enabled),
        )),
        BoundExprKind::Exists(subquery) => BoundExprKind::Exists(Box::new(rewrite_query_plan(
            subquery,
            catalog,
            summary,
            uuid_arrow_workaround_enabled,
        ))),
        BoundExprKind::Like {
            expr: input,
            pattern,
            negated,
            case_insensitive,
        } => BoundExprKind::Like {
            expr: Box::new(rewrite_expr(
                input,
                scope,
                allow_column_wrap,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )),
            pattern: Box::new(rewrite_expr(
                pattern,
                scope,
                allow_column_wrap,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )),
            negated: *negated,
            case_insensitive: *case_insensitive,
        },
        BoundExprKind::IsNull {
            expr: inner,
            negated,
        } => BoundExprKind::IsNull {
            expr: Box::new(rewrite_expr(
                inner,
                scope,
                allow_column_wrap,
                catalog,
                summary,
                uuid_arrow_workaround_enabled,
            )),
            negated: *negated,
        },
        BoundExprKind::Tuple(items) => BoundExprKind::Tuple(
            items
                .iter()
                .map(|item| {
                    rewrite_expr(
                        item,
                        scope,
                        allow_column_wrap,
                        catalog,
                        summary,
                        uuid_arrow_workaround_enabled,
                    )
                })
                .collect(),
        ),
        BoundExprKind::Array(items) => BoundExprKind::Array(
            items
                .iter()
                .map(|item| {
                    rewrite_expr(
                        item,
                        scope,
                        allow_column_wrap,
                        catalog,
                        summary,
                        uuid_arrow_workaround_enabled,
                    )
                })
                .collect(),
        ),
    };

    BoundExpr {
        kind,
        data_type: expr.data_type.clone(),
        nullable: expr.nullable,
        node: expr.node.clone(),
    }
}

fn is_existing_wrapper(function: &BoundFunctionCall, scope: &SelectScope) -> bool {
    if function.distinct
        || function.filter.is_some()
        || function.over.is_some()
        || function.args.len() != 1
    {
        return false;
    }

    let BoundExprKind::Column(column) = &function.args[0].kind else {
        return false;
    };

    scope
        .resolve_wrapper(column.relation.as_deref(), &column.name)
        .is_some_and(|resolved| resolved.wrapper.matches(&function.function))
}
