# Fuzzing

QueryFabric ships two `cargo-fuzz` harnesses:

- `parse_sql_no_panic`: exercises the generic SQL and SyQL parse entrypoints
- `bind_portable_no_panic`: exercises parse-plus-bind against a portable in-memory catalog

The corpus is seeded from:

- the public portable subset corpus in `conformance/portable-subset.json`
- the SyQL differential examples
- malformed SQL and directive-heavy edge cases

## Usage

Enter the QueryFabric devshell from the repository root so `cargo-fuzz` is on `PATH`:

```bash
nix develop . -c bash
```

Then run the fuzz commands from the `fuzz/` directory:

```bash
cd queryfabric/fuzz && cargo fuzz build parse_sql_no_panic
cd queryfabric/fuzz && cargo fuzz build bind_portable_no_panic
cd queryfabric/fuzz && cargo fuzz run parse_sql_no_panic corpus/parse_sql_no_panic
cd queryfabric/fuzz && cargo fuzz run bind_portable_no_panic corpus/bind_portable_no_panic
```

The release process requires successful `cargo fuzz build` for both targets and
a short manual fuzzing session before publication.
