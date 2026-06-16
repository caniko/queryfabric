# Phase 06 — Tighten the Community and Reviewer Surface

> **Recommended model: deepseek/deepseek-v4-flash (opencode) — effort `high`**
>
> Routed: `carter route -c moderate -r leaf --needs writing`
> → `deepseek/deepseek-v4-flash` / `high`
>
> Moderate review-surface writing: this phase checks roadmap, contribution,
> governance, and public issue evidence. A weaker model may overstate community
> maturity rather than honestly mitigating the single-maintainer signal.

## Working tree

`/data/nvme0/can/Projects/queryfabric`. Prerequisite: Phase 01 evidence map
exists.

## Goal

This phase succeeds when the application can honestly point reviewers to a
public roadmap, contribution path, governance/security policy, and concrete
entry points for contributors.

## Why this matters now

The grant-readiness report identifies single-contributor credibility as a weak
axis. The mitigation is not pretending a community already exists; it is making
the project legible and easy to join.

## Out of scope

- Do not claim active community participation that does not exist.
- Do not create fake issues or low-quality placeholder issues.
- Do not rewrite governance unless it is inaccurate.
- Do not touch release or demo deployment.

## Plan

1. Read `ROADMAP.md`, `CONTRIBUTING.md`, `GOVERNANCE.md`, `SECURITY.md`,
   `COMPATIBILITY.md`, README repository guide, and issue templates.
2. Confirm `ROADMAP.md` visibly maps to WP1–WP4 from the grant application.
3. Confirm first-contribution guidance names real files and scoped tasks.
4. If the forge is public and available, create real public issues from the
   in-repo first-contribution list. If not, record that as a limitation.
5. Ensure reviewer-facing surfaces link the roadmap, governance, security,
   contribution, compatibility, and release policy docs.
6. Update the evidence map with URLs or repository paths.

Validation command:

```bash
rg -n 'WP1|WP2|WP3|WP4|NGI Fediversity|good first|security|governance' \
  ROADMAP.md CONTRIBUTING.md README.md docs/src
```

## Acceptance criteria

- [ ] Roadmap grant-scope items map to WP1–WP4.
- [ ] Contribution guidance names real scoped tasks.
- [ ] Issue templates exist and are either publicly usable or the limitation is
      recorded.
- [ ] Evidence map contains reviewer-surface proof.
- [ ] Application does not overclaim existing community size.

## Files likely touched

- `ROADMAP.md`
- `CONTRIBUTING.md`
- `README.md`
- `.forgejo/issue_template/*.yaml`
- `docs/grants/claim-evidence-map.md`
- Public forge issues, if created outside the repository.

## Pitfalls

- **Community overclaim.** Symptom: application says "community" where only
  onboarding docs exist. Cause: trying to mask bus factor. Recovery: state the
  mitigation honestly.
- **Unreal good-first issues.** Symptom: issues require deep architecture
  knowledge. Cause: vague curation. Recovery: choose documentation,
  conformance, or small diagnostics tasks with named files.
- **Forge unavailable.** Symptom: cannot create public issues. Cause: account
  or network limitation. Recovery: record blocker; do not claim live issues.

## Reference

- `docs/grants/ngi-fediversity-application-plan.md`, risk section.
- `docs/grants/ideal-project-set.md`, Project 6.
