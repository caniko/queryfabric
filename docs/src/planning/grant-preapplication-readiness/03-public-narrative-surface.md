# Phase 03 — Verify the Public Narrative Surface

## Working tree

`/data/nvme0/can/Projects/queryfabric`. Prerequisite: Phase 01 evidence map
exists.

## Goal

This phase succeeds when the reviewer-facing public surfaces support the
application narrative and pass their local validation commands.

## Why this matters now

The grant plan says the application succeeds or fails on framing: QueryFabric
must be legible as a data-sovereignty and query-portability layer for
self-hosted services without losing its scientific-platform identity. The
existing docs include this narrative, but the application must cite public
pages, not private draft text.

## Out of scope

- No broad website redesign.
- No new feature claims.
- No import-side portability, new backend, federation hardening, or admin UI
  implementation.
- No release publishing or deployment work.

## Plan

1. Read:
   - `README.md`
   - `website/plinth-project.toml`
   - `docs/src/concepts/self-hosting-and-data-sovereignty.md`
   - `docs/src/deployment/self-hosting-nixos.md`
   - `docs/src/project/threat-model.md`
   - `docs/src/project/accessibility.md`
   - `docs/src/SUMMARY.md`
2. Check that public prose distinguishes:
   - implemented today,
   - documented design,
   - grant-funded future work.
3. Patch only inaccurate or unsupported wording.
4. Run:
   ```bash
   nix develop -c mdbook build docs
   nix develop -c plinth-project check --config website/plinth-project.toml
   ```
5. Update `docs/grants/claim-evidence-map.md` with the public paths or URLs
   that application answers should cite.

## Acceptance criteria

- [ ] README still contains both the scientific-platform framing and the
      self-hosting/data-sovereignty framing.
- [ ] `docs/src/SUMMARY.md` links the relevant public chapters.
- [ ] `nix develop -c mdbook build docs` exits 0.
- [ ] `nix develop -c plinth-project check --config website/plinth-project.toml`
      exits 0.
- [ ] Claim evidence map records the public narrative sources.

## Files likely touched

- `README.md`
- `website/plinth-project.toml`
- `docs/src/**/*.md` only for correcting unsupported wording
- `docs/grants/claim-evidence-map.md`

## Pitfalls

- **Overclaiming planned work.** Symptom: docs say import-side portability
  works today. Cause: roadmap prose drifting into implementation prose.
  Recovery: rewrite as planned WP1 work.
- **Broken mdBook links.** Symptom: mdBook build fails. Cause: moved or stale
  relative paths. Recovery: fix links and re-run mdBook.
- **Website and docs diverge.** Symptom: website pitch conflicts with README.
  Cause: editing one surface only. Recovery: align wording conservatively.

## Reference

- `docs/grants/ngi-fediversity-application-plan.md`, narrative bridge.
- `docs/grants/ideal-project-set.md`, Project 2.
