# Plan: NGI Fediversity readiness

> **Recommended model for plan-set orchestration: gpt-5.4-mini (codex) — effort `medium`**
>
> Routed: `carter route -c moderate -r orchestrator -p codex`
> → `gpt-5.4-mini` / `medium`
>
> Coordinating this set is dispatch and merge sequencing over well-specified,
> standalone phase docs — moderate complexity, orchestrator role. A weaker model
> would only risk mis-sequencing the shared-file merges (SUMMARY.md, README.md),
> which the wave table below makes explicit. Provider restricted to codex per
> user request.

## Scope and current state

This plan executes the Tier 1 and Tier 2 coding work from the grant-readiness
report at `docs/grants/ngi-fediversity-application-plan.md`, ahead of the NGI
Fediversity 12th-call deadline (2026-08-01 12:00 CEST).

Current state: QueryFabric is a 35-crate Rust workspace (workspace version
0.1.1) presenting as "a portable analytical query compiler for scientific
platforms". It already has an mdBook (`docs/`), a Plinth project site (`website/`), a
hardened NixOS module (`nix/modules/queryfabric.nix`) with a VM test
(`nix/tests/selfhost.nix`), Forgejo CI, a conformance corpus, fuzzing, and a
staged release script (`scripts/release.sh`). It lacks: a hosting/data-
sovereignty narrative, REUSE/SPDX compliance, a public roadmap, issue
templates, a threat model, footprint benchmarks, an HA design doc,
multi-instance NixOS module support, and a tagged v0.2.0.

Out of scope for the whole set: standing up the public demo instance (ops, not
repo work), submitting the grant application itself, crates.io publication
(phase 08 prepares it; the maintainer publishes), and all Tier 3 items
(import-side portability, new backends, federation hardening) — those are the
grant's funded work packages and must NOT be pre-built.

## Phase table

| # | Phase | File | Depends on | Model / Effort | Parallel with | Blocking? |
|---|---|---|---|---|---|---|
| 01 | Narrative bridge | [01-narrative-bridge.md](01-narrative-bridge.md) | — | gpt-5.4-mini / medium | 04, 05, 06, 07 (rebase SUMMARY.md) | Blocks 02, 03, 08 |
| 02 | REUSE compliance | [02-reuse-compliance.md](02-reuse-compliance.md) | 01 (README.md conflict) | gpt-5.4-mini / low | 03, 04, 05, 06, 07 | Blocks 08 |
| 03 | Roadmap & community surface | [03-roadmap-and-community.md](03-roadmap-and-community.md) | 01 (narrative consistency) | gpt-5.4-mini / low | 02, 04, 05, 06, 07 | Blocks 08 |
| 04 | Threat model | [04-threat-model.md](04-threat-model.md) | — | gpt-5.4 / medium | 01, 02, 03, 05, 06, 07 (rebase SUMMARY.md) | Blocks 08 |
| 05 | Footprint benchmarks | [05-footprint-benchmarks.md](05-footprint-benchmarks.md) | — | gpt-5.4-mini / medium | 01, 02, 03, 04, 06, 07 (rebase SUMMARY.md) | Blocks 08 |
| 06 | HA design doc | [06-ha-design-doc.md](06-ha-design-doc.md) | — | gpt-5.4 / medium | 01, 02, 03, 04, 05, 07 (rebase SUMMARY.md) | Blocks 08 |
| 07 | Multi-instance NixOS module | [07-multi-instance-nixos-module.md](07-multi-instance-nixos-module.md) | — | gpt-5.3-codex / medium | 01, 02, 03, 04, 05, 06 | Blocks 08 |
| 08 | v0.2.0 release prep | [08-release-prep.md](08-release-prep.md) | 01–07 | gpt-5.4-mini / low | — | Terminal |

## Parallelism layers

**Shared-file constraint that shapes every wave:** `docs/src/SUMMARY.md` is
touched by phases 01, 04, 05, and 06 (one additive line each). These phases may
*run* concurrently, but must *land* serially — each rebases SUMMARY.md before
merging. `README.md` is rewritten by 01 and gets a badge from 02, so 02 waits
for 01 to land.

- **Wave 0 — start from the current tree:** 01, 04, 05, 06, 07.
  - 07 is fully disjoint (touches only `nix/`, `flake.nix`, and the existing
    `docs/src/deployment/self-hosting-nixos.md`, which no other phase edits).
  - 01, 04, 05, 06 overlap only on SUMMARY.md; suggested landing order
    01 → 05 → 06 → 04 (narrative first so later docs match its framing).
  - Unlocks: 02 and 03 when 01 lands; 08 when everything lands.
- **Wave 1 — after 01 lands:** 02, 03.
  - 02 adds the REUSE badge to the post-01 README and SPDX coverage for all
    files including those created in wave 0 (rebase onto the latest trunk
    before starting).
  - 03 writes ROADMAP.md and templates consistent with 01's narrative.
- **Wave 2 — after all of 01–07 land:** 08 (CHANGELOG roll-up, version bump to
  0.2.0, `scripts/release.sh check` green). The plan is exhausted after 08.

## Whole-set acceptance criteria

- [ ] README, website, and a new docs chapter carry the self-hosting /
      data-sovereignty narrative without dropping the scientific-platform identity.
- [ ] `reuse lint` passes; CI enforces it; README carries the badge.
- [ ] `ROADMAP.md` exists and its near-term items match the grant work packages
      in `docs/grants/ngi-fediversity-application-plan.md` §6.
- [ ] Issue templates and an accessibility statement exist.
- [ ] Threat model chapter merged and linked from SECURITY.md.
- [ ] Reproducible footprint benchmark script + docs chapter with real numbers.
- [ ] HA design doc merged, honest about implemented-today vs grant-funded.
- [ ] `services.queryfabric.instances.<name>` works; VM test covers ≥2 instances.
- [ ] Workspace at 0.2.0, CHANGELOG finalized, `scripts/release.sh check` exits 0.
- [ ] `mdbook build docs` succeeds after every phase that touches `docs/src/`.

## Global constraints

- **Signed commits:** use the global Git signing defaults as-is — plain
  `git commit`, no overriding `commit.gpgsign`/`user.signingkey`, no
  `--no-gpg-sign`. If signing blocks (token/pinentry/agent unavailable), stop
  and report; do not bypass.
- **Don't pre-build Tier 3.** Import-side portability, new backends, and
  federation hardening are the grant's funded scope. Documents may *name* them
  as future work; no code.
- **mdBook gate:** any phase touching `docs/src/` must finish with a clean
  `mdbook build docs` (or `nix build .#docs`).
- **Honesty rule for docs:** design docs (HA, threat model, roadmap) must
  distinguish "implemented today" from "planned" — the grant reviewers will
  check claims against the repo.
- **Dispatch:** all phases route to codex; effort is set per phase via
  `codex --model <id> -c model_reasoning_effort=<tier>` (exact invocation in
  each phase's callout).

## References

- Grant-readiness report: `docs/grants/ngi-fediversity-application-plan.md`
- Decision log (deliberate boundaries the docs must respect): `DECISIONS.md`
- Release process: `RELEASE.md`, `scripts/release.sh`
- Routing: `carter route` (codex provider; enabled via temporary XDG overlay —
  the durable enable belongs in the carter home-manager module)
