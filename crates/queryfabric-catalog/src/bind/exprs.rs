use queryfabric_ir::{
    BoundColumnRef, BoundExpr, BoundExprKind, BoundWhenThen, DataType, ResultField, SyntaxExpr,
    SyntaxExprKind, SyntaxWhenThen,
};

use super::scope::ColumnResolution;
use super::suggest::top_similar;
use super::{
    Binder, ExpectedType, NullableConstraint, Scope, functions::bind_function,
    helpers::bind_literal,
};

impl Binder<'_> {
    pub(super) fn bind_expr(
        &mut self,
        expr: &SyntaxExpr,
        scope: &Scope,
        outer_scope: Option<&Scope>,
        expected: ExpectedType<'_>,
    ) -> BoundExpr {
        match &expr.kind {
            SyntaxExprKind::Column { relation, name } => {
                match scope.resolve_column(relation.as_deref(), name) {
                    ColumnResolution::Local(column) => BoundExpr {
                        kind: BoundExprKind::Column(BoundColumnRef {
                            relation: column.relation.clone(),
                            name: name.clone(),
                        }),
                        data_type: column.field.data_type.clone(),
                        nullable: column.field.nullable,
                        node: expr.node.clone(),
                    },
                    ColumnResolution::Ambiguous => self.unsupported_expr(
                        expr,
                        "QF0016",
                        format!("Ambiguous column reference `{name}`."),
                        Some("Qualify the column with a relation alias."),
                    ),
                    ColumnResolution::Missing => {
                        if outer_scope
                            .and_then(|scope| scope.try_resolve_column(relation.as_deref(), name))
                            .is_some()
                        {
                            self.unsupported_expr(
                            expr,
                            "QF0014",
                            "Correlated subqueries are outside the verified portable subset.",
                            Some("Rewrite the subquery so it depends only on its local FROM scope."),
                        )
                        } else {
                            let remediation =
                                missing_column_remediation(scope, relation.as_deref(), name);
                            self.unsupported_expr(
                                expr,
                                "QF0015",
                                format!("Unknown column `{name}`."),
                                remediation.as_deref(),
                            )
                        }
                    }
                }
            }
            SyntaxExprKind::Literal(value) => bind_literal(expr, value.clone()),
            SyntaxExprKind::Parameter(reference) => {
                let reference = self.allocate_parameter(reference.clone());
                self.record_parameter_use(&reference, expected, &expr.node);
                BoundExpr {
                    kind: BoundExprKind::Parameter(reference),
                    data_type: expected.data_type.cloned().unwrap_or(DataType::Unknown),
                    nullable: !matches!(expected.nullable, NullableConstraint::NonNull),
                    node: expr.node.clone(),
                }
            }
            SyntaxExprKind::Unary { op, expr: inner } => {
                let inner = self.bind_expr(inner, scope, outer_scope, ExpectedType::default());
                BoundExpr {
                    kind: BoundExprKind::Unary {
                        op: *op,
                        expr: Box::new(inner.clone()),
                    },
                    data_type: if matches!(op, queryfabric_ir::UnaryOperator::Not) {
                        DataType::Boolean
                    } else {
                        inner.data_type.clone()
                    },
                    nullable: inner.nullable,
                    node: expr.node.clone(),
                }
            }
            SyntaxExprKind::Binary { op, left, right } => {
                bind_binary_expr(self, expr, *op, left, right, scope, outer_scope)
            }
            SyntaxExprKind::Function(function) => {
                bind_function(self, expr, function, scope, outer_scope)
            }
            SyntaxExprKind::Case {
                operand,
                when_then,
                else_result,
            } => bind_case(
                self,
                expr,
                operand.as_deref(),
                when_then,
                else_result.as_deref(),
                scope,
                outer_scope,
            ),
            SyntaxExprKind::Cast {
                expr: inner,
                data_type,
            } => {
                let inner = self.bind_expr(
                    inner,
                    scope,
                    outer_scope,
                    ExpectedType {
                        data_type: Some(data_type),
                        nullable: NullableConstraint::Nullable,
                    },
                );
                BoundExpr {
                    kind: BoundExprKind::Cast {
                        expr: Box::new(inner.clone()),
                        data_type: data_type.clone(),
                    },
                    data_type: data_type.clone(),
                    nullable: inner.nullable,
                    node: expr.node.clone(),
                }
            }
            SyntaxExprKind::Between {
                expr: input,
                low,
                high,
                negated,
            } => {
                let input = self.bind_expr(input, scope, outer_scope, ExpectedType::default());
                let low = self.bind_expr(
                    low,
                    scope,
                    outer_scope,
                    ExpectedType {
                        data_type: (!input.data_type.is_unknown()).then_some(&input.data_type),
                        nullable: NullableConstraint::NonNull,
                    },
                );
                let high = self.bind_expr(
                    high,
                    scope,
                    outer_scope,
                    ExpectedType {
                        data_type: (!input.data_type.is_unknown()).then_some(&input.data_type),
                        nullable: NullableConstraint::NonNull,
                    },
                );
                let nullable = input.nullable || low.nullable || high.nullable;
                BoundExpr {
                    kind: BoundExprKind::Between {
                        expr: Box::new(input.clone()),
                        low: Box::new(low),
                        high: Box::new(high),
                        negated: *negated,
                    },
                    data_type: DataType::Boolean,
                    nullable,
                    node: expr.node.clone(),
                }
            }
            SyntaxExprKind::InList {
                expr: input,
                list,
                negated,
            } => {
                let input = self.bind_expr(input, scope, outer_scope, ExpectedType::default());
                let list =
                    if list.len() == 1 && matches!(list[0].kind, SyntaxExprKind::Parameter(_)) {
                        let list_type = DataType::List(Box::new(input.data_type.clone()));
                        vec![self.bind_expr(
                            &list[0],
                            scope,
                            outer_scope,
                            ExpectedType {
                                data_type: Some(&list_type),
                                nullable: NullableConstraint::NonNull,
                            },
                        )]
                    } else {
                        list.iter()
                            .map(|item| {
                                self.bind_expr(
                                    item,
                                    scope,
                                    outer_scope,
                                    ExpectedType {
                                        data_type: (!input.data_type.is_unknown())
                                            .then_some(&input.data_type),
                                        nullable: NullableConstraint::NonNull,
                                    },
                                )
                            })
                            .collect()
                    };
                let nullable = input.nullable || list.iter().any(|item| item.nullable);
                BoundExpr {
                    kind: BoundExprKind::InList {
                        expr: Box::new(input.clone()),
                        list,
                        negated: *negated,
                    },
                    data_type: DataType::Boolean,
                    nullable,
                    node: expr.node.clone(),
                }
            }
            SyntaxExprKind::InSubquery {
                expr: input,
                subquery,
                negated,
            } => {
                let input = self.bind_expr(input, scope, outer_scope, ExpectedType::default());
                let subquery = self.bind_query(subquery, Some(scope));
                let subquery_field =
                    subquery_single_field(self, expr, &subquery, "QF0024", "IN subqueries");
                BoundExpr {
                    kind: BoundExprKind::InSubquery {
                        expr: Box::new(input.clone()),
                        subquery: Box::new(subquery),
                        negated: *negated,
                    },
                    data_type: DataType::Boolean,
                    nullable: input.nullable
                        || subquery_field.as_ref().is_some_and(|field| field.nullable),
                    node: expr.node.clone(),
                }
            }
            SyntaxExprKind::ScalarSubquery(subquery) => {
                let subquery = self.bind_query(subquery, Some(scope));
                let field =
                    subquery_single_field(self, expr, &subquery, "QF0023", "Scalar subqueries")
                        .unwrap_or_else(|| ResultField::new("subquery", DataType::Unknown, true));
                BoundExpr {
                    kind: BoundExprKind::ScalarSubquery(Box::new(subquery)),
                    data_type: field.data_type,
                    // Scalar subqueries can legally yield NULL when the inner query
                    // produces zero rows, so the bound contract stays conservative
                    // until row-count reasoning becomes explicit in the IR.
                    nullable: true,
                    node: expr.node.clone(),
                }
            }
            SyntaxExprKind::Exists(subquery) => BoundExpr {
                kind: BoundExprKind::Exists(Box::new(self.bind_query(subquery, Some(scope)))),
                data_type: DataType::Boolean,
                nullable: false,
                node: expr.node.clone(),
            },
            SyntaxExprKind::Like {
                expr: input,
                pattern,
                negated,
                case_insensitive,
            } => {
                let input = self.bind_expr(
                    input,
                    scope,
                    outer_scope,
                    ExpectedType {
                        data_type: Some(&DataType::Utf8),
                        nullable: NullableConstraint::NonNull,
                    },
                );
                let pattern = self.bind_expr(
                    pattern,
                    scope,
                    outer_scope,
                    ExpectedType {
                        data_type: Some(&DataType::Utf8),
                        nullable: NullableConstraint::NonNull,
                    },
                );
                let nullable = input.nullable || pattern.nullable;
                BoundExpr {
                    kind: BoundExprKind::Like {
                        expr: Box::new(input),
                        pattern: Box::new(pattern),
                        negated: *negated,
                        case_insensitive: *case_insensitive,
                    },
                    data_type: DataType::Boolean,
                    nullable,
                    node: expr.node.clone(),
                }
            }
            SyntaxExprKind::IsNull {
                expr: inner,
                negated,
            } => {
                if let SyntaxExprKind::Parameter(reference) = &inner.kind {
                    let reference = self.allocate_parameter(reference.clone());
                    self.record_parameter_use(
                        &reference,
                        ExpectedType {
                            data_type: None,
                            nullable: NullableConstraint::Nullable,
                        },
                        &inner.node,
                    );
                }
                let inner = self.bind_expr(inner, scope, outer_scope, ExpectedType::default());
                BoundExpr {
                    kind: BoundExprKind::IsNull {
                        expr: Box::new(inner),
                        negated: *negated,
                    },
                    data_type: DataType::Boolean,
                    nullable: false,
                    node: expr.node.clone(),
                }
            }
            SyntaxExprKind::Tuple(items) => {
                let items = items
                    .iter()
                    .map(|item| self.bind_expr(item, scope, outer_scope, ExpectedType::default()))
                    .collect::<Vec<_>>();
                BoundExpr {
                    kind: BoundExprKind::Tuple(items.clone()),
                    data_type: DataType::Struct(
                        items
                            .iter()
                            .enumerate()
                            .map(|(idx, item)| {
                                ResultField::new(
                                    format!("_{idx}"),
                                    item.data_type.clone(),
                                    item.nullable,
                                )
                            })
                            .collect(),
                    ),
                    nullable: items.iter().any(|item| item.nullable),
                    node: expr.node.clone(),
                }
            }
            SyntaxExprKind::Array(items) => {
                let items = items
                    .iter()
                    .map(|item| self.bind_expr(item, scope, outer_scope, ExpectedType::default()))
                    .collect::<Vec<_>>();
                let item_type = items
                    .iter()
                    .map(|item| item.data_type.clone())
                    .reduce(|left, right| {
                        DataType::common_type(&left, &right).unwrap_or(DataType::Unknown)
                    })
                    .unwrap_or(DataType::Unknown);
                BoundExpr {
                    kind: BoundExprKind::Array(items.clone()),
                    data_type: DataType::List(Box::new(item_type)),
                    nullable: items.iter().any(|item| item.nullable),
                    node: expr.node.clone(),
                }
            }
            SyntaxExprKind::Unsupported { description } => {
                self.unsupported_expr(expr, "QF0012", description.clone(), None)
            }
        }
    }
}

