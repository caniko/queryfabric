# Contributing

Thank you for considering a contribution to QueryFabric. This guide covers the
development workflow, design principles, and review expectations.

## Design Principles

QueryFabric is kept intentionally neutral:

- **Host code stays out of core.** Routing, auth, execution, and
  product-specific metadata resolution belong outside the `queryfabric-*`
  crates. The host boundary is the point of the project.
- **Backend code goes in adapter crates.** ClickHouse-specific logic lives in
  `queryfabric-adapter-clickhouse`, not in the facade or IR crates.
- **Generic examples in the facade crate.** Host-specific examples go under
  `examples/host/` — they are not part of the public promise.
- **Prefer traits over feature flags.** A new capability should be a trait that
  hosts can implement, not a Cargo feature that gates conditional compilation.

## Development Setup

### With Nix (recommended)

```bash
nix develop
```

This provides the exact toolchain, `cargo-fuzz`, and all system dependencies.

### Without Nix

You need:

- Rust 1.88+ (check `rust-version` in `Cargo.toml`)
- A running ClickHouse or PostgreSQL instance for integration tests (optional)

```bash
cargo check --workspace
cargo test --workspace
```

## Running Tests

```bash
# All tests (fast — most are unit tests with no external deps)
cargo test --workspace

# Specific crate
cargo test -p queryfabric-catalog

# Integration tests (require Docker/Podman for ClickHouse + Postgres)
cargo test -p queryfabric-test-rig -- --ignored
```

### Fuzz targets

```bash
cd fuzz
cargo fuzz build parse_sql_no_panic
cargo fuzz build bind_portable_no_panic
```

## Workspace Layout

```
crates/
├── queryfabric/              # Public facade crate (depends on everything)
├── queryfabric-ir/           # Internal IR types
├── queryfabric-catalog/      # Catalog + function registry
├── queryfabric-opt/          # Optimization passes
├── queryfabric-dialect-*/    # Parsers
├── queryfabric-adapter-*/    # Backend adapters
├── queryfabric-runtime/      # Execution runtime traits
├── queryfabric-runtime-k8s/  # K8s execution driver
├── queryfabric-worker/       # One-shot Flight worker
├── queryfabric-contract/     # Neutral contract traits
├── queryfabric-access/       # GDPR access control
├── queryfabric-portability/  # Export bundles
├── queryfabric-tenancy/      # Multi-tenancy
├── queryfabric-provenance/   # Provenance log
├── queryfabric-federation/   # Federated query layer
├── queryfabric-cluster/      # libp2p cluster substrate
├── queryfabric-cli-toolbelt/ # CLI helpers
├── queryfabric-cmd-runner/   # Subprocess runner
├── queryfabric-test-rig/     # Test infrastructure
├── queryfabric-web/          # Web UI assets
├── queryfabric-leptos/       # Leptos SyQL editor
├── queryfabric-python/       # Python bindings
├── queryfabric-demo/         # Self-host demonstrator
├── queryfabric-paseto/       # Auth tokens
├── queryfabric-session/      # Session cookies
└── queryfabric-*/            # Utility crates (content-hash,
                              # namespace-uuid, prom, seaorm-ext,
                              # types, etc.)
```

See the [Crate Catalog](../integration/crate-catalog.md) for the full list.

## Pull Request Workflow

1. **Open an issue first** for non-trivial changes. This avoids wasted work if
   the change conflicts with the project direction.
2. **Keep commits atomic.** Each commit should be a single logical change.
   Prefer `git commit --amend` over fixup commits during review.
3. **Run the full test suite** before pushing:
   ```bash
   cargo test --workspace
   cargo clippy --all-targets -- -D warnings
   ```
4. **Update docs.** User-facing API changes must update the relevant
   `docs/src/` page and the facade crate's doc comments.
5. **Changelog.** Add an entry under `[Unreleased]` in `CHANGELOG.md`.

## Release Process

Release follows a staged script at `scripts/release.sh`:

1. Run `scripts/release.sh check` — verifies fmt, clippy, test, doc, package.
2. Run `scripts/release.sh publish` — publishes crates in dependency order.
3. Push the tag created by the script.

The release is driven by the `main` maintainer. External contributors do not
need to worry about this step.

## Code Review

Reviewers check for:

- **Neutrality.** Does the change introduce host-specific logic into a generic
  crate?
- **Composability.** Is the new API a trait, not a flag?
- **Test coverage.** Are there tests for the new behaviour?
- **Documentation.** Are public items documented with examples?

## Questions?

Open a discussion issue on Codeberg, or reach out through the project's
community channels listed in the README.
