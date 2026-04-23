# queryfabric

Stable facade crate for QueryFabric's portable query compiler.

This crate re-exports the supported public API:

- dialect parsing into `ParsedQuery`
- strict catalog-aware binding into `BoundQuery`
- normalization via `OptimizationPipeline`
- backend analysis via `BackendAdapter::analyze`
- backend-normalized emission into typed `EmitArtifact` values

Examples:

- `examples/quickstart.rs`: shortest parse-bind-analyze-emit path
- `examples/multi_backend.rs`: bind once, then compare ClickHouse and PostgreSQL
  analysis and SQL emission from the same portable query

For repository-level docs, release workflow, and fuzzing instructions, see the
workspace root at <https://codeberg.org/caniko/queryfabric>.
