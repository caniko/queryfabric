use std::collections::BTreeSet;

use queryfabric_catalog::{
    BackendAdapter, BackendAnalysis, BackendExecutionLimits, BackendFeature, CapabilitySet,
    Catalog, CostEstimateError, EmitArtifact, EstimatedCost, PlanCostEstimator, RelationKind,
    RelationSchema, ResultDeliveryFormat, SqlBackend, analyze_backend_support, emit_sql_artifact,
    unsupported,
};
use queryfabric_ir::{
    BinaryOperator, BoundColumnRef, BoundExpr, BoundExprKind, BoundFunctionCall, BoundOrderByExpr,
    BoundProjectionItem, BoundQuery, BoundQueryPlan, BoundRelation, BoundSelect, BoundSetExpr,
    BoundTableWithJoins, DataType, FunctionRef, LiteralValue, QueryDiagnostic, Result, ResultField,
    ResultSchema, SyntaxNode,
};

mod cost;
use cost::estimate_clickhouse_cost;

pub mod arrow;
pub mod driver;
mod runtime;
pub mod types;

pub use arrow::{clickhouse_arrow_safe_artifact_sql, clickhouse_arrow_safe_sql};
pub use driver::{ClickHouseConfig, ClickHouseError, DynamicClient, downcast_view_types};
pub use runtime::{ClickHouseArrowTransport, ClickHouseRuntime};
pub use types::{ChType, SimpleColumnType};

#[derive(Debug, Default, Clone, Copy)]
pub struct ClickHouseAdapter;

impl BackendAdapter for ClickHouseAdapter {
    fn name(&self) -> &'static str {
        "clickhouse"
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::from_features([
            BackendFeature::CommonTableExpressions,
            BackendFeature::DerivedTables,
            BackendFeature::Joins,
            BackendFeature::Windows,
            BackendFeature::SetOperations,
            BackendFeature::Aggregates,
            BackendFeature::DistinctAggregates,
            BackendFeature::ScalarSubqueries,
            BackendFeature::InSubqueries,
            BackendFeature::NamespacedFunctions,
            BackendFeature::ApproximateAggregates,
            BackendFeature::Explain,
            BackendFeature::LimitOffset,
            BackendFeature::IsolatedExecution,
            BackendFeature::UuidToStringInArrowOutput,
        ])
        .with_limits(BackendExecutionLimits {
            max_rows: None,
            max_bytes_scanned: None,
            max_result_bytes: None,
            max_concurrent_queries: None,
            interactive_byte_limit: 512 * 1024 * 1024,
            batch_byte_limit: 4 * 1024 * 1024 * 1024,
        })
        .with_result_formats([
            ResultDeliveryFormat::ArrowIpc,
            ResultDeliveryFormat::Parquet,
            ResultDeliveryFormat::Csv,
            ResultDeliveryFormat::Json,
        ])
        .with_async_export(true)
        .with_federated_execution(true)
    }

    fn analyze(&self, query: &BoundQuery, catalog: &dyn Catalog) -> BackendAnalysis {
        let mut analysis =
            analyze_backend_support(query, catalog, self.name(), self.capabilities(), true);
        let (_, mv_summary) =
            rewrite_query_for_clickhouse(query, catalog, self.uuid_arrow_workaround_enabled());
        analysis
            .diagnostics
            .extend(mv_summary.analysis_diagnostics(self.name()));
        analysis.supported = !analysis.diagnostics.iter().any(QueryDiagnostic::is_error);
        analysis
    }

    fn emit(&self, query: &BoundQuery, catalog: &dyn Catalog) -> Result<EmitArtifact> {
        let analysis = self.analyze(query, catalog);
        if !analysis.supported {
            return Err(unsupported(
                "clickhouse-emission",
                diagnostic_summary(&analysis.diagnostics),
            ));
        }

        let (rewritten_query, mv_summary) =
            rewrite_query_for_clickhouse(query, catalog, self.uuid_arrow_workaround_enabled());
        let mut artifact = emit_sql_artifact(&rewritten_query, catalog, SqlBackend::ClickHouse)?;
        if let Some(rewritten_to) = mv_summary.rewritten_to_metadata() {
            artifact
                .metadata
                .insert("clickhouse.rewritten_to".into(), rewritten_to);
        }
        Ok(EmitArtifact::Sql(artifact))
    }
}

