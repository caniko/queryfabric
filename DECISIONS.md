# Decisions

## D001: Use `sqlparser` AST as the initial neutral query tree

The first extraction cut reuses `sqlparser`'s AST instead of creating a second
custom SQL AST. This keeps the new surface neutral and reduces migration risk.

## D002: Keep execution hints outside relational semantics

`SCOPE`, `DOWNLOAD`, tracing IDs, and similar host directives are represented as
`ExecutionHints`, not as relational operators.

## D003: Keep host execution in SynDB

QueryFabric emits SQL or a DataFusion `LogicalPlan`. It does not execute queries,
manage auth, or own job orchestration.
