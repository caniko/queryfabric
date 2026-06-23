# Crate Catalog

QueryFabric is composed of 35 crates. This page maps every crate so you know
which one to reach for and which ones to ignore as a host author.

## Facade (start here)

| Crate | `cargo add` | Purpose |
|-------|-------------|---------|
| `queryfabric` | `queryfabric` | Re-exports the stable public API: `QueryCompiler`, `parse`, `bind_and_validate`, `analyze`, `emit`, `MemoryCatalog`, `ClickHouseAdapter`, `PostgresAdapter`. This is the only crate host code needs to `use`. |

## Compiler core

Internal compiler pipeline crates. You rarely depend on these directly — the
facade re-exports what you need.

| Crate | Purpose |
|-------|---------|
| `queryfabric-ir` | Backend-neutral IR types: `ParsedQuery`, `BoundQuery`, `BoundExpr`, `DataType`, `ResultSchema`, `ProvenanceReceipt`. |
| `queryfabric-catalog` | Catalog and function registry traits + `MemoryCatalog` implementation. |
| `queryfabric-opt` | Optimization passes (federation scatter-gather planning, metadata rewrite infrastructure). |

## Dialects

| Crate | Purpose |
|-------|---------|
| `queryfabric-dialect-sql` | Generic SQL parser (PostgreSQL-flavoured). |
| `queryfabric-dialect-syql` | SyQL dialect parser — a curated SQL subset for scientific query APIs. |

## Backend adapters

| Crate | Purpose |
|-------|---------|
| `queryfabric-adapter-clickhouse` | ClickHouse backend: `ClickHouseAdapter`, `ClickHouseConfig`, `DynamicClient` (Arrow-native HTTP client with fallback retry), materialized-view aware SQL emission. |
| `queryfabric-adapter-postgres` | PostgreSQL backend: `PostgresAdapter`, SQL emission for PostgreSQL dialect. |

## Integration and deployment

| Crate | Purpose |
|-------|---------|
| `queryfabric-python` | Python bindings via PyO3. Same parse → bind → analyze → emit flow from Python. |
| `queryfabric-leptos` | Leptos (Rust WASM) components for SyQL editor widgets. |
| `queryfabric-web` | SyQL validation helpers and static JS assets for web UIs. |
| `queryfabric-demo` | Runnable self-host demonstrator — portable queries + data sovereignty over Postgres and S3. |

## Execution and runtime

| Crate | Purpose |
|-------|---------|
| `queryfabric-runtime` | Traits for execution backends (`ExecutionRuntime`, `IsolatedExecutionDriver`, `RecordBatchStream`). |
| `queryfabric-runtime-k8s` | Kubernetes `IsolatedExecutionDriver` — creates Jobs, streams Arrow Flight results, cleans up. |
| `queryfabric-worker` | One-shot Arrow Flight worker for isolated query execution. |
| `queryfabric-job-queue` | Priority job queue backed by thespis actors with cancellation, recovery, and Axum API routes. |

## Networking

| Crate | Purpose |
|-------|---------|
| `queryfabric-federation` | Multi-node federation: resource locality, schema sync, hub/cluster actor protocol. |
| `queryfabric-cluster` | Generic actor + libp2p cluster substrate: swarm bootstrap, DHT registry, health monitoring, routing. |
| `queryfabric-flight-pool` | Lock-free Arrow Flight connection pool. |
| `queryfabric-flight-cache` | Parquet file cache for Flight query results. |
| `queryfabric-tcp-tuned` | TCP listener with performance-tuned socket options. |
| `queryfabric-store` | S3-compatible object store via OpenDAL with presigned URLs. |
| `queryfabric-fetch` | Retrying parallel HTTP downloader with backoff. |

## Data sovereignty and portability

| Crate | Purpose |
|-------|---------|
| `queryfabric-access` | Access-control tiers and GDPR rights traits (access, rectification, erasure). |
| `queryfabric-portability` | Content-addressed export bundles, DOI minting, citation metadata. |
| `queryfabric-tenancy` | Multi-tenant accounts, collections, and groups. |
| `queryfabric-provenance` | Append-only provenance activity log. |
| `queryfabric-contract` | Neutral contract traits: `AccessDecision`, `DomainActivity`, `Subject`, `ResourceRef`. |

## Auth and session management

| Crate | Purpose |
|-------|---------|
| `queryfabric-paseto` | PASETO v4.local token validation, bearer extraction. |
| `queryfabric-session` | Browser cookie helpers. |
| `queryfabric-problem-details` | RFC 7807 Problem Details for HTTP APIs. |

## CLI and test tooling

| Crate | Purpose |
|-------|---------|
| `queryfabric-cli-toolbelt` | Arrow Flight client, auth store, K8s helpers, ClickHouse connection args, subprocess runner. |
| `queryfabric-cmd-runner` | Async subprocess runner with capture and tail truncation. MCP format helpers. |
| `queryfabric-test-rig` | Docker/Podman test harness: start Postgres, ClickHouse, MinIO; probe ports; resolve registry auth. |

## Utility crates

| Crate | Purpose |
|-------|---------|
| `queryfabric-content-hash` | Deterministic BLAKE3 directory content hashing. |
| `queryfabric-namespace-uuid` | Typed UUIDv5 namespace derivation. |
| `queryfabric-prom` | Prometheus histogram helpers and registry wrapper. |
| `queryfabric-seaorm-ext` | SeaORM utilities: `SharedDatabaseConnection`, `I16Vec`, `UuidVec`. |
| `queryfabric-types` | Validated string newtypes (`Email`, `Doi`, `CountryCode`) and portable enums (`UserType`, `OAuthProviderName`). |

## What to ignore as a host author

Most of these crates are internal compiler plumbing or infrastructure for the
demo/deployment side of the project. A host author only needs to depend on
`queryfabric` (the facade crate) and, optionally, the adapter crate for the
backend they target (e.g. `queryfabric-adapter-clickhouse`).

The remaining crates (`queryfabric-cluster`, `queryfabric-job-queue`,
`queryfabric-provenance`, etc.) are only needed when you're deploying the
QueryFabric self-hosted stack, not when embedding the compiler.
