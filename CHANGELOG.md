# Changelog

## [Unreleased]

### Added

- CI workflows for `queryfabric-release` crate, PyPI publishing, and release orchestration
- `release.yml` — tag-pushed release workflow with smoke checks and crate publication
- Documentation: new mdBook config (`docs/book.toml`), narrative pages (Why QueryFabric?, user scenarios, tutorials, contributing guide, crate catalog, CLI tooling, custom adapter, Docker deployment), and reorganized sidebar navigation
- `README.md` — 'Who is this for?' audience section and links to crate catalog and user scenarios
- `CONTRIBUTING.md` — cross-reference to the new contributing guide in the documentation
- `queryfabric-runtime::util` — `spawn_traced` panic-safe background task spawner
- `queryfabric-adapter-clickhouse::arrow` — `clickhouse_arrow_safe_sql` for Arrow-compatible SQL wrapping
- `queryfabric-adapter-clickhouse::types` — `ChType`, `SimpleColumnType`, `ChType::to_arrow()` type mapping
- `queryfabric-adapter-clickhouse::driver` — `ClickHouseConfig`, `ClickHouseError`, `DynamicClient` with fallback-host retry and Arrow IPC streaming
- `queryfabric-adapter-clickhouse::cost` — ClickHouse cost model integration
- `queryfabric-cli-toolbelt::auth` — `AuthStore`, `load_auth`, `save_auth`, `load_auth_token`
- `queryfabric-cli-toolbelt::clickhouse` — `ClickHouseConnArgs` with env-var defaults
- `queryfabric-cli-toolbelt::flight` — `FlightClient` with `do_get` (feature-gated)
- `queryfabric-cli-toolbelt::k8s` — K8s resource types, kubectl helpers, `parse_quantity`
- `queryfabric-test-rig::probe` — `wait_for_tcp_port`, `WaitConfig`
- `queryfabric-test-rig::constants` — default image tags and test credentials
- `queryfabric-test-rig::docker_auth` — `resolve_registry_auth` for Docker credentials
- `queryfabric-test-rig::clickhouse` — multi-node ClickHouse test helpers (`cluster_xml`, `execute_ch`, `split_ddl_statements`)
- `queryfabric-cmd-runner::mcp` — `format_result` for MCP `CallToolResult` conversion
- `queryfabric-web::ssr` — `SsrSettings` with env-prefix parameterization and `ApiClient` SSR proxy (feature-gated)
- `queryfabric-runtime-k8s` crate — Kubernetes isolated execution driver with configurable label keys
- `queryfabric-seaorm-ext` crate — `SharedDatabaseConnection`, `I16Vec`, `UuidVec`
- `queryfabric-types` crate — validated string newtypes (`Email`, `Doi`, `CountryCode`, etc.) and portable enums (`UserType`, `OAuthProviderName`)
- `queryfabric-worker` crate — one-shot Arrow Flight worker with `QueryExecutor` trait
- `queryfabric-changelog` crate — multi-ecosystem changelog compiler with git diff parsers

### Changed

- Upgrade OpenDAL dependencies to 0.58
- Upgrade Apache Arrow, Arrow Flight, and Parquet dependencies to 59.1.0
- CI: migrate all workflow runners from Codeberg shared runners to self-hosted `atlas` with Nix-based tooling
- Publish: migrate all crate publish workflows to Nix-based atlas runner with flake checks and audit/deny gates
- Flake: add `rust-overlay` input and pin input-follows for `plinth`, `treefmt-nix`, and `git-hooks`
- `simit.toml`: enable Nix runtime, PyPI publishing, nextest, and release GPG signing
- README: update CI badge from `managed+extra` to `drift`
- `queryfabric-runtime` Cargo.toml: add `tokio` and `tracing` deps
- `queryfabric-adapter-clickhouse` Cargo.toml: add `secrecy`, `serde_json`, `thiserror` deps
- `queryfabric-cli-toolbelt`: add `flight` and `polars` features
- `queryfabric-cmd-runner`: add `rmcp` dependency for MCP support

### Fixed

- `queryfabric-adapter-clickhouse::driver` — replace 3 `.expect()` in retry loop with safe error propagation

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

### Added

- REUSE.toml: add `.envrc` and `article/` to SPDX path annotations

### Changed

- CI: regenerate Forgejo Pages workflow under simit management with domain validation and `atlas` runner
- simit.toml: add `[ci.pages]` section with repo and canonical domain config
- Website: refresh plinth-project site content with updated nav, features, quick-start steps, and coverage trust panel

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
