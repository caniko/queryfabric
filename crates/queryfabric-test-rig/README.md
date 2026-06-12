# bollard-test-rig

Shared integration-test fixture helpers for Docker or rootless Podman.

## Reuse contract

- Reuse one `TestRig` per test binary via `OnceLock`.
- Reset shared backends at the start of each test or shared setup helper.
- Use OS-assigned ports by default. `with_*()` binds to `127.0.0.1:0` internally.
- Only pin ports with `with_*_on_port()` when a test must surface the port to another process.

## Reset helpers

- `PostgresService::truncate_all()` clears all tables in `public` and restarts identities.
- `ClickHouseService::reset_database("db")` truncates ordinary tables in a database.

These helpers are intended for shared-rig tests that write into a long-lived backend instead of using a per-test schema or database.
