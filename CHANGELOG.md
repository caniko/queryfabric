# Changelog

## Unreleased

### Added

- staged `scripts/release.sh` helper for local release checks, ordered crates.io
  publication, and local tagging
- crate-local README files and crates.io metadata for the public secondary crates
- `multi_backend.rs` example showing bind-once, analyze-many, emit-many usage
- aligned fuzzing docs around `cd queryfabric/fuzz && cargo fuzz build <target>`

## 0.1.0

### Added

- stable `queryfabric` facade crate
- parser-agnostic public `ParsedQuery` and `BoundQuery` contracts
- typed result schemas, parameter schemas, diagnostics, and provenance receipts
- catalog and function-registry contracts with backend mappings
- ClickHouse and PostgreSQL adapters for the verified portable subset
- runnable quickstart, compatibility policy, migration guide, capability manifest,
  and conformance corpus
- threat model documentation
