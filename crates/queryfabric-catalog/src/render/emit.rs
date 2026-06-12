use std::collections::BTreeMap;

use queryfabric_ir::{
    BackendClause, BoundExpr, BoundExprKind, BoundFunctionCall, BoundOrderByExpr,
    BoundProjectionItem, BoundQuery, BoundQueryPlan, BoundRelation, BoundSelect, BoundSetExpr,
    NameRef, ParameterRef, Result,
};

use super::helpers::DataTypeExt;
use super::helpers::{
    backend_type_name, ordered_parameters, render_binary_operator, render_literal, unsupported,
};
use crate::model::{Catalog, SqlArtifact};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlBackend {
    ClickHouse,
    Postgres,
}

impl SqlBackend {
    pub fn name(self) -> &'static str {
        match self {
            Self::ClickHouse => "clickhouse",
            Self::Postgres => "postgres",
        }
    }
}

pub fn emit_sql_artifact(
    query: &BoundQuery,
    catalog: &dyn Catalog,
    backend: SqlBackend,
) -> Result<SqlArtifact> {
    let ordered_parameters = ordered_parameters(query);
    let renderer = SqlRenderer::new(backend, catalog, &ordered_parameters);
    let text = renderer.render_query(query.plan(), query.parsed().explain())?;

    Ok(SqlArtifact {
        dialect: backend.name().into(),
        text,
        parameters: ordered_parameters,
        result_schema: query.result_schema().clone(),
        metadata: BTreeMap::new(),
        provenance: query
            .provenance()
            .clone()
            .with_backend(backend.name())
            .with_artifact_identity(format!("sql:{}", backend.name())),
    })
}

struct SqlRenderer<'a> {
    backend: SqlBackend,
    catalog: &'a dyn Catalog,
    parameter_order: Vec<queryfabric_ir::ParameterSchema>,
    parameter_positions: BTreeMap<ParameterRef, usize>,
}

impl<'a> SqlRenderer<'a> {
    fn new(
        backend: SqlBackend,
        catalog: &'a dyn Catalog,
        parameters: &[queryfabric_ir::ParameterSchema],
    ) -> Self {
        let parameter_positions = parameters
            .iter()
            .enumerate()
            .map(|(idx, schema)| (schema.reference.clone(), idx + 1))
            .collect();
        Self {
            backend,
            catalog,
            parameter_order: parameters.to_vec(),
            parameter_positions,
        }
    }

    fn render_query(&self, query: &BoundQueryPlan, explain: bool) -> Result<String> {
        let mut sql = String::new();
        if explain {
            sql.push_str("EXPLAIN ");
        }
        sql.push_str(&self.render_query_inner(query)?);
        Ok(sql)
    }

