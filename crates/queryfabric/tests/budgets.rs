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
