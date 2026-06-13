# Phase 03 — Publish roadmap, issue templates, and accessibility statement

## Working tree

`/data/nvme0/can/Projects/queryfabric` (this repo). **Prerequisite: phase 01
must have landed** (the roadmap and templates must use its hosting/sovereignty
framing); rebase onto latest `trunk` before starting.

## Goal

This phase succeeds when the project presents a credible public surface for
reviewers and new contributors: a `ROADMAP.md` whose near-term items visibly
contain the grant work packages, Forgejo issue templates, a curated
good-first-issue list, and a short accessibility statement for the web
surfaces.

## Why this matters now

The grant application's weakest axis is team credibility: a single visible
contributor and no public trajectory (grant-readiness report §2 and §8).
Reviewers must see (a) that the grant accelerates an existing roadmap rather
than inventing one — the report's §6 work packages (WP1 service portability,
WP2 federation/HA, WP3 backend breadth, WP4 security/community) must be a
visible subset of `ROADMAP.md` — and (b) that a newcomer can land a first
contribution. NGI also asks UI projects about accessibility; one honest
paragraph beats silence (report §4, Tier 2 item 10).

## Out of scope

- No code changes, no NixOS module changes.
- Do not promise Tier 3 implementations as already underway — the roadmap
  lists them as planned, with the grant named as the intended funding path.
- No CONTRIBUTING.md rewrite — only a short "first contributions" pointer
  section appended.
- Creating actual issues in the tracker is maintainer follow-up (note it in
  the commit message); this phase ships the in-repo list and templates.
- No Matrix/chat infrastructure — out of repo scope.

## Plan

1. Rebase onto `trunk` (post-phase-01).
2. **`ROADMAP.md`** (repo root, linked from README's existing project links if
   a natural spot exists): three horizons.
   - *Now (pre-1.0, 2026 H2):* v0.2.0 release; REUSE compliance; threat model;
     footprint benchmarks; HA design; multi-instance NixOS module (i.e. this
     plan set — phrase as outcomes, not plan-set references).
   - *Next (grant scope, 2026 H2–2027):* import-side portable bundles
     (export → transfer → import → verify round-trip); federation hardening
     (hub failover, NAT traversal, schema-sync conflicts); embedded backend
     (SQLite or DataFusion) with conformance-corpus expansion; external
     security audit; nixpkgs module upstreaming. Mark these "subject of an
     NGI Fediversity grant application".
   - *Later:* 1.0 API stabilization per `COMPATIBILITY.md`; additional
     backends via the open artifact seam (`DECISIONS.md` D003).
3. **Issue templates** under `.forgejo/issue_template/` (Forgejo YAML form
   syntax): `bug-report.yaml` (version, backend, minimal query, expected vs
   actual emission/diagnostics), `feature-request.yaml` (use case, affected
   crates, portable-subset impact), `config.yaml` if blank issues should stay
   enabled.
4. **Good first issues**: a `## Your first contribution` section appended to
   `CONTRIBUTING.md` listing 5–10 concrete, real, scoped items discovered by
   inspecting the repo (e.g. conformance-corpus case additions, missing crate
   README examples, doc gaps, small diagnostics polish). Each item names the
   file(s) involved. Do not invent items you cannot point at.
5. **Accessibility statement** `docs/src/project/accessibility.md` (~half a
   page, honest): scope = the demo web UI (`crates/queryfabric-web`,
   `crates/queryfabric-leptos`) and the docs/website; current state (not yet
   audited), commitments (semantic HTML, keyboard navigation, contrast), and
   that WCAG review is planned under the grant's WP4. Add the SUMMARY line
   under `# Project`:
   `- [Accessibility](./project/accessibility.md)`.
   SUMMARY.md is shared with phases 01/04/05/06 — rebase before landing.
6. Verify `mdbook build docs` exits 0.
7. One CHANGELOG line under Unreleased: "public roadmap, issue templates, and
   accessibility statement".
8. Commit (plain `git commit`, default signing).

## Acceptance criteria

- [ ] `ROADMAP.md` exists with the three horizons; every "Next" item maps to a
      §6 work package in `docs/grants/ngi-fediversity-application-plan.md`.
- [ ] `.forgejo/issue_template/bug-report.yaml` and `feature-request.yaml`
      exist and are valid YAML (`nix develop -c python3 -c "import yaml,sys;
      yaml.safe_load(open(sys.argv[1]))" <file>` or equivalent).
- [ ] CONTRIBUTING.md gained a `## Your first contribution` section with ≥5
      items, each naming a real file path that exists in the tree.
- [ ] `docs/src/project/accessibility.md` exists; `mdbook build docs` exits 0.
- [ ] ROADMAP makes no claim that Tier 3 work is implemented.

## Files likely touched

- `ROADMAP.md` (new)
- `.forgejo/issue_template/bug-report.yaml`, `feature-request.yaml` (new)
- `CONTRIBUTING.md` (append one section)
- `docs/src/project/accessibility.md` (new)
- `docs/src/SUMMARY.md` (one line; shared with 01/04/05/06 — rebase)
- `CHANGELOG.md` (one line)

## Pitfalls

- **Roadmap drift from the grant report.** Symptom: "Next" items that aren't
  WP1–WP4. Cause: free-styling. Recovery: diff against report §6; the
  application will cite ROADMAP.md, so they must agree.
- **Forgejo template syntax vs GitHub's.** Symptom: templates don't render on
  the Forgejo instance. Cause: wrong directory or schema. Recovery: Forgejo
  reads `.forgejo/issue_template/` (also `.gitea/`); use Forgejo-documented
  form YAML, not GitHub's `.github/ISSUE_TEMPLATE` conventions.
- **Fabricated good-first-issues.** Symptom: items reference nonexistent files.
  Cause: writing from imagination. Recovery: each item is verified with a
  `ls`/`git grep` before inclusion (the acceptance criterion checks this).
- **Over-promising accessibility.** Symptom: statement claims WCAG AA today.
  Cause: template copying. Recovery: state current reality + plan; honesty
  rule from the plan README applies.

## Reference

- Grant-readiness report §4 (Tier 1 items 4–5, Tier 2 item 10), §6 (WPs), §8:
  `docs/grants/ngi-fediversity-application-plan.md`
- Plan set: `docs/src/planning/ngi-fediversity-readiness/README.md`
- Prerequisite: phase 01 (`01-narrative-bridge.md`)
- Forgejo issue forms: https://forgejo.org/docs/latest/user/issue-pull-request-templates/
