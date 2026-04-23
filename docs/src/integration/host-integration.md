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
