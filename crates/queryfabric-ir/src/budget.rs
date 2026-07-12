use serde::{Deserialize, Serialize};

use crate::bound::ParsedQuery;
use crate::error::Result;
use crate::syntax::{
    SyntaxCte, SyntaxExpr, SyntaxExprKind, SyntaxQuery, SyntaxRelation, SyntaxSetExpr,
    SyntaxTableWithJoins,
};

/// The independently configurable dimensions of a compilation budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum QueryBudgetDimension {
    InputBytes,
    Parameters,
    SyntaxNodes,
    NestingDepth,
    Joins,
    Ctes,
}

impl std::fmt::Display for QueryBudgetDimension {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InputBytes => "input bytes",
            Self::Parameters => "parameters",
            Self::SyntaxNodes => "syntax nodes",
            Self::NestingDepth => "nesting depth",
            Self::Joins => "joins",
            Self::Ctes => "CTEs",
        })
    }
}

/// Conservative defaults for untrusted compiler input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryBudget {
    pub max_input_bytes: usize,
    pub max_parameters: usize,
    pub max_syntax_nodes: usize,
    pub max_nesting_depth: usize,
    pub max_joins: usize,
    pub max_ctes: usize,
}

impl Default for QueryBudget {
    fn default() -> Self {
        Self {
            max_input_bytes: 1024 * 1024,
            max_parameters: 256,
            max_syntax_nodes: 10_000,
            max_nesting_depth: 64,
            max_joins: 256,
            max_ctes: 256,
        }
    }
}

/// Measured compiler input used for deterministic budget enforcement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryBudgetUsage {
    pub input_bytes: usize,
    pub parameters: usize,
    pub syntax_nodes: usize,
    pub nesting_depth: usize,
    pub joins: usize,
    pub ctes: usize,
}

impl QueryBudgetUsage {
    /// Measure the syntax tree produced by a dialect parser.
    pub fn for_parsed(parsed: &ParsedQuery) -> Self {
        let mut usage = measure_query(parsed.syntax());
        usage.input_bytes = parsed.source_sql().len();
        usage
    }

    pub fn with_parameters(mut self, parameters: usize) -> Self {
        self.parameters = parameters;
        self
    }
}

impl QueryBudget {
    /// Return a stable structured error for the first exceeded dimension.
    pub fn check(&self, usage: &QueryBudgetUsage) -> Result<()> {
        let limits = [
            (
                QueryBudgetDimension::InputBytes,
                self.max_input_bytes,
                usage.input_bytes,
            ),
            (
                QueryBudgetDimension::Parameters,
                self.max_parameters,
                usage.parameters,
            ),
            (
                QueryBudgetDimension::SyntaxNodes,
                self.max_syntax_nodes,
                usage.syntax_nodes,
            ),
            (
                QueryBudgetDimension::NestingDepth,
                self.max_nesting_depth,
                usage.nesting_depth,
            ),
            (QueryBudgetDimension::Joins, self.max_joins, usage.joins),
            (QueryBudgetDimension::Ctes, self.max_ctes, usage.ctes),
        ];
        for (dimension, limit, actual) in limits {
            if actual > limit {
                return Err(crate::QueryFabricError::BudgetExceeded {
                    dimension,
                    limit,
                    actual,
                });
            }
        }
        Ok(())
    }
}

fn measure_query(query: &SyntaxQuery) -> QueryBudgetUsage {
    let mut usage = QueryBudgetUsage {
        syntax_nodes: 1,
        ..QueryBudgetUsage::default()
    };
    usage.ctes = query.ctes.len();
    for cte in &query.ctes {
        merge(&mut usage, measure_cte(cte), 1);
    }
    merge(&mut usage, measure_set_expr(&query.body, 1), 0);
    for order in &query.order_by {
        usage.syntax_nodes += 1;
        merge(&mut usage, measure_expr(&order.expr, 1), 0);
    }
    if let Some(limit) = &query.limit {
        merge(&mut usage, measure_expr(limit, 1), 0);
    }
    if let Some(offset) = &query.offset {
        merge(&mut usage, measure_expr(offset, 1), 0);
    }
    usage
}

fn measure_cte(cte: &SyntaxCte) -> QueryBudgetUsage {
    let mut usage = QueryBudgetUsage {
        syntax_nodes: 1,
        ctes: cte.query.ctes.len(),
        ..QueryBudgetUsage::default()
    };
    merge(&mut usage, measure_query(&cte.query), 1);
    usage
}

fn measure_set_expr(expr: &SyntaxSetExpr, depth: usize) -> QueryBudgetUsage {
    match expr {
        SyntaxSetExpr::Select(select) => {
            let mut usage = QueryBudgetUsage {
                syntax_nodes: 1,
                nesting_depth: depth,
                ..QueryBudgetUsage::default()
            };
            for item in &select.projection {
                usage.syntax_nodes += 1;
                if let Some(item) = item.as_expr() {
                    merge(&mut usage, measure_expr(&item.expr, depth), 0);
                }
            }
            for table in &select.from {
                merge(&mut usage, measure_table(table, depth), 0);
            }
            if let Some(selection) = &select.selection {
                merge(&mut usage, measure_expr(selection, depth), 0);
            }
            for expr in &select.group_by {
                merge(&mut usage, measure_expr(expr, depth), 0);
            }
            if let Some(having) = &select.having {
                merge(&mut usage, measure_expr(having, depth), 0);
            }
            usage
        }
        SyntaxSetExpr::UnionAll { left, right, .. } => {
            let mut usage = QueryBudgetUsage {
                syntax_nodes: 1,
                nesting_depth: depth,
                ..QueryBudgetUsage::default()
            };
            merge(&mut usage, measure_set_expr(left, depth + 1), 0);
            merge(&mut usage, measure_set_expr(right, depth + 1), 0);
            usage
        }
        SyntaxSetExpr::Unsupported { .. } => QueryBudgetUsage {
            syntax_nodes: 1,
            nesting_depth: depth,
            ..QueryBudgetUsage::default()
        },
    }
}

