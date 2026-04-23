# Quick Start

The shortest useful QueryFabric flow is:

1. parse source text with a dialect
2. bind against a catalog and parameters
3. analyze a backend adapter
4. emit a backend artifact

```rust
use queryfabric::{
    ClickHouseAdapter, ColumnSchema, DataType, GenericSqlDialect, MemoryCatalog, QueryCompiler,
    QueryParameters, RelationKind, RelationSchema, bind_and_validate_query,
};

fn main() -> Result<(), queryfabric::QueryFabricError> {
    let compiler = QueryCompiler::default();
    let parsed = compiler.parse(
        &GenericSqlDialect,
        "SELECT neuron_id, cable_length FROM neurons WHERE cable_length > $1 LIMIT 5",
    )?;

    let mut catalog = MemoryCatalog::default();
    catalog.set_snapshot_id("quickstart-catalog");
    catalog.register_relation(RelationSchema {
        namespace: None,
        name: "neurons".into(),
        aliases: vec!["n".into()],
        kind: RelationKind::Table,
        columns: vec![
            ColumnSchema {
                name: "neuron_id".into(),
                data_type: DataType::Uuid,
                nullable: false,
                metadata: Default::default(),
            },
            ColumnSchema {
                name: "cable_length".into(),
                data_type: DataType::Float64,
                nullable: true,
                metadata: Default::default(),
            },
        ],
        metadata: Default::default(),
    });

    let mut parameters = QueryParameters::default();
    parameters.insert_positional(1, queryfabric::ParameterValue::Float64("100.0".into()));

    let bound = bind_and_validate_query(&parsed, &catalog, &parameters)?;
    let analysis = compiler.analyze(&bound, &ClickHouseAdapter, &catalog);
    let artifact = compiler.emit(&bound, &ClickHouseAdapter, &catalog)?;
    let sql = artifact.as_sql().expect("SQL artifact");

    println!("supported: {}", analysis.supported);
    println!("sql:\n{}", sql.text);

    Ok(())
}
```

## What This Gives You

- `parsed` retains the source-level structure and spans
- `bound` captures resolved names, functions, and types
- `analysis` tells you whether the backend supports the query and why
- `artifact` contains emitted SQL plus result schema and provenance

## Next Step

Run the richer multi-backend example if you want the intended host-facing shape:

```bash
cargo run --manifest-path crates/queryfabric/Cargo.toml --example multi_backend
```

That example binds one portable query once and then analyzes and emits it
against both ClickHouse and PostgreSQL.
