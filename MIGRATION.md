# Migration Guide

## From host-internal query usage to standalone QueryFabric

The old host-internal flow mixed parsing, metadata semantics, and
ClickHouse-specific compilation. QueryFabric splits the portable parts out.

## Old Shape

Typical host-internal usage looked like:

1. parse SyQL
2. validate against host-specific rules
3. resolve metadata and planning through host storage
4. compile to ClickHouse SQL

## New Shape

The standalone portable flow is:

1. `Dialect::parse(&str) -> ParsedQuery`
2. `bind_and_validate(parsed, catalog, params) -> BoundQuery`
3. `BackendAdapter::analyze(bound, catalog) -> BackendAnalysis`
4. `BackendAdapter::emit(bound, catalog) -> EmitArtifact`

## Key Boundary Changes

- `SCOPE` and `DOWNLOAD` are no longer neutral core fields.
  They survive as SyQL dialect metadata.
- `sqlparser` AST types are not exposed through the stable facade.
- typed parameters and result schemas are first-class public contracts
- provenance receipts are attached to analysis and emission

## Host Responsibilities That Stay Outside QueryFabric

- metadata resolution against PostgreSQL
- backend routing policy
- auth, access control, and job orchestration
- federation execution
- metadata-driven relation routing and execution policy

ClickHouse materialized-view aggregate wrapping and near-miss advisories now
live in `queryfabric-adapter-clickhouse`, driven by neutral catalog metadata.

## Examples

- [`crates/queryfabric/examples/quickstart.rs`](crates/queryfabric/examples/quickstart.rs):
  shortest parse-bind-analyze-emit flow
- [`crates/queryfabric/examples/multi_backend.rs`](crates/queryfabric/examples/multi_backend.rs):
  preferred host-oriented example showing one bound portable query analyzed and
  emitted against both ClickHouse and PostgreSQL
