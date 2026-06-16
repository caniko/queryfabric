# Phase 07 — Refresh Measurement, HA, and Risk Evidence

> **Recommended model: gpt-5.4 (codex) — effort `medium`**
>
> Routed: `carter route -c complex -r subagent --needs writing`
> → `gpt-5.4` / `medium`
>
> Complex evidence work: the phase combines release-build measurements,
> security/threat-model claims, and HA wording. A weaker model may treat stale
> measurements as current or blur documented design with implemented behavior.

## Working tree

`/data/nvme0/can/Projects/queryfabric`. Prerequisites: Phase 01 evidence map
and Phase 04 release proof.

## Goal

This phase succeeds when resource-footprint numbers, HA claims, and
security/risk claims in the application are backed by current release-build
evidence and public docs.

## Why this matters now

The draft answers still contain footprint placeholders (`[X] MB`, `[Y] MB`,
`[Z] MB`). The application also relies on the threat model and HA design to
show credible risk handling. These claims must be revalidated against the
actual release build being cited.

## Out of scope

- No performance tuning unless measurement invalidates a claim and a small fix
  is clearly release-blocking.
- No HA feature implementation.
- No security remediation beyond correcting documentation claims.
- No external audit execution.

## Plan

1. Confirm the release identity from Phase 04.
2. Run the footprint script on the release build:
   ```bash
   nix develop -c scripts/footprint.sh
   ```
3. Update `docs/src/deployment/resource-footprint.md` only with numbers from
   that run and the hardware/build context.
4. Read `docs/src/deployment/high-availability.md` and confirm it separates:
   - safe promises today,
   - current single points of failure,
   - planned WP2 work.
5. Read `docs/src/project/threat-model.md` and `SECURITY.md`; confirm the
   application's security claims match public docs.
6. Run:
   ```bash
   nix develop -c mdbook build docs
   ```
7. Replace footprint placeholders in the application answers and update the
   evidence map.

## Acceptance criteria

- [ ] Footprint numbers in docs and application come from the current release
      build.
- [ ] `docs/src/deployment/high-availability.md` does not claim WP2 work is
      implemented.
- [ ] `docs/src/project/threat-model.md` is linked from `SECURITY.md`.
- [ ] `nix develop -c mdbook build docs` exits 0.
- [ ] Evidence map records footprint, HA, and threat-model evidence.

## Files likely touched

- `docs/src/deployment/resource-footprint.md`
- `docs/src/deployment/high-availability.md` only for claim corrections
- `docs/src/project/threat-model.md` only for claim corrections
- `SECURITY.md` only if the threat model link is missing
- `docs/grants/claim-evidence-map.md`
- `docs/grants/ngi-fediversity-application-answers.md`

## Pitfalls

- **Stale measurement.** Symptom: docs numbers predate release. Cause:
  reusing old table. Recovery: rerun `scripts/footprint.sh`.
- **Measurement blocker.** Symptom: script cannot start Postgres or demo.
  Cause: missing local dependency or broken release. Recovery: stop and report
  upstream producer and validation command.
- **HA overclaim.** Symptom: application says hub failover exists. Cause:
  confusing design doc with implementation. Recovery: mark as planned WP2.

## Reference

- `scripts/footprint.sh`
- `docs/src/deployment/resource-footprint.md`
- `docs/src/deployment/high-availability.md`
- `docs/src/project/threat-model.md`
- `docs/grants/ideal-project-set.md`, Project 5.
