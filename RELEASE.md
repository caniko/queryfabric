# Release Process

QueryFabric releases are performed manually from `trunk`. CI verifies the
documented gates, but crates.io publication, tag pushes, and the Codeberg
release are intentionally local, explicit steps.

## Release Flow

1. Update [`CHANGELOG.md`](CHANGELOG.md), [`COMPATIBILITY.md`](COMPATIBILITY.md),
   and [`MIGRATION.md`](MIGRATION.md) as needed.
2. Run the staged local release gate:

   ```bash
   scripts/release.sh check
   ```

3. Rehearse the current publishable step without pushing anything:

   ```bash
   scripts/release.sh publish --version <x.y.z>
   ```

   This non-executing rehearsal uses `cargo publish --dry-run --allow-dirty`
   for the current staged step so local release prep can be validated before the
   final commit is in place.

4. Publish crates in dependency order from `trunk`:

   ```bash
   scripts/release.sh publish --version <x.y.z> --execute
   ```

5. Wait for crates.io propagation between crates. The publish script polls
   crates.io visibility after each successful publish before moving to the next
   crate.
6. If a publish is interrupted, resume at the next staged step:

   ```bash
   scripts/release.sh publish --version <x.y.z> --from <crate> --execute
   ```

7. Create the local annotated tag:

   ```bash
   scripts/release.sh tag --version <x.y.z>
   ```

8. Push `trunk`.
9. Push the release tag.
10. Create the Codeberg release from the finalized changelog entry.

## Staged Publish Constraint

The workspace cannot promise a single "dry-run all crates" path before
publication. Dependent crates such as `queryfabric` cannot dry-run or publish
cleanly until earlier crates are visible on crates.io for the same version.

That is a real staged-publication constraint, not a repository bug.

The release helper handles this by:

- validating the current publishable step with `cargo publish --dry-run`
- publishing only when `--execute` is passed
- polling crates.io visibility before continuing
- supporting `--from <crate>` so interrupted releases can resume cleanly

## What `check` Runs

`scripts/release.sh check` runs the non-publishing release gate from the repo
root:

1. `cargo fmt --all --check`
2. `cargo fmt --manifest-path fuzz/Cargo.toml --all --check`
3. `cargo clippy --workspace --all-targets -- -D warnings`
4. `cargo test --workspace --all-targets`
5. `cd queryfabric/fuzz && cargo fuzz build parse_sql_no_panic`
6. `cd queryfabric/fuzz && cargo fuzz build bind_portable_no_panic`
7. `cargo build --manifest-path crates/queryfabric/Cargo.toml --examples`

Manual release review should also verify:

- [`capabilities/builtin-capability-manifest.json`](capabilities/builtin-capability-manifest.json)
- [`conformance/portable-subset.json`](conformance/portable-subset.json)
- a short `cargo fuzz run` session for both targets
- green stable, MSRV, and fuzz CI runs

## Publishing Order

Crates are published in this order:

1. `queryfabric-ir`
2. `queryfabric-catalog`
3. `queryfabric-opt`
4. `queryfabric-dialect-sql`
5. `queryfabric-dialect-syql`
6. `queryfabric-adapter-clickhouse`
7. `queryfabric-adapter-postgres`
8. `queryfabric`

The facade crate is published last because it depends on the full public leaf
set.
