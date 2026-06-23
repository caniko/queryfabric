# Plan: Grant pre-application readiness

> **Recommended model for plan-set orchestration: gpt-5.4-mini (codex) — effort `medium`**
>
> Routed: `carter route -c moderate -r orchestrator`
> → `gpt-5.4-mini` / `medium`
>
> Moderate orchestration: this set coordinates repository proof, release proof,
> deployment proof, and final application text without changing the funded
> technical scope. A weaker model could miss sequencing constraints between
> evidence, release, demo, and application claims.

## Scope and current state

This plan turns `docs/grants/ideal-project-set.md` into executable standalone
phase documents before applying for grants under `docs/grants/`, starting with
the NGI Fediversity application.

Current validated state from 2026-06-14:

- `nix develop -c mdbook build docs` passes.
- `nix develop -c plinth-project check --config website/plinth-project.toml`
  passes.
- `nix develop -c reuse lint` fails on four Forgejo YAML files.
- `Cargo.toml` is at `0.2.0`, and `CHANGELOG.md` contains a `0.2.0` section.
- No local `v*` tag was found in this checkout.
- `docs/grants/ngi-fediversity-application-answers.md` still contains
  applicant-owned placeholders.

Out of scope for this plan: implementing import-side portability, adding a new
SQLite/DataFusion backend, building hub failover/NAT traversal/schema conflict
resolution, adding a full admin UI, or remediating an external security audit.
Those are grant-funded outcomes, not pre-application chores.

## Phase table

| # | Phase | File | Depends on | Model / Effort | Parallel with | Blocking? |
|---|---|---|---|---|---|---|
| 01 | Claim freeze and evidence map | [01-claim-freeze-evidence-map.md](01-claim-freeze-evidence-map.md) | — | deepseek/deepseek-v4-flash / high | — | Blocks all claim-bearing work |
| 02 | Legal and openness hygiene | [02-legal-openness-hygiene.md](02-legal-openness-hygiene.md) | 01 | deepseek/deepseek-v4-flash / high | 03, 06 | Blocks 04 and final REUSE claims |
| 03 | Public narrative surface | [03-public-narrative-surface.md](03-public-narrative-surface.md) | 01 | deepseek/deepseek-v4-flash / high | 02, 06 | Blocks public-link claims |
| 04 | Release proof | [04-release-proof.md](04-release-proof.md) | 01, 02 | gpt-5.3-codex / medium | 05 after tag decision | Blocks 05, 07, 08 |
| 05 | Public demo instance | [05-public-demo-instance.md](05-public-demo-instance.md) | 01, 03, 04 | gpt-5.3-codex / medium | 06 | Blocks demo claims |
| 06 | Community and reviewer surface | [06-community-reviewer-surface.md](06-community-reviewer-surface.md) | 01 | deepseek/deepseek-v4-flash / high | 02, 03, 05 | Blocks community claims |
| 07 | Measurement, HA, and risk evidence | [07-measurement-ha-risk-evidence.md](07-measurement-ha-risk-evidence.md) | 01, 04 | gpt-5.4 / medium | 05, 06 | Blocks footprint and risk claims |
| 08 | Application packet finalization | [08-application-packet-finalization.md](08-application-packet-finalization.md) | 01–07 | gpt-5.4-mini / high | — | Terminal |

## Parallelism layer

**Wave 0 — evidence freeze.** Run phase 01 alone. It establishes the evidence
map and prevents later phases from improving unsupported claims.

**Wave 1 — repository proof.** After 01, phases 02, 03, and 06 may run in
parallel. Phase 02 touches licensing metadata and CI; phase 03 touches
reviewer-facing docs/website narrative; phase 06 touches roadmap/community
surface. If they touch `README.md` or `docs/src/SUMMARY.md`, land those edits
serially and re-run the relevant validation command.

**Wave 2 — release proof.** Phase 04 waits for 02 because the release must not
ship with a false REUSE claim. It may uncover code or packaging blockers; those
must be fixed inside the release proof boundary or reported as blockers.

**Wave 3 — public proof.** Phases 05 and 07 can run after 04. Demo smoke checks
and footprint measurements are independent, but both consume the release
identity and both feed the final application answers.