fn measure_table(table: &SyntaxTableWithJoins, depth: usize) -> QueryBudgetUsage {
    let mut usage = measure_relation(&table.relation, depth);
    for join in &table.joins {
        usage.joins += 1;
        usage.syntax_nodes += 1;
        merge(&mut usage, measure_relation(&join.relation, depth), 0);
        if let Some(on) = &join.on {
            merge(&mut usage, measure_expr(on, depth), 0);
        }
    }
    usage
}

fn measure_relation(relation: &SyntaxRelation, depth: usize) -> QueryBudgetUsage {
    match relation {
        SyntaxRelation::Table { .. } | SyntaxRelation::Unsupported { .. } => QueryBudgetUsage {
            syntax_nodes: 1,
            nesting_depth: depth,
            ..QueryBudgetUsage::default()
        },
        SyntaxRelation::Derived { query, .. } => {
            let mut usage = QueryBudgetUsage {
                syntax_nodes: 1,
                nesting_depth: depth,
                ..QueryBudgetUsage::default()
            };
            merge(&mut usage, measure_query(query), depth + 1);
            usage
        }
        SyntaxRelation::NestedJoin {
            table_with_joins, ..
        } => {
            let mut usage = QueryBudgetUsage {
                syntax_nodes: 1,
                nesting_depth: depth,
                ..QueryBudgetUsage::default()
            };
            merge(&mut usage, measure_table(table_with_joins, depth + 1), 0);
            usage
        }
    }
}

fn measure_expr(expr: &SyntaxExpr, depth: usize) -> QueryBudgetUsage {
    let mut usage = QueryBudgetUsage {
        syntax_nodes: 1,
        nesting_depth: depth,
        ..QueryBudgetUsage::default()
    };
    match &expr.kind {
        SyntaxExprKind::Unary { expr, .. }
        | SyntaxExprKind::IsNull { expr, .. }
        | SyntaxExprKind::Cast { expr, .. } => merge(&mut usage, measure_expr(expr, depth + 1), 0),
        SyntaxExprKind::Binary { left, right, .. } => {
            merge(&mut usage, measure_expr(left, depth + 1), 0);
            merge(&mut usage, measure_expr(right, depth + 1), 0);
        }
        SyntaxExprKind::Function(function) => {
            for arg in &function.args {
                merge(&mut usage, measure_expr(arg, depth + 1), 0);
            }
            if let Some(filter) = &function.filter {
                merge(&mut usage, measure_expr(filter, depth + 1), 0);
            }
            if let Some(window) = &function.over {
                for expr in &window.partition_by {
                    merge(&mut usage, measure_expr(expr, depth + 1), 0);
                }
                for expr in &window.order_by {
                    merge(&mut usage, measure_expr(&expr.expr, depth + 1), 0);
                }
            }
        }
        SyntaxExprKind::Case {
            operand,
            when_then,
            else_result,
        } => {
            if let Some(operand) = operand {
                merge(&mut usage, measure_expr(operand, depth + 1), 0);
            }
            for pair in when_then {
                merge(&mut usage, measure_expr(&pair.condition, depth + 1), 0);
                merge(&mut usage, measure_expr(&pair.result, depth + 1), 0);
            }
            if let Some(else_result) = else_result {
                merge(&mut usage, measure_expr(else_result, depth + 1), 0);
            }
        }
        SyntaxExprKind::Between {
            expr, low, high, ..
        } => {
            merge(&mut usage, measure_expr(expr, depth + 1), 0);
            merge(&mut usage, measure_expr(low, depth + 1), 0);
            merge(&mut usage, measure_expr(high, depth + 1), 0);
        }
        SyntaxExprKind::InList { expr, list, .. } => {
            merge(&mut usage, measure_expr(expr, depth + 1), 0);
            for item in list {
                merge(&mut usage, measure_expr(item, depth + 1), 0);
            }
        }
        SyntaxExprKind::InSubquery { expr, subquery, .. } => {
            merge(&mut usage, measure_expr(expr, depth + 1), 0);
            merge(&mut usage, measure_query(subquery), depth + 1);
        }
        SyntaxExprKind::ScalarSubquery(query) | SyntaxExprKind::Exists(query) => {
            merge(&mut usage, measure_query(query), depth + 1);
        }
        SyntaxExprKind::Like { expr, pattern, .. } => {
            merge(&mut usage, measure_expr(expr, depth + 1), 0);
            merge(&mut usage, measure_expr(pattern, depth + 1), 0);
        }
        SyntaxExprKind::Tuple(items) | SyntaxExprKind::Array(items) => {
            for item in items {
                merge(&mut usage, measure_expr(item, depth + 1), 0);
            }
        }
        SyntaxExprKind::Column { .. }
        | SyntaxExprKind::Literal(_)
        | SyntaxExprKind::Parameter(_)
        | SyntaxExprKind::Unsupported { .. } => {}
    }
    usage
}

fn merge(target: &mut QueryBudgetUsage, source: QueryBudgetUsage, extra_depth: usize) {
    target.syntax_nodes += source.syntax_nodes;
    target.nesting_depth = target.nesting_depth.max(source.nesting_depth + extra_depth);
    target.joins += source.joins;
    target.ctes += source.ctes;
}