impl ClickHouseAdapter {
    fn uuid_arrow_workaround_enabled(&self) -> bool {
        self.capabilities()
            .supports(BackendFeature::UuidToStringInArrowOutput)
    }
}

impl PlanCostEstimator for ClickHouseAdapter {
    fn estimate(
        &self,
        plan: &BoundQuery,
        catalog: &dyn Catalog,
    ) -> std::result::Result<EstimatedCost, CostEstimateError> {
        estimate_clickhouse_cost(plan, catalog)
    }
}

fn diagnostic_summary(diagnostics: &[QueryDiagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

fn rewrite_query_for_clickhouse(
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

#[derive(Debug, Clone)]
struct ScopeBinding {
    binding_name: String,
    relation_display: String,
    relation: RelationSchema,
}

#[derive(Debug, Default, Clone)]
struct SelectScope {
    bindings: Vec<ScopeBinding>,
}

impl SelectScope {
    fn push(&mut self, binding_name: String, relation_display: String, relation: RelationSchema) {
        self.bindings.push(ScopeBinding {
            binding_name,
            relation_display,
            relation,
        });
    }

    fn target_binding_name(&self, qualifier: Option<&str>) -> Option<&str> {
        match qualifier {
            Some(qualifier) => self
                .bindings
                .iter()
                .find(|binding| binding.binding_name.eq_ignore_ascii_case(qualifier))
                .map(|binding| binding.binding_name.as_str()),
            None if self.bindings.len() == 1 => Some(self.bindings[0].binding_name.as_str()),
            None => None,
        }
    }

    fn resolve_wrapper(&self, relation: Option<&str>, name: &str) -> Option<ResolvedWrapper> {
        let mut matches = self
            .bindings
            .iter()
            .filter(|binding| {
                relation.is_none_or(|want| binding.binding_name.eq_ignore_ascii_case(want))
            })
            .filter_map(|binding| {
                binding
                    .relation
                    .columns
                    .iter()
                    .find(|column| column.name.eq_ignore_ascii_case(name))
                    .filter(|_| binding.relation.kind == RelationKind::MaterializedView)
                    .and_then(|column| {
                        column
                            .metadata
                            .extensions
                            .get("clickhouse.mv.merge_fn")
                            .and_then(|merge_fn| WrapperSpec::from_merge_fn(merge_fn))
                            .map(|wrapper| ResolvedWrapper {
                                relation_display: binding.relation_display.clone(),
                                binding_name: binding.binding_name.clone(),
                                column_name: column.name.clone(),
                                wrapper,
                            })
                    })
            });

        let first = matches.next()?;
        matches.next().is_none().then_some(first)
    }

    fn detect_wrapper_near_miss(&self, function: &BoundFunctionCall) -> Option<WrapperNearMiss> {
        if function.distinct
            || function.filter.is_some()
            || function.over.is_some()
            || function.args.len() != 1
        {
            return None;
        }

        let actual = WrapperSpec::from_function(&function.function)?;
        let BoundExprKind::Column(column) = &function.args[0].kind else {
            return None;
        };
        let expected = self.resolve_wrapper(column.relation.as_deref(), &column.name)?;
        (actual != expected.wrapper).then_some(WrapperNearMiss {
            relation_display: expected.relation_display,
            binding_name: expected.binding_name,
            column_name: expected.column_name,
            current_wrapper: actual,
            expected_wrapper: expected.wrapper,
        })
    }
}

#[derive(Debug, Clone)]
struct ResolvedWrapper {
    relation_display: String,
    binding_name: String,
    column_name: String,
    wrapper: WrapperSpec,
}

#[derive(Debug, Clone)]
struct WrapperNearMiss {
    relation_display: String,
    binding_name: String,
    column_name: String,
    current_wrapper: WrapperSpec,
    expected_wrapper: WrapperSpec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WrapperSpec {
    namespace: Option<&'static str>,
    name: &'static str,
}

impl WrapperSpec {
    fn from_merge_fn(merge_fn: &str) -> Option<Self> {
        match merge_fn.to_ascii_lowercase().as_str() {
            "avgmerge" => Some(Self {
                namespace: Some("ch"),
                name: "avg_merge",
            }),
            "countmerge" => Some(Self {
                namespace: Some("ch"),
                name: "count_merge",
            }),
            "summerge" => Some(Self {
                namespace: Some("ch"),
                name: "sum_merge",
            }),
            "stddevpopmerge" => Some(Self {
                namespace: Some("ch"),
                name: "stddevpop_merge",
            }),
            "varpopmerge" => Some(Self {
                namespace: Some("ch"),
                name: "varpop_merge",
            }),
            "min" => Some(Self {
                namespace: None,
                name: "min",
            }),
            "max" => Some(Self {
                namespace: None,
                name: "max",
            }),
            _ => None,
        }
    }

    fn from_function(function: &FunctionRef) -> Option<Self> {
        match (
            function.namespace.as_deref().map(str::to_ascii_lowercase),
            function.name.to_ascii_lowercase().as_str(),
        ) {
            (Some(namespace), "avg_merge") if namespace == "ch" => Some(Self {
                namespace: Some("ch"),
                name: "avg_merge",
            }),
            (Some(namespace), "count_merge") if namespace == "ch" => Some(Self {
                namespace: Some("ch"),
                name: "count_merge",
            }),
            (Some(namespace), "sum_merge") if namespace == "ch" => Some(Self {
                namespace: Some("ch"),
                name: "sum_merge",
            }),
            (Some(namespace), "stddevpop_merge") if namespace == "ch" => Some(Self {
                namespace: Some("ch"),
                name: "stddevpop_merge",
            }),
            (Some(namespace), "varpop_merge") if namespace == "ch" => Some(Self {
                namespace: Some("ch"),
                name: "varpop_merge",
            }),
            (None, "min") => Some(Self {
                namespace: None,
                name: "min",
            }),
            (None, "max") => Some(Self {
                namespace: None,
                name: "max",
            }),
            _ => None,
        }
    }

    fn function_ref(self) -> FunctionRef {
        FunctionRef {
            namespace: self.namespace.map(str::to_owned),
            name: self.name.to_owned(),
        }
    }

    fn display_name(self) -> String {
        self.function_ref().display_name()
    }

    fn matches(self, function: &FunctionRef) -> bool {
        function.name.eq_ignore_ascii_case(self.name)
            && match (self.namespace, function.namespace.as_deref()) {
                (None, None) => true,
                (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
                _ => false,
            }
    }
}

#[derive(Debug, Default, Clone)]
struct ClickHouseMvSummary {
    rewritten_relations: BTreeSet<String>,
    wrap_events: Vec<WrapEvent>,
    near_miss_events: Vec<NearMissEvent>,
}

impl ClickHouseMvSummary {
    fn record_wrap(&mut self, resolved: &ResolvedWrapper, node: &SyntaxNode) {
        self.rewritten_relations
            .insert(resolved.relation_display.clone());
        self.wrap_events.push(WrapEvent {
            relation_display: resolved.relation_display.clone(),
            binding_name: resolved.binding_name.clone(),
            column_name: resolved.column_name.clone(),
            wrapper: resolved.wrapper,
            node: node.clone(),
        });
    }

    fn record_near_miss(&mut self, mismatch: WrapperNearMiss, node: &SyntaxNode) {
        self.near_miss_events.push(NearMissEvent {
            relation_display: mismatch.relation_display,
            binding_name: mismatch.binding_name,
            column_name: mismatch.column_name,
            current_wrapper: mismatch.current_wrapper,
            expected_wrapper: mismatch.expected_wrapper,
            node: node.clone(),
        });
    }

    fn rewritten_to_metadata(&self) -> Option<String> {
        (!self.rewritten_relations.is_empty()).then(|| {
            self.rewritten_relations
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(",")
        })
    }

    fn analysis_diagnostics(&self, backend: &str) -> Vec<QueryDiagnostic> {
        let mut diagnostics = Vec::new();
        let mut seen_wraps = BTreeSet::new();
        for event in &self.wrap_events {
            let key = format!(
                "{}|{}|{}|{}|{}",
                event.relation_display,
                event.binding_name,
                event.column_name,
                event.wrapper.display_name(),
                event.node.node_id
            );
            if !seen_wraps.insert(key) {
                continue;
            }
            diagnostics.push(
                diagnostic_with_node(
                    QueryDiagnostic::note(
                        "QFCH201",
                        format!(
                            "ClickHouse emission will wrap materialized-view column `{}.{}` from `{}` with `{}`.",
                            event.binding_name,
                            event.column_name,
                            event.relation_display,
                            event.wrapper.display_name(),
                        ),
                    )
                    .with_backend(backend)
                    .with_remediation(format!(
                        "Use `{}` explicitly if you want the backend-specific wrapper in source.",
                        wrapper_call_example(event.wrapper, &event.binding_name, &event.column_name)
                    )),
                    &event.node,
                ),
            );
        }

        let mut seen_near_misses = BTreeSet::new();
        for event in &self.near_miss_events {
            let key = format!(
                "{}|{}|{}|{}|{}|{}",
                event.relation_display,
                event.binding_name,
                event.column_name,
                event.current_wrapper.display_name(),
                event.expected_wrapper.display_name(),
                event.node.node_id
            );
            if !seen_near_misses.insert(key) {
                continue;
            }
            diagnostics.push(
                diagnostic_with_node(
                    QueryDiagnostic::warning(
                        "QFCH202",
                        format!(
                            "Materialized-view column `{}.{}` from `{}` expects `{}` but is already wrapped with `{}`.",
                            event.binding_name,
                            event.column_name,
                            event.relation_display,
                            event.expected_wrapper.display_name(),
                            event.current_wrapper.display_name(),
                        ),
                    )
                    .with_backend(backend)
                    .with_remediation(format!(
                        "Replace the wrapper with `{}` or let the ClickHouse adapter auto-wrap the raw column.",
                        wrapper_call_example(
                            event.expected_wrapper,
                            &event.binding_name,
                            &event.column_name,
                        )
                    )),
                    &event.node,
                ),
            );
        }

        diagnostics
    }
}

#[derive(Debug, Clone)]
struct WrapEvent {
    relation_display: String,
    binding_name: String,
    column_name: String,
    wrapper: WrapperSpec,
    node: SyntaxNode,
}

#[derive(Debug, Clone)]
struct NearMissEvent {
    relation_display: String,
    binding_name: String,
    column_name: String,
    current_wrapper: WrapperSpec,
    expected_wrapper: WrapperSpec,
    node: SyntaxNode,
}

fn diagnostic_with_node(mut diagnostic: QueryDiagnostic, node: &SyntaxNode) -> QueryDiagnostic {
    diagnostic = diagnostic.with_node_id(node.node_id.clone());
    if let Some(span) = node.span {
        diagnostic = diagnostic.with_span(span);
    }
    diagnostic
}

fn wrapper_call_example(wrapper: WrapperSpec, relation: &str, column: &str) -> String {
    format!("{}({relation}.{column})", wrapper.display_name())
}

#[cfg(test)]
mod tests {
    use queryfabric_catalog::{
        BackendAdapter, BackendFeature, Catalog, ColumnSchema, EmitArtifact, MemoryCatalog,
        PlanCostEstimator, RelationKind, RelationSchema, bind_and_validate,
    };
    use queryfabric_dialect_sql::GenericSqlDialect;
    use queryfabric_ir::{DataType, Dialect, FieldMetadata, QueryParameters};

    use super::ClickHouseAdapter;

    fn catalog() -> impl Catalog {
        let mut catalog = MemoryCatalog::default();
        catalog.register_relation(RelationSchema {
            namespace: None,
            name: "records".into(),
            aliases: Vec::new(),
            kind: RelationKind::Table,
            columns: vec![
                ColumnSchema {
                    name: "dataset_id".into(),
                    data_type: DataType::Utf8,
                    nullable: false,
                    metadata: Default::default(),
                },
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
            ],
            metadata: [
                ("estimated_rows".into(), "1000000".into()),
                ("average_row_bytes".into(), "96".into()),
                ("partition_column".into(), "dataset_id".into()),
                ("partition_count".into(), "32".into()),
            ]
            .into_iter()
            .collect(),
        });
        catalog.register_relation(RelationSchema {
            namespace: None,
            name: "mv_dataset_summary".into(),
            aliases: Vec::new(),
            kind: RelationKind::MaterializedView,
            columns: vec![
                ColumnSchema {
                    name: "dataset_id".into(),
                    data_type: DataType::Uuid,
                    nullable: false,
                    metadata: Default::default(),
                },
                ColumnSchema {
                    name: "table_name".into(),
                    data_type: DataType::Utf8,
                    nullable: false,
                    metadata: Default::default(),
                },
                mv_column("row_count", DataType::Int64, "countmerge"),
                mv_column(
                    "last_updated",
                    DataType::Timestamp { timezone: None },
                    "max",
                ),
            ],
            metadata: [
                ("estimated_rows".into(), "128".into()),
                ("average_row_bytes".into(), "128".into()),
                ("partition_column".into(), "dataset_id".into()),
                ("partition_count".into(), "8".into()),
            ]
            .into_iter()
            .collect(),
        });
        catalog
    }

    fn mv_column(name: &str, data_type: DataType, merge_fn: &str) -> ColumnSchema {
        let mut metadata = FieldMetadata::default();
        metadata
            .extensions
            .insert("clickhouse.mv.merge_fn".into(), merge_fn.into());
        ColumnSchema {
            name: name.into(),
            data_type,
            nullable: true,
            metadata,
        }
    }

    fn bind(sql: &str) -> queryfabric_ir::BoundQuery {
        let catalog = catalog();
        let parsed = GenericSqlDialect.parse(sql).expect("parse");
        bind_and_validate(&parsed, &catalog, &QueryParameters::default()).expect("bind")
    }

    #[test]
    fn emits_sql_for_plain_select() {
        let catalog = catalog();
        let bound = bind("SELECT record_id, score FROM records LIMIT 10");
        let artifact = ClickHouseAdapter.emit(&bound, &catalog).expect("emit");
        let EmitArtifact::Sql(sql) = artifact else {
            panic!("expected SQL artifact");
        };
        assert_eq!(
            sql.text,
            "SELECT toString(records.record_id) AS record_id, records.score FROM records LIMIT 10"
        );
        assert_eq!(sql.result_schema.fields().len(), 2);
        assert_eq!(sql.result_schema.fields()[0].data_type, DataType::Utf8);
        assert!(sql.metadata.is_empty());
    }

    #[test]
    fn advertises_isolated_execution_capability() {
        assert!(
            ClickHouseAdapter
                .capabilities()
                .supports(BackendFeature::IsolatedExecution)
        );
    }

    #[test]
    fn advertises_uuid_arrow_workaround_capability() {
        assert!(
            ClickHouseAdapter
                .capabilities()
                .supports(BackendFeature::UuidToStringInArrowOutput)
        );
    }

    #[test]
    fn rewrites_uuid_projection_and_group_by_for_arrow_output() {
        let catalog = catalog();
        let bound = bind("SELECT record_id, count() AS n FROM records GROUP BY record_id");
        let artifact = ClickHouseAdapter.emit(&bound, &catalog).expect("emit");
        let EmitArtifact::Sql(sql) = artifact else {
            panic!("expected SQL artifact");
        };
        assert_eq!(
            sql.text,
            "SELECT toString(records.record_id) AS record_id, count(*) AS n FROM records GROUP BY toString(records.record_id)"
        );
        assert_eq!(sql.result_schema.fields()[0].data_type, DataType::Utf8);
        assert_eq!(sql.result_schema.fields()[0].name, "record_id");
    }

    #[test]
    fn rewrites_uuid_wildcard_projection_for_arrow_output() {
        let catalog = catalog();
        let bound = bind("SELECT * FROM records");
        let artifact = ClickHouseAdapter.emit(&bound, &catalog).expect("emit");
        let EmitArtifact::Sql(sql) = artifact else {
            panic!("expected SQL artifact");
        };
        assert_eq!(
            sql.text,
            "SELECT records.dataset_id AS dataset_id, toString(records.record_id) AS record_id, records.score AS score FROM records"
        );
        assert_eq!(sql.result_schema.fields()[1].data_type, DataType::Utf8);
    }

    #[test]
    fn leaves_non_uuid_projection_unchanged_for_arrow_output() {
        let catalog = catalog();
        let bound = bind("SELECT count() AS n FROM records");
        let artifact = ClickHouseAdapter.emit(&bound, &catalog).expect("emit");
        let EmitArtifact::Sql(sql) = artifact else {
            panic!("expected SQL artifact");
        };
        assert_eq!(sql.text, "SELECT count(*) AS n FROM records");
        assert_eq!(sql.result_schema.fields()[0].data_type, DataType::Int64);
    }

    #[test]
    fn preserves_existing_uuid_to_string_cast_without_double_wrap() {
        let catalog = catalog();
        let bound = bind("SELECT toString(record_id) AS record_id FROM records");
        let artifact = ClickHouseAdapter.emit(&bound, &catalog).expect("emit");
        let EmitArtifact::Sql(sql) = artifact else {
            panic!("expected SQL artifact");
        };
        assert_eq!(
            sql.text,
            "SELECT toString(records.record_id) AS record_id FROM records"
        );
        assert!(!sql.text.contains("toString(toString"), "{}", sql.text);
        assert_eq!(sql.result_schema.fields()[0].data_type, DataType::Utf8);
    }

    #[test]
    fn estimates_cost_for_grouped_query() {
        let catalog = catalog();
        let parsed = GenericSqlDialect
            .parse(
                "SELECT dataset_id, count(record_id) \
                 FROM records \
                 WHERE dataset_id = 'fafb' \
                 GROUP BY dataset_id",
            )
            .expect("parse");
        let bound =
            bind_and_validate(&parsed, &catalog, &QueryParameters::default()).expect("bind");

        let estimate = ClickHouseAdapter
            .estimate(&bound, &catalog)
            .expect("estimate");

        assert!(estimate.rows_scanned > 0, "{estimate:#?}");
        assert!(estimate.memory_bytes > 0, "{estimate:#?}");
        assert_eq!(estimate.partitions_touched, 1);
        assert!(estimate.wallclock_estimate_ms > 0, "{estimate:#?}");
    }

    #[test]
    fn rewrites_mv_projection_and_sets_metadata() {
        let catalog = catalog();
        let bound = bind(
            "SELECT ds.table_name, ds.row_count, ds.last_updated \
             FROM mv_dataset_summary AS ds \
             GROUP BY ds.table_name",
        );

        let analysis = ClickHouseAdapter.analyze(&bound, &catalog);
        assert!(
            analysis
                .diagnostics
                .iter()
                .any(|diag| diag.code == "QFCH201" && diag.message.contains("ds.row_count")),
            "{:#?}",
            analysis.diagnostics
        );

        let artifact = ClickHouseAdapter.emit(&bound, &catalog).expect("emit");
        let EmitArtifact::Sql(sql) = artifact else {
            panic!("expected SQL artifact");
        };
        assert_eq!(
            sql.metadata
                .get("clickhouse.rewritten_to")
                .map(String::as_str),
            Some("mv_dataset_summary")
        );
        assert!(
            sql.text.contains("countMerge(ds.row_count)"),
            "{}",
            sql.text
        );
        assert!(sql.text.contains("max(ds.last_updated)"), "{}", sql.text);
    }

    #[test]
    fn preserves_existing_wrapper_without_double_wrap() {
        let catalog = catalog();
        let bound = bind("SELECT ch.count_merge(row_count) FROM mv_dataset_summary");
        let artifact = ClickHouseAdapter.emit(&bound, &catalog).expect("emit");
        let EmitArtifact::Sql(sql) = artifact else {
            panic!("expected SQL artifact");
        };
        assert_eq!(
            sql.text,
            "SELECT countMerge(mv_dataset_summary.row_count) FROM mv_dataset_summary"
        );
        assert!(!sql.text.contains("countMerge(countMerge"), "{}", sql.text);
        assert!(!sql.metadata.contains_key("clickhouse.rewritten_to"));
    }

    #[test]
    fn surfaces_wrapper_near_miss_diagnostic() {
        let catalog = catalog();
        let bound = bind("SELECT ch.sum_merge(row_count) FROM mv_dataset_summary");
        let analysis = ClickHouseAdapter.analyze(&bound, &catalog);
        assert!(
            analysis.diagnostics.iter().any(|diag| {
                diag.code == "QFCH202"
                    && diag.message.contains("count_merge")
                    && diag.message.contains("sum_merge")
            }),
            "{:#?}",
            analysis.diagnostics
        );

        let artifact = ClickHouseAdapter.emit(&bound, &catalog).expect("emit");
        let EmitArtifact::Sql(sql) = artifact else {
            panic!("expected SQL artifact");
        };
        assert!(
            sql.text
                .contains("sumMerge(countMerge(mv_dataset_summary.row_count))"),
            "{}",
            sql.text
        );
    }
}
