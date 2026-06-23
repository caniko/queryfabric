# Scenario: Embed QueryFabric in a Python Application

**Who this is for:** You run a Python-based scientific data platform (Django,
FastAPI, Flask) and your users write queries against a catalog of tables. You
want to validate, analyze, and emit backend SQL before execution.

**What you'll end up with:** A Python endpoint that parses a user's SyQL query,
binds it against your catalog, analyzes it against ClickHouse, and returns
either a diagnostic or the emitted SQL — all through the `queryfabric` Python
package.

## Prerequisites

- Python 3.10+
- `queryfabric` Python package installed (via `pip` or `uv`)

## Step 1: Install

```bash
pip install queryfabric
# or
uv add queryfabric
```

## Step 2: Build your catalog

The catalog tells QueryFabric what tables and columns exist. Build it from your
own schema metadata:

```python
import queryfabric as qf

catalog = qf.MemoryCatalog()

# Register a table
catalog.add_relation(
    "measurements",
    columns=[
        ("sample_id", qf.DataType.Uuid, False),
        ("concentration", qf.DataType.Float64, True),
        ("recorded_at", qf.DataType.Timestamp(None), False),
    ],
    kind=qf.RelationKind.Table,
)
```

## Step 3: Create an API endpoint

```python
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
import queryfabric as qf

app = FastAPI()
catalog = build_catalog()  # your function from step 2

class QueryRequest(BaseModel):
    syql: str

class QueryResponse(BaseModel):
    valid: bool
    sql: str | None = None
    error: str | None = None

@app.post("/query/compile")
async def compile_query(request: QueryRequest):
    try:
        # 1. Parse
        parsed = qf.parse_syql(request.syql)

        # 2. Bind
        bound = qf.bind_and_validate(parsed, catalog, {})

        # 3. Analyze
        analysis = qf.analyze(bound, "clickhouse", catalog)

        if not analysis.supported:
            return QueryResponse(
                valid=False,
                error="; ".join(d.message for d in analysis.diagnostics),
            )

        # 4. Emit
        artifact = qf.emit_clickhouse_sql(bound, catalog)
        return QueryResponse(valid=True, sql=artifact.text)

    except qf.QueryFabricError as e:
        return QueryResponse(valid=False, error=str(e))
```

## Step 4: Use it

```bash
curl -X POST http://localhost:8000/query/compile \
  -H "Content-Type: application/json" \
  -d '{"syql": "FROM measurements WHERE concentration > 0.5"}'
```

## Key points

- The Python bindings mirror the Rust facade: parse → bind → analyze → emit.
- Catalog construction from your own schema is the main integration effort.
- The same code works for ClickHouse and PostgreSQL backends — pass
  `"postgres"` instead of `"clickhouse"` to `qf.analyze()`.
