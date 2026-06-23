# Phase 04 — Prove the Release

> **Recommended model: gpt-5.3-codex (codex) — effort `medium`**
>
> Routed: `carter route -c complex -r subagent --needs coding`
> → `gpt-5.3-codex` / `medium`
>
> Complex release engineering: this phase runs the full release gate, handles
> crate publication sequencing, and verifies the tag. A weaker model may treat
> a version bump as equivalent to a release or skip staged crates.io behavior.

## Working tree

`/data/nvme0/can/Projects/queryfabric`. Prerequisites: Phase 01 evidence map
exists and Phase 02 REUSE validation is green.

## Goal

This phase succeeds when the release claims in the application are backed by a
passing release check and either a published/tagged `v0.2.0` release or an
explicit evidence-map note saying publication/tagging did not happen and why.

## Why this matters now

`Cargo.toml` is already at `0.2.0`, but no local `v*` tag was found. The grant
application must not imply a published release if only local release prep
exists.

## Out of scope

- Do not bypass failing tests.
- Do not publish crates without maintainer credentials and intent.
- Do not create unsigned tags or override Git signing settings.
- Do not implement unrelated fixes beyond release blockers.

## Plan

1. Confirm workspace state:
   ```bash
   git status --short
   git tag --list 'v*'
   ```
2. Run the full release gate:
   ```bash
   nix develop -c scripts/release.sh check
   ```
3. If the gate fails, fix only defects required for the release check, then
   re-run the command. If a foundational dependency is unavailable, stop and
   report the missing artifact, upstream producer, regeneration workflow, and
   validation command.
4. If maintainer credentials are available and the release is intended, run:
   ```bash
   scripts/release.sh publish --version 0.2.0 --execute
   scripts/release.sh tag --version 0.2.0
   git tag --list 'v0.2.0'
   ```
5. Record crates.io URLs, tag URL, and command output summary in
   `docs/grants/claim-evidence-map.md`.
6. If publication/tagging is intentionally deferred, remove or downgrade
   application claims that require a public release.

## Acceptance criteria

- [ ] `nix develop -c scripts/release.sh check` exits 0, or the blocker is
      documented with an upstream producer and validation command.
- [ ] `git tag --list 'v0.2.0'` returns the tag if the application claims a
      tagged release.
- [ ] crates.io URLs are recorded if the application claims publication.
- [ ] Evidence map reflects the exact release state.
- [ ] No publish or tag command was run with signing disabled.

## Files likely touched

- Release-blocking code or metadata only if the release check fails.
- `docs/grants/claim-evidence-map.md`
- `docs/grants/ngi-fediversity-application-answers.md` only if release claims
  must be downgraded.

## Pitfalls

- **Version prep mistaken for release.** Symptom: `Cargo.toml` says `0.2.0`
  but no tag exists. Cause: release prep already landed. Recovery: publish/tag
  or downgrade claims.
- **Staged publication failure.** Symptom: downstream crate dry-run fails
  because prior crate is not on crates.io. Cause: expected workspace publish
  ordering. Recovery: follow `scripts/release.sh publish --from <crate>`.
- **Unavailable credentials.** Symptom: crates.io publish fails auth. Cause:
  missing maintainer token. Recovery: stop and report; do not fabricate proof.

## Reference

- `scripts/release.sh`
- `CHANGELOG.md`
- `docs/grants/ideal-project-set.md`, Project 3.
