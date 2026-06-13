# Phase 05 — Add reproducible resource-footprint benchmarks

> **Recommended model: gpt-5.4-mini (codex) — effort `medium`**
>
> Routed: `carter route -c moderate -r subagent -n coding -p codex`
> → `gpt-5.4-mini` / `medium`
>
> Bounded scripting + measurement + docs work: moderate complexity (the
> measurement methodology must be honest and repeatable, but the surface is
> one binary), subagent role, coding-weighted. A weaker tier risks
> non-reproducible numbers (measuring a debug build, ignoring warm-up), which
> the methodology checklist below pins down.
>
> Dispatch: `codex --model gpt-5.4-mini -c model_reasoning_effort=medium`

## Working tree

`/data/nvme0/can/Projects/queryfabric` (this repo, branch off `trunk`). No
phase prerequisite; `docs/src/SUMMARY.md` is shared with phases 01/04/06 —
rebase before landing.

## Goal

This phase succeeds when a reproducible script measures the demo service's
resource footprint (binary size, idle RSS, under-load RSS, cold-start time)
and a docs chapter publishes the numbers with the exact commands to reproduce
them, supporting the claim "runs on a small VPS".

## Why this matters now

The NGI Fediversity call names resource efficiency and e-waste reduction
explicitly, and the grant application wants a citable footprint page
(grant-readiness report §4, Tier 2 item 7). Today no footprint numbers exist
anywhere in the repo. The numbers also feed the budget answer ("hosting costs
negligible — commodity VPS").

## Out of scope

- No performance *optimization* — measure, don't tune. Surprising numbers are
  reported, not fixed (fixes are roadmap items).
- No criterion/micro-benchmarks of compiler internals — this is whole-service
  footprint, not query-latency benchmarking.
- No CI integration of the benchmark (numbers vary across runners; the script
  is for maintainer-run measurement). A build of the script's *syntax* path in
  CI is optional, not required.
- No load-testing infrastructure beyond a simple request loop.

## Plan

1. Branch from latest `trunk`.
2. Inspect how the demo runs locally: `crates/queryfabric-demo` (flags, env,
   minimal config — memory store mode should avoid needing Postgres/MinIO for
   a baseline; check `nix/tests/selfhost.nix` for the canonical invocation).
3. Write `scripts/footprint.sh` (bash, `set -euo pipefail`), measuring on a
   release build (`nix build .#queryfabric-demo` or
   `cargo build --release -p queryfabric-demo`):
   - **Binary size**: `du -h` of the stripped release binary; closure size via
     `nix path-info -S` for the Nix package.
   - **Cold start**: time from spawn to first successful HTTP response on the
     configured port (poll with `curl --max-time`), repeated 5×, report
     median.
   - **Idle RSS**: after 10 s settle, read `/proc/<pid>/status` VmRSS,
     report median of 5 runs.
   - **Under-load RSS**: simple loop of N concurrent compile requests against
     the demo's HTTP surface (reuse an endpoint exercised by the VM test),
     sample peak VmRSS.
   - Output a markdown table to stdout so the docs chapter can be refreshed by
     re-running the script.
4. Run it on this machine; capture real numbers.
5. Write `docs/src/deployment/resource-footprint.md`: methodology (exact
   script invocation, hardware description of the measurement box, build
   provenance — release profile, rust version from `rust-toolchain`/MSRV),
   the results table, and an honest sizing recommendation (e.g. "1 vCPU /
   512 MB" only if the measured peak supports it with margin).
6. Add the SUMMARY line under `# Deployment`:
   `- [Resource Footprint](./deployment/resource-footprint.md)` (rebase —
   shared file).
7. Verify `mdbook build docs` exits 0 and `shellcheck scripts/footprint.sh`
   is clean (shellcheck is available via nixpkgs; add to devShell only if
   it's already the repo's pattern — otherwise run ad hoc).
8. One CHANGELOG line under Unreleased: "reproducible footprint benchmark and
   deployment sizing docs".
9. Commit (plain `git commit`, default signing).

## Acceptance criteria

- [ ] `scripts/footprint.sh` exists, is executable, passes
      `shellcheck` with zero warnings, and emits a markdown table.
- [ ] Running the script twice on the same machine yields RSS numbers within
      ±10% (reproducibility sanity check — record both runs in the PR/commit
      message).
- [ ] `docs/src/deployment/resource-footprint.md` contains real measured
      numbers (not placeholders), the measurement hardware, and the exact
      reproduce command.
- [ ] All numbers come from a release-profile build — the doc states this.
- [ ] `mdbook build docs` exits 0.

## Files likely touched

- `scripts/footprint.sh` (new)
- `docs/src/deployment/resource-footprint.md` (new)
- `docs/src/SUMMARY.md` (one line; shared with 01/04/06 — rebase)
- `CHANGELOG.md` (one line)

## Pitfalls

- **Measuring a debug build.** Symptom: 10× the expected binary size/RSS.
  Cause: `cargo run` default profile. Recovery: the script hard-codes
  release/Nix builds and echoes the binary path + build provenance.
- **Demo needs external services.** Symptom: demo exits at startup wanting a
  database URL. Cause: baseline config requires Postgres. Recovery: use the
  memory store backend if the module/demo supports it (check
  `nix/modules/queryfabric.nix` `store.backend = "memory"`); if a database is
  unavoidable, document that the measurement includes a local Postgres and
  measure only the demo process RSS.
- **Port collisions / leftover processes.** Symptom: cold-start poll succeeds
  instantly (stale server) or bind fails. Recovery: script picks a free port,
  traps EXIT to kill its child, and verifies the pid it measures is the one
  it spawned.
- **Nix sandbox vs networking.** Symptom: trying to run measurements inside
  `nix build`. Cause: over-Nixifying. Recovery: the script is run by a human
  in a dev shell, not as a flake check.

## Reference

- Grant-readiness report §4 (Tier 2 item 7), §5 (budget answer):
  `docs/grants/ngi-fediversity-application-plan.md`
- Plan set: `docs/src/planning/ngi-fediversity-readiness/README.md`
- Canonical demo invocation: `nix/tests/selfhost.nix`, `crates/queryfabric-demo`