fn missing_column_remediation(scope: &Scope, relation: Option<&str>, name: &str) -> Option<String> {
    match relation {
        Some(relation) if !scope.has_relation(relation) => Some(format!(
            "Unknown relation alias `{relation}`. Check the relation aliases in scope."
        )),
        Some(relation) => scope.relation_column_names(relation).map(|columns| {
            let suggestions = top_similar(name, columns.iter().map(String::as_str), 3);
            if suggestions.is_empty() {
                format!("Check the relation alias `{relation}` and its column names.")
            } else {
                format!(
                    "Check the column name on `{relation}`. Did you mean: {}?",
                    suggestions.join(", ")
                )
            }
        }),
        None => {
            let columns = scope.all_column_names();
            let suggestions = top_similar(name, columns.iter().map(String::as_str), 3);
            if suggestions.is_empty() {
                Some("Check the relation aliases and column names in scope.".into())
            } else {
                Some(format!(
                    "Check the relation aliases and column names in scope. Did you mean: {}?",
                    suggestions.join(", ")
                ))
            }
        }
    }
}

fn bind_binary_expr(
    binder: &mut Binder<'_>,
    expr: &SyntaxExpr,
    op: queryfabric_ir::BinaryOperator,
    left: &SyntaxExpr,
    right: &SyntaxExpr,
    scope: &Scope,
    outer_scope: Option<&Scope>,
) -> BoundExpr {
    use queryfabric_ir::BinaryOperator::*;

    match op {
        And | Or => {
            let left = binder.bind_expr(
                left,
                scope,
                outer_scope,
                ExpectedType {
                    data_type: Some(&DataType::Boolean),
                    nullable: NullableConstraint::NonNull,
                },
            );
            let right = binder.bind_expr(
                right,
                scope,
                outer_scope,
                ExpectedType {
                    data_type: Some(&DataType::Boolean),
                    nullable: NullableConstraint::NonNull,
                },
            );
            BoundExpr {
                kind: BoundExprKind::Binary {
                    op,
                    left: Box::new(left.clone()),
                    right: Box::new(right.clone()),
                },
                data_type: DataType::Boolean,
                nullable: left.nullable || right.nullable,
                node: expr.node.clone(),
            }
        }
        Add | Subtract | Multiply | Divide => {
            let left = binder.bind_expr(
                left,
                scope,
                outer_scope,
                ExpectedType {
                    data_type: Some(&DataType::Float64),
                    nullable: NullableConstraint::NonNull,
                },
            );
            let right = binder.bind_expr(
                right,
                scope,
                outer_scope,
                ExpectedType {
                    data_type: Some(&DataType::Float64),
                    nullable: NullableConstraint::NonNull,
                },
            );
            BoundExpr {
                kind: BoundExprKind::Binary {
                    op,
                    left: Box::new(left.clone()),
                    right: Box::new(right.clone()),
                },
                data_type: DataType::common_type(&left.data_type, &right.data_type)
                    .unwrap_or(DataType::Float64),
                nullable: left.nullable || right.nullable,
                node: expr.node.clone(),
            }
        }
        Eq | NotEq | Lt | LtEq | Gt | GtEq => {
            let left = binder.bind_expr(left, scope, outer_scope, ExpectedType::default());
            let right = binder.bind_expr(
                right,
                scope,
                outer_scope,
                ExpectedType {
                    data_type: (!left.data_type.is_unknown()).then_some(&left.data_type),
                    nullable: NullableConstraint::NonNull,
                },
            );
            BoundExpr {
                kind: BoundExprKind::Binary {
                    op,
                    left: Box::new(left.clone()),
                    right: Box::new(right.clone()),
                },
                data_type: DataType::Boolean,
                nullable: left.nullable || right.nullable,
                node: expr.node.clone(),
            }
        }
    }
}

