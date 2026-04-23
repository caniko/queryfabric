# Compatibility Policy

## Public API Surface

The supported public API is the `queryfabric` facade crate.

The workspace leaf crates are published, but they are treated as implementation
modules rather than the primary compatibility boundary. New consumers should
prefer the facade unless they have a concrete reason to depend on internals.

## Semver and `0.x`

- QueryFabric follows semver.
- While the project remains in `0.x`, minor releases may include breaking
  changes.
- Breaking changes to the facade crate require a documented migration note in
  [`MIGRATION.md`](MIGRATION.md) and a changelog entry.
- Parser-internal dependencies such as `sqlparser` are not part of the stable
  promise.

## MSRV

- Minimum supported Rust version: `1.85`
- CI should run on MSRV and stable before release.
- CI also maintains a nightly fuzz-build lane that runs:
  `cd queryfabric/fuzz && cargo fuzz build parse_sql_no_panic`
  and
  `cd queryfabric/fuzz && cargo fuzz build bind_portable_no_panic`

## Backend Support Matrix

Built-in adapters at `0.1.0`:

- ClickHouse SQL emission
- PostgreSQL SQL emission

Portable subset support is declared by:

1. `BackendAdapter::capabilities()`
2. [`capabilities/builtin-capability-manifest.json`](capabilities/builtin-capability-manifest.json)
3. [`conformance/portable-subset.json`](conformance/portable-subset.json)

## Deprecation

- Public facade APIs should carry at least one minor release of deprecation
  before removal where feasible.
- Diagnostics codes and provenance field names should remain stable once
  published unless a correctness issue requires a change.
