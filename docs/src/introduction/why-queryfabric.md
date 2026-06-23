# Why QueryFabric?

You are building a scientific data platform. Users submit queries over your
catalog. You need to validate them, analyze whether your backend supports them,
and emit backend-specific SQL — all without coupling your query surface to
your storage backend.

You have options. Here is how QueryFabric compares.

## vs. Apache Calcite

Calcite is the industry standard for portable query compilation in the JVM
ecosystem. If your stack is in Java, Calcite is the right choice.

QueryFabric is the same idea for the Rust ecosystem: a portable query compiler
that decouples query authoring from backend execution. But QueryFabric adds:

- **A defined host boundary.** Calcite assumes it owns the full query lifecycle.
  QueryFabric deliberately stops short of execution, routing, auth, and
  orchestration — those belong to the host. This makes the compiler smaller,
  easier to audit, and trivial to embed.
- **Provenance receipts.** Every emitted artifact records the compiler version,
  catalog snapshot, and dialect — so downstream systems can answer "where did
  this SQL come from?"
- **Capability analysis before emission.** QueryFabric can tell you, without
  emitting a single byte, whether a given backend supports a query and why not.

## vs. Apache DataFusion

DataFusion is a fast, Arrow-native query engine in Rust. If you need to execute
queries in-process against Parquet or Arrow data, DataFusion is the right tool.

QueryFabric is not a query engine. It does not execute queries. It compiles
them. If your architecture looks like "user writes SQL → validate → analyze →
emit ClickHouse SQL → execute on a remote cluster", then DataFusion is the
wrong layer — you want a compiler, not an engine.

QueryFabric also handles what DataFusion does not:

- **Multiple backends.** One query, emitted for ClickHouse AND PostgreSQL.
- **Dialect separation.** SyQL, SQL, or a custom dialect — the parser is
  decoupled from the binder.
- **Schema-driven analysis.** QueryFabric resolves columns, functions, and types
  through a catalog before any backend touches the query.

## vs. SQLAlchemy

SQLAlchemy is the Python standard for programmatic SQL generation. If your
users write Python and you want an ORM or expression DSL, use SQLAlchemy.

QueryFabric is for the case where your users write **raw text queries** (SyQL,
SQL) and you need them to compile reliably against a backend you do not control
at the application layer. SQLAlchemy does not do capability analysis, does not
emit capability-aware diagnostics, and does not produce provenance receipts.

## vs. hand-rolled string templating

This is the most common starting point for scientific platforms. You have a
query template, a few `format!()` calls, and a prayer that the user didn't
write `; DROP TABLE`.

QueryFabric replaces that with:

- **Structured parsing** — syntax errors before execution.
- **Catalog binding** — column names and types are resolved, not guessed.
- **Parameter typing** — `$1` is not a string replacement puzzle.
- **Backend analysis** — "your JSON column works on ClickHouse but not on
  Postgres" before you send the query.
- **Provenance** — every emitted artifact knows what catalog and compiler
  version produced it.

## When NOT to use QueryFabric

- You need an in-process query **execution** engine (use DataFusion).
- Your stack is in Java (use Calcite).
- Your users write Python and you want an ORM (use SQLAlchemy).
- You have exactly one backend, no plans to change, and no need for capability
  analysis (stick with string templating — but add parameterized queries).
