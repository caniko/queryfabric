# Compiler Stages

QueryFabric is designed around a semantic compiler boundary instead of a
string-in, string-out helper.

## Parsed Query

`Dialect::parse(&str)` produces a `ParsedQuery`.

This stage owns:

- syntax validation
- source spans
- dialect metadata such as downstream SyQL directives
- lightweight structural inspection such as parameter discovery

At this point the query is still unresolved. Names, types, and function
signatures have not been validated against a catalog yet.

### Compilation budgets

Hosts that accept untrusted query text can configure `QueryCompiler` with a
`QueryBudget`. The default budget bounds input bytes, distinct parameters,
syntax nodes, nesting depth, joins, and CTEs. The same limits are checked
before binding and again after parameter finalization; an exceeded dimension
returns `QueryFabricError::BudgetExceeded` with the limit and measured value.
Execution row, byte, and time limits remain host/runtime responsibilities.

## Bound Query

`bind_and_validate_query(parsed, catalog, params)` produces a `BoundQuery`.

This is the stage where QueryFabric commits to meaning:

- relations and columns are resolved through the catalog
- functions are matched through the registry
- placeholders are typed
- coercions and result shapes are checked
- capability requirements are computed

If a query is going to fail because of the schema, parameter shape, or a
missing function family, it should fail here with structured diagnostics.

## Backend Analysis

`QueryCompiler::analyze(bound, adapter, catalog)` produces `BackendAnalysis`.

Analysis is intentionally separate from emission. Hosts can ask each backend:

- is this query supported?
- what diagnostics or rejections apply?
- what result schema would come back?
- what provenance and cost-class metadata should be attached?

That makes backend choice an explicit host decision instead of a hidden side
effect of emission.

## Emission

`QueryCompiler::emit(bound, adapter, catalog)` produces `EmitArtifact`.

For `0.1`, QueryFabric emits SQL artifacts for ClickHouse and PostgreSQL. The
artifact seam remains open for future non-SQL targets, but the stable promise
today is SQL emission for the verified portable subset.

Catalog-derived relation, column, alias, CTE, and mapped-function segments are
validated and rendered through the backend identifier helpers. ClickHouse
table-target helpers apply the same segment-by-segment rule; values remain
parameters rather than SQL text.

## Provenance

Analysis and emission both attach provenance receipts. These record enough
information for downstream systems to answer basic reproducibility questions:

- which query shape was compiled?
- which catalog snapshot was used?
- which backend adapter made the decision?
- which emitted artifact corresponds to that decision?
