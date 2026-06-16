# Phase 08 — Finalize the Application Packet

## Working tree

`/data/nvme0/can/Projects/queryfabric`. Prerequisite: Phases 01–07 are complete
or their blockers have been explicitly reflected in the evidence map.

## Goal

This phase succeeds when `docs/grants/ngi-fediversity-application-answers.md`
has no unresolved placeholders, every claim is backed by the evidence map, and
the application is ready to submit by 2026-07-29.

## Why this matters now

The NGI Fediversity call deadline is 2026-08-01 12:00 CEST, and the target
submission date is 2026-07-29. The final packet must consume actual release,
demo, measurement, and applicant facts rather than assumptions.

## Out of scope

- Do not invent applicant history, funding status, public URLs, or costs.
- Do not add new technical promises to make the application sound stronger.
- Do not submit through the grant portal unless the applicant explicitly does
  that outside this repo workflow.
- Do not modify code.

## Plan

1. Read `docs/grants/claim-evidence-map.md` completely.
2. Replace applicant-owned placeholders in
   `docs/grants/ngi-fediversity-application-answers.md`:
   - personal background and prior work,
   - NixOS/nixpkgs contribution examples,
   - forge and crates.io profile URLs,
   - demo URL and hosting cost,
   - measured footprint numbers,
   - SynDB/funding/employment boundary.
3. Run:
   ```bash
   rg -n '\[[^]\n]+\]' docs/grants/ngi-fediversity-application-answers.md
   ```
   Inspect every result. Ordinary Markdown links are fine; unresolved
   placeholders are not.
4. Verify each retained claim has a corresponding evidence-map row.
5. Check the live grant form's field and character limits. If the live form
   differs from the draft structure, trim the answers and record the limit in
   the evidence map.
6. Verify all URLs in the answers with a browser or curl as appropriate.
7. Add a final checklist section to the evidence map with pass/fail status for
   every final submission gate.

## Acceptance criteria

- [ ] No unresolved placeholders remain in the application answers.
- [ ] Every non-applicant factual claim has evidence in
      `docs/grants/claim-evidence-map.md`.
- [ ] Applicant-owned facts are supplied by the applicant, not inferred.
- [ ] Field/character limits from the live form are recorded.
- [ ] Final answer text does not claim unavailable release, demo, REUSE, or
      footprint proof.
- [ ] Target submission date remains 2026-07-29 or is explicitly revised.

## Files likely touched

- `docs/grants/ngi-fediversity-application-answers.md`
- `docs/grants/claim-evidence-map.md`

## Pitfalls

- **Markdown links mistaken for placeholders.** Symptom: placeholder regex
  still reports links. Cause: broad matching. Recovery: inspect each result
  manually and only resolve real placeholders.
- **Live form mismatch.** Symptom: pasted answer exceeds limit. Cause: draft
  answers were written before checking the portal. Recovery: trim against the
  real limit and record it.
- **Last-minute unsupported claim.** Symptom: prose introduces a new claim not
  in the evidence map. Cause: final polishing. Recovery: add evidence or remove
  the claim.

## Reference

- `docs/grants/claim-evidence-map.md`
- `docs/grants/ngi-fediversity-application-answers.md`
- `docs/grants/ideal-project-set.md`, Project 7.