fn bind_case(
    binder: &mut Binder<'_>,
    expr: &SyntaxExpr,
    operand: Option<&SyntaxExpr>,
    when_then: &[SyntaxWhenThen],
    else_result: Option<&SyntaxExpr>,
    scope: &Scope,
    outer_scope: Option<&Scope>,
) -> BoundExpr {
    let operand = operand
        .map(|expr| Box::new(binder.bind_expr(expr, scope, outer_scope, ExpectedType::default())));
    let when_then = when_then
        .iter()
        .map(|pair| BoundWhenThen {
            condition: binder.bind_expr(
                &pair.condition,
                scope,
                outer_scope,
                ExpectedType {
                    data_type: Some(&DataType::Boolean),
                    nullable: NullableConstraint::NonNull,
                },
            ),
            result: binder.bind_expr(&pair.result, scope, outer_scope, ExpectedType::default()),
        })
        .collect::<Vec<_>>();
    let else_result = else_result
        .map(|expr| Box::new(binder.bind_expr(expr, scope, outer_scope, ExpectedType::default())));
    let data_type = when_then
        .iter()
        .map(|pair| pair.result.data_type.clone())
        .chain(else_result.iter().map(|expr| expr.data_type.clone()))
        .reduce(|left, right| DataType::common_type(&left, &right).unwrap_or(DataType::Unknown))
        .unwrap_or(DataType::Unknown);
    let nullable = when_then.iter().any(|pair| pair.result.nullable)
        || else_result.as_ref().is_none_or(|expr| expr.nullable);
    BoundExpr {
        kind: BoundExprKind::Case {
            operand,
            when_then,
            else_result,
        },
        data_type,
        nullable,
        node: expr.node.clone(),
    }
}

fn subquery_single_field(
    binder: &mut Binder<'_>,
    expr: &SyntaxExpr,
    subquery: &queryfabric_ir::BoundQueryPlan,
    code: &str,
    context: &str,
) -> Option<ResultField> {
    match subquery.result_schema.fields.as_slice() {
        [field] => Some(field.clone()),
        fields => {
            binder.push_error(
                code,
                format!(
                    "{context} must project exactly one column in the verified portable subset; found {}.",
                    fields.len()
                ),
                &expr.node,
                Some("Rewrite the subquery to return exactly one projected column."),
            );
            None
        }
    }
}
