# Contributing

This QueryFabric workspace is kept intentionally neutral:

- Avoid host-specific public symbols in the `queryfabric-*` crates.
- Keep backend-specific behavior inside adapter crates.
- Keep host-specific behavior such as routing, auth, jobs, and access control
  outside QueryFabric.
- Prefer generic examples in crate docs and the facade crate; keep host notes
  isolated under `examples/host`.
- Update [`conformance/portable-subset.json`](conformance/portable-subset.json)
  when changing the verified subset.
- Keep the capability manifest and release notes in sync with code changes.

## Your first contribution

If you are looking for a first issue, these are small, concrete places to start:

- Add one more portable subset example or explanation in
  [`crates/queryfabric-catalog/README.md`](crates/queryfabric-catalog/README.md).
- Add a usage example or expansion note in
  [`crates/queryfabric-dialect-sql/README.md`](crates/queryfabric-dialect-sql/README.md).
- Add a short dialect note or example in
  [`crates/queryfabric-dialect-syql/README.md`](crates/queryfabric-dialect-syql/README.md).
- Add a backend capability example in
  [`crates/queryfabric-adapter-clickhouse/README.md`](crates/queryfabric-adapter-clickhouse/README.md).
- Add a PostgreSQL-specific example or caveat in
  [`crates/queryfabric-adapter-postgres/README.md`](crates/queryfabric-adapter-postgres/README.md).
- Expand the host-integration guidance in
  [`examples/host/README.md`](examples/host/README.md).
- Tighten the test-rig instructions in
  [`crates/queryfabric-test-rig/README.md`](crates/queryfabric-test-rig/README.md).
- Add one conformance case or comment to
  [`conformance/portable-subset.json`](conformance/portable-subset.json).
