# QueryFabric

QueryFabric is a portable analytical query compiler for scientific platforms.
It gives hosts a stable semantic boundary for:

- parsing SQL or downstream dialects such as SyQL into `ParsedQuery`
- binding against a catalog into `BoundQuery`
- computing capability requirements and structured diagnostics
- analyzing backend support before execution
- emitting typed backend artifacts with reproducibility metadata

The stable public API lives in the [`queryfabric`](crates/queryfabric) facade
crate. Internal crates remain modular, but they are not the public promise.

## What QueryFabric Owns

- portable query parsing and canonicalization
- typed parameters and result schemas
- catalog and function-registry contracts
- capability analysis and backend diagnostics
- SQL emission for the verified portable subset
- provenance receipts for analysis and emission

## What Stays Out of Core

- host routing and fan-out
- auth and job orchestration
- SynDB-specific ontology and metadata resolution
- backend execution itself

ClickHouse-specific materialized-view aggregate wrapping and advisory diagnostics
live in the ClickHouse adapter. Hosts still own routing, execution, and any
metadata-driven policy which relation to query in the first place.

SyQL directives such as `SCOPE` and `DOWNLOAD` are preserved as opaque
dialect metadata rather than neutral core semantics.

## Verified Portable Subset

The first public release targets a documented, test-backed subset:

- `SELECT`, `WHERE`, `GROUP BY`, `HAVING`, `ORDER BY`, `LIMIT`, `OFFSET`, `DISTINCT`
- `CASE`
- typed positional and named parameters
- `INNER`, `LEFT`, `RIGHT`, `FULL`, `CROSS` joins
- non-recursive CTEs
- derived subqueries in `FROM`
- scalar subqueries and `IN` subqueries where validated
- `UNION ALL`
- common scalar and aggregate functions through the function registry
- window functions used heavily in analytics:
  `RANK`, `DENSE_RANK`, `ROW_NUMBER`, `LAG`, `LEAD`, `FIRST_VALUE`, `LAST_VALUE`

Portability is defined by the support matrix and conformance tests, not by what
the parser happens to accept.

## Quickstart

See [`crates/queryfabric/examples/quickstart.rs`](crates/queryfabric/examples/quickstart.rs)
for the shortest runnable example.

```rust
use queryfabric::{
    bind_and_validate_query, ClickHouseAdapter, ColumnSchema, DataType, GenericSqlDialect,
    MemoryCatalog, QueryCompiler, QueryParameters, RelationKind, RelationSchema,
};

let compiler = QueryCompiler::default();
let parsed = compiler
    .parse(&GenericSqlDialect, "SELECT neuron_id FROM neurons LIMIT 5")?;

let mut catalog = MemoryCatalog::default();
catalog.register_relation(RelationSchema {
    namespace: None,
    name: "neurons".into(),
    aliases: Vec::new(),
    kind: RelationKind::Table,
    columns: vec![ColumnSchema {
        name: "neuron_id".into(),
        data_type: DataType::Uuid,
        nullable: false,
        metadata: Default::default(),
    }],
    metadata: Default::default(),
});

let bound = bind_and_validate_query(&parsed, &catalog, &QueryParameters::default())?;
let artifact = compiler.emit(&bound, &ClickHouseAdapter, &catalog)?;
let sql = artifact.as_sql().unwrap();
assert!(sql.text.contains("FROM neurons"));
assert!(sql.text.contains("neuron_id"));
# Ok::<(), queryfabric::QueryFabricError>(())
```

See [`crates/queryfabric/examples/multi_backend.rs`](crates/queryfabric/examples/multi_backend.rs)
for a richer example that binds one portable query once and then analyzes and
emits it against both ClickHouse and PostgreSQL.

Runnable example commands:

```bash
cargo run --manifest-path crates/queryfabric/Cargo.toml --example quickstart
cargo run --manifest-path crates/queryfabric/Cargo.toml --example multi_backend
```

Python bindings live under [`packages/queryfabric`](packages/queryfabric) and
mirror the same facade-first stages: parse, inspect parameters, bind, analyze,
and emit.

## Repository Guide

- [`COMPATIBILITY.md`](COMPATIBILITY.md): semver, MSRV, backend matrix, support policy
- [`MIGRATION.md`](MIGRATION.md): moving from SynDB-internal usage to standalone QueryFabric
- [`docs/`](docs/): mdBook documentation for Codeberg Pages and local browsing
- [`website/`](website/): Zola landing site for the public project homepage
- [`fuzz/README.md`](fuzz/README.md): parser and binder fuzz harnesses plus seed corpora
- [`capabilities/builtin-capability-manifest.json`](capabilities/builtin-capability-manifest.json):
  machine-readable built-in backend capabilities
- [`conformance/portable-subset.json`](conformance/portable-subset.json):
  public conformance corpus for the portable subset
- [`examples/syndb/README.md`](examples/syndb/README.md): SynDB host integration notes
- [`scripts/release.sh`](scripts/release.sh): staged release helper for local checks,
  crates.io publication order, and local tagging

## Fuzzing

Use the SynDB devshell so `cargo-fuzz` is available, then build or run the
targets from the `fuzz/` directory:

```bash
nix develop . -c bash
cd queryfabric/fuzz && cargo fuzz build parse_sql_no_panic
cd queryfabric/fuzz && cargo fuzz build bind_portable_no_panic
```

## Website and Docs

QueryFabric ships a standalone Codeberg Pages layout:

```bash
nix build .#website
nix build .#docs
nix build .#site
cd website && zola serve
cd docs && mdbook serve
```

## Status

This repository is ready for standalone iteration and publication. DataFusion
and other non-SQL emitters are intentionally deferred; the artifact seam is
kept open so those backends can be added without changing the stable facade.
Parser and binder fuzz harnesses live under [`fuzz/`](fuzz/README.md) and are
part of the release-quality gate.