**Wave 4 — final application packet.** Phase 08 runs last and consumes all
evidence. The plan is exhausted when the application answers have no unresolved
placeholders and every claim points to evidence.

## Whole-set acceptance criteria

- [ ] `docs/grants/claim-evidence-map.md` exists and every application claim is
      classified as public URL, repo path, validation command, applicant fact,
      removed, or future-tense funded scope.
- [ ] `nix develop -c reuse lint` passes.
- [ ] `nix develop -c mdbook build docs` passes.
- [ ] `nix develop -c plinth-project check --config website/plinth-project.toml`
      passes.
- [ ] `nix develop -c scripts/release.sh check` passes, or release-ready claims
      are removed from the application.
- [ ] `v0.2.0` is published and tagged, or the application states exactly what
      has not been published.
- [ ] The public demo URL passes `/healthz`, `/federation/status`, and
      `/resources` smoke checks, or demo claims are removed.
- [ ] Footprint numbers in the application come from the cited release build.
- [ ] `rg -n '\[[^]\n]+\]' docs/grants/ngi-fediversity-application-answers.md`
      has no unresolved placeholders.

## Global constraints

- No shortcuts: do not fabricate, synthesize, or substitute missing evidence.
- If a foundational input is missing, stop and report the missing artifact,
  why it is required, the upstream producer, the regeneration command/workflow,
  and the validation command.
- Keep grant-funded work future-tense. Do not pre-build Tier 3 features.
- Use global Git signing defaults for commits/tags; do not bypass signing.
- Treat current dirty worktree changes as user-owned unless a phase explicitly
  edits the same file and can account for them.

## Shared-file lockstep

Several phases may touch `docs/grants/claim-evidence-map.md` and
`docs/grants/ngi-fediversity-application-answers.md`. Phase 01 creates the
map; after that, phases may update their own rows, but final wording in the
answers must serialize through Phase 08. If two phases touch the same grant
draft, land the evidence-map update first and re-read the answers before
editing prose.

## External repo coordination

Phase 05 may require a deployment repository or host configuration outside
this working tree. Any external path must be named in the phase's evidence-map
row. Do not copy secrets or private deployment-only facts into this repository;
only cite redacted NixOS config snippets, public URLs, and smoke commands.

## Merge-readiness checklist

Before Phase 08 starts, the dispatcher should verify:

- Phase 01 evidence map exists and has no unclassified claims.
- Phase 02 REUSE command passes.
- Phase 03 mdBook and website checks pass.
- Phase 04 release state is explicit: published/tagged or downgraded.
- Phase 05 demo state is explicit: smoke-tested URL or removed claims.
- Phase 07 footprint numbers are from the cited release build.

## PR sequencing and cross-owner coordination

Repository-only phases can land together if their files are disjoint, but any
phase that needs maintainer-owned credentials, grant-portal access, crates.io
publication, tag signing, public forge issue creation, or host deployment must
stop at a clear handoff when that authority is missing. The handoff must name
the upstream producer and validation command.

## Infrastructure SPOF

The demo URL, crates.io publication, and release tag are single proof points.
If any one is unavailable, the application should degrade the corresponding
claim rather than block all grant work indefinitely. Record the degraded claim
in the evidence map and keep the submission packet internally consistent.

## Serial-chain recovery

The chain `01 → 02 → 04 → 07 → 08` is the critical path. If Phase 04 fails,
Phase 05 can still prepare deployment configuration, but it must not advertise
a release-backed public demo until the release state is resolved. If Phase 07
fails, remove measured-footprint claims and keep qualitative resource-efficiency
claims only where backed by repository design docs.

## Cleanup batch

Small mechanical cleanups found while executing this set should be batched only
when they directly unblock an acceptance criterion. Do not turn this readiness
plan into a general repository cleanup pass.

## References

- Grant pre-application project set: `docs/grants/ideal-project-set.md`
- Readiness report: `docs/grants/ngi-fediversity-application-plan.md`
- Draft answers: `docs/grants/ngi-fediversity-application-answers.md`
- Existing NGI execution plan:
  `docs/src/planning/ngi-fediversity-readiness/README.md`
