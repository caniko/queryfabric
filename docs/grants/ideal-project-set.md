# Ideal Pre-Application Project Set

This document defines the project set to complete before applying for the
grants described in `docs/grants/`, starting with the NGI Fediversity
application. It is a grant-facing coordination layer over the detailed phase
plan in `docs/src/planning/ngi-fediversity-readiness/`.

The rule for this set is simple: do not submit an application that depends on
private memory, assumed infrastructure, or unverified repository claims. Every
claim in the application must have either a public URL, a local validation
command that passes, or an explicit applicant-owned placeholder that is
resolved before submission.

## Sources

- Grant readiness report:
  `docs/grants/ngi-fediversity-application-plan.md`
- Draft application answers:
  `docs/grants/ngi-fediversity-application-answers.md`
- Detailed execution plan:
  `docs/src/planning/ngi-fediversity-readiness/README.md`

## Current validated state

Validated locally on 2026-06-14:

- `nix develop -c mdbook build docs` passes.
- `nix develop -c plinth-project check --config website/plinth-project.toml`
  passes.
- `nix develop -c reuse lint` fails. Missing REUSE metadata remains for:
  `.forgejo/issue_template/bug-report.yaml`,
  `.forgejo/issue_template/config.yaml`,
  `.forgejo/issue_template/feature-request.yaml`, and
  `.forgejo/workflows/pages.yaml`.
- `Cargo.toml` is at workspace version `0.2.0`, and `CHANGELOG.md` has a
  `0.2.0 - 2026-06-13` section.
- No local `v*` git tag was found, so the release is not proven tagged from
  this checkout.
- `docs/grants/ngi-fediversity-application-answers.md` still contains
  applicant-owned placeholders such as `[demo URL]`, `[N]`, `[€X]`,
  `[forge URL]`, and footprint placeholders.

Commands that were not run in this planning pass remain gates, not assumed
facts:

```bash
nix develop -c scripts/release.sh check
nix flake check
```

## Project set

### Project 0: Claim Freeze and Evidence Map

Goal: create the evidence map that prevents accidental overclaiming.

Actions:

1. List every factual claim in
   `docs/grants/ngi-fediversity-application-answers.md`.
2. For each claim, attach exactly one proof:
   - public URL,
   - repository path and line,
   - validation command,
   - or applicant-owned fact that must be supplied manually.
3. Delete or downgrade any claim that cannot be proven before submission.
4. Keep all grant-funded scope clearly future-tense.

Validation:

```bash
rg -n '\[[^]\n]+\]' docs/grants/ngi-fediversity-application-answers.md
```

Submission gate: zero unresolved bracket placeholders except Markdown links.

### Project 1: Legal and Openness Hygiene

Goal: make the openness claims true before reviewers see the repository.

Actions:

1. Fix REUSE coverage for the four currently missing Forgejo files.
2. Re-run REUSE through the project dev shell.
3. Confirm the README badge is not misleading by requiring the lint command to
   pass locally.
4. Keep CI enforcement in the stable lane.

Validation:

```bash
nix develop -c reuse lint
```

Blocked state: if `reuse lint` still fails, do not submit an application that
says the project is REUSE-compliant. The upstream producer is the repository
maintainer; regenerate by extending `REUSE.toml` annotations or adding SPDX
headers to the missing files.

### Project 2: Public Narrative Surface

Goal: make the reviewer-facing story visible without reading the application
draft.

Actions:

1. Keep the README framing additive: scientific-platform identity plus
   self-hosting and data-sovereignty relevance.
2. Keep the website and mdBook in sync with that same narrative.
3. Link the self-hosting/data-sovereignty, NixOS deployment, threat model,
   footprint, HA, and accessibility docs from the public docs surface.
4. Avoid claiming import-side portability, new backends, or federation
   hardening as implemented. Those are grant-funded work packages.

Validation:

```bash
nix develop -c mdbook build docs
nix develop -c plinth-project check --config website/plinth-project.toml
```

Submission gate: the application can cite public pages for the narrative, not
only local draft text.

### Project 3: Release Proof

Goal: turn "release process exists" into a verifiable release.

Actions:

1. Run the full release check.
2. Fix only release-blocking defects.
3. Publish the staged crates.io release as a maintainer action.
4. Create and push the `v0.2.0` tag after publication.
5. Add the release URL and tag URL to the application evidence map.

Validation:

```bash
nix develop -c scripts/release.sh check
scripts/release.sh publish --version 0.2.0 --execute
scripts/release.sh tag --version 0.2.0
git tag --list 'v0.2.0'
```

Blocked state: if crates.io credentials or tag authority are missing, stop.
The upstream producer is the maintainer with publish rights. The proof is the
crates.io version page plus the pushed git tag.

### Project 4: Public Demo Instance

Goal: provide the cheapest visible proof of the NixOS, federation, portability,
and sovereignty story.

Actions:

