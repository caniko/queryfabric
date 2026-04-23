# Python Bindings

QueryFabric ships a neutral Python package named `queryfabric`.

The goal is not to expose Rust internals. The goal is to expose the same stable
facade stages that the Rust API exposes:

- `parse_sql(text)`
- `parse_syql(text)`
- `inspect_parameters(parsed)`
- `bind_and_validate(parsed, catalog, params=None)`
- `analyze_clickhouse(bound, catalog)`
- `analyze_postgres(bound, catalog)`
- `emit_clickhouse_sql(bound, catalog)`
- `emit_postgres_sql(bound, catalog)`

## Installation

From the repo:

```bash
cd packages/queryfabric
maturin develop
pytest
```

## Catalog Construction

The Python bindings include the same basic catalog-building surface:

- `MemoryCatalog`
- `RelationSchema`
- `ColumnSchema`
- `DataType`
- `RelationKind`
- `QueryParameters`

That makes the bindings useful for notebooks, UI validation, smoke tests, and
other environments where embedding the Rust crate directly is awkward.

## JSON-Friendly Results

Parsed, bound, analysis, and artifact wrappers expose `to_dict()` and
`to_json()` helpers. That matters for UI routes and scripting contexts that
want structured compiler output without understanding Rust data models.

## Scope Boundary

The Python package stays neutral.

It does not own:

- host metadata resolution
- federation routing
- backend execution
- application-specific policy
