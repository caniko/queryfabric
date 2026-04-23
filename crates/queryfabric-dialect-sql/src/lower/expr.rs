use super::Lowerer;
use crate::helpers::{
    lower_binary_operator, lower_data_type, lower_function_ref, lower_unary_operator,
    placeholder_to_parameter_ref,
};
use queryfabric_ir::{
    LiteralValue, QuerySourceSpan, SyntaxExpr, SyntaxExprKind, SyntaxFunctionCall, SyntaxNode,
    SyntaxWhenThen, WindowSpec,
};
use sqlparser::ast::Spanned;
use sqlparser::ast::{
    Expr, Function, FunctionArg, FunctionArgExpr, FunctionArguments, Value, WindowType,
};

impl Lowerer<'_> {
    pub(crate) fn lower_expr(&mut self, expr: &Expr, path: &str) -> SyntaxExpr {
        let node = self.node(expr, path);
        let kind = match expr {
            Expr::Identifier(identifier) => SyntaxExprKind::Column {
                relation: None,
                name: identifier.value.clone(),
            },
            Expr::CompoundIdentifier(parts) => {
                let mut values = parts
                    .iter()
                    .map(|part| part.value.clone())
                    .collect::<Vec<_>>();
                let name = values.pop().unwrap_or_default();
                let relation = (!values.is_empty()).then(|| values.join("."));
                SyntaxExprKind::Column { relation, name }
            }
            Expr::Value(value) => match &value.value {
                Value::Boolean(boolean) => SyntaxExprKind::Literal(LiteralValue::Boolean(*boolean)),
                Value::Number(number, _) => {
                    if number.contains('.') {
                        SyntaxExprKind::Literal(LiteralValue::Float64(number.clone()))
                    } else if let Ok(integer) = number.parse::<i64>() {
                        SyntaxExprKind::Literal(LiteralValue::Int64(integer))
                    } else {
                        SyntaxExprKind::Literal(LiteralValue::Float64(number.clone()))
                    }
                }
                Value::SingleQuotedString(text) | Value::DoubleQuotedString(text) => {
                    SyntaxExprKind::Literal(LiteralValue::Utf8(text.clone()))
                }
                Value::Null => SyntaxExprKind::Literal(LiteralValue::Null),
                Value::Placeholder(placeholder) => {
                    SyntaxExprKind::Parameter(placeholder_to_parameter_ref(placeholder))
                }
                _ => SyntaxExprKind::Unsupported {
                    description: "Unsupported literal value.".into(),
                },
            },
            Expr::BinaryOp { left, op, right } => {
                if let Some(op) = lower_binary_operator(op) {
                    SyntaxExprKind::Binary {
                        op,
                        left: Box::new(self.lower_expr(left, &format!("{path}.left"))),
                        right: Box::new(self.lower_expr(right, &format!("{path}.right"))),
                    }
                } else {
                    SyntaxExprKind::Unsupported {
                        description: format!("Unsupported binary operator `{op}`."),
                    }
                }
            }
            Expr::UnaryOp { op, expr } => {
                if let Some(op) = lower_unary_operator(op) {
                    SyntaxExprKind::Unary {
                        op,
                        expr: Box::new(self.lower_expr(expr, &format!("{path}.expr"))),
                    }
                } else {
                    SyntaxExprKind::Unsupported {
                        description: format!("Unsupported unary operator `{op}`."),
                    }
                }
            }
            Expr::Nested(expr) => return self.lower_expr(expr, path),
            Expr::Function(function) => {
                SyntaxExprKind::Function(self.lower_function(function, path))
            }
            Expr::Case {
                operand,
                conditions,
                else_result,
                ..
            } => SyntaxExprKind::Case {
                operand: operand
                    .as_ref()
                    .map(|expr| Box::new(self.lower_expr(expr, &format!("{path}.operand")))),
                when_then: conditions
                    .iter()
                    .enumerate()
                    .map(|(idx, when)| SyntaxWhenThen {
                        condition: self.lower_expr(
                            &when.condition,
                            &format!("{path}.when_then[{idx}].condition"),
                        ),
                        result: self
                            .lower_expr(&when.result, &format!("{path}.when_then[{idx}].result")),
                    })
                    .collect(),
                else_result: else_result
                    .as_ref()
                    .map(|expr| Box::new(self.lower_expr(expr, &format!("{path}.else")))),
            },
            Expr::Cast {
                expr, data_type, ..
            } => SyntaxExprKind::Cast {
                expr: Box::new(self.lower_expr(expr, &format!("{path}.expr"))),
                data_type: lower_data_type(data_type),
            },
            Expr::Between {
                expr,
                low,
                high,
                negated,
            } => SyntaxExprKind::Between {
                expr: Box::new(self.lower_expr(expr, &format!("{path}.expr"))),
                low: Box::new(self.lower_expr(low, &format!("{path}.low"))),
                high: Box::new(self.lower_expr(high, &format!("{path}.high"))),
                negated: *negated,
            },
            Expr::InList {
                expr,
                list,
                negated,
            } => SyntaxExprKind::InList {
                expr: Box::new(self.lower_expr(expr, &format!("{path}.expr"))),
                list: list
                    .iter()
                    .enumerate()
                    .map(|(idx, expr)| self.lower_expr(expr, &format!("{path}.list[{idx}]")))
                    .collect(),
                negated: *negated,
            },
            Expr::InSubquery {
                expr,
                subquery,
                negated,
            } => SyntaxExprKind::InSubquery {
                expr: Box::new(self.lower_expr(expr, &format!("{path}.expr"))),
                subquery: Box::new(self.lower_query(subquery, &format!("{path}.subquery"))),
                negated: *negated,
            },
            Expr::Subquery(subquery) => SyntaxExprKind::ScalarSubquery(Box::new(
                self.lower_query(subquery, &format!("{path}.subquery")),
            )),
            Expr::Exists { subquery, .. } => SyntaxExprKind::Exists(Box::new(
                self.lower_query(subquery, &format!("{path}.subquery")),
            )),
            Expr::Like {
                expr,
                pattern,
                negated,
                ..
            } => SyntaxExprKind::Like {
                expr: Box::new(self.lower_expr(expr, &format!("{path}.expr"))),
                pattern: Box::new(self.lower_expr(pattern, &format!("{path}.pattern"))),
                negated: *negated,
                case_insensitive: false,
            },
            Expr::ILike {
                expr,
                pattern,
                negated,
                ..
            } => SyntaxExprKind::Like {
                expr: Box::new(self.lower_expr(expr, &format!("{path}.expr"))),
                pattern: Box::new(self.lower_expr(pattern, &format!("{path}.pattern"))),
                negated: *negated,
                case_insensitive: true,
            },
            Expr::IsNull(expr) => SyntaxExprKind::IsNull {
                expr: Box::new(self.lower_expr(expr, &format!("{path}.expr"))),
                negated: false,
            },
            Expr::IsNotNull(expr) => SyntaxExprKind::IsNull {
                expr: Box::new(self.lower_expr(expr, &format!("{path}.expr"))),
                negated: true,
            },
            Expr::Tuple(exprs) => SyntaxExprKind::Tuple(
                exprs
                    .iter()
                    .enumerate()
                    .map(|(idx, expr)| self.lower_expr(expr, &format!("{path}.tuple[{idx}]")))
                    .collect(),
            ),
            Expr::Array(array) => SyntaxExprKind::Array(
                array
                    .elem
                    .iter()
                    .enumerate()
                    .map(|(idx, expr)| self.lower_expr(expr, &format!("{path}.array[{idx}]")))
                    .collect(),
            ),
            _ => SyntaxExprKind::Unsupported {
                description: format!("Unsupported expression `{expr}`."),
            },
        };
        SyntaxExpr { kind, node }
    }

    fn lower_function(&mut self, function: &Function, path: &str) -> SyntaxFunctionCall {
        if function.uses_odbc_syntax {
            self.emit_unsupported(
                function,
                &format!("{path}.odbc"),
                "ODBC function syntax is outside the verified portable subset.",
            );
        }
        if !function.within_group.is_empty() {
            self.emit_unsupported(
                function,
                &format!("{path}.within_group"),
                "WITHIN GROUP is outside the verified portable subset.",
            );
        }
        if function.null_treatment.is_some() {
            self.emit_unsupported(
                function,
                &format!("{path}.null_treatment"),
                "Function NULL treatment is outside the verified portable subset.",
            );
        }

        let distinct = match &function.args {
            FunctionArguments::List(list) => list.duplicate_treatment.is_some(),
            _ => false,
        };

        SyntaxFunctionCall {
            function: lower_function_ref(&function.name),
            args: lower_function_args(self, &function.name.to_string(), &function.args, path),
            distinct,
            filter: function
                .filter
                .as_ref()
                .map(|expr| Box::new(self.lower_expr(expr, &format!("{path}.filter")))),
            over: function
                .over
                .as_ref()
                .map(|over| self.lower_window(over, path)),
        }
    }

    fn lower_window(&mut self, window: &WindowType, path: &str) -> WindowSpec {
        match window {
            WindowType::WindowSpec(spec) => {
                let span = spec
                    .window_name
                    .as_ref()
                    .and_then(|ident| self.source_map.map_sql_span(ident.span))
                    .into_iter()
                    .chain(
                        spec.partition_by
                            .iter()
                            .filter_map(|expr| self.source_map.map_sql_span(expr.span())),
                    )
                    .chain(
                        spec.order_by
                            .iter()
                            .filter_map(|expr| self.source_map.map_sql_span(expr.span())),
                    )
                    .reduce(QuerySourceSpan::union);
                if spec.window_name.is_some() {
                    self.emit_unsupported_span(
                        span,
                        &format!("{path}.over.name"),
                        "Named window inheritance is outside the verified portable subset.",
                    );
                }
                if spec.window_frame.is_some() {
                    self.emit_unsupported_span(
                        span,
                        &format!("{path}.over.frame"),
                        "Window frames are outside the verified portable subset.",
                    );
                }
                WindowSpec {
                    partition_by: spec
                        .partition_by
                        .iter()
                        .enumerate()
                        .map(|(idx, expr)| {
                            self.lower_expr(expr, &format!("{path}.over.partition[{idx}]"))
                        })
                        .collect(),
                    order_by: spec
                        .order_by
                        .iter()
                        .enumerate()
                        .map(|(idx, expr)| {
                            self.lower_order_by_expr(expr, &format!("{path}.over.order[{idx}]"))
                        })
                        .collect(),
                    node: self.node_with_span(span, &format!("{path}.over")),
                }
            }
            WindowType::NamedWindow(name) => {
                let span = self.source_map.map_sql_span(name.span);
                self.emit_unsupported_span(
                    span,
                    &format!("{path}.over.named"),
                    "Named windows are outside the verified portable subset.",
                );
                WindowSpec {
                    partition_by: Vec::new(),
                    order_by: Vec::new(),
                    node: SyntaxNode::new(span, format!("{path}.over")),
                }
            }
        }
    }
}