1. Deploy `queryfabric-demo` from the NixOS module, not by hand.
2. Enable federation mode and the object-store backed export path.
3. Put it behind HTTPS at a stable public URL.
4. Publish the exact deployment snippet or flake reference, with secrets
   omitted.
5. Capture one screenshot or asciinema that shows the demo responding.

Validation:

```bash
curl --fail https://<demo-host>/healthz
curl --fail https://<demo-host>/federation/status
curl --fail https://<demo-host>/resources
```

Blocked state: if no public demo URL exists, do not keep `[demo URL]` or
`[URL]` in the application. The upstream producer is the deployment owner. The
proof is the HTTPS URL plus the deployment config reference.

### Project 5: Measurement, HA, and Risk Evidence

Goal: make the resource-efficiency, HA, and security claims reviewable.

Actions:

1. Re-run footprint measurements on the exact release build that will be cited.
2. Replace footprint placeholders in the application with measured numbers.
3. Keep the HA document explicit about what works today versus WP2 grant work.
4. Keep the threat model linked from `SECURITY.md`.
5. Treat surprising numbers as facts. Do not optimize during this project set
   unless the measurement invalidates the grant story.

Validation:

```bash
nix develop -c scripts/footprint.sh
nix develop -c mdbook build docs
```

Blocked state: if the footprint script cannot run reproducibly, stop and fix
the script or remove measured-footprint claims. The upstream producer is the
release/measurement owner. The proof is the generated table committed to docs.

### Project 6: Community and Reviewer Surface

Goal: mitigate the single-maintainer signal without pretending the project is
larger than it is.

Actions:

1. Keep `ROADMAP.md` aligned with the four proposed work packages.
2. Keep issue templates and the first-contribution list concrete.
3. Create real public issues from the in-repo good-first-issue list if the
   forge is public.
4. Confirm `GOVERNANCE.md`, `SECURITY.md`, `CONTRIBUTING.md`, and
   `COMPATIBILITY.md` are linked from reviewer-facing surfaces.
5. Ask one external peer to read the application for credibility and field
   length.

Validation:

```bash
rg -n 'WP1|WP2|WP3|WP4|NGI Fediversity|good first|security|governance' \
  ROADMAP.md CONTRIBUTING.md README.md docs/src
```

Blocked state: if no public forge issues or peer read-through can happen
before submission, state that honestly in the application and do not claim an
active contributor community.

### Project 7: Application Packet Finalization

Goal: submit a complete application with no hidden dependencies.

Actions:

1. Replace every applicant-owned placeholder:
   - personal background and prior relevant work,
   - NixOS/nixpkgs contribution examples,
   - public forge and crates.io profile URLs,
   - demo URL and monthly hosting cost,
   - measured footprint numbers,
   - employment or funding boundary around SynDB, if applicable.
2. Confirm the live grant form's field and character limits.
3. Trim the abstract and budget answers to the live limits.
4. Verify every URL in the answers.
5. Submit by 2026-07-29, leaving two days before the 2026-08-01 12:00 CEST
   deadline.

Validation:

```bash
rg -n '\[[^]\n]+\]' docs/grants/ngi-fediversity-application-answers.md
```

Blocked state: if the live form requirements differ from the draft answers,
the upstream producer is the applicant using the grant portal. Regenerate the
answers against the real limits and re-run the placeholder check.

## Execution order

1. Project 0 first. It defines the evidence map and prevents later work from
   optimizing for claims we cannot submit.
2. Projects 1, 2, 5, and 6 can run in parallel after Project 0. They are mostly
   repository and documentation proof.
3. Project 3 should run after Project 1 is green, because the release should
   not ship with a false REUSE claim.
4. Project 4 can run in parallel with Project 3, but the application should not
   cite it until the public URL passes smoke checks.
5. Project 7 is last. It consumes the proof from every earlier project.

## Do not pre-build as pre-application work

These are the grant-funded outcomes, not pre-submission chores:

- import-side portable bundle ingestion,
- new embedded backend work such as SQLite or DataFusion,
- hub failover, NAT traversal, and schema-sync conflict resolution,
- full operator admin UX,
- external security audit remediation.

Before submission, documentation may describe these as planned work. Code
should not be rushed into the pre-application set just to make the proposal
look larger.

## Final submission gate

Submit only when all of the following are true:

- `nix develop -c reuse lint` passes.
- `nix develop -c mdbook build docs` passes.
- `nix develop -c plinth-project check --config website/plinth-project.toml`
  passes.
- `nix develop -c scripts/release.sh check` passes, or the application no
  longer claims a release-ready state.
- `v0.2.0` is published and tagged, or the application says exactly what has
  not yet been published.
- The demo URL passes the smoke checks, or demo claims are removed.
- Footprint numbers are from the cited release build.
- `rg -n '\[[^]\n]+\]' docs/grants/ngi-fediversity-application-answers.md`
  has no unresolved placeholders.
- Every claim in the answers has a public URL, repository path, or validation
  command behind it.
