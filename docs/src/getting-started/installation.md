# Installation

QueryFabric currently supports two practical ways to start:

1. use the local workspace directly
2. consume the facade crate from a path dependency during the pre-publish phase

## Requirements

- Rust `1.85` or newer
- Python `3.11` or newer if you want the Python bindings
- Nix if you want the repo-local dev shell with `zola`, `mdbook`, and
  `cargo-fuzz`

## Rust Workspace

Clone the repository and run the standard checks:

```bash
cargo test --workspace --all-targets
cargo run --manifest-path crates/queryfabric/Cargo.toml --example quickstart
cargo run --manifest-path crates/queryfabric/Cargo.toml --example multi_backend
```

## Path Dependency

Before crates.io publication, consume QueryFabric directly from a local checkout:

```toml
[dependencies]
queryfabric = { path = "../queryfabric/crates/queryfabric" }
```

That gives downstream hosts the stable facade crate without tying them to the
internal crate graph.

## Python Bindings

The Python package name is `queryfabric`. From the repo root:

```bash
cd packages/queryfabric
maturin develop
pytest
```

The Python surface mirrors the same facade-first stages: parse, inspect
parameters, bind, analyze, and emit.

## Nix Dev Shell

The repo ships a standalone `flake.nix` for local development:

```bash
nix develop
```

That shell includes:

- Rust toolchain and `cargo-fuzz`
- `zola` for the landing site
- `mdbook` for the documentation site
- `maturin` and Python for the bindings

Local site commands:

```bash
cd website && zola serve
cd docs && mdbook serve
```
