# Phase 02 — Make the repository REUSE-compliant

## Working tree

`/data/nvme0/can/Projects/queryfabric` (this repo). **Prerequisite: phase 01
must have landed** — this phase edits the post-narrative README and must cover
all files created by wave-0 phases; rebase onto latest `trunk` before starting.

## Goal

This phase succeeds when `reuse lint` exits 0 on a clean checkout, CI enforces
it, and the README carries the REUSE-compliance badge.

## Why this matters now

The repo currently has zero SPDX markers (`grep -r SPDX` returns nothing, no
`.reuse/` directory, no `LICENSES/`). The NGI/NLnet ecosystem treats REUSE
compliance as table stakes for funded projects, and the grant-readiness report
(`docs/grants/ngi-fediversity-application-plan.md` §4, Tier 1 item 1) lists it
as a pre-submission gate. It is the cheapest credible openness signal the
application can point at.

## Out of scope

- No license *change* — the project is Apache-2.0 (`LICENSE` at repo root);
  this phase only annotates.
- No relicensing decisions for vendored or generated content beyond annotation.
- No CHANGELOG entry beyond one line under Unreleased (phase 08 finalizes).
- No other CI changes (don't touch the stable/MSRV/fuzz lanes).

## Plan

1. Rebase onto `trunk` (post-phase-01).
2. Add the `reuse` tool to the dev shell (`flake.nix` devShell:
   `pkgs.reuse`) so the check is reproducible.
3. Create `LICENSES/Apache-2.0.txt` (canonical text via `reuse download
   Apache-2.0`).
4. Prefer **`REUSE.toml` with glob annotations** over per-file headers to keep
   the diff small and avoid conflicting with concurrently running phases:
   annotate `crates/**`, `docs/**`, `website/**`, `nix/**`, `scripts/**`,
   `examples/**`, `conformance/**`, `fuzz/**`, `packages/**`, `capabilities/**`
   and root files as `Apache-2.0`, copyright the QueryFabric maintainers.
   Exclude build outputs (`target/`, `result`) — they are git-ignored but be
   explicit if `reuse lint` complains in dirty trees.
5. Decide per-file headers only for new top-level source entry points if the
   maintainer later wants them; do not sweep headers across 35 crates in this
   phase.
6. Run `reuse lint` until clean. Treat `Cargo.lock`, `flake.lock`, and binary
   assets via `REUSE.toml` annotations (license `Apache-2.0`, or `CC0-1.0`
   for lockfiles if preferred — pick one and state it in the commit message).
7. Add a CI step to `.forgejo/workflows/ci.yml` in the stable lane (or a tiny
   separate job): run `reuse lint` (via `nix develop -c reuse lint` or a
   pinned container — match how the existing lanes provision tools).
8. Add the badge to README under the title:
   `[![REUSE status](https://api.reuse.software/badge/<repo-url>)](https://api.reuse.software/info/<repo-url>)`
   — derive `<repo-url>` from `Cargo.toml`'s workspace `repository` field; if
   the repo isn't registered with api.reuse.software yet, use a plain
   "REUSE compliant" badge and note registration as a maintainer follow-up.
9. One line under `## Unreleased` in `CHANGELOG.md`: "REUSE/SPDX licensing
   compliance with CI enforcement".
10. Commit (plain `git commit`, default signing).

## Acceptance criteria

- [ ] `reuse lint` exits 0 on a clean checkout.
- [ ] `LICENSES/Apache-2.0.txt` exists; `REUSE.toml` covers every tracked file.
- [ ] `.forgejo/workflows/ci.yml` contains a `reuse lint` step and the
      workflow YAML parses (the other lanes are untouched in the diff).
- [ ] README shows the REUSE badge in the first 10 lines.
- [ ] `nix develop -c reuse --version` works (tool in dev shell).
- [ ] CHANGELOG Unreleased section gained exactly one line.

## Files likely touched

- `REUSE.toml` (new)
- `LICENSES/Apache-2.0.txt` (new)
- `flake.nix` (devShell package list)
- `.forgejo/workflows/ci.yml`
- `README.md` (badge line only — phase 01 owns the prose)
- `CHANGELOG.md` (one line)

## Pitfalls

- **Lint failures on lockfiles/binary assets.** Symptom: `reuse lint` lists
  `Cargo.lock`, images under `website/`. Cause: no annotation matches.
  Recovery: extend `REUSE.toml` globs; don't add headers to generated files.
- **CI tool provisioning mismatch.** Symptom: CI step fails with
  `reuse: command not found`. Cause: the lane doesn't enter the dev shell.
  Recovery: invoke via `nix develop -c reuse lint` exactly as sibling steps
  provision their tools — read the existing lanes first.
- **README conflict.** Symptom: merge conflict on README. Cause: started
  before phase 01 landed. Recovery: rebase; the badge is one line under the
  title, independent of phase 01's prose.
- **Annotating files created by wave-0 phases.** Symptom: lint clean locally,
  red after merging phases 04–07. Cause: new docs/nix files outside globs.
  Recovery: globs in step 4 already cover `docs/**` and `nix/**`; verify with
  a final `reuse lint` after the last wave-0 merge.

## Reference

- Grant-readiness report §4 Tier 1: `docs/grants/ngi-fediversity-application-plan.md`
- Plan set: `docs/src/planning/ngi-fediversity-readiness/README.md`
- REUSE spec v3.3 / `REUSE.toml`: https://reuse.software/spec/
- Prerequisite: phase 01 (`01-narrative-bridge.md`)