fn lower_function_args(
    lowerer: &mut Lowerer<'_>,
    function_name: &str,
    args: &FunctionArguments,
    path: &str,
) -> Vec<SyntaxExpr> {
    match args {
        FunctionArguments::None => Vec::new(),
        FunctionArguments::Subquery(query) => vec![SyntaxExpr {
            kind: SyntaxExprKind::ScalarSubquery(Box::new(
                lowerer.lower_query(query.as_ref(), &format!("{path}.subquery")),
            )),
            node: lowerer.node(query.as_ref(), &format!("{path}.subquery")),
        }],
        FunctionArguments::List(list) => list
            .args
            .iter()
            .enumerate()
            .filter_map(|(idx, arg)| match arg {
                FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => {
                    Some(lowerer.lower_expr(expr, &format!("{path}.args[{idx}]")))
                }
                FunctionArg::Unnamed(FunctionArgExpr::QualifiedWildcard(_))
                | FunctionArg::Unnamed(FunctionArgExpr::Wildcard) => {
                    if function_name.eq_ignore_ascii_case("count") {
                        None
                    } else {
                        Some(SyntaxExpr {
                            kind: SyntaxExprKind::Unsupported {
                                description:
                                    "Wildcard function arguments are outside the verified portable subset."
                                        .into(),
                            },
                            node: lowerer.node(arg, &format!("{path}.args[{idx}]")),
                        })
                    }
                }
                _ => {
                    lowerer.emit_unsupported(
                        arg,
                        &format!("{path}.args[{idx}]"),
                        "Named function arguments are outside the verified portable subset.",
                    );
                    None
                }
            })
            .collect(),
    }
}
