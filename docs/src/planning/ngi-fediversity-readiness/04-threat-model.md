# Phase 04 — Write the threat model

## Working tree

`/data/nvme0/can/Projects/queryfabric` (this repo, branch off `trunk`). No
phase prerequisite; `docs/src/SUMMARY.md` is shared with phases 01/05/06 —
rebase before landing.

## Goal

This phase succeeds when a 1–2 page threat model exists as an mdBook chapter,
grounded in the real attack surface of the compiler and federation protocol,
linked from SECURITY.md, and explicitly distinguishing QueryFabric-owned risks
from host-owned risks.

## Why this matters now

SECURITY.md currently lists reportable bug classes ("SQL or artifact
generation bugs, placeholder handling and parameter propagation, incorrect
capability classification or unsafe backend emission, provenance or schema
metadata mismatches") but there is no threat model. NGI-funded projects get
free external security audits (e.g. Radically Open Security); arriving with a
threat model and asking for the audit is a known-good move the grant
application should make (grant-readiness report §4, Tier 1 item 6). The
document also pre-seeds the grant's WP4 (security hardening).

## Out of scope

- No code changes, no new mitigations — document what exists; gaps become
  WP4/roadmap items, not patches in this phase.
- No CVE process / disclosure-policy overhaul; SECURITY.md gains only a link
  and (optionally) a contact clarification.
- Host-side concerns (authn/authz, query execution, network policy) are
  documented as explicitly out of QueryFabric's trust boundary — per
  `DECISIONS.md` D003 — not analyzed in depth.

## Plan

1. Branch from latest `trunk`.
2. Read, at minimum: `SECURITY.md`, `DECISIONS.md` (D001–D006),
   `crates/queryfabric-dialect-sql/src/` (parser entry, placeholder
   handling), `crates/queryfabric-ir/` (capability classification),
   `crates/queryfabric/src/` (facade seams), `crates/queryfabric-federation/src/`
   (message types: Register, HealthPing, SchemaSync, ResourceAnnouncement,
   CatalogRequest, GetFlightEndpoint), `crates/queryfabric-paseto/`,
   `crates/queryfabric-session/`, and `fuzz/` targets
   (`parse_sql_no_panic`, `bind_portable_no_panic`).
3. Write `docs/src/project/threat-model.md` with this structure:
   - **System context & trust boundaries**: untrusted query text in → typed
     artifacts out; catalog as semi-trusted input; federation peers as
     untrusted-until-registered; host responsibilities excluded (cite D003).
   - **Assets**: emission correctness (no injection through placeholders),
     capability classification soundness (a misclassified query is an authz
     bypass primitive for the host), provenance integrity (content hashes,
     receipts), bundle integrity (BLAKE3 digests), federation registry
     honesty.
   - **Threats per surface** (use STRIDE labels but per real surface):
     parser (malformed/adversarial SQL → panics, quadratic blowup), binder
     (catalog spoofing/confusion), emitter (dialect-specific injection via
     identifier quoting or placeholder propagation), federation (message
     forgery, malicious SchemaSync, resource-announcement spam), tokens/
     sessions (paseto misuse), supply chain (crates.io deps, Nix inputs).
   - **Existing mitigations**, each with a file/crate citation: fuzzing in CI,
     typed placeholder propagation, structured diagnostics over silent
     fallback, content addressing, systemd hardening + LoadCredential in
     `nix/modules/queryfabric.nix`.
   - **Known gaps / planned work**: name them honestly and map to the grant's
     WP4 (external audit, federation authn hardening) — keep consistent with
     `ROADMAP.md` if phase 03 has landed; otherwise with the grant report §6.
4. Add the SUMMARY line under `# Project`:
   `- [Threat Model](./project/threat-model.md)` (rebase — shared file).
5. Link from SECURITY.md: one sentence pointing to the chapter.
6. Verify `mdbook build docs` exits 0.
7. One CHANGELOG line under Unreleased: "threat model documentation".
8. Commit (plain `git commit`, default signing).

## Acceptance criteria

- [ ] `docs/src/project/threat-model.md` exists, ≤ ~2 pages rendered, and
      every claimed mitigation cites a real file or crate path.
- [ ] The document names the four SECURITY.md bug classes and places each
      inside a trust boundary.
- [ ] Federation message surface is covered (all six message types mentioned).
- [ ] D003 (no execution in core) is cited as the boundary rationale.
- [ ] SECURITY.md links to the chapter; `mdbook build docs` exits 0.
- [ ] No mitigation is claimed that does not exist in the tree today.

## Files likely touched

- `docs/src/project/threat-model.md` (new)
- `SECURITY.md` (one linking sentence)
- `docs/src/SUMMARY.md` (one line; shared with 01/05/06 — rebase)
- `CHANGELOG.md` (one line)

## Pitfalls

- **Boilerplate STRIDE.** Symptom: threats that could describe any project.
  Cause: writing before reading the crates. Recovery: every threat names the
  crate/function surface it enters through; delete any that don't.
- **Claiming absent mitigations.** Symptom: "all federation messages are
  authenticated" without code to back it. Cause: optimistic inference.
  Recovery: verify each mitigation with `git grep` before writing; gaps go in
  the gaps section — that section is what justifies WP4 funding.
- **Scope bleed into host concerns.** Symptom: pages on TLS termination and
  user authn. Cause: losing the D003 boundary. Recovery: one paragraph stating
  host responsibilities, then stop.
- **SUMMARY conflict.** Symptom: mdbook build fails post-merge. Recovery:
  rebase, keep all additive lines.

## Reference

- Grant-readiness report §4 (Tier 1 item 6), §5 (technical challenges), §6
  (WP4): `docs/grants/ngi-fediversity-application-plan.md`
- Plan set: `docs/src/planning/ngi-fediversity-readiness/README.md`
- `SECURITY.md`, `DECISIONS.md` (D003), `fuzz/` targets
- NGI security audits: https://nlnet.nl/NGI0/services/ (Radically Open Security)
