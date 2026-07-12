# Tutorial: Your First Query

This tutorial walks through the four stages of the QueryFabric pipeline:
parse, bind, analyze, emit.

## Setup

Create a new Rust project:

```bash
cargo new queryfabric-demo && cd queryfabric-demo
cargo add queryfabric
```

## Step 1: Parse

```rust,ignore
use queryfabric::{
    GenericSqlDialect, QueryCompiler, QueryParameters, MemoryCatalog, RelationSchema,
    ColumnSchema, RelationKind, DataType, bind_and_validate_query, ClickHouseAdapter,
};

let compiler = QueryCompiler::default();
let sql = "SELECT record_id, score FROM records WHERE score > $1 LIMIT 5";

let parsed = compiler.parse(&GenericSqlDialect, sql)?;
```

`ParsedQuery` holds the syntax tree and source spans. At this point, names
are still unresolved — we haven't checked if `records` or `score` exist.

## Step 2: Bind

Build a catalog that describes your schema:

```rust,ignore
let mut catalog = MemoryCatalog::default();
catalog.register_relation(RelationSchema {
    namespace: None,
    name: "records".into(),
    aliases: vec!["r".into()],
    kind: RelationKind::Table,
    columns: vec![
        ColumnSchema {
            name: "record_id".into(),
            data_type: DataType::Uuid,
            nullable: false,
            metadata: Default::default(),
        },
        ColumnSchema {
            name: "score".into(),
            data_type: DataType::Float64,
            nullable: true,
            metadata: Default::default(),
        },
    ],
    metadata: Default::default(),
});
```

Now bind:

```rust,ignore
let mut params = QueryParameters::default();
params.insert_positional(1, queryfabric::ParameterValue::Float64("100.0".into()));
let bound = bind_and_validate_query(&parsed, &catalog, &params)?;
```

If the query references a table or column that doesn't exist in the catalog,
binding fails with a structured diagnostic explaining what's wrong.

## Step 3: Analyze

```rust,ignore
let analysis = compiler.analyze(&bound, &ClickHouseAdapter, &catalog);
println!("Supported: {}", analysis.supported);
for diagnostic in &analysis.diagnostics {
    println!("  [{}] {}", diagnostic.severity, diagnostic.message);
}
```

The analysis checks:

- Does ClickHouse support every feature this query uses?
- Are there any advisory notes (e.g., materialized view wrapping)?
- What result schema would the query produce?

## Step 4: Emit

```rust,ignore
let artifact = compiler.emit(&bound, &ClickHouseAdapter, &catalog)?;
let sql = artifact.as_sql().expect("SQL artifact");

println!("Emitted SQL:\n{}", sql.text);
println!("Result schema:");
for field in sql.result_schema.fields() {
    println!("  {}: {:?}", field.name, field.data_type);
}
```

The emitted SQL is backend-specific. The same `BoundQuery` can be emitted for
PostgreSQL by passing `&PostgresAdapter`.

## Full example

```rust,ignore
fn main() -> Result<(), queryfabric::QueryFabricError> {
    let compiler = QueryCompiler::default();
    let parsed = compiler.parse(&GenericSqlDialect,
        "SELECT record_id, score FROM records WHERE score > $1 LIMIT 5")?;

    let mut catalog = MemoryCatalog::default();
    catalog.register_relation(RelationSchema {
        namespace: None, name: "records".into(), aliases: vec![],
        kind: RelationKind::Table,
        columns: vec![
            ColumnSchema { name: "record_id".into(), data_type: DataType::Uuid, nullable: false, metadata: Default::default() },
            ColumnSchema { name: "score".into(), data_type: DataType::Float64, nullable: true, metadata: Default::default() },
        ],
        metadata: Default::default(),
    });

    let mut params = QueryParameters::default();
    params.insert_positional(1, queryfabric::ParameterValue::Float64("100.0".into()));
    let bound = bind_and_validate_query(&parsed, &catalog, &params)?;

    let analysis = compiler.analyze(&bound, &ClickHouseAdapter, &catalog);
    println!("supported: {}", analysis.supported);

    let artifact = compiler.emit(&bound, &ClickHouseAdapter, &catalog)?;
    println!("{}", artifact.as_sql().unwrap().text);
    Ok(())
}
```

## What's next

- Run the [multi-backend example] to see one query emitted for two backends.
- Read the [host integration guide] to embed QueryFabric into your application.
- Try the [Python bindings tutorial] for a Python-based workflow.

[multi-backend example]: ../backends/clickhouse-and-postgres.md
[host integration guide]: ../integration/host-integration.md
[Python bindings tutorial]: ./python-tutorial.md
