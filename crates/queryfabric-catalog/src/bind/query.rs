use std::collections::BTreeMap;

use queryfabric_ir::{
    BackendClause, BoundCte, BoundJoin, BoundOrderByExpr, BoundProjectionItem, BoundQueryPlan,
    BoundRelation, BoundRelationBinding, BoundSelect, BoundSetExpr, BoundTableWithJoins,
    CapabilityRequirement, CapabilityRequirements, DataType, JoinKind, NameRef, ResultField,
    ResultSchema, SyntaxProjectionItem, SyntaxQuery, SyntaxRelation, SyntaxSelect, SyntaxSetExpr,
    SyntaxTableWithJoins,
};

use crate::features::plan_features_from_bound;
use crate::model::Catalog;

use super::{
    Binder, ExpectedType, NullableConstraint, Scope, helpers::expression_name,
    scope::scope_schema_for_table,
};

impl Binder<'_> {
    pub(super) fn bind_query(
        &mut self,
        query: &SyntaxQuery,
        outer_scope: Option<&Scope>,
    ) -> BoundQueryPlan {
        let mut cte_bindings = BTreeMap::new();
        let mut bound_ctes = Vec::new();

        if query.with_recursive {
            self.push_error(
                "QF0001",
                "WITH RECURSIVE is outside the verified QueryFabric portable subset.",
                &query.node,
                Some("Rewrite the query using a non-recursive CTE or a host-side expansion."),
            );
        }

        for cte in &query.ctes {
            let plan = self.bind_query(&cte.query, None);
            let mut result_schema = plan.result_schema.clone();
            if !cte.columns.is_empty() && cte.columns.len() == result_schema.fields.len() {
                for (field, alias) in result_schema.fields.iter_mut().zip(&cte.columns) {
                    field.name = alias.clone();
                }
            }
            let binding = BoundRelationBinding {
                binding_name: cte.name.clone(),
                relation_name: Some(NameRef {
                    namespace: None,
                    name: cte.name.clone(),
                }),
                schema: result_schema.clone(),
            };
            cte_bindings.insert(cte.name.to_ascii_lowercase(), binding.clone());
            bound_ctes.push(BoundCte {
                name: cte.name.clone(),
                columns: cte.columns.clone(),
                query: Box::new(plan),
                result_schema,
                node: cte.node.clone(),
            });
        }

        let body = self.bind_set_expr(&query.body, &cte_bindings, outer_scope);
        let output_scope = Scope::from_output_schema(body.result_schema());
        let order_by = query
            .order_by
            .iter()
            .map(|expr| self.bind_order_by_expr(expr, &output_scope, None))
            .collect();
        let limit = query.limit.as_ref().map(|expr| {
            self.bind_expr(
                expr,
                &output_scope,
                None,
                ExpectedType {
                    data_type: Some(&DataType::Int64),
                    nullable: NullableConstraint::NonNull,
                },
            )
        });
        let offset = query.offset.as_ref().map(|expr| {
            self.bind_expr(
                expr,
                &output_scope,
                None,
                ExpectedType {
                    data_type: Some(&DataType::Int64),
                    nullable: NullableConstraint::NonNull,
                },
            )
        });

        for clause in &query.backend_clauses {
            match clause {
                BackendClause::ClickHouseSettings { node, .. } => self.push_warning(
                    "QF0101",
                    "ClickHouse SETTINGS are preserved only for backend-specific adapters.",
                    node,
                    Some("Do not rely on SETTINGS for portable execution."),
                ),
                BackendClause::ClickHouseFormat { node, .. } => self.push_warning(
                    "QF0102",
                    "ClickHouse FORMAT is preserved only for backend-specific adapters.",
                    node,
                    Some("Prefer result consumers to choose the output format outside QueryFabric core."),
                ),
            }
        }

        BoundQueryPlan {
            node: query.node.clone(),
            ctes: bound_ctes,
            body: body.clone(),
            order_by,
            limit,
            offset,
            backend_clauses: query.backend_clauses.clone(),
            result_schema: body.result_schema().clone(),
        }
    }

    pub(super) fn bind_set_expr(
        &mut self,
        expr: &SyntaxSetExpr,
        ctes: &BTreeMap<String, BoundRelationBinding>,
        outer_scope: Option<&Scope>,
    ) -> BoundSetExpr {
        match expr {
            SyntaxSetExpr::Select(select) => {
                BoundSetExpr::select(self.bind_select(select, ctes, outer_scope))
            }
            SyntaxSetExpr::UnionAll { left, right, node } => {
                let left = self.bind_set_expr(left, ctes, outer_scope);
                let right = self.bind_set_expr(right, ctes, outer_scope);
                let schema = unify_union_schema(
                    &mut self.diagnostics,
                    left.result_schema(),
                    right.result_schema(),
                    node,
                );
                BoundSetExpr::UnionAll {
                    left: Box::new(left),
                    right: Box::new(right),
                    node: node.clone(),
                    result_schema: schema,
                }
            }
            SyntaxSetExpr::Unsupported { description, node } => {
                self.push_error("QF0002", description.clone(), node, None);
                BoundSetExpr::Unsupported {
                    description: description.clone(),
                    node: node.clone(),
                    result_schema: ResultSchema::default(),
                }
            }
        }
    }

    pub(super) fn bind_select(
        &mut self,
        select: &SyntaxSelect,
        ctes: &BTreeMap<String, BoundRelationBinding>,
        outer_scope: Option<&Scope>,
    ) -> BoundSelect {
        let mut scope = Scope::default();
        let mut from = Vec::new();

        for table in &select.from {
            let bound_table = self.bind_table_with_joins(table, ctes, outer_scope, &scope);
            scope.merge_table(&bound_table);
            from.push(bound_table);
        }

        let selection = select.selection.as_ref().map(|expr| {
            self.bind_expr(
                expr,
                &scope,
                outer_scope,
                ExpectedType {
                    data_type: Some(&DataType::Boolean),
                    nullable: NullableConstraint::NonNull,
                },
            )
        });
        let group_by = select
            .group_by
            .iter()
            .map(|expr| self.bind_expr(expr, &scope, outer_scope, ExpectedType::default()))
            .collect::<Vec<_>>();
        let having = select.having.as_ref().map(|expr| {
            self.bind_expr(
                expr,
                &scope,
                outer_scope,
                ExpectedType {
                    data_type: Some(&DataType::Boolean),
                    nullable: NullableConstraint::NonNull,
                },
            )
        });

        let mut projection = Vec::new();
        let mut result_fields = Vec::new();
        for item in &select.projection {
            match item {
                SyntaxProjectionItem::Wildcard { qualifier, node } => {
                    let fields = qualifier
                        .as_deref()
                        .map(|qualifier| scope.expand_qualified(qualifier))
                        .unwrap_or_else(|| Some(scope.expand_all()))
                        .unwrap_or_else(|| {
                            self.push_error(
                                "QF0007",
                                format!(
                                    "Unknown relation qualifier `{}` in wildcard projection.",
                                    qualifier.clone().unwrap_or_default()
                                ),
                                node,
                                Some("Use an in-scope relation alias or remove the qualifier."),
                            );
                            Vec::new()
                        });
                    result_fields.extend(fields.clone());
                    projection.push(BoundProjectionItem::Wildcard {
                        qualifier: qualifier.clone(),
                        fields,
                        node: node.clone(),
                    });
                }
                SyntaxProjectionItem::Expr(details) => {
                    let bound =
                        self.bind_expr(&details.expr, &scope, outer_scope, ExpectedType::default());
                    let field = ResultField {
                        name: details
                            .alias
                            .clone()
                            .unwrap_or_else(|| expression_name(&details.expr)),
                        data_type: bound.data_type.clone(),
                        nullable: bound.nullable,
                        metadata: queryfabric_ir::FieldMetadata::default(),
                    };
                    result_fields.push(field.clone());
                    projection.push(BoundProjectionItem::expr(
                        bound,
                        details.alias.clone(),
                        field,
                        details.node.clone(),
                    ));
                }
                SyntaxProjectionItem::Unsupported { description, node } => {
                    self.push_error("QF0008", description.clone(), node, None);
                    projection.push(BoundProjectionItem::Unsupported {
                        description: description.clone(),
                        node: node.clone(),
                    });
                }
            }
        }

        BoundSelect {
            distinct: select.distinct,
            projection,
            from,
            selection,
            group_by,
            having,
            result_schema: ResultSchema::new(result_fields),
            node: select.node.clone(),
        }
    }

    pub(super) fn bind_table_with_joins(
        &mut self,
        table: &SyntaxTableWithJoins,
        ctes: &BTreeMap<String, BoundRelationBinding>,
        outer_scope: Option<&Scope>,
        current_scope: &Scope,
    ) -> BoundTableWithJoins {
        let mut relation = self.bind_relation(&table.relation, ctes, outer_scope, current_scope);
        let mut join_scope = current_scope.clone();
        join_scope.push_relation(&relation);
        let mut joins = Vec::new();
        for join in &table.joins {
            let mut joined_relation =
                self.bind_relation(&join.relation, ctes, outer_scope, &join_scope);
            let mut on_scope = join_scope.clone();
            on_scope.push_relation(&joined_relation);
            let on = join.on.as_ref().map(|expr| {
                self.bind_expr(
                    expr,
                    &on_scope,
                    outer_scope,
                    ExpectedType {
                        data_type: Some(&DataType::Boolean),
                        nullable: NullableConstraint::NonNull,
                    },
                )
            });

            match join.kind {
                JoinKind::Left => make_relation_nullable(&mut joined_relation),
                JoinKind::Right => {
                    make_relation_nullable(&mut relation);
                    make_join_relations_nullable(&mut joins);
                    join_scope.make_all_nullable();
                }
                JoinKind::Full => {
                    make_relation_nullable(&mut relation);
                    make_join_relations_nullable(&mut joins);
                    join_scope.make_all_nullable();
                    make_relation_nullable(&mut joined_relation);
                }
                JoinKind::Inner | JoinKind::Cross => {}
            }

            join_scope.push_relation(&joined_relation);
            joins.push(BoundJoin {
                kind: join.kind,
                relation: joined_relation,
                on,
                node: join.node.clone(),
            });
        }
        BoundTableWithJoins {
            relation,
            joins,
            node: table.node.clone(),
        }
    }

    pub(super) fn bind_relation(
        &mut self,
        relation: &SyntaxRelation,
        ctes: &BTreeMap<String, BoundRelationBinding>,
        _outer_scope: Option<&Scope>,
        current_scope: &Scope,
    ) -> BoundRelation {
        match relation {
            SyntaxRelation::Table { name, alias, node } => {
                if name.namespace.is_none()
                    && let Some(binding) = ctes.get(&name.name.to_ascii_lowercase())
                {
                    let mut binding = binding.clone();
                    if let Some(alias) = alias {
                        binding.binding_name = alias.clone();
                    }
                    return BoundRelation::Table {
                        binding,
                        node: node.clone(),
                    };
                }
                match self
                    .catalog
                    .resolve_relation(name.namespace.as_deref(), &name.name)
                {
                    Some(schema) => BoundRelation::Table {
                        binding: BoundRelationBinding {
                            binding_name: alias.clone().unwrap_or_else(|| name.name.clone()),
                            relation_name: Some(name.clone()),
                            schema: schema.to_result_schema(),
                        },
                        node: node.clone(),
                    },
                    None => {
                        self.push_error(
                            "QF0005",
                            format!("Unknown relation `{}`.", name.display_name()),
                            node,
                            Some("Register the relation in the catalog or qualify it with the correct namespace."),
                        );
                        BoundRelation::Unsupported {
                            description: format!("unknown relation `{}`", name.display_name()),
                            binding_name: alias.clone().unwrap_or_else(|| name.name.clone()),
                            node: node.clone(),
                        }
                    }
                }
            }
            SyntaxRelation::Derived { query, alias, node } => {
                let plan = self.bind_query(query, Some(current_scope));
                let binding_name = alias
                    .clone()
                    .unwrap_or_else(|| node.node_id.replace('.', "_"));
                BoundRelation::Derived {
                    binding: BoundRelationBinding {
                        binding_name,
                        relation_name: None,
                        schema: plan.result_schema.clone(),
                    },
                    query: Box::new(plan),
                    node: node.clone(),
                }
            }
            SyntaxRelation::NestedJoin {
                table_with_joins,
                alias,
                node,
            } => {
                let table = self.bind_table_with_joins(table_with_joins, ctes, None, current_scope);
                BoundRelation::NestedJoin {
                    binding: BoundRelationBinding {
                        binding_name: alias
                            .clone()
                            .unwrap_or_else(|| node.node_id.replace('.', "_")),
                        relation_name: None,
                        schema: scope_schema_for_table(&table),
                    },
                    table_with_joins: Box::new(table),
                    node: node.clone(),
                }
            }
            SyntaxRelation::Unsupported { description, node } => {
                self.push_error("QF0009", description.clone(), node, None);
                BoundRelation::Unsupported {
                    description: description.clone(),
                    binding_name: node.node_id.clone(),
                    node: node.clone(),
                }
            }
        }
    }

    pub(super) fn bind_order_by_expr(
        &mut self,
        expr: &queryfabric_ir::SyntaxOrderByExpr,
        scope: &Scope,
        outer_scope: Option<&Scope>,
    ) -> BoundOrderByExpr {
        BoundOrderByExpr {
            expr: self.bind_expr(&expr.expr, scope, outer_scope, ExpectedType::default()),
            asc: expr.asc,
            nulls_first: expr.nulls_first,
            node: expr.node.clone(),
        }
    }
}

