use queryfabric::{GenericSqlDialect, QueryBudget, QueryCompiler, QueryFabricError};

#[test]
fn input_budget_returns_structured_error() {
    let compiler = QueryCompiler::default().with_budget(QueryBudget {
        max_input_bytes: 7,
        ..QueryBudget::default()
    });
    let error = compiler
        .parse(&GenericSqlDialect, "SELECT 1")
        .expect_err("input should exceed the budget");
    assert!(matches!(
        error,
        QueryFabricError::BudgetExceeded {
            dimension: queryfabric::QueryBudgetDimension::InputBytes,
            limit: 7,
            actual: 8,
        }
    ));
}

#[test]
fn structural_budget_rejects_nested_queries() {
    let compiler = QueryCompiler::default().with_budget(QueryBudget {
        max_nesting_depth: 1,
        ..QueryBudget::default()
    });
    let error = compiler
        .parse(
            &GenericSqlDialect,
            "SELECT record_id FROM (SELECT record_id FROM records) AS nested",
        )
        .expect_err("nested query should exceed the budget");
    assert!(matches!(
        error,
        QueryFabricError::BudgetExceeded {
            dimension: queryfabric::QueryBudgetDimension::NestingDepth,
            ..
        }
    ));
}

#[test]
fn every_budget_dimension_has_a_stable_failure() {
    let cases = [
        (
            QueryBudget {
                max_parameters: 0,
                ..QueryBudget::default()
            },
            queryfabric::QueryBudgetUsage {
                parameters: 1,
                ..Default::default()
            },
            queryfabric::QueryBudgetDimension::Parameters,
        ),
        (
            QueryBudget {
                max_syntax_nodes: 0,
                ..QueryBudget::default()
            },
            queryfabric::QueryBudgetUsage {
                syntax_nodes: 1,
                ..Default::default()
            },
            queryfabric::QueryBudgetDimension::SyntaxNodes,
        ),
        (
            QueryBudget {
                max_joins: 0,
                ..QueryBudget::default()
            },
            queryfabric::QueryBudgetUsage {
                joins: 1,
                ..Default::default()
            },
            queryfabric::QueryBudgetDimension::Joins,
        ),
        (
            QueryBudget {
                max_ctes: 0,
                ..QueryBudget::default()
            },
            queryfabric::QueryBudgetUsage {
                ctes: 1,
                ..Default::default()
            },
            queryfabric::QueryBudgetDimension::Ctes,
        ),
    ];
    for (budget, usage, dimension) in cases {
        assert!(matches!(
            budget.check(&usage),
            Err(QueryFabricError::BudgetExceeded {
                dimension: actual,
                limit: 0,
                actual: 1,
            }) if actual == dimension
        ));
    }
}

#[test]
fn compiler_counts_join_and_cte_nodes_before_binding() {
    let joins = QueryCompiler::default().with_budget(QueryBudget {
        max_joins: 0,
        ..QueryBudget::default()
    });
    assert!(matches!(
        joins.parse(
            &GenericSqlDialect,
            "SELECT a.id FROM a INNER JOIN b ON a.id = b.id"
        ),
        Err(QueryFabricError::BudgetExceeded {
            dimension: queryfabric::QueryBudgetDimension::Joins,
            ..
        })
    ));

    let ctes = QueryCompiler::default().with_budget(QueryBudget {
        max_ctes: 0,
        ..QueryBudget::default()
    });
    assert!(matches!(
        ctes.parse(
            &GenericSqlDialect,
            "WITH recent AS (SELECT 1) SELECT 1"
        ),
        Err(QueryFabricError::BudgetExceeded {
            dimension: queryfabric::QueryBudgetDimension::Ctes,
            ..
        })
    ));
}
