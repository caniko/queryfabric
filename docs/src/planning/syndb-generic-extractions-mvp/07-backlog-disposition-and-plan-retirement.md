# Phase 07: Backlog Disposition And Plan Retirement

## Goal

Resolve copied-but-unproven utility work, correct durable project evidence, and
retire obsolete planning documents only after a fresh implementation audit.

This phase is deliberately evidence-driven. “Already copied” is not a reason to
keep or publish a crate.

## Backlog Matrix

| Item | Default disposition | Graduation evidence |
|---|---|---|
| `queryfabric-types` | registry-unpublished; compare with SynDB `shared` and remove unused duplicate | second consumer, full type/conversion/schema parity, SynDB adoption |
| `queryfabric-seaorm-ext` | retain neutral connection/vector helpers registry-unpublished; defer active-enum macro | SeaORM version compatibility, moved macro tests, two consumers |
| `queryfabric-changelog` | registry-unpublished or remove incomplete copy | injectable fetching/config, original SynDB extraction/report tests, real consumer |
| `queryfabric-cli-toolbelt` | registry-unpublished until every public module is behaviorally correct | real Flight auth/timeouts, correct `500m` quantity handling, Docker helper stdin protocol, tests |
| `queryfabric-test-rig` | keep generic Docker/process primitives registry-unpublished; leave service stacks in SynDB | parameterized names/networks, no SynDB defaults, portable service fixture |
| `queryfabric-cmd-runner` | registry-unpublished unless used outside release/dev tooling | stable command/error contract and second consumer |
| `queryfabric-paseto` typed profile | generalize or move host profile back to SynDB | versioned neutral resource scope; no dataset/table fields |
| `queryfabric-release` and legacy script | one internal producer, not a public library | simit-compatible exact-SemVer workflow and generated metadata |
| `queryfabric-flight` | registry-unpublished through Phase 05 | server/client parity, real auth/metadata/stream tests, SynDB adoption |
| duplicate `spawn_traced` and Arrow wrappers | consolidate if semantics are identical | focused tests and no loss of domain error context |
| federation substrate | remain registry-unpublished/experimental | separate RFC and production transport/data-plane/persistence proof |

For each row, choose one:

1. **graduate** — satisfy the extraction contract, adopt in SynDB, remove the
   duplicate, and reconsider publication;
2. **keep registry-unpublished** — useful public-source internal code with
   honest experimental docs;
3. **move back** — QueryFabric copy lacks a neutral consumer or behavior; or
4. **delete** — dead/unconsumed copy with source history preserved by Git.

## Durable Documentation Work

### Verify pre-MVP evidence rather than creating it late

Phase 02 owns the first repair of broken grant-plan links. Phase 04 verifies
those links and owns portability/security claims, accessibility evidence,
documentation examples, and two-run footprint evidence. Those artifacts are
reviewer-facing MVP evidence and cannot wait for post-MVP cleanup.

This phase performs a fresh verification that the durable roadmap, threat
model, compatibility, migration, changelog, accessibility findings, and
footprint report still match the implemented release. If implementation has
changed, refresh the evidence before retirement. Do not synthesize applicant
facts or silently copy an old claim map merely to preserve a historical link.

## Plan Retirement Workflow

Run a fresh read-only progress audit over:

- this plan;
- SynDB `docs/src/planning/queryfabric-upstream/`;
- any vendored QueryFabric planning/grant directories; and
- repository state, tests, CI, package registries, and deployed demo evidence.

For every old task, record:

- implemented and still tested;
- superseded by a named phase/durable document;
- intentionally rejected with rationale;
- deferred with an owner/entry criterion; or
- blocked under a complete missing-artifact contract.

Move durable architecture, operations, migration, and security guidance into
stable docs. Remove planning-only pages from navigation and delete old plan
files only when no unresolved work or unique evidence would be lost.

## Deliverables

- explicit disposition for every copied/optional crate and old-plan item;
- adopted implementations or deleted/registry-unpublished copies, with tests;
- verification that Phase 04's canonical links and public evidence remain
  current;
- honest final roadmap/threat/release status; and
- retirement audit and cleanup of obsolete planning files.

## Acceptance

- [ ] Cargo metadata and documentation agree on workspace and publish tiers.
- [ ] No registry-published crate contains ignored security/timeout inputs,
      zero-test copied behavior, or guaranteed `NotImplemented` primary paths.
- [ ] Every deleted SynDB implementation has an adopted upstream replacement
      and moved behavior tests.
- [ ] Every retained domain implementation has a written boundary rationale.
- [ ] No canonical page links to an absent application plan or resurrects stale
      applicant/budget/release facts.
- [ ] Documentation builds and link checks pass:

  ```bash
  nix develop -c mdbook build docs
  nix develop -c plinth-project check --config website/plinth-project.toml
  ```

- [ ] Phase 04's two-run footprint and accessibility/security/documentation
      evidence still names the released revision and remains current.
- [ ] A plan-progress report demonstrates that no unresolved task disappeared
      during retirement.
- [ ] Old planning pages are absent from published navigation.
- [ ] Both repositories pass their full gates after final cleanup.

## Non-Goals

- forcing every generic utility into QueryFabric;
- publishing internal developer tooling;
- retaining grant-application planning forever;
- delaying reviewer-facing accessibility, security, documentation, grant-link,
  or footprint evidence until this post-MVP phase;
- declaring production federation without its own implementation plan; or
- deleting historical evidence before durable guidance is mapped.

## Stop Conditions

If an old plan contains the only source of an unresolved requirement,
acceptance test, operational procedure, security decision, or applicant-owned
fact, stop retirement. Identify the durable destination and responsible
producer before removing the source page.
