# queryfabric-catalog

Catalog, function-registry, analysis, and artifact contracts for QueryFabric.

This crate defines the neutral `Catalog` and `BackendAdapter` seams, the
in-memory catalog used by examples and tests, and the typed SQL artifact
surface.

It is public for advanced integrations, but the normal entry point is the
[`queryfabric`](https://docs.rs/queryfabric) facade crate.
