# Writing a Custom Adapter

Every backend adapter implements `BackendAdapter`. This page walks through the
three methods you need to implement.

## The trait

```rust
pub trait BackendAdapter: Send + Sync {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> CapabilitySet;
    fn analyze(&self, query: &BoundQuery, catalog: &dyn Catalog) -> BackendAnalysis;
    fn emit(&self, query: &BoundQuery, catalog: &dyn Catalog) -> Result<EmitArtifact>;
}
```

## 1. `name()` — backend identifier

Return a short string that identifies your backend. This appears in
diagnostics and provenance receipts.

```rust
fn name(&self) -> &'static str { "duckdb" }
```

## 2. `capabilities()` — what your backend supports

List the `BackendFeature` values your backend can handle. Unsupported features
produce diagnostics during analysis.

```rust
fn capabilities(&self) -> CapabilitySet {
    CapabilitySet::from_features([
        BackendFeature::Aggregates,
        BackendFeature::Joins,
        BackendFeature::CommonTableExpressions,
        BackendFeature::Windows,
        BackendFeature::LimitOffset,
        BackendFeature::ApproximateAggregates,
    ])
    .with_limits(BackendExecutionLimits {
        max_rows: Some(10_000_000),
        max_bytes_scanned: Some(1_000_000_000),
        interactive_byte_limit: 100 * 1024 * 1024,
        batch_byte_limit: 1_000 * 1024 * 1024,
        ..Default::default()
    })
    .with_result_formats([
        ResultDeliveryFormat::ArrowIpc,
        ResultDeliveryFormat::Csv,
    ])
}
```

## 3. `analyze()` — check support and return diagnostics

Use the `analyze_backend_support` helper to run the standard capability check,
then add backend-specific diagnostics:

```rust
fn analyze(&self, query: &BoundQuery, catalog: &dyn Catalog) -> BackendAnalysis {
    let mut analysis = queryfabric::analyze_backend_support(
        query, catalog, self.name(), self.capabilities(), true,
    );

    // Add backend-specific checks
    for field in query.result_schema.fields() {
        if field.data_type == DataType::Json {
            analysis.diagnostics.push(QueryDiagnostic::warning(
                "MYBACK001",
                format!("JSON columns are cast to String on this backend"),
            ));
        }
    }

    analysis.supported = !analysis.diagnostics.iter().any(|d| d.is_error());
    analysis
}
```

## 4. `emit()` — produce backend-specific SQL

The simplest path uses `emit_sql_artifact` with `SqlBackend::Other`:

```rust
fn emit(&self, query: &BoundQuery, catalog: &dyn Catalog) -> Result<EmitArtifact> {
    emit_sql_artifact(query, catalog, SqlBackend::Other("duckdb"))
}
```

This reuses the generic SQL emitter which handles `SELECT`, `FROM`, `WHERE`,
`GROUP BY`, `JOIN`, `CTE`, `UNION ALL`, window functions, scalar and aggregate
functions, and typed parameters.

If you need backend-specific SQL rendering (different function names,
different type casts), walk the `BoundQuery` AST directly:

```rust
fn emit(&self, query: &BoundQuery, catalog: &dyn Catalog) -> Result<EmitArtifact> {
    let sql_text = render_my_sql(query, catalog)?;
    Ok(EmitArtifact::sql(sql_text, query.result_schema()))
}
```

## Testing your adapter

```rust
#[test]
fn test_my_adapter_emits_valid_sql() {
    let adapter = MyAdapter;
    let catalog = build_catalog();
    let bound = bind("SELECT * FROM test_table");

    let analysis = adapter.analyze(&bound, &catalog);
    assert!(analysis.supported, "{:?}", analysis.diagnostics);

    let artifact = adapter.emit(&bound, &catalog).expect("emit");
    let sql = artifact.as_sql().expect("sql artifact");
    assert!(sql.text.contains("test_table"));
}
```

## Full example

See the [Custom Backend scenario](../scenarios/custom-backend.md) for a
complete, runnable example with DuckDB.
