# Release Process

Releases use `simit` as the authority for the publishable crate set, dependency
order, version bumps, changelog management, and tagging. CI (`simit init ci`)
runs per-crate gates on every push. QueryFabric is currently pre-release: this
document describes the intended procedure but does not claim that crates, a
signed tag, or a Codeberg release already exist.

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
   scripts/release.sh plan
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

6. CI is configured to publish crates to crates.io when it detects the tag. If
   that path is unavailable or fails, use the metadata-derived staged helper:

   ```bash
   scripts/release.sh publish --version <x.y.z> --execute
   # Resume after index propagation if needed:
   scripts/release.sh publish --version <x.y.z> --from <crate> --execute
   ```

7. Create the Codeberg release from the finalized changelog entry.

## Staged Publish Constraint

The workspace cannot promise a single "dry-run all crates" path before
publication. Dependent crates such as `queryfabric` cannot dry-run or publish
cleanly until earlier crates are visible on crates.io for the same version.

`simit release plan --workspace` is the sole source of the dependency order.
`scripts/release.sh` reads its JSON output at runtime and refuses unknown crate
names; it deliberately keeps no second hard-coded list.

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

Do not copy a numbered crate list into release documentation. Resolve and
review it from Cargo metadata through either command:

```bash
simit release plan --workspace
scripts/release.sh plan
```

For the current `0.2.0` workspace both commands must report exactly ten
publishable crates. Validate that fact before creating a release candidate:

```bash
test "$(simit release plan --workspace --json | jq 'length')" -eq 10
```