    fn render_query_inner(&self, query: &BoundQueryPlan) -> Result<String> {
        let mut sql = String::new();
        if !query.ctes.is_empty() {
            sql.push_str("WITH ");
            sql.push_str(
                &query
                    .ctes
                    .iter()
                    .map(|cte| {
                        let mut text = cte.name.clone();
                        if !cte.columns.is_empty() {
                            text.push_str(" (");
                            text.push_str(&cte.columns.join(", "));
                            text.push(')');
                        }
                        text.push_str(" AS (");
                        text.push_str(&self.render_query_inner(&cte.query).unwrap_or_default());
                        text.push(')');
                        text
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            sql.push(' ');
        }
        sql.push_str(&self.render_set_expr(&query.body)?);
        if !query.order_by.is_empty() {
            sql.push_str(" ORDER BY ");
            sql.push_str(
                &query
                    .order_by
                    .iter()
                    .map(|expr| self.render_order_by(expr))
                    .collect::<Result<Vec<_>>>()?
                    .join(", "),
            );
        }
        if let Some(limit) = &query.limit {
            sql.push_str(" LIMIT ");
            sql.push_str(&self.render_expr(limit)?);
        }
        if let Some(offset) = &query.offset {
            sql.push_str(" OFFSET ");
            sql.push_str(&self.render_expr(offset)?);
        }
        for clause in &query.backend_clauses {
            match clause {
                BackendClause::ClickHouseSettings { text, .. }
                    if matches!(self.backend, SqlBackend::ClickHouse) =>
                {
                    sql.push_str(" SETTINGS ");
                    sql.push_str(text);
                }
                BackendClause::ClickHouseFormat { text, .. }
                    if matches!(self.backend, SqlBackend::ClickHouse) =>
                {
                    sql.push(' ');
                    sql.push_str(text);
                }
                _ => {}
            }
        }
        Ok(sql)
    }

    fn render_set_expr(&self, expr: &BoundSetExpr) -> Result<String> {
        match expr {
            BoundSetExpr::Select(select) => self.render_select(select),
            BoundSetExpr::UnionAll { left, right, .. } => Ok(format!(
                "{} UNION ALL {}",
                self.wrap_set_expr(left)?,
                self.wrap_set_expr(right)?
            )),
            BoundSetExpr::Unsupported { description, .. } => Err(unsupported(
                "render",
                format!("cannot render unsupported set expression: {description}"),
            )),
        }
    }

    fn wrap_set_expr(&self, expr: &BoundSetExpr) -> Result<String> {
        match expr {
            BoundSetExpr::Select(_) => self.render_set_expr(expr),
            _ => Ok(format!("({})", self.render_set_expr(expr)?)),
        }
    }

    fn render_select(&self, select: &BoundSelect) -> Result<String> {
        let mut sql = String::from("SELECT ");
        if select.distinct {
            sql.push_str("DISTINCT ");
        }
        sql.push_str(
            &select
                .projection
                .iter()
                .map(|item| self.render_projection(item))
                .collect::<Result<Vec<_>>>()?
                .join(", "),
        );
        if !select.from.is_empty() {
            sql.push_str(" FROM ");
            sql.push_str(
                &select
                    .from
                    .iter()
                    .map(|table| self.render_table_with_joins(table))
                    .collect::<Result<Vec<_>>>()?
                    .join(", "),
            );
        }
        if let Some(selection) = &select.selection {
            sql.push_str(" WHERE ");
            sql.push_str(&self.render_expr(selection)?);
        }
        if !select.group_by.is_empty() {
            sql.push_str(" GROUP BY ");
            sql.push_str(
                &select
                    .group_by
                    .iter()
                    .map(|expr| self.render_expr(expr))
                    .collect::<Result<Vec<_>>>()?
                    .join(", "),
            );
        }
        if let Some(having) = &select.having {
            sql.push_str(" HAVING ");
            sql.push_str(&self.render_expr(having)?);
        }
        Ok(sql)
    }

    fn render_projection(&self, item: &BoundProjectionItem) -> Result<String> {
        match item {
            BoundProjectionItem::Wildcard { qualifier, .. } => Ok(match qualifier {
                Some(qualifier) => format!("{qualifier}.*"),
                None => "*".into(),
            }),
            BoundProjectionItem::Expr(details) => {
                let rendered = self.render_expr(&details.expr)?;
                Ok(match &details.alias {
                    Some(alias) => format!("{rendered} AS {alias}"),
                    None => rendered,
                })
            }
            BoundProjectionItem::Unsupported { description, .. } => Err(unsupported(
                "render",
                format!("cannot render unsupported projection: {description}"),
            )),
        }
    }

    fn render_table_with_joins(
        &self,
        table: &queryfabric_ir::BoundTableWithJoins,
    ) -> Result<String> {
        let mut sql = self.render_relation(&table.relation)?;
        for join in &table.joins {
            sql.push(' ');
            sql.push_str(match join.kind {
                queryfabric_ir::JoinKind::Inner => "INNER JOIN ",
                queryfabric_ir::JoinKind::Left => "LEFT JOIN ",
                queryfabric_ir::JoinKind::Right => "RIGHT JOIN ",
                queryfabric_ir::JoinKind::Full => "FULL JOIN ",
                queryfabric_ir::JoinKind::Cross => "CROSS JOIN ",
            });
            sql.push_str(&self.render_relation(&join.relation)?);
            if let Some(on) = &join.on {
                sql.push_str(" ON ");
                sql.push_str(&self.render_expr(on)?);
            }
        }
        Ok(sql)
    }

    fn render_relation(&self, relation: &BoundRelation) -> Result<String> {
        match relation {
            BoundRelation::Table { binding, .. } => {
                let mut sql = binding
                    .relation_name
                    .as_ref()
                    .map(NameRef::display_name)
                    .unwrap_or_else(|| binding.binding_name.clone());
                if binding
                    .relation_name
                    .as_ref()
                    .is_some_and(|name| !name.name.eq_ignore_ascii_case(&binding.binding_name))
                {
                    sql.push_str(" AS ");
                    sql.push_str(&binding.binding_name);
                }
                Ok(sql)
            }
            BoundRelation::Derived { binding, query, .. } => {
                let mut sql = format!("({})", self.render_query_inner(query)?);
                sql.push_str(" AS ");
                sql.push_str(&binding.binding_name);
                Ok(sql)
            }
            BoundRelation::NestedJoin {
                binding,
                table_with_joins,
                ..
            } => {
                let mut sql = format!("({})", self.render_table_with_joins(table_with_joins)?);
                sql.push_str(" AS ");
                sql.push_str(&binding.binding_name);
                Ok(sql)
            }
            BoundRelation::Unsupported { description, .. } => Err(unsupported(
                "render",
                format!("cannot render unsupported relation: {description}"),
            )),
        }
    }

    fn render_order_by(&self, expr: &BoundOrderByExpr) -> Result<String> {
        let mut sql = self.render_expr(&expr.expr)?;
        if let Some(asc) = expr.asc {
            sql.push_str(if asc { " ASC" } else { " DESC" });
        }
        if let Some(nulls_first) = expr.nulls_first {
            sql.push_str(if nulls_first {
                " NULLS FIRST"
            } else {
                " NULLS LAST"
            });
        }
        Ok(sql)
    }

    fn render_expr(&self, expr: &BoundExpr) -> Result<String> {
        match &expr.kind {
            BoundExprKind::Column(column) => Ok(match &column.relation {
                Some(relation) => format!("{relation}.{}", column.name),
                None => column.name.clone(),
            }),
            BoundExprKind::Literal(value) => Ok(render_literal(value)),
            BoundExprKind::Parameter(reference) => self.render_parameter(reference),
            BoundExprKind::Unary { op, expr } => Ok(format!(
                "{}{}",
                match op {
                    queryfabric_ir::UnaryOperator::Plus => "+",
                    queryfabric_ir::UnaryOperator::Minus => "-",
                    queryfabric_ir::UnaryOperator::Not => "NOT ",
                },
                self.render_expr(expr)?
            )),
            BoundExprKind::Binary { op, left, right } => Ok(format!(
                "({} {} {})",
                self.render_expr(left)?,
                render_binary_operator(*op),
                self.render_expr(right)?
            )),
            BoundExprKind::Function(function) => self.render_function(function),
            BoundExprKind::Case {
                operand,
                when_then,
                else_result,
            } => {
                let mut sql = String::from("CASE");
                if let Some(operand) = operand {
                    sql.push(' ');
                    sql.push_str(&self.render_expr(operand)?);
                }
                for pair in when_then {
                    sql.push_str(" WHEN ");
                    sql.push_str(&self.render_expr(&pair.condition)?);
                    sql.push_str(" THEN ");
                    sql.push_str(&self.render_expr(&pair.result)?);
                }
                if let Some(else_result) = else_result {
                    sql.push_str(" ELSE ");
                    sql.push_str(&self.render_expr(else_result)?);
                }
                sql.push_str(" END");
                Ok(sql)
            }
            BoundExprKind::Cast { expr, data_type } => Ok(format!(
                "CAST({} AS {})",
                self.render_expr(expr)?,
                backend_type_name(self.backend, data_type)?
            )),
            BoundExprKind::Between {
                expr,
                low,
                high,
                negated,
            } => Ok(format!(
                "{} {}BETWEEN {} AND {}",
                self.render_expr(expr)?,
                if *negated { "NOT " } else { "" },
                self.render_expr(low)?,
                self.render_expr(high)?
            )),
            BoundExprKind::InList {
                expr,
                list,
                negated,
            } => {
                if let [only] = list.as_slice()
                    && let BoundExprKind::Parameter(reference) = &only.kind
                    && self.parameter_schema(reference)?.data_type.is_list()
                {
                    return match self.backend {
                        SqlBackend::Postgres => {
                            let op = if *negated { "<> ALL" } else { "= ANY" };
                            Ok(format!(
                                "{} {}({})",
                                self.render_expr(expr)?,
                                op,
                                self.render_parameter(reference)?
                            ))
                        }
                        SqlBackend::ClickHouse => Ok(format!(
                            "{} {}IN {}",
                            self.render_expr(expr)?,
                            if *negated { "NOT " } else { "" },
                            self.render_parameter(reference)?
                        )),
                    };
                }
                Ok(format!(
                    "{} {}IN ({})",
                    self.render_expr(expr)?,
                    if *negated { "NOT " } else { "" },
                    list.iter()
                        .map(|expr| self.render_expr(expr))
                        .collect::<Result<Vec<_>>>()?
                        .join(", ")
                ))
            }
            BoundExprKind::InSubquery {
                expr,
                subquery,
                negated,
            } => Ok(format!(
                "{} {}IN ({})",
                self.render_expr(expr)?,
                if *negated { "NOT " } else { "" },
                self.render_query_inner(subquery)?
            )),
            BoundExprKind::ScalarSubquery(subquery) => {
                Ok(format!("({})", self.render_query_inner(subquery)?))
            }
            BoundExprKind::Exists(subquery) => {
                Ok(format!("EXISTS ({})", self.render_query_inner(subquery)?))
            }
            BoundExprKind::Like {
                expr,
                pattern,
                negated,
                case_insensitive,
            } => Ok(format!(
                "{} {}{} {}",
                self.render_expr(expr)?,
                if *negated { "NOT " } else { "" },
                if *case_insensitive { "ILIKE" } else { "LIKE" },
                self.render_expr(pattern)?
            )),
            BoundExprKind::IsNull { expr, negated } => Ok(format!(
                "{} IS {}NULL",
                self.render_expr(expr)?,
                if *negated { "NOT " } else { "" }
            )),
            BoundExprKind::Tuple(items) => Ok(format!(
                "({})",
                items
                    .iter()
                    .map(|expr| self.render_expr(expr))
                    .collect::<Result<Vec<_>>>()?
                    .join(", ")
            )),
            BoundExprKind::Array(items) => match self.backend {
                SqlBackend::ClickHouse => Ok(format!(
                    "[{}]",
                    items
                        .iter()
                        .map(|expr| self.render_expr(expr))
                        .collect::<Result<Vec<_>>>()?
                        .join(", ")
                )),
                SqlBackend::Postgres => Ok(format!(
                    "ARRAY[{}]",
                    items
                        .iter()
                        .map(|expr| self.render_expr(expr))
                        .collect::<Result<Vec<_>>>()?
                        .join(", ")
                )),
            },
            BoundExprKind::Unsupported { description } => Err(unsupported(
                "render",
                format!("cannot render unsupported expression: {description}"),
            )),
        }
    }

    fn render_function(&self, function: &BoundFunctionCall) -> Result<String> {
        let signature = self
            .catalog
            .resolve_function(
                function.function.namespace.as_deref(),
                &function.function.name,
            )
            .ok_or_else(|| {
                unsupported(
                    "render",
                    format!("unknown function `{}`", function.function.display_name()),
                )
            })?;
        let mapped = signature
            .backend_mapping(self.backend.name())
            .ok_or_else(|| {
                unsupported(
                    "render",
                    format!(
                        "function `{}` has no backend mapping",
                        function.function.display_name()
                    ),
                )
            })?;
        let rendered_args = function
            .args
            .iter()
            .map(|expr| self.render_expr(expr))
            .collect::<Result<Vec<_>>>()?
            .join(", ");
        let args_sql =
            if function.function.name.eq_ignore_ascii_case("count") && function.args.is_empty() {
                "*".to_owned()
            } else {
                rendered_args
            };
        let mut sql = format!(
            "{}({}{})",
            mapped.display_name(),
            if function.distinct { "DISTINCT " } else { "" },
            args_sql
        );
        if let Some(filter) = &function.filter {
            sql.push_str(" FILTER (WHERE ");
            sql.push_str(&self.render_expr(filter)?);
            sql.push(')');
        }
        if let Some(over) = &function.over {
            let mut parts = Vec::new();
            if !over.partition_by.is_empty() {
                parts.push(format!(
                    "PARTITION BY {}",
                    over.partition_by
                        .iter()
                        .map(|expr| self.render_expr(expr))
                        .collect::<Result<Vec<_>>>()?
                        .join(", ")
                ));
            }
            if !over.order_by.is_empty() {
                parts.push(format!(
                    "ORDER BY {}",
                    over.order_by
                        .iter()
                        .map(|expr| self.render_order_by(expr))
                        .collect::<Result<Vec<_>>>()?
                        .join(", ")
                ));
            }
            sql.push_str(" OVER (");
            sql.push_str(&parts.join(" "));
            sql.push(')');
        }
        Ok(sql)
    }

    fn parameter_schema(
        &self,
        reference: &ParameterRef,
    ) -> Result<&queryfabric_ir::ParameterSchema> {
        self.parameter_order
            .iter()
            .find(|schema| &schema.reference == reference)
            .ok_or_else(|| unsupported("render", format!("unknown parameter `{reference}`")))
    }

    fn render_parameter(&self, reference: &ParameterRef) -> Result<String> {
        let schema = self.parameter_schema(reference)?;
        let position = self
            .parameter_positions
            .get(reference)
            .copied()
            .ok_or_else(|| unsupported("render", format!("unknown parameter `{reference}`")))?;
        match self.backend {
            SqlBackend::Postgres => Ok(format!("${position}")),
            SqlBackend::ClickHouse => Ok(format!(
                "{{p{position}:{}}}",
                backend_type_name(self.backend, &schema.data_type)?
            )),
        }
    }
}
