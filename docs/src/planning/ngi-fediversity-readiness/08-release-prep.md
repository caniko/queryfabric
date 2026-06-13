# Phase 08 — Prepare the v0.2.0 release

> **Recommended model: gpt-5.4-mini (codex) — effort `low`**
>
> Routed: `carter route -c moderate -r leaf -n coding -p codex`
> → `gpt-5.4-mini` / `low`
>
> Mechanical release hygiene with a scripted verifier
> (`scripts/release.sh check`) — moderate/leaf. The judgment calls (what goes
> in the CHANGELOG) are constrained by the merged phase commits; the risk at
> any tier is missing an entry, which the acceptance criteria cross-check
> against `git log`.
>
> Dispatch: `codex --model gpt-5.4-mini -c model_reasoning_effort=low`

## Working tree

`/data/nvme0/can/Projects/queryfabric` (this repo, branch off `trunk`).
**Prerequisite: phases 01–07 have all landed on `trunk`.** This is the
terminal phase of the plan set.

## Goal

This phase succeeds when the workspace is at 0.2.0 with a finalized CHANGELOG
and `scripts/release.sh check` exits 0 — everything staged so the maintainer
can run `scripts/release.sh publish --version 0.2.0 --execute` and
`scripts/release.sh tag --version 0.2.0` themselves.

## Why this matters now

The grant application converts "release process exists" into "project
releases" only if a tagged version actually ships before submission
(grant-readiness report §4, Tier 1 item 2; §9 checklist). CHANGELOG.md
currently has an Unreleased section (release.sh helper, crate READMEs,
multi_backend example, fuzzing docs) that phases 01–07 will have grown;
workspace version sits at 0.1.1 (`Cargo.toml` line 43).

## Out of scope

- **Do NOT publish to crates.io and do NOT push tags** — publication and
  tagging are maintainer actions (they require crates.io credentials and a
  deliberate point of no return). This phase prepares and verifies only.
- No version-policy changes (`COMPATIBILITY.md` stays as-is; 0.x minor may
  break per existing policy).
- No new features or fixes — if `release.sh check` surfaces a real defect,
  report it and stop rather than patching unrelated code in this phase.

## Plan

1. Branch from latest `trunk`; confirm phases 01–07 are merged
   (`git log --oneline -20` should show all of them).
2. **CHANGELOG.md**: convert `## Unreleased` into `## 0.2.0` (with date).
   Cross-check completeness against `git log v0.1.1..HEAD --oneline` (or
   since the 0.1.x release commit if no tag exists — check `git tag`):
   every user-visible change from phases 01–07 (narrative docs, REUSE
   compliance, roadmap/templates/accessibility, threat model, footprint
   docs, HA doc, multi-instance NixOS module) plus the pre-existing
   Unreleased entries. Group under Added/Changed per the existing style.
3. **Version bump**: workspace `version = "0.1.1"` → `"0.2.0"` in
   `Cargo.toml`; sweep for stragglers:
   `git grep -n '0\.1\.1' -- '*.toml' '*.nix' docs/ website/` and update
   intra-workspace dependency version requirements if they pin `0.1`.
   Run `cargo update --workspace` so `Cargo.lock` reflects the bump.
4. Run the full local gate: `cargo fmt --check`, `cargo clippy --workspace`,
   `cargo test --workspace`, and `scripts/release.sh check`. All must pass.
5. Run `nix flake check` (at least the fast gate) to confirm the Nix packages
   still build with the bumped version.
6. Verify the release order list in `scripts/release.sh` still matches the
   publishable crate set (new public crates since the list was written would
   silently miss publication — compare against `cargo metadata` workspace
   members with `publish != false`).
7. Commit (plain `git commit`, default signing). Leave publication and
   tagging commands in the commit/PR description for the maintainer:
   `scripts/release.sh publish --version 0.2.0 --execute` then
   `scripts/release.sh tag --version 0.2.0`.

## Acceptance criteria

- [ ] `CHANGELOG.md` has a dated `## 0.2.0` section and no `## Unreleased`
      content remains (or an empty Unreleased stub per house style).
- [ ] Every phase 01–07 user-visible deliverable appears in the 0.2.0 section
      (8 items minimum: narrative, REUSE, roadmap, templates+accessibility,
      threat model, footprint, HA doc, multi-instance module).
- [ ] `grep -n 'version = "0.2.0"' Cargo.toml` hits the workspace version;
      `git grep -n '0\.1\.1' -- '*.toml'` returns nothing.
- [ ] `scripts/release.sh check` exits 0.
- [ ] `cargo test --workspace` and `cargo clippy --workspace` exit 0.
- [ ] The release.sh crate order covers every publishable workspace member
      (state the comparison result in the commit message).
- [ ] No `git tag` created, nothing published.

## Files likely touched

- `CHANGELOG.md`
- `Cargo.toml` (workspace version, possibly workspace-dep version reqs)
- `Cargo.lock`
- possibly crate-level `Cargo.toml`s if any pin sibling versions explicitly

## Pitfalls

- **Stale sibling version pins.** Symptom: `release.sh check` dry-run fails
  with version mismatch. Cause: intra-workspace deps pinned `=0.1.1` or
  `0.1`. Recovery: the step-3 grep; align to `0.2.0` per the workspace
  pattern.
- **Missing CHANGELOG entries.** Symptom: a reviewer finds a merged phase
  absent from 0.2.0 notes. Recovery: the step-2 git-log cross-check is the
  guard; do it commit-by-commit, not from memory.
- **Publishing by accident.** Symptom: crates on crates.io. Cause: running
  `publish --execute` instead of `check`. Recovery: none — that's why this
  phase forbids it; `--execute` must not appear in anything this phase runs.
- **New unpublishable crate breaks the order list.** Symptom: `release.sh
  publish` (run later by maintainer) fails midway. Recovery: step 6 catches
  it now; fix the list in this phase, not during publication.

## Reference

- Grant-readiness report §4 (Tier 1 item 2), §9 checklist:
  `docs/grants/ngi-fediversity-application-plan.md`
- Plan set: `docs/src/planning/ngi-fediversity-readiness/README.md`
- Release process: `RELEASE.md`, `scripts/release.sh`, `COMPATIBILITY.md`
- Prerequisites: phases 01–07 (all files in this directory)
