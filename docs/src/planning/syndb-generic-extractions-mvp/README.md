# Plan: SynDB Generic Extractions And QueryFabric MVP

- **Status:** proposed
- **Research basis:** [research dossier](../syndb-generic-extractions-mvp-research.md)
- **Grant lens:** NGI Fediversity alignment (moved to the [applications checkout](https://codeberg.org/caniko/applications))
- **MVP gate:** end of Phase 04C
- **Historical plan:** SynDB `docs/src/planning/queryfabric-upstream/`

This execution plan is intentionally absent from `docs/src/SUMMARY.md` while
active. Durable guidance moves into published chapters during Phase 07.

## North Star

Release QueryFabric as a trustworthy portable compiler and prove a bounded
host-to-host tabular resource-portability path: export, transfer, validate, and
import one tabular resource conforming to a published profile between
independently configured, reproducibly deployed NixOS reference hosts. Move
only genuinely neutral,
behavior-preserving components out of SynDB.

The plan does not equate “copied into QueryFabric” with “extracted.” An item is
complete only after upstream behavior, downstream adoption, duplicate removal,
and release-tier disposition are all proven.

## Scope

### Compiler baseline

- stable parse → bind → analyze → emit facade for SQL and SyQL;
- immutable catalog snapshots and typed parameters;
- safe PostgreSQL and ClickHouse emission;
- structured result schema, provenance, capabilities, and diagnostics;
- PostgreSQL reference-host execution with prepared values, read-only
  credentials, limits, and cancellation;
- honest crate/package/release automation;
- one canonical QueryFabric revision consumed by SynDB; and
- upstream and downstream reproducibility gates.

This baseline is necessary but is not, by itself, a strong Fediversity
outcome. The grant-facing value comes from the host-to-host tabular-resource
proof below.

### Host-to-host tabular resource-portability MVP

- legacy/export-only bundle 1.0 plus import-ready bundle 2.0 using RFC 8785,
  typed BLAKE3-256 digests, one normative tabular CSV import profile, and public
  cross-language conformance fixtures;
- a transport-neutral validator and import planner that carries citations,
  licence/restriction, and origin-attributed source provenance without treating
  source actors as target authorization;
- authenticated, operator-mediated export -> transfer -> dry-run -> import
  between two hosts with separate PostgreSQL databases, object-store buckets,
  credentials, state directories, and catalog identities;
- bundle and artifact integrity checks, bounded parsing, explicit trust policy,
  predeclared relation/resource mapping, conflict diagnostics, idempotency, and
  atomic visibility of a successful import;
- persistent import receipts, imported rows, carried source evidence, locally
  assigned policy/owner, target import event, and mapping that survive a
  target-host restart;
- a reproducible NixOS migration test, current operator documentation,
  accessibility/security evidence, clean two-run footprint evidence, honest
  governance, and an upstream-ready module handoff; and
- a public, FLOS, objectively verifiable release handoff. A reachable tag or
  publication remains an explicit maintainer action.

### Post-MVP extensions

- full generic Arrow Flight service/client contract;
- isolated Kubernetes runtime and one-shot worker;
- production federation transport and data-plane integration;
- production high availability, provider failover, NAT traversal, and adaptive
  placement;
- a second embedded backend such as SQLite or DataFusion;
- broad generic CLI/test orchestration; and
- optional utility extractions that lack a second consumer or parity tests.

### Explicit non-goals

- moving SynDB's `ChQuery`, query host, dataset descriptors, database schemas,
  ETL/GPU/search/neuro code, or domain authorization into QueryFabric;
- treating the demonstrator's process-local stores as production persistence;
- claiming public packages, production federation, or isolated execution
  before their evidence exists;
- claiming full service migration, Fediverse interoperability, Fediversity
  adoption/conformance, easy non-expert self-hosting, low resource use, or
  WCAG conformance before the corresponding proof exists;
- claiming GDPR/legal compliance from narrowly tested access, export, or erase
  mechanisms; and
- publishing incomplete crates merely because Cargo metadata currently permits
  it.

## MVP Architecture

| Boundary | QueryFabric owns | Host owns |
|---|---|---|
| input | dialect, query text, typed parameter schema/values, catalog trait | identity, authorization, catalog construction, requested backend policy |
| compilation | parse, bind, normalize, capability analysis, result schema, provenance, safe emission | backend choice and policy-specific rejection |
| execution | typed artifact contract and cancellation-aware runtime traits | credentials, prepared execution, connection pooling, row/time/size budgets |
| response | schema, provenance, diagnostics, backend artifact metadata | rows/stream transport, redaction, audit persistence |
| portability | bundle/profile schema, canonicalization, bounded validation, integrity facts, neutral import plan/report | authentication, trusted hash/signature source, artifact transport, URI allowlist, predeclared-target mapping, transactional apply, durable receipt/state |
| operations | compiler/package conformance and reference fixtures | deployment, queueing, durable state, federation, worker lifecycle |

The stable library surface stays aligned with `DECISIONS.md` D003/D004. The
demo is an acceptance host, not a reason to move host policy into the facade.
Its NixOS test is a provider-oriented deployability and
tabular-resource-migration conformance fixture; it is not evidence of adoption
by two real providers or a claim that QueryFabric is a universal self-hosting
product.

## Extraction Graduation Contract

Every extraction phase must satisfy all gates:

- [ ] A neutral contract is backed by a second consumer or the public compiler
      facade.
- [ ] Original source tests are ported before source behavior is removed.
- [ ] QueryFabric implementation code is free of SynDB domain names, defaults,
      environment variables, images, and resources.
- [ ] Default and all-feature focused tests pass upstream.
- [ ] SynDB consumes the canonical upstream revision and focused downstream
      tests pass.
- [ ] Duplicate source is deleted in the same phase; only thin domain adapters
      or re-exports remain.
- [ ] Security, timeout, and lifecycle arguments are honored; no publishable API
      is an unconditional stub.
- [ ] The crate is assigned to stable, registry-unpublished/experimental, or
      rejected release tier.

Useful audits:

```bash
rg -n 'syndb|SyndbTable|neurometa|GraphTrainingSet|SYNDB_' crates
rg -n 'struct DynamicClient|enum ChType|struct ClickHouseConfig' \
  /data/can/canix/projects/repos/owned/github.com/memorycircuits/SynDB/crates
```

Matches in docs, migration shims, or explicit fixtures need a written
justification. Matches in reusable implementation code fail the gate.

## Phase Map

| Phase | Outcome | Depends on | MVP critical |
|---|---|---|---|
| [00](00-foundations-and-lineage.md) | reproducible validation and one canonical QueryFabric lineage | — | yes |
| [01](01-restore-syndb-flight-safety.md) | SynDB production Flight no longer selects incomplete handlers | 00 | yes, downstream safety |
| [02](02-core-compiler-hardening.md) | safe compiler contract and honest stable crate tier | 00 | yes |
| [03](03-retained-extraction-convergence.md) | retained generic work is adopted once and topic-only behavior is canonical | 00, 01, 02 | yes |
| [04A](04-reference-host-and-release-proof.md#gate-04a-versioned-tabular-portability-profile) | bundle 2.0, tabular profile, validator, and import plan are normative | 01, 02, 03 | yes |
| [04B](04-reference-host-and-release-proof.md#gate-04b-reference-host-apply-and-persistence) | predeclared-target apply, role separation, durable state, replay/failure semantics pass | 04A | yes |
| [04C](04-reference-host-and-release-proof.md#gate-04c-multi-node-migration-and-release-evidence) | multi-node migration, typed query, public evidence, and release handoff pass | 04B | **MVP gate** |
| [05](05-flight-contract-graduation.md) | generic Flight service/client reaches parity before SynDB migration | 04C | no |
| [06](06-isolated-execution-extension.md) | worker/K8s protocol, artifact, and Kind proof are self-contained | 05 | no |
| [07](07-backlog-disposition-and-plan-retirement.md) | optional copies are adopted or removed and stale plans are retired | 04C; can overlap 05/06 | no |

## Execution Waves

| Wave | Phases | Coordination |
|---|---|---|
| 0 | 00 | no code migration until inputs and lineage are valid |
| 1 | 01, 02 | SynDB Flight safety and compiler hardening use separate worktrees |
| 2 | 03 | DynamicClient convergence consumes Phase 02's identifier contract |
| 3A | 04A | freeze the import-ready format/profile and neutral validation artifacts |
| 3B | 04B | implement host apply/persistence only after 04A fixtures pass |
| 3C | 04C | integrate the independent-host/release proof only after 04B failure/role gates pass |
| 4 | 05 and the audit portion of 07 | post-MVP; Flight and backlog analysis can proceed independently |
| 5 | 06 and remaining 07 cleanup | isolated execution depends on the graduated Flight protocol |

## Stable Release Tier

The proposed 0.2 Rust dependency closure is:

1. `queryfabric-contract`
2. `queryfabric-ir`
3. `queryfabric-catalog`
4. `queryfabric-dialect-sql`
5. `queryfabric-dialect-syql`
6. `queryfabric-runtime`, limited to honest contracts and with pre-graduation
   Flight moved out of its published feature surface
7. `queryfabric-adapter-postgres`
8. `queryfabric-adapter-clickhouse`
9. `queryfabric-opt`
10. `queryfabric`

The following currently publishable crates become registry-unpublished
(`publish = false`) until a later graduation phase proves them:
`queryfabric-changelog`,
`queryfabric-cli-toolbelt`, `queryfabric-cmd-runner`,
`queryfabric-release`, `queryfabric-runtime-k8s`,
`queryfabric-seaorm-ext`, `queryfabric-test-rig`,
`queryfabric-types`, and `queryfabric-worker`.

This list must be generated/checked from Cargo metadata rather than duplicated
across scripts and documentation. `queryfabric-portability` and its support
crates may remain registry-unpublished for 0.2, but their bundle schema,
fixtures, source, and tests are public FLOS deliverables. `publish = false`
must never be described as closed-source or private.

## Grant Milestone Overlay

This is an engineering plan, not proposal prose or a budget. No owner, effort,
rate, amount, adopter, or upstream commitment is inferred.

| Plan work | Grant treatment | Public verification |
|---|---|---|
| Phase 00 | prerequisite/compliance; not a standalone R&D claim | reproducible root gates and one canonical lineage |
| Phase 01 | downstream regression repair; exclude from proposed funded milestones by default | SynDB production-path tests |
| Phase 02 | compiler/security baseline; fund only genuinely new, scoped R&D | adversarial compiler and release-tier evidence |
| Phase 03 | extraction convergence/maintenance; exclude by default | duplicate removal and downstream adoption |
| Phase 04A | primary portability-format R&D outcome | schema/profile, fixtures, validator/import tests, threat-model evidence |
| Phase 04B | primary host-application R&D outcome | dry-run/apply, target mapping, durable state, role separation, replay/failure evidence |
| Phase 04C | primary deployability and impact proof | independent NixOS export-transfer-import test, restart proof, docs/accessibility/footprint evidence, release handoff |
| Phases 05-06 | post-MVP extensions outside the default application scope | their own later RFCs and black-box gates |
| Phase 07 | post-MVP disposition and plan retirement | fresh progress audit and evidence-preserving cleanup |

Each proposal milestone derived from this table still needs applicant-supplied
effort, rate, schedule, dependencies, risks, and a public result location.

## Whole-Plan Guardrails

- Work from a new branch based on canonical QueryFabric `trunk`. The vendored
  topic branch is evidence, not a merge base.
- Do not delete source behavior until its upstream tests pass and SynDB has
  switched consumers.
- Do not use `/tmp` Cargo isolation as the final workflow gate; repair the dev
  shell in Phase 00.
- Do not bypass failed pure Nix evaluation when claiming SynDB compatibility.
- Do not convert ignored tests into release evidence.
- Do not invent missing worker images, charts, grant facts, package releases,
  or registry state.
- Do not treat content hashes as signatures. Until a signature design exists,
  authenticity comes from an authenticated operator plus an expected hash
  delivered through a trusted channel.
- Do not fetch bundle-provided URIs or JSON-LD contexts automatically. The host
  must apply an explicit scheme/domain/path policy and bounded transfer.
- Do not use AI-assisted planning text in an application unless the applicant
  has preserved the disclosure and prompt/output record required by NLnet.
- Stop a phase when a required source artifact is missing and record the
  producer, regeneration workflow, and proof command from the dossier.
- Publishing, tagging, pushing, and deployment remain explicit operator
  actions even after the local release-candidate gate passes.

## Whole-Plan Acceptance

Before calling the MVP complete:

- [ ] QueryFabric's normal repo-root build, Clippy, test, `nix flake check`,
      REUSE, docs, default/all-feature, MSRV, and package gates pass.
- [ ] the canonical applications checkout's grant packet has producer-supplied
      copyright and SPDX metadata before submission; it remains outside the
      QueryFabric release tree rather than being silently relicensed.
- [ ] adversarial tests cover identifiers and every other catalog-derived
      emitted token, and CTE error-propagation tests pass for PostgreSQL and
      ClickHouse.
- [x] the demo executes a parameterized query and returns result schema,
      provenance, diagnostics, and catalog snapshot identity.
- [ ] the demo rejects unauthorized/malformed/over-budget requests and executes
      through a read-only database role with cancellation.
- [x] the existing NixOS self-host VM test covers the new query contract and
      relevant rejection cases, and a separate multi-node migration check
      isolates alpha, beta, and transfer credentials.
- [x] alpha exports a profile-conforming tabular bundle and artifact, an
      operator transfers them without sharing alpha's database/bucket
      credentials, and an initially empty beta dry-runs and imports them.
- [x] beta returns the imported rows plus origin-attributed source evidence,
      local policy/owner and target import event, persists the mapping/receipt,
      survives restart, and treats replay idempotently.
- [ ] apply re-authorizes and binds the plan to the current target revision and
      immutable staged objects, then revalidates bytes/schema/row count before
      commit.
- [ ] schema-admin, read-only query, and narrow import-writer principals are
      distinct, and the query path cannot reach the writer.
- [ ] tampered/unsupported/oversized bundles and artifacts, forbidden URIs,
      conflicts, unauthorized imports, and injected mid-apply failure leave no
      partially visible resource.
- [ ] accessibility automation and a scoped manual review are published without
      claiming unproven WCAG conformance.
- [ ] offline-capable Nix audit/deny checks use a flake-locked RustSec database
      and are the same checks CI executes.
- [x] documentation examples either compile in a real test target or are
      explicitly classified as non-Rust/illustrative; the portability chapter
      uses only implemented APIs.
- [ ] release footprint evidence is measured twice from the clean candidate
      under a tolerance declared before measurement.
- [ ] governance names only real maintainers and records the actual decision,
      release, security, and continuity process without inventing a team.
- [ ] the NixOS package/module has an upstream-style self-contained test,
      generated option docs, and a reviewable patch/handoff; no public
      upstream/adoption claim precedes an authorized real issue/PR or merge.
- [ ] SynDB's submodule and flake input resolve to the same reachable canonical
      QueryFabric commit.
- [ ] SynDB focused tests plus workspace build, Clippy, and test pass under its
      repaired Nix/UV environment.
- [ ] only the ten stable compiler crates are packageable/publishable.
- [ ] no publish workflow targets a registry-unpublished crate, and no stable
      crate exposes the unfinished Flight feature.
- [ ] exact-SemVer release automation has one source of truth and no guaranteed
      failing artifact job.
- [ ] documentation accurately distinguishes compiler, demonstrator,
      experimental Flight, isolated execution, federation, and unpublished
      packages.
- [ ] proposal-facing material distinguishes the bounded tabular-resource
      migration proof from arbitrary resources, dynamic schema creation, full
      service migration, or HA and contains no unresolved link to a missing
      grant plan.

## Plan Completion And Retirement

Phase 07 must run a fresh progress audit against both repositories. Only then
may the old SynDB `queryfabric-upstream` plan be removed or condensed into
durable architecture/migration documentation. Unresolved work must be moved
forward explicitly; unchecked files must not disappear merely because this
replacement plan exists.
