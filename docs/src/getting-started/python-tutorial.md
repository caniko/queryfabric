# Tutorial: Python Bindings

QueryFabric's Python bindings mirror the Rust facade: parse → bind → analyze
→ emit. This tutorial shows the same pipeline from Python.

## Setup

```bash
pip install queryfabric
# or
uv add queryfabric
```

## Step 1: Parse

```python
import queryfabric as qf

sql = "SELECT record_id, score FROM records WHERE score > $1 LIMIT 5"
parsed = qf.parse_syql(sql)
```

`parse_syql` accepts SyQL syntax. For standard SQL use `parse_sql`.

## Step 2: Bind

```python
catalog = qf.MemoryCatalog()
catalog.add_relation(
    "records",
    columns=[
        ("record_id", qf.DataType.Uuid, False),
        ("score", qf.DataType.Float64, True),
    ],
    kind=qf.RelationKind.Table,
)

params = {"1": qf.ParameterValue.Float64("100.0")}
bound = qf.bind_and_validate(parsed, catalog, params)
```

## Step 3: Analyze

```python
analysis = qf.analyze(bound, "clickhouse", catalog)
print(f"Supported: {analysis.supported}")
for d in analysis.diagnostics:
    print(f"  [{d.severity}] {d.message}")
```

## Step 4: Emit

```python
artifact = qf.emit_clickhouse_sql(bound, catalog)
print(artifact.text)
```

## Full script

```python
#!/usr/bin/env python3
import queryfabric as qf

sql = "SELECT record_id, score FROM records WHERE score > $1 LIMIT 5"
parsed = qf.parse_syql(sql)

catalog = qf.MemoryCatalog()
catalog.add_relation("records", [
    ("record_id", qf.DataType.Uuid, False),
    ("score", qf.DataType.Float64, True),
])

params = {"1": qf.ParameterValue.Float64("100.0")}
bound = qf.bind_and_validate(parsed, catalog, params)

analysis = qf.analyze(bound, "clickhouse", catalog)
print(f"supported: {analysis.supported}")

artifact = qf.emit_clickhouse_sql(bound, catalog)
print(artifact.text)
```

## API reference

| Python | Rust equivalent |
|--------|----------------|
| `qf.parse_syql(text)` | `compiler.parse(&SyqlDialect, text)` |
| `qf.parse_sql(text)` | `compiler.parse(&GenericSqlDialect, text)` |
| `qf.bind_and_validate(parsed, catalog, params)` | `bind_and_validate_query(parsed, catalog, params)` |
| `qf.analyze(bound, backend, catalog)` | `compiler.analyze(bound, adapter, catalog)` |
| `qf.emit_clickhouse_sql(bound, catalog)` | `compiler.emit(bound, &ClickHouseAdapter, catalog)` |
| `qf.Catalog` | `impl Catalog` trait (Rust) |
| `qf.MemoryCatalog()` | `MemoryCatalog::default()` |
