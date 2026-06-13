# Changelog

## Unreleased

## 0.2.0 - 2026-06-13

### Added

- self-hosting narrative, high-availability guidance, and threat model docs
- public roadmap, issue templates, and accessibility statement
- REUSE/SPDX licensing compliance with CI enforcement
- multi-instance support in the QueryFabric NixOS module (`services.queryfabric.instances.<name>`)
- reproducible footprint benchmark and deployment sizing docs
- crate-local README files and crates.io metadata for the public secondary crates
- `multi_backend.rs` example showing bind-once, analyze-many, emit-many usage
- aligned fuzzing docs around `cd queryfabric/fuzz && cargo fuzz build <target>`
- staged `scripts/release.sh` helper for local release checks, ordered crates.io
  publication, and local tagging

### Changed

- finalized the release notes for the 0.2.0 workspace cut

## 0.1.0

### Added

- stable `queryfabric` facade crate
- parser-agnostic public `ParsedQuery` and `BoundQuery` contracts
- typed result schemas, parameter schemas, diagnostics, and provenance receipts
- catalog and function-registry contracts with backend mappings
- ClickHouse and PostgreSQL adapters for the verified portable subset
- runnable quickstart, compatibility policy, migration guide, capability manifest,
  and conformance corpus