fn unify_union_schema(
    diagnostics: &mut Vec<queryfabric_ir::QueryDiagnostic>,
    left: &ResultSchema,
    right: &ResultSchema,
    node: &queryfabric_ir::SyntaxNode,
) -> ResultSchema {
    if left.fields.len() != right.fields.len() {
        let mut diagnostic = queryfabric_ir::QueryDiagnostic::error(
            "QF0022",
            format!(
                "UNION ALL requires equal column counts but found {} and {}.",
                left.fields.len(),
                right.fields.len()
            ),
        )
        .with_node_id(node.node_id.clone());
        if let Some(span) = node.span {
            diagnostic = diagnostic.with_span(span);
        }
        diagnostics.push(diagnostic);
        return ResultSchema::default();
    }
    ResultSchema::new(
        left.fields
            .iter()
            .zip(&right.fields)
            .map(|(left, right)| ResultField {
                name: left.name.clone(),
                data_type: DataType::common_type(&left.data_type, &right.data_type)
                    .unwrap_or(DataType::Unknown),
                nullable: left.nullable || right.nullable,
                metadata: left.metadata.clone(),
            })
            .collect(),
    )
}

fn make_join_relations_nullable(joins: &mut [BoundJoin]) {
    for join in joins {
        make_relation_nullable(&mut join.relation);
    }
}

