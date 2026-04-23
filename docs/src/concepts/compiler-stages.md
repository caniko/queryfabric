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

## Provenance

Analysis and emission both attach provenance receipts. These record enough
information for downstream systems to answer basic reproducibility questions:

- which query shape was compiled?
- which catalog snapshot was used?
- which backend adapter made the decision?
- which emitted artifact corresponds to that decision?
