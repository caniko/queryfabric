# Changelog

## Unreleased

### Added

- vendored libp2p (v0.56.0, typed-builder fork) and rs-thespis (v0.19.2) under `vendor/`
- Plinth project-site definition replacing Zola website at `website/plinth-project.toml`
- Forgejo Pages deployment workflow at `.forgejo/workflows/pages.yaml`
- `plinth` flake input and restructured `docs`/`site` derivations
- grant pre-application readiness phase plan under `docs/src/planning/grant-preapplication-readiness/`
- claim evidence map and ideal project set at `docs/grants/`
- pre-commit hook config at `nix/pre-commit.nix`

### Changed

- Website migrated from Zola to Plinth project-site
- Cargo.toml: wire vendored deps via `[patch]` sections, bump pyo3/pythonize to 0.29
- Nix build: replace `website` (Zola) derivation with separate `docs` (mdBook) and `site` (Plinth) packages
- README, installation docs, and narrative bridge updated to reference Plinth
- Resource footprint benchmark numbers refreshed from local rerun
- NixOS module: harden instance `enable` checks with explicit attribute-or-false

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
