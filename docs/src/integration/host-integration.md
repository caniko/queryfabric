# Host Integration

QueryFabric is built for hosts that need a trustworthy query compiler boundary.

The intended embedding model is:

1. build or expose a neutral catalog
2. parse query text with the chosen dialect
3. bind and validate against the catalog
4. analyze candidate backends
5. let the host choose a backend
6. emit an artifact
7. execute outside QueryFabric

## Why This Shape Matters

Scientific platforms usually need more than SQL text generation. They need:

- typed schemas before execution
- stable diagnostics for UI and notebook tooling
- reproducibility metadata
- explicit backend support decisions

QueryFabric gives that surface without taking ownership of product policy.

## What the Host Still Owns

- authorization
- execution routing
- queueing and job lifecycle
- metadata-specific rewrites
- backend connection management

That split is what keeps the compiler reusable across platforms instead of
turning it into a thin layer over one application.

## Recommended Integration Pattern

For a host with multiple backends:

1. parse and bind once
2. analyze every backend adapter you are willing to consider
3. choose using host policy
4. emit once for the chosen adapter
5. persist provenance alongside the emitted artifact

The host should not depend on parser internals or adapter-private crate types.
Use the stable `queryfabric` facade as the contract.

## TLS Crypto Provider

The crates that make HTTP requests (`queryfabric-adapter-clickhouse`,
`queryfabric-store` with `s3`, `queryfabric-fetch`, `queryfabric-changelog`,
`queryfabric-cli-toolbelt`, `queryfabric-web` with `ssr`,
`queryfabric-test-rig`) build reqwest with `rustls-no-provider`, so they do not
force aws-lc-rs and its C build on the host. The host picks the crypto provider: install
one at the start of `main`, before any client is built, or reqwest panics with
"No rustls crypto provider is configured".

```rust,ignore
rustls::crypto::ring::default_provider()
    .install_default()
    .expect("install the rustls crypto provider");
```

Depend on `rustls = { version = "0.23", default-features = false, features =
["ring", "std"] }` for this. Tests that build clients need the same call; use
`let _ = ...install_default();` there, since tests in one process share the
installed provider. `queryfabric-test-rig` installs ring itself when no
provider is set.
