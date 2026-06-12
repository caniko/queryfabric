use std::collections::BTreeMap;
use std::convert::Infallible;

use queryfabric_ir::{
    BinaryOperator, SyntaxExpr, SyntaxExprKind, SyntaxFunctionCall, SyntaxJoin, SyntaxNode,
    SyntaxOrderByExpr, SyntaxProjectionItem, SyntaxQuery, SyntaxRelation, SyntaxSelect,
    SyntaxSetExpr, SyntaxTableWithJoins, SyntaxWhenThen, WindowSpec,
};

pub trait SyntaxTransformer {
    type Error;

    fn transform_query(&mut self, query: &SyntaxQuery) -> Result<SyntaxQuery, Self::Error> {
        transform_query_children(self, query)
    }

    fn transform_set_expr(&mut self, expr: &SyntaxSetExpr) -> Result<SyntaxSetExpr, Self::Error> {
        transform_set_expr_children(self, expr)
    }

    fn transform_select(&mut self, select: &SyntaxSelect) -> Result<SyntaxSelect, Self::Error> {
        transform_select_children(self, select)
    }

    fn transform_projection_item(
        &mut self,
        item: &SyntaxProjectionItem,
    ) -> Result<SyntaxProjectionItem, Self::Error> {
        transform_projection_item_children(self, item)
    }

    fn transform_table_with_joins(
        &mut self,
        table: &SyntaxTableWithJoins,
    ) -> Result<SyntaxTableWithJoins, Self::Error> {
        transform_table_with_joins_children(self, table)
    }

    fn transform_join(&mut self, join: &SyntaxJoin) -> Result<SyntaxJoin, Self::Error> {
        transform_join_children(self, join)
    }

    fn transform_relation(
        &mut self,
        relation: &SyntaxRelation,
    ) -> Result<SyntaxRelation, Self::Error> {
        transform_relation_children(self, relation)
    }

    fn transform_order_by_expr(
        &mut self,
        expr: &SyntaxOrderByExpr,
    ) -> Result<SyntaxOrderByExpr, Self::Error> {
        transform_order_by_expr_children(self, expr)
    }

    fn transform_expr(&mut self, expr: &SyntaxExpr) -> Result<SyntaxExpr, Self::Error> {
        transform_expr_children(self, expr)
    }

    fn transform_when_then(
        &mut self,
        branch: &SyntaxWhenThen,
    ) -> Result<SyntaxWhenThen, Self::Error> {
        transform_when_then_children(self, branch)
    }
}

