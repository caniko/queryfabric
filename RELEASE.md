# Release Process

Releases use `simit` for version bumps, changelog management, tagging, and
publish order planning. CI (`simit init ci`) runs per-crate gates on every push.

## Release Flow

1. Update [`CHANGELOG.md`](CHANGELOG.md), [`COMPATIBILITY.md`](COMPATIBILITY.md),
   and [`MIGRATION.md`](MIGRATION.md) as needed.

2. Run the staged local release gate:

   ```bash
   nix flake check
   scripts/release.sh check
   ```

3. Preview the dependency-ordered publish plan:

   ```bash
   simit release plan --workspace
   ```

4. Bump versions, commit, and tag with a single command:

   ```bash
   simit release patch --workspace -m "Prepare release v0.2.1"
   ```

   This runs local release checks, promotes the CHANGELOG, bumps all publishable
   workspace crates, commits, and creates a signed annotated tag. Use `minor` or
   `major` instead of `patch` for larger bumps. Use `--dry-run` to preview
   without changing files.

5. Push the release commit and tag:

   ```bash
   git push --follow-tags
   ```

6. CI publishes crates to crates.io automatically when it detects the tag. If
   automatic publish fails, publish manually:

   ```bash
   simit release patch --workspace --no-tag --no-changelog
   cargo publish -p <crate>
   ```

7. Create the Codeberg release from the finalized changelog entry.

## Staged Publish Constraint

The workspace cannot promise a single "dry-run all crates" path before
publication. Dependent crates such as `queryfabric` cannot dry-run or publish
cleanly until earlier crates are visible on crates.io for the same version.

`simit release plan --workspace` shows the correct dependency order. To resume
a partial publish, publish the remaining crates individually with `cargo publish`.

## What `check` Runs

`scripts/release.sh check` runs the non-publishing release gate from the repo
root:

1. `cargo fmt --all --check`
2. `cargo fmt --manifest-path fuzz/Cargo.toml --all --check`
3. `cargo clippy --workspace --all-targets -- -D warnings`
4. `cargo test --workspace --all-targets`
5. `cd queryfabric/fuzz && cargo fuzz build --sanitizer none parse_sql_no_panic`
6. `cd queryfabric/fuzz && cargo fuzz build --sanitizer none bind_portable_no_panic`
7. `cargo build --manifest-path crates/queryfabric/Cargo.toml --examples`

Manual release review should also verify:

- [`capabilities/builtin-capability-manifest.json`](capabilities/builtin-capability-manifest.json)
- [`conformance/portable-subset.json`](conformance/portable-subset.json)
- a short `cargo fuzz run` session for both targets
- green CI runs (per-crate CI + msrv + fuzz + audit + deny)

## Publishing Order

Resolved by `simit release plan --workspace` at release time. As of 0.2.0:

1. `queryfabric-changelog`
2. `queryfabric-cli-toolbelt`
3. `queryfabric-cmd-runner`
4. `queryfabric-contract`
5. `queryfabric-ir`
6. `queryfabric-dialect-sql`
7. `queryfabric-catalog`
8. `queryfabric-adapter-postgres`
9. `queryfabric-dialect-syql`
10. `queryfabric-runtime`
11. `queryfabric-adapter-clickhouse`
12. `queryfabric-opt`
13. `queryfabric`
14. `queryfabric-runtime-k8s`
15. `queryfabric-seaorm-ext`
16. `queryfabric-test-rig`
17. `queryfabric-types`
18. `queryfabric-worker`
