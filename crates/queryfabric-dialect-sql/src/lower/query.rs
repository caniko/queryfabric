use super::Lowerer;
use crate::helpers::lower_name_ref;
use queryfabric_ir::{
    BackendClause, JoinKind, QuerySourceSpan, SyntaxCte, SyntaxExpr, SyntaxExprKind, SyntaxJoin,
    SyntaxNode, SyntaxOrderByExpr, SyntaxProjectionItem, SyntaxQuery, SyntaxRelation, SyntaxSelect,
    SyntaxSetExpr, SyntaxTableWithJoins,
};
use sqlparser::ast::Spanned;
use sqlparser::ast::{
    GroupByExpr, JoinConstraint, JoinOperator, LimitClause, OrderBy, OrderByExpr, OrderByKind,
    Query, Select, SelectItem, SetExpr, SetOperator, SetQuantifier, TableFactor, TableWithJoins,
};

impl Lowerer<'_> {
    pub(crate) fn lower_query(&mut self, query: &Query, path: &str) -> SyntaxQuery {
        if query.fetch.is_some() {
            self.emit_unsupported(
                query,
                &format!("{path}.fetch"),
                "FETCH is outside the verified portable subset.",
            );
        }
        if !query.locks.is_empty() {
            self.emit_unsupported(
                query,
                &format!("{path}.locks"),
                "Query locks are outside the verified portable subset.",
            );
        }
        if query.for_clause.is_some() {
            self.emit_unsupported(
                query,
                &format!("{path}.for"),
                "FOR clauses are outside the verified portable subset.",
            );
        }
        if !query.pipe_operators.is_empty() {
            self.emit_unsupported(
                query,
                &format!("{path}.pipes"),
                "Pipe operators are outside the verified portable subset.",
            );
        }

        let mut limit = None;
        let mut offset = None;
        if let Some(limit_clause) = &query.limit_clause {
            match limit_clause {
                LimitClause::LimitOffset {
                    limit: limit_expr,
                    offset: offset_expr,
                    limit_by,
                } => {
                    if !limit_by.is_empty() {
                        self.emit_unsupported(
                            limit_clause,
                            &format!("{path}.limit_by"),
                            "LIMIT BY is outside the verified portable subset.",
                        );
                    }
                    limit = limit_expr
                        .as_ref()
                        .map(|expr| self.lower_expr(expr, &format!("{path}.limit")));
                    offset = offset_expr
                        .as_ref()
                        .map(|expr| self.lower_expr(&expr.value, &format!("{path}.offset")));
                }
                LimitClause::OffsetCommaLimit {
                    offset: off,
                    limit: lim,
                } => {
                    offset = Some(self.lower_expr(off, &format!("{path}.offset")));
                    limit = Some(self.lower_expr(lim, &format!("{path}.limit")));
                }
            }
        }

        SyntaxQuery {
            node: self.node(query, path),
            ctes: query
                .with
                .as_ref()
                .map(|with| {
                    with.cte_tables
                        .iter()
                        .enumerate()
                        .map(|(idx, cte)| SyntaxCte {
                            name: cte.alias.name.value.clone(),
                            columns: cte
                                .alias
                                .columns
                                .iter()
                                .map(|ident| ident.name.value.clone())
                                .collect(),
                            query: Box::new(
                                self.lower_query(&cte.query, &format!("{path}.with[{idx}]")),
                            ),
                            node: self.node(cte, &format!("{path}.with[{idx}]")),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            with_recursive: query.with.as_ref().is_some_and(|with| with.recursive),
            body: self.lower_set_expr(query.body.as_ref(), &format!("{path}.body")),
            order_by: query
                .order_by
                .as_ref()
                .map(|order_by| self.lower_order_by(order_by, &format!("{path}.order_by")))
                .unwrap_or_default(),
            limit,
            offset,
            backend_clauses: self.lower_backend_clauses(query, path),
        }
    }

    pub(crate) fn lower_backend_clauses(
        &mut self,
        query: &Query,
        path: &str,
    ) -> Vec<BackendClause> {
        let mut clauses = Vec::new();
        if let Some(settings) = &query.settings {
            clauses.push(BackendClause::ClickHouseSettings {
                text: settings
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                node: SyntaxNode::new(
                    settings
                        .iter()
                        .filter_map(|setting| {
                            self.source_map
                                .map_sql_span(setting.key.span)
                                .or_else(|| self.source_map.map_sql_span(setting.value.span()))
                        })
                        .reduce(QuerySourceSpan::union),
                    format!("{path}.settings"),
                ),
            });
        }
        if let Some(format_clause) = &query.format_clause {
            clauses.push(BackendClause::ClickHouseFormat {
                text: format_clause.to_string(),
                node: self.node_with_span(
                    self.source_map.map_sql_span(query.span()),
                    &format!("{path}.format"),
                ),
            });
        }
        clauses
    }

    pub(crate) fn lower_set_expr(&mut self, expr: &SetExpr, path: &str) -> SyntaxSetExpr {
        match expr {
            SetExpr::Select(select) => SyntaxSetExpr::select(self.lower_select(select, path)),
            SetExpr::Query(query) => self.lower_set_expr(query.body.as_ref(), path),
            SetExpr::SetOperation {
                op,
                set_quantifier,
                left,
                right,
            } => {
                if matches!(op, SetOperator::Union) && matches!(set_quantifier, SetQuantifier::All)
                {
                    SyntaxSetExpr::UnionAll {
                        left: Box::new(self.lower_set_expr(left, &format!("{path}.left"))),
                        right: Box::new(self.lower_set_expr(right, &format!("{path}.right"))),
                        node: self.node(expr, path),
                    }
                } else {
                    SyntaxSetExpr::Unsupported {
                        description: format!(
                            "Set operation `{op} {set_quantifier}` is outside the verified portable subset."
                        ),
                        node: self.node(expr, path),
                    }
                }
            }
            _ => SyntaxSetExpr::Unsupported {
                description: "Unsupported query body.".into(),
                node: self.node(expr, path),
            },
        }
    }

    pub(crate) fn lower_select(&mut self, select: &Select, path: &str) -> SyntaxSelect {
        SyntaxSelect {
            distinct: select.distinct.is_some(),
            projection: select
                .projection
                .iter()
                .enumerate()
                .map(|(idx, item)| {
                    self.lower_projection_item(item, &format!("{path}.projection[{idx}]"))
                })
                .collect(),
            from: select
                .from
                .iter()
                .enumerate()
                .map(|(idx, item)| {
                    self.lower_table_with_joins(item, &format!("{path}.from[{idx}]"))
                })
                .collect(),
            selection: select
                .selection
                .as_ref()
                .map(|expr| self.lower_expr(expr, &format!("{path}.where"))),
            group_by: match &select.group_by {
                GroupByExpr::Expressions(exprs, _) => exprs
                    .iter()
                    .enumerate()
                    .map(|(idx, expr)| self.lower_expr(expr, &format!("{path}.group_by[{idx}]")))
                    .collect(),
                GroupByExpr::All(_) => vec![SyntaxExpr {
                    kind: SyntaxExprKind::Unsupported {
                        description: "GROUP BY ALL is outside the verified portable subset.".into(),
                    },
                    node: self.node(select, &format!("{path}.group_by_all")),
                }],
            },
            having: select
                .having
                .as_ref()
                .map(|expr| self.lower_expr(expr, &format!("{path}.having"))),
            node: self.node(select, path),
        }
    }

    pub(crate) fn lower_projection_item(
        &mut self,
        item: &SelectItem,
        path: &str,
    ) -> SyntaxProjectionItem {
        match item {
            SelectItem::Wildcard(_) => SyntaxProjectionItem::Wildcard {
                qualifier: None,
                node: self.node(item, path),
            },
            SelectItem::QualifiedWildcard(name, _) => {
                let qualifier = match name {
                    sqlparser::ast::SelectItemQualifiedWildcardKind::ObjectName(name) => {
                        name.0.last().map(|part| match part {
                            sqlparser::ast::ObjectNamePart::Identifier(identifier) => {
                                identifier.value.clone()
                            }
                            sqlparser::ast::ObjectNamePart::Function(function) => {
                                function.to_string()
                            }
                        })
                    }
                    sqlparser::ast::SelectItemQualifiedWildcardKind::Expr(_) => None,
                };
                SyntaxProjectionItem::Wildcard {
                    qualifier,
                    node: self.node(item, path),
                }
            }
            SelectItem::UnnamedExpr(expr) => SyntaxProjectionItem::expr(
                self.lower_expr(expr, &format!("{path}.expr")),
                None,
                self.node(item, path),
            ),
            SelectItem::ExprWithAlias { expr, alias } => SyntaxProjectionItem::expr(
                self.lower_expr(expr, &format!("{path}.expr")),
                Some(alias.value.clone()),
                self.node(item, path),
            ),
        }
    }

    pub(crate) fn lower_table_with_joins(
        &mut self,
        table: &TableWithJoins,
        path: &str,
    ) -> SyntaxTableWithJoins {
        SyntaxTableWithJoins {
            relation: self.lower_relation(&table.relation, &format!("{path}.relation")),
            joins: table
                .joins
                .iter()
                .enumerate()
                .map(|(idx, join)| {
                    let (kind, on) = match &join.join_operator {
                        JoinOperator::Join(JoinConstraint::On(on))
                        | JoinOperator::Inner(JoinConstraint::On(on)) => (
                            JoinKind::Inner,
                            Some(self.lower_expr(on, &format!("{path}.joins[{idx}].on"))),
                        ),
                        JoinOperator::Left(JoinConstraint::On(on))
                        | JoinOperator::LeftOuter(JoinConstraint::On(on)) => (
                            JoinKind::Left,
                            Some(self.lower_expr(on, &format!("{path}.joins[{idx}].on"))),
                        ),
                        JoinOperator::Right(JoinConstraint::On(on))
                        | JoinOperator::RightOuter(JoinConstraint::On(on)) => (
                            JoinKind::Right,
                            Some(self.lower_expr(on, &format!("{path}.joins[{idx}].on"))),
                        ),
                        JoinOperator::FullOuter(JoinConstraint::On(on)) => (
                            JoinKind::Full,
                            Some(self.lower_expr(on, &format!("{path}.joins[{idx}].on"))),
                        ),
                        JoinOperator::CrossJoin(_) => (JoinKind::Cross, None),
                        JoinOperator::Join(JoinConstraint::None)
                        | JoinOperator::Inner(JoinConstraint::None) => (JoinKind::Inner, None),
                        _ => {
                            self.emit_unsupported(
                                &join.join_operator,
                                &format!("{path}.joins[{idx}]"),
                                "Join type or constraint is outside the verified portable subset.",
                            );
                            (JoinKind::Inner, None)
                        }
                    };
                    SyntaxJoin {
                        kind,
                        relation: self.lower_relation(
                            &join.relation,
                            &format!("{path}.joins[{idx}].relation"),
                        ),
                        on,
                        node: self.node(&join.join_operator, &format!("{path}.joins[{idx}]")),
                    }
                })
                .collect(),
            node: self.node(table, path),
        }
    }

    pub(crate) fn lower_relation(&mut self, relation: &TableFactor, path: &str) -> SyntaxRelation {
        match relation {
            TableFactor::Table { name, alias, .. } => SyntaxRelation::Table {
                name: lower_name_ref(name),
                alias: alias.as_ref().map(|alias| alias.name.value.clone()),
                node: self.node(relation, path),
            },
            TableFactor::Derived {
                subquery, alias, ..
            } => SyntaxRelation::Derived {
                query: Box::new(self.lower_query(subquery, &format!("{path}.subquery"))),
                alias: alias.as_ref().map(|alias| alias.name.value.clone()),
                node: self.node(relation, path),
            },
            TableFactor::NestedJoin {
                table_with_joins,
                alias,
            } => SyntaxRelation::NestedJoin {
                table_with_joins: Box::new(
                    self.lower_table_with_joins(table_with_joins, &format!("{path}.nested")),
                ),
                alias: alias.as_ref().map(|alias| alias.name.value.clone()),
                node: self.node(relation, path),
            },
            _ => SyntaxRelation::Unsupported {
                description: "Unsupported FROM relation.".into(),
                node: self.node(relation, path),
            },
        }
    }

    pub(crate) fn lower_order_by(
        &mut self,
        order_by: &OrderBy,
        path: &str,
    ) -> Vec<SyntaxOrderByExpr> {
        match &order_by.kind {
            OrderByKind::Expressions(exprs) => exprs
                .iter()
                .enumerate()
                .map(|(idx, expr)| self.lower_order_by_expr(expr, &format!("{path}[{idx}]")))
                .collect(),
            _ => {
                self.emit_unsupported(
                    order_by,
                    path,
                    "ORDER BY variant is outside the verified portable subset.",
                );
                Vec::new()
            }
        }
    }

    pub(crate) fn lower_order_by_expr(
        &mut self,
        expr: &OrderByExpr,
        path: &str,
    ) -> SyntaxOrderByExpr {
        SyntaxOrderByExpr {
            expr: self.lower_expr(&expr.expr, &format!("{path}.expr")),
            asc: expr.options.asc,
            nulls_first: expr.options.nulls_first,
            node: self.node(expr, path),
        }
    }
}