pub fn transform_query_children<T>(
    transformer: &mut T,
    query: &SyntaxQuery,
) -> Result<SyntaxQuery, T::Error>
where
    T: SyntaxTransformer + ?Sized,
{
    Ok(SyntaxQuery {
        node: query.node.clone(),
        ctes: query
            .ctes
            .iter()
            .map(|cte| {
                Ok(queryfabric_ir::SyntaxCte {
                    name: cte.name.clone(),
                    columns: cte.columns.clone(),
                    query: Box::new(transformer.transform_query(&cte.query)?),
                    node: cte.node.clone(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        with_recursive: query.with_recursive,
        body: transformer.transform_set_expr(&query.body)?,
        order_by: query
            .order_by
            .iter()
            .map(|expr| transformer.transform_order_by_expr(expr))
            .collect::<Result<Vec<_>, _>>()?,
        limit: query
            .limit
            .as_ref()
            .map(|expr| transformer.transform_expr(expr))
            .transpose()?,
        offset: query
            .offset
            .as_ref()
            .map(|expr| transformer.transform_expr(expr))
            .transpose()?,
        backend_clauses: query.backend_clauses.clone(),
    })
}

pub fn transform_set_expr_children<T>(
    transformer: &mut T,
    expr: &SyntaxSetExpr,
) -> Result<SyntaxSetExpr, T::Error>
where
    T: SyntaxTransformer + ?Sized,
{
    match expr {
        SyntaxSetExpr::Select(select) => Ok(SyntaxSetExpr::Select(Box::new(
            transformer.transform_select(select)?,
        ))),
        SyntaxSetExpr::UnionAll { left, right, node } => Ok(SyntaxSetExpr::UnionAll {
            left: Box::new(transformer.transform_set_expr(left)?),
            right: Box::new(transformer.transform_set_expr(right)?),
            node: node.clone(),
        }),
        SyntaxSetExpr::Unsupported { description, node } => Ok(SyntaxSetExpr::Unsupported {
            description: description.clone(),
            node: node.clone(),
        }),
    }
}

pub fn transform_select_children<T>(
    transformer: &mut T,
    select: &SyntaxSelect,
) -> Result<SyntaxSelect, T::Error>
where
    T: SyntaxTransformer + ?Sized,
{
    Ok(SyntaxSelect {
        distinct: select.distinct,
        projection: select
            .projection
            .iter()
            .map(|item| transformer.transform_projection_item(item))
            .collect::<Result<Vec<_>, _>>()?,
        from: select
            .from
            .iter()
            .map(|table| transformer.transform_table_with_joins(table))
            .collect::<Result<Vec<_>, _>>()?,
        selection: select
            .selection
            .as_ref()
            .map(|expr| transformer.transform_expr(expr))
            .transpose()?,
        group_by: select
            .group_by
            .iter()
            .map(|expr| transformer.transform_expr(expr))
            .collect::<Result<Vec<_>, _>>()?,
        having: select
            .having
            .as_ref()
            .map(|expr| transformer.transform_expr(expr))
            .transpose()?,
        node: select.node.clone(),
    })
}

pub fn transform_projection_item_children<T>(
    transformer: &mut T,
    item: &SyntaxProjectionItem,
) -> Result<SyntaxProjectionItem, T::Error>
where
    T: SyntaxTransformer + ?Sized,
{
    match item {
        SyntaxProjectionItem::Wildcard { qualifier, node } => Ok(SyntaxProjectionItem::Wildcard {
            qualifier: qualifier.clone(),
            node: node.clone(),
        }),
        SyntaxProjectionItem::Expr(details) => Ok(SyntaxProjectionItem::expr(
            transformer.transform_expr(&details.expr)?,
            details.alias.clone(),
            details.node.clone(),
        )),
        SyntaxProjectionItem::Unsupported { description, node } => {
            Ok(SyntaxProjectionItem::Unsupported {
                description: description.clone(),
                node: node.clone(),
            })
        }
    }
}

pub fn transform_table_with_joins_children<T>(
    transformer: &mut T,
    table: &SyntaxTableWithJoins,
) -> Result<SyntaxTableWithJoins, T::Error>
where
    T: SyntaxTransformer + ?Sized,
{
    Ok(SyntaxTableWithJoins {
        relation: transformer.transform_relation(&table.relation)?,
        joins: table
            .joins
            .iter()
            .map(|join| transformer.transform_join(join))
            .collect::<Result<Vec<_>, _>>()?,
        node: table.node.clone(),
    })
}

pub fn transform_join_children<T>(
    transformer: &mut T,
    join: &SyntaxJoin,
) -> Result<SyntaxJoin, T::Error>
where
    T: SyntaxTransformer + ?Sized,
{
    Ok(SyntaxJoin {
        kind: join.kind,
        relation: transformer.transform_relation(&join.relation)?,
        on: join
            .on
            .as_ref()
            .map(|expr| transformer.transform_expr(expr))
            .transpose()?,
        node: join.node.clone(),
    })
}

pub fn transform_relation_children<T>(
    transformer: &mut T,
    relation: &SyntaxRelation,
) -> Result<SyntaxRelation, T::Error>
where
    T: SyntaxTransformer + ?Sized,
{
    match relation {
        SyntaxRelation::Table { name, alias, node } => Ok(SyntaxRelation::Table {
            name: name.clone(),
            alias: alias.clone(),
            node: node.clone(),
        }),
        SyntaxRelation::Derived { query, alias, node } => Ok(SyntaxRelation::Derived {
            query: Box::new(transformer.transform_query(query)?),
            alias: alias.clone(),
            node: node.clone(),
        }),
        SyntaxRelation::NestedJoin {
            table_with_joins,
            alias,
            node,
        } => Ok(SyntaxRelation::NestedJoin {
            table_with_joins: Box::new(transformer.transform_table_with_joins(table_with_joins)?),
            alias: alias.clone(),
            node: node.clone(),
        }),
        SyntaxRelation::Unsupported { description, node } => Ok(SyntaxRelation::Unsupported {
            description: description.clone(),
            node: node.clone(),
        }),
    }
}

pub fn transform_order_by_expr_children<T>(
    transformer: &mut T,
    expr: &SyntaxOrderByExpr,
) -> Result<SyntaxOrderByExpr, T::Error>
where
    T: SyntaxTransformer + ?Sized,
{
    Ok(SyntaxOrderByExpr {
        expr: transformer.transform_expr(&expr.expr)?,
        asc: expr.asc,
        nulls_first: expr.nulls_first,
        node: expr.node.clone(),
    })
}

pub fn transform_expr_children<T>(
    transformer: &mut T,
    expr: &SyntaxExpr,
) -> Result<SyntaxExpr, T::Error>
where
    T: SyntaxTransformer + ?Sized,
{
    let kind = match &expr.kind {
        SyntaxExprKind::Column { relation, name } => SyntaxExprKind::Column {
            relation: relation.clone(),
            name: name.clone(),
        },
        SyntaxExprKind::Literal(value) => SyntaxExprKind::Literal(value.clone()),
        SyntaxExprKind::Parameter(reference) => SyntaxExprKind::Parameter(reference.clone()),
        SyntaxExprKind::Unary { op, expr: inner } => SyntaxExprKind::Unary {
            op: *op,
            expr: Box::new(transformer.transform_expr(inner)?),
        },
        SyntaxExprKind::Binary { op, left, right } => SyntaxExprKind::Binary {
            op: *op,
            left: Box::new(transformer.transform_expr(left)?),
            right: Box::new(transformer.transform_expr(right)?),
        },
        SyntaxExprKind::Function(function) => SyntaxExprKind::Function(SyntaxFunctionCall {
            function: function.function.clone(),
            args: function
                .args
                .iter()
                .map(|arg| transformer.transform_expr(arg))
                .collect::<Result<Vec<_>, _>>()?,
            distinct: function.distinct,
            filter: function
                .filter
                .as_deref()
                .map(|filter| transformer.transform_expr(filter))
                .transpose()?
                .map(Box::new),
            over: function
                .over
                .as_ref()
                .map(|window| {
                    Ok(WindowSpec {
                        partition_by: window
                            .partition_by
                            .iter()
                            .map(|expr| transformer.transform_expr(expr))
                            .collect::<Result<Vec<_>, _>>()?,
                        order_by: window
                            .order_by
                            .iter()
                            .map(|expr| transformer.transform_order_by_expr(expr))
                            .collect::<Result<Vec<_>, _>>()?,
                        node: window.node.clone(),
                    })
                })
                .transpose()?,
        }),
        SyntaxExprKind::Case {
            operand,
            when_then,
            else_result,
        } => SyntaxExprKind::Case {
            operand: operand
                .as_deref()
                .map(|expr| transformer.transform_expr(expr))
                .transpose()?
                .map(Box::new),
            when_then: when_then
                .iter()
                .map(|branch| transformer.transform_when_then(branch))
                .collect::<Result<Vec<_>, _>>()?,
            else_result: else_result
                .as_deref()
                .map(|expr| transformer.transform_expr(expr))
                .transpose()?
                .map(Box::new),
        },
        SyntaxExprKind::Cast {
            expr: inner,
            data_type,
        } => SyntaxExprKind::Cast {
            expr: Box::new(transformer.transform_expr(inner)?),
            data_type: data_type.clone(),
        },
        SyntaxExprKind::Between {
            expr: inner,
            low,
            high,
            negated,
        } => SyntaxExprKind::Between {
            expr: Box::new(transformer.transform_expr(inner)?),
            low: Box::new(transformer.transform_expr(low)?),
            high: Box::new(transformer.transform_expr(high)?),
            negated: *negated,
        },
        SyntaxExprKind::InList {
            expr: inner,
            list,
            negated,
        } => SyntaxExprKind::InList {
            expr: Box::new(transformer.transform_expr(inner)?),
            list: list
                .iter()
                .map(|item| transformer.transform_expr(item))
                .collect::<Result<Vec<_>, _>>()?,
            negated: *negated,
        },
        SyntaxExprKind::InSubquery {
            expr: inner,
            subquery,
            negated,
        } => SyntaxExprKind::InSubquery {
            expr: Box::new(transformer.transform_expr(inner)?),
            subquery: Box::new(transformer.transform_query(subquery)?),
            negated: *negated,
        },
        SyntaxExprKind::ScalarSubquery(subquery) => {
            SyntaxExprKind::ScalarSubquery(Box::new(transformer.transform_query(subquery)?))
        }
        SyntaxExprKind::Exists(subquery) => {
            SyntaxExprKind::Exists(Box::new(transformer.transform_query(subquery)?))
        }
        SyntaxExprKind::Like {
            expr: inner,
            pattern,
            negated,
            case_insensitive,
        } => SyntaxExprKind::Like {
            expr: Box::new(transformer.transform_expr(inner)?),
            pattern: Box::new(transformer.transform_expr(pattern)?),
            negated: *negated,
            case_insensitive: *case_insensitive,
        },
        SyntaxExprKind::IsNull {
            expr: inner,
            negated,
        } => SyntaxExprKind::IsNull {
            expr: Box::new(transformer.transform_expr(inner)?),
            negated: *negated,
        },
        SyntaxExprKind::Tuple(items) => SyntaxExprKind::Tuple(
            items
                .iter()
                .map(|item| transformer.transform_expr(item))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        SyntaxExprKind::Array(items) => SyntaxExprKind::Array(
            items
                .iter()
                .map(|item| transformer.transform_expr(item))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        SyntaxExprKind::Unsupported { description } => SyntaxExprKind::Unsupported {
            description: description.clone(),
        },
    };

    Ok(SyntaxExpr {
        kind,
        node: expr.node.clone(),
    })
}

pub fn transform_when_then_children<T>(
    transformer: &mut T,
    branch: &SyntaxWhenThen,
) -> Result<SyntaxWhenThen, T::Error>
where
    T: SyntaxTransformer + ?Sized,
{
    Ok(SyntaxWhenThen {
        condition: transformer.transform_expr(&branch.condition)?,
        result: transformer.transform_expr(&branch.result)?,
    })
}

pub fn apply_selection_overrides(
    query: &SyntaxQuery,
    selection_overrides: &BTreeMap<String, Option<SyntaxExpr>>,
) -> SyntaxQuery {
    let mut transformer = SelectionOverrideTransformer {
        selection_overrides,
    };
    match transform_query_children(&mut transformer, query) {
        Ok(query) => query,
        Err(never) => match never {},
    }
}

pub fn flatten_boolean_and(expr: &SyntaxExpr) -> Vec<&SyntaxExpr> {
    match &expr.kind {
        SyntaxExprKind::Binary {
            op: BinaryOperator::And,
            left,
            right,
        } => {
            let mut items = flatten_boolean_and(left);
            items.extend(flatten_boolean_and(right));
            items
        }
        _ => vec![expr],
    }
}

pub fn rebuild_boolean_and(mut items: Vec<SyntaxExpr>, node: &SyntaxNode) -> Option<SyntaxExpr> {
    if items.is_empty() {
        return None;
    }

    let mut current = items.remove(0);
    for (index, item) in items.into_iter().enumerate() {
        current = SyntaxExpr {
            kind: SyntaxExprKind::Binary {
                op: BinaryOperator::And,
                left: Box::new(current),
                right: Box::new(item),
            },
            node: SyntaxNode::new(node.span, format!("{}.and[{index}]", node.node_id)),
        };
    }

    Some(current)
}

struct SelectionOverrideTransformer<'a> {
    selection_overrides: &'a BTreeMap<String, Option<SyntaxExpr>>,
}

impl SyntaxTransformer for SelectionOverrideTransformer<'_> {
    type Error = Infallible;

    fn transform_select(&mut self, select: &SyntaxSelect) -> Result<SyntaxSelect, Self::Error> {
        Ok(SyntaxSelect {
            distinct: select.distinct,
            projection: select
                .projection
                .iter()
                .map(|item| self.transform_projection_item(item))
                .collect::<Result<Vec<_>, _>>()?,
            from: select
                .from
                .iter()
                .map(|table| self.transform_table_with_joins(table))
                .collect::<Result<Vec<_>, _>>()?,
            selection: self
                .selection_overrides
                .get(&select.node.node_id)
                .cloned()
                .unwrap_or_else(|| select.selection.clone()),
            group_by: select
                .group_by
                .iter()
                .map(|expr| self.transform_expr(expr))
                .collect::<Result<Vec<_>, _>>()?,
            having: select
                .having
                .as_ref()
                .map(|expr| self.transform_expr(expr))
                .transpose()?,
            node: select.node.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::convert::Infallible;

    use queryfabric_dialect_sql::GenericSqlDialect;
    use queryfabric_ir::{
        BinaryOperator, Dialect, LiteralValue, SyntaxExpr, SyntaxExprKind, SyntaxNode,
        SyntaxRelation, SyntaxSetExpr,
    };

    use super::{
        SyntaxTransformer, apply_selection_overrides, flatten_boolean_and, rebuild_boolean_and,
        transform_query_children,
    };

    struct LimitZeroingTransformer;

    impl SyntaxTransformer for LimitZeroingTransformer {
        type Error = Infallible;

        fn transform_expr(&mut self, expr: &SyntaxExpr) -> Result<SyntaxExpr, Self::Error> {
            if matches!(&expr.kind, SyntaxExprKind::Literal(LiteralValue::Int64(5))) {
                return Ok(SyntaxExpr {
                    kind: SyntaxExprKind::Literal(LiteralValue::Int64(0)),
                    node: expr.node.clone(),
                });
            }
            super::transform_expr_children(self, expr)
        }
    }

    #[test]
    fn rewrites_nested_limit_and_binary_children() {
        let parsed = GenericSqlDialect
            .parse(
                "SELECT record_id FROM (SELECT record_id FROM records WHERE score > 5 LIMIT 5) n LIMIT 5",
            )
            .expect("parse");
        let mut transformer = LimitZeroingTransformer;
        let rewritten =
            transform_query_children(&mut transformer, parsed.syntax()).expect("rewrite");

        assert!(matches!(
            rewritten.limit.as_ref().map(|expr| &expr.kind),
            Some(SyntaxExprKind::Literal(LiteralValue::Int64(0)))
        ));

        let inner_select = match &rewritten.body {
            SyntaxSetExpr::Select(select) => match &select.from[0].relation {
                SyntaxRelation::Derived { query, .. } => match &query.body {
                    SyntaxSetExpr::Select(inner) => inner,
                    other => panic!("expected inner select, got {other:?}"),
                },
                other => panic!("expected derived relation, got {other:?}"),
            },
            other => panic!("expected outer select, got {other:?}"),
        };

        assert!(matches!(
            inner_select.selection.as_ref().map(|expr| &expr.kind),
            Some(SyntaxExprKind::Binary {
                op: BinaryOperator::Gt,
                right,
                ..
            }) if matches!(right.kind, SyntaxExprKind::Literal(LiteralValue::Int64(0)))
        ));
    }

    #[test]
    fn selection_overrides_replace_matching_select_selection_only() {
        let parsed = GenericSqlDialect
            .parse("SELECT record_id FROM records WHERE score > 5")
            .expect("parse");
        let select = match &parsed.syntax().body {
            SyntaxSetExpr::Select(select) => select,
            other => panic!("expected select body, got {other:?}"),
        };
        let replacement = SyntaxExpr {
            kind: SyntaxExprKind::Literal(LiteralValue::Boolean(true)),
            node: SyntaxNode::new(None, "replacement"),
        };

        let rewritten = apply_selection_overrides(
            parsed.syntax(),
            &BTreeMap::from([(select.node.node_id.clone(), Some(replacement.clone()))]),
        );

        let rewritten_select = match rewritten.body {
            SyntaxSetExpr::Select(select) => select,
            other => panic!("expected select body, got {other:?}"),
        };

        assert_eq!(rewritten_select.selection, Some(replacement));
    }

    #[test]
    fn flatten_and_rebuild_boolean_and_preserve_conjuncts() {
        let parsed = GenericSqlDialect
            .parse(
                "SELECT record_id FROM records WHERE species = 'mouse' AND score > 5 AND record_id IS NOT NULL",
            )
            .expect("parse");
        let select = match &parsed.syntax().body {
            SyntaxSetExpr::Select(select) => select,
            other => panic!("expected select body, got {other:?}"),
        };
        let selection = select.selection.as_ref().expect("selection");
        let conjuncts = flatten_boolean_and(selection);

        assert_eq!(conjuncts.len(), 3);

        let rebuilt =
            rebuild_boolean_and(conjuncts.into_iter().cloned().collect(), &selection.node)
                .expect("rebuilt");

        assert_eq!(flatten_boolean_and(&rebuilt).len(), 3);
    }
}
