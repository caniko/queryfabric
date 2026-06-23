# Phase 01 — Freeze Claims and Build the Evidence Map

## Working tree

`/data/nvme0/can/Projects/queryfabric`.

## Goal

This phase succeeds when `docs/grants/claim-evidence-map.md` exists and every
factual claim in `docs/grants/ngi-fediversity-application-answers.md` is mapped
to a public URL, repository path, validation command, applicant-owned fact, a
removed claim, or explicitly future-tense grant-funded scope.

## Why this matters now

The current application answers still contain bracketed placeholders such as
`[demo URL]`, `[N]`, `[€X]`, `[forge URL]`, and footprint placeholders. The repo
standard says missing required data must not be fabricated or silently
substituted. The evidence map is the upstream artifact that every later phase
uses to decide which claims are safe to keep.

## Out of scope

- Do not rewrite the application prose except to remove obviously invalid
  claims discovered during mapping.
- Do not invent applicant facts such as prior funding, fleet size, profile
  URLs, or hosting cost.
- Do not stand up infrastructure or run release publishing.
- Do not implement grant-funded technical work.

## Plan

1. Read:
   - `docs/grants/ideal-project-set.md`
   - `docs/grants/ngi-fediversity-application-plan.md`
   - `docs/grants/ngi-fediversity-application-answers.md`
   - `README.md`, `ROADMAP.md`, `SECURITY.md`, `CONTRIBUTING.md`,
     `COMPATIBILITY.md`, and `CHANGELOG.md`
2. Extract unresolved placeholders:
   ```bash
   rg -n '\[[^]\n]+\]' docs/grants/ngi-fediversity-application-answers.md
   ```
3. Create `docs/grants/claim-evidence-map.md` with columns:
   `Claim`, `Application section`, `Evidence type`, `Evidence`, `Status`,
   `Owner`, and `Notes`.
4. Classify each claim as one of:
   - `public-url`
   - `repo-path`
   - `validation-command`
   - `applicant-fact`
   - `future-funded-scope`
   - `remove-or-downgrade`
5. For `applicant-fact` rows, name the exact input the applicant must supply.
6. For `remove-or-downgrade` rows, either patch the draft answers or leave a
   blocking note explaining why the claim cannot be submitted.
7. Add a short "Current blockers" section at the top of the evidence map.

## Acceptance criteria

- [ ] `docs/grants/claim-evidence-map.md` exists.
- [ ] Every current bracket placeholder from the `rg` command appears in the
      map or has been removed from the answers.
- [ ] No applicant-owned fact is filled in from inference.
- [ ] Every future grant work package remains future-tense.
- [ ] The map names upstream producers for missing facts.

## Files likely touched

- `docs/grants/claim-evidence-map.md` (new)
- `docs/grants/ngi-fediversity-application-answers.md` (only if removing or
  downgrading unsupported claims)

## Pitfalls

- **Placeholder hidden in Markdown link syntax.** Symptom: the placeholder
  regex reports normal links. Cause: broad bracket matching. Recovery: ignore
  ordinary Markdown links only after confirming they are not applicant facts.
- **Evidence by memory.** Symptom: notes say "I know this is true". Cause:
  missing URL/path/command. Recovery: mark `applicant-fact` or
  `remove-or-downgrade`.
- **Present-tense funded work.** Symptom: application says import-side
  portability exists. Cause: confusing roadmap with implementation. Recovery:
  rewrite as planned WP1 work.

## Reference

- `docs/grants/ideal-project-set.md`
- `docs/grants/ngi-fediversity-application-answers.md`
- Repo instruction: no fabricated or substituted missing required data.
