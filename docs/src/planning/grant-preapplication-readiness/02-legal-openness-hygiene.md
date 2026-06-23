# Phase 02 — Make Openness Claims True

## Working tree

`/data/nvme0/can/Projects/queryfabric`. Prerequisite: Phase 01 evidence map
exists.

## Goal

This phase succeeds when the repository can truthfully claim REUSE compliance:
`nix develop -c reuse lint` exits 0 and the claim is reflected in the evidence
map.

## Why this matters now

Validation on 2026-06-14 failed:

```text
The following files have no copyright and licensing information:
* .forgejo/issue_template/bug-report.yaml
* .forgejo/issue_template/config.yaml
* .forgejo/issue_template/feature-request.yaml
* .forgejo/workflows/pages.yaml
```

Submitting a grant application that says "REUSE-compliant" while this command
fails would violate the work standard and undermine openness claims.

## Out of scope

- No license change.
- No mass per-file SPDX header sweep unless `REUSE.toml` cannot express the
  coverage cleanly.
- No unrelated CI, Nix, or README rewrites.
- No release publishing.

## Plan

1. Read `REUSE.toml` and the four failing files.
2. Fix coverage by extending `REUSE.toml` globs or adding SPDX headers to the
   four files. Prefer the smallest diff that makes `reuse lint` true.
3. Run:
   ```bash
   nix develop -c reuse lint
   ```
4. If REUSE reports additional files, fix only missing metadata needed for a
   clean lint.
5. Update `docs/grants/claim-evidence-map.md` with the passing command and
   mark REUSE claims as proven.

## Acceptance criteria

- [ ] `nix develop -c reuse lint` exits 0.
- [ ] The four currently failing Forgejo YAML files are covered by REUSE.
- [ ] `docs/grants/claim-evidence-map.md` records the passing validation.
- [ ] No unrelated files are reformatted or relicensed.

## Files likely touched

- `REUSE.toml`
- `.forgejo/issue_template/bug-report.yaml` (only if using headers)
- `.forgejo/issue_template/config.yaml` (only if using headers)
- `.forgejo/issue_template/feature-request.yaml` (only if using headers)
- `.forgejo/workflows/pages.yaml` (only if using headers)
- `docs/grants/claim-evidence-map.md`

## Pitfalls

- **Wrong workflow glob.** Symptom: `.forgejo/workflows/pages.yaml` remains
  uncovered. Cause: `REUSE.toml` covers `*.yml` but not `*.yaml`. Recovery:
  add the `.yaml` glob.
- **Dirty generated outputs.** Symptom: REUSE reports build outputs. Cause:
  running against untracked generated files. Recovery: inspect and ignore
  generated outputs without deleting user-owned files.
- **Badge ahead of proof.** Symptom: README claims compliance before lint
  passes. Recovery: treat the lint result as the only proof.

## Reference

- Phase 01 evidence map.
- `docs/grants/ideal-project-set.md`, Project 1.
- REUSE spec: https://reuse.software/spec/
