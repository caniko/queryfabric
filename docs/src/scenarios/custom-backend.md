# Scenario: Build a Custom Backend Adapter

**Who this is for:** You run a query engine that is not ClickHouse or
PostgreSQL — DuckDB, Druid, BigQuery, or a custom in-memory store. You want
QueryFabric to emit SQL (or another artifact) for it.

**What you'll end up with:** A `BackendAdapter` implementation that plugs into
the standard QueryFabric pipeline: parse → bind → analyze → **your emit**.

## The adapter trait

Every backend adapter implements `BackendAdapter`:

```rust,ignore
use queryfabric::{
    BackendAdapter, BackendAnalysis, CapabilitySet, Catalog, EmitArtifact, Result,
};

struct MyAdapter;

impl BackendAdapter for MyAdapter {
    fn name(&self) -> &'static str { "my-backend" }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::from_features([
            BackendFeature::Aggregates,
            BackendFeature::LimitOffset,
            // ... see BackendFeature for all options
        ])
    }

    fn analyze(&self, query: &BoundQuery, catalog: &dyn Catalog) -> BackendAnalysis {
        // Check capability support, return diagnostics
    }

    fn emit(&self, query: &BoundQuery, catalog: &dyn Catalog) -> Result<EmitArtifact> {
        // Walk the BoundQuery AST and produce SQL (or another format)
    }
}
```

## Minimal example: DuckDB

```rust,ignore
use queryfabric::{BackendAdapter, BackendAnalysis, CapabilitySet, Catalog, EmitArtifact,
    Result, BoundQuery, SqlBackend, emit_sql_artifact};

struct DuckDbAdapter;

impl BackendAdapter for DuckDbAdapter {
    fn name(&self) -> &'static str { "duckdb" }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::from_features([
            BackendFeature::Aggregates,
            BackendFeature::Joins,
            BackendFeature::CommonTableExpressions,
            BackendFeature::Windows,
            BackendFeature::LimitOffset,
        ])
    }

    fn analyze(&self, query: &BoundQuery, catalog: &dyn Catalog) -> BackendAnalysis {
        let mut analysis = queryfabric::analyze_backend_support(
            query, catalog, self.name(), self.capabilities(), true,
        );
        analysis.supported = !analysis.diagnostics.iter().any(|d| d.is_error());
        analysis
    }

    fn emit(&self, query: &BoundQuery, catalog: &dyn Catalog) -> Result<EmitArtifact> {
        // Reuse the generic SQL emitter with "duckdb" as the backend name.
        // The emitter handles SELECT, FROM, WHERE, GROUP BY, etc. generically.
        emit_sql_artifact(query, catalog, SqlBackend::Other("duckdb"))
    }
}
```

## Using your adapter

```rust,ignore
let compiler = QueryCompiler::default();
let parsed = compiler.parse(&GenericSqlDialect, "SELECT * FROM records")?;
let bound = bind_and_validate_query(&parsed, &catalog, &QueryParameters::default())?;

let analysis = compiler.analyze(&bound, &DuckDbAdapter, &catalog);
let artifact = compiler.emit(&bound, &DuckDbAdapter, &catalog)?;
```

## Adding backend-specific function mappings

If your backend uses different function names (e.g. `LEN` instead of
`LENGTH`), register them in the catalog:

```rust,ignore
catalog.register_function(FunctionSignature {
    namespace: None,
    name: "length".into(),
    kind: FunctionKind::Scalar,
    arg_types: vec![DataType::Utf8],
    return_type: DataType::Int64,
    backend_mappings: vec![
        BackendFunctionMapping {
            backend: "duckdb".into(),
            namespace: None,
            name: "len".into(),
        },
    ],
    // ...
});
```

## What you get for free

- Parsing (SQL, SyQL, or custom dialects)
- Catalog binding and type resolution
- Capability analysis
- Provenance receipts
- Parameter validation
- The optimization pipeline (federation planning, etc.)

You only write the backend-specific analysis and emission.
