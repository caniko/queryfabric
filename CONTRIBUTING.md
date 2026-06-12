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