fn make_relation_nullable(relation: &mut BoundRelation) {
    if let Some(binding) = relation_binding_mut(relation) {
        for field in &mut binding.schema.fields {
            field.nullable = true;
        }
    }
}

fn relation_binding_mut(relation: &mut BoundRelation) -> Option<&mut BoundRelationBinding> {
    match relation {
        BoundRelation::Table { binding, .. }
        | BoundRelation::Derived { binding, .. }
        | BoundRelation::NestedJoin { binding, .. } => Some(binding),
        BoundRelation::Unsupported { .. } => None,
    }
}

pub(super) fn capability_requirements_from_plan(
    plan: &BoundQueryPlan,
    explain: bool,
    catalog: &dyn Catalog,
) -> CapabilityRequirements {
    let features = plan_features_from_bound(plan, catalog);
    let mut requirements = CapabilityRequirements::default();
    if features.has_ctes {
        requirements.require(CapabilityRequirement::CommonTableExpressions);
    }
    if features.has_derived_tables {
        requirements.require(CapabilityRequirement::DerivedTables);
    }
    if features.has_joins {
        requirements.require(CapabilityRequirement::Joins);
    }
    if features.has_windows {
        requirements.require(CapabilityRequirement::Windows);
    }
    if features.has_set_operations {
        requirements.require(CapabilityRequirement::SetOperations);
    }
    if features.has_aggregates {
        requirements.require(CapabilityRequirement::Aggregates);
    }
    if features.has_distinct_aggregates {
        requirements.require(CapabilityRequirement::DistinctAggregates);
    }
    if features.has_scalar_subqueries {
        requirements.require(CapabilityRequirement::ScalarSubqueries);
    }
    if features.has_in_subqueries {
        requirements.require(CapabilityRequirement::InSubqueries);
    }
    if features.has_limit_offset {
        requirements.require(CapabilityRequirement::LimitOffset);
    }
    if explain {
        requirements.require(CapabilityRequirement::Explain);
    }
    for function in features.functions {
        if function.namespace.is_some() {
            requirements.require(CapabilityRequirement::NamespacedFunctions);
        }
        if catalog
            .resolve_function(function.namespace.as_deref(), &function.name)
            .is_some_and(|signature| signature.metadata_flag("approximate"))
        {
            requirements.require(CapabilityRequirement::ApproximateAggregates);
        }
        requirements.record_function(function);
    }
    requirements
}
