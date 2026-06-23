# QueryFabric

<!-- simit:badges:start -->
[![Nix](https://img.shields.io/badge/Nix-managed-5277c3)](flake.nix) [![docs](https://img.shields.io/badge/docs-enabled-6f42c1)](docs) [![crates.io](https://img.shields.io/badge/crates.io-ready-f46623)](https://crates.io/crates/queryfabric)
<!-- simit:badges:end -->

[![REUSE status](https://api.reuse.software/badge/codeberg.org/caniko/queryfabric)](https://api.reuse.software/info/codeberg.org/caniko/queryfabric)

QueryFabric is a portable analytical query compiler for scientific platforms.
It also provides a data-sovereignty and query-portability layer for self-hosted
and federated services.

**Who is this for?** Platform engineers building scientific data platforms who
need to parse, validate, analyze, and emit queries against multiple backends
without coupling their query surface to any one storage engine. Python data
scientists who want to validate SyQL queries server-side. Sysadmins deploying
self-hosted data services with GDPR portability requirements.

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
- host-specific ontology and metadata resolution
- backend execution itself

ClickHouse-specific materialized-view aggregate wrapping and advisory diagnostics
live in the ClickHouse adapter. Hosts still own routing, execution, and any
metadata-driven policy which relation to query in the first place.

SyQL directives such as `SCOPE` and `DOWNLOAD` are preserved as opaque
dialect metadata rather than neutral core semantics.

## Why this matters for self-hosting

- [`crates/queryfabric-access`](crates/queryfabric-access): GDPR Art. 15, 16,
  and 17 traits for access, rectification, and erasure against generic
  resources.
- [`crates/queryfabric-portability`](crates/queryfabric-portability):
  content-addressed export bundles, provenance records, citation metadata, and
  DOI minting.
- [`crates/queryfabric-tenancy`](crates/queryfabric-tenancy): multi-tenant
  accounts, collections, and groups so hosts can keep ownership and isolation
  outside the compiler core.
- [`crates/queryfabric-federation`](crates/queryfabric-federation) and
  [`crates/queryfabric-cluster`](crates/queryfabric-cluster): wire-stable
  libp2p federation, routing, and health messaging between nodes.
- [`nix/modules/queryfabric.nix`](nix/modules/queryfabric.nix) and
  [`nix/tests/selfhost.nix`](nix/tests/selfhost.nix): hardened NixOS
  deployment wiring with secrets kept out of the store and covered by an
  end-to-end VM test.

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
    .parse(&GenericSqlDialect, "SELECT record_id FROM records LIMIT 5")?;

let mut catalog = MemoryCatalog::default();
catalog.register_relation(RelationSchema {
    namespace: None,
    name: "records".into(),
    aliases: Vec::new(),
    kind: RelationKind::Table,
    columns: vec![ColumnSchema {
        name: "record_id".into(),
        data_type: DataType::Uuid,
        nullable: false,
        metadata: Default::default(),
    }],
    metadata: Default::default(),
});

let bound = bind_and_validate_query(&parsed, &catalog, &QueryParameters::default())?;
let artifact = compiler.emit(&bound, &ClickHouseAdapter, &catalog)?;
let sql = artifact.as_sql().unwrap();
assert!(sql.text.contains("FROM records"));
assert!(sql.text.contains("record_id"));
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
- [`ROADMAP.md`](ROADMAP.md): public near-term, grant-scope, and later-stage direction
- [`MIGRATION.md`](MIGRATION.md): moving from host-internal usage to standalone QueryFabric
- [`docs/`](docs/): mdBook documentation for Codeberg Pages and local browsing
- [**Crate Catalog**](docs/src/integration/crate-catalog.md): what each of the 35 crates does
- [**User Scenarios**](docs/src/scenarios/): concrete walkthroughs for embedding, deploying, federating, and extending
- [`website/`](website/): Plinth project-site definition for the public homepage
- [`fuzz/README.md`](fuzz/README.md): parser and binder fuzz harnesses plus seed corpora
- [`capabilities/builtin-capability-manifest.json`](capabilities/builtin-capability-manifest.json):
  machine-readable built-in backend capabilities
- [`conformance/portable-subset.json`](conformance/portable-subset.json):
  public conformance corpus for the portable subset
- [`examples/host/README.md`](examples/host/README.md): host integration notes
- [`scripts/release.sh`](scripts/release.sh): staged release helper for local checks,
  crates.io publication order, and local tagging

## Fuzzing

Use the QueryFabric devshell so `cargo-fuzz` is available, then build or run the
targets from the `fuzz/` directory:

```bash
nix develop . -c bash
cd queryfabric/fuzz && cargo fuzz build parse_sql_no_panic
cd queryfabric/fuzz && cargo fuzz build bind_portable_no_panic
```

## Website and Docs

QueryFabric ships a standalone Codeberg Pages layout:

```bash
nix build .#docs
nix build .#site
plinth-project dev --config website/plinth-project.toml
cd docs && mdbook serve
```

## Status

This repository is ready for standalone iteration and publication. DataFusion
and other non-SQL emitters are intentionally deferred; the artifact seam is
kept open so those backends can be added without changing the stable facade.
Parser and binder fuzz harnesses live under [`fuzz/`](fuzz/README.md) and are
part of the release-quality gate.
