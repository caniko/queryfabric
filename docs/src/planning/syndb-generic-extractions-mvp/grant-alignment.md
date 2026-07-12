# NGI Fediversity Alignment For The QueryFabric MVP

- **Status:** verified engineering orientation; not an application or budget
- **Observed:** 2026-07-12
- **Local context pack:** applications checkout `docs/grants/NGI_Fediversity_2026_LLM_Context.{md,json}`
  (outside the QueryFabric release tree)
- **Authority:** current official NLnet/Fediversity sources override the local
  context pack
- **AI provenance:** this planning document was produced with AI assistance;
  see [Proposal-use provenance](#proposal-use-provenance)

## Purpose

Use the supplied grant research to improve the MVP boundary without turning an
engineering plan into applicant-authored proposal prose. This document records
verified programme constraints, the technical delta they create, portfolio
positioning, and the facts that still require the applicant or upstream
projects.

It deliberately does **not** supply an applicant identity, European dimension,
partners, adopters, prior funding, patents, schedule, hours, rates, overhead, or
requested amount. None of those facts can be inferred from the repository.

## Source Authority And Verification

The four supplied files were present and their JSON variants parsed with
`jq empty`. Their SHA-256 digests at observation time were:

| Artifact | SHA-256 |
|---|---|
| `applications/docs/grants/Generic_Grant_Application_Template.json` | `0468682d53553daa47398246efcd1fe62e32bd232a40699cc0c52d6a1c63edf4` |
| `applications/docs/grants/Generic_Grant_Application_Template.md` | `1fbdaab410589a4c2446f951a35f533af92c90d61bedb827095f48eac79ba644` |
| `applications/docs/grants/NGI_Fediversity_2026_LLM_Context.json` | `0115afa7b72e28581215dcffc85cdb8e5d8c13a0f4c03bd80974e99f767a3361` |
| `applications/docs/grants/NGI_Fediversity_2026_LLM_Context.md` | `d5768db9440ca92c450318f4c98290da7cd3864c208ea3892faf276b12463953` |

The pack was checked against these official sources:

- [Call 12](https://nlnet.nl/fediversity/)
- [Guide for applicants](https://nlnet.nl/fediversity/guideforapplicants/)
- [Eligibility rules](https://nlnet.nl/fediversity/eligibility/)
- [FAQ](https://nlnet.nl/fediversity/faq/)
- [Live proposal form](https://nlnet.nl/propose/)
- [Generative-AI policy v1.1](https://nlnet.nl/foundation/policies/generativeAI/)
- [Fediversity portfolio](https://nlnet.nl/thema/NGIFediversityFund.html)
- [Programme background](https://nlnet.nl/fediversity/background/)

Materially confirmed facts are: the call is open until 2026-08-01 at 12:00
CEST; requests are EUR 5,000-50,000 subject to the EUR 60,000 lifetime cap;
Stage 1 is scored 30% technical feasibility, 40% relevance/impact, and 30%
value for money; and milestones need public, verifiable FLOS/open-access
results. English, alignment, R&D primacy, and European dimension are the four
explicit Stage-1 knock-out categories. FLOS/open-access output and cost
eligibility are also mandatory conditions, but the guide does not label them
as additional items in that four-part list.

The pack's AI guidance is directionally correct. The live v1.1 policy permits
disclosed use, while the FAQ's short answer prefers applicant-written text.
The safest path is applicant-written final prose with the complete required
disclosure retained separately.

## Readiness Matrix

| Requirement or claim | Current status | Evidence or missing producer |
|---|---|---|
| English application | ready as a constraint | repository and planning materials are English; final form still belongs to applicant |
| R&D as primary purpose | conditionally aligned | import validation, transactional application, trust policy, and conformance are new R&D; regression repair/routine extraction should not be sold as grant milestones |
| Fediversity alignment | strong if bounded | NixOS reproducibility, provider portability, PostgreSQL/S3 resources, security, docs, and public conformance fit official themes |
| European dimension | **unknown** | applicant must provide concrete European relevance, users, collaborators, standards, or ecosystem effect |
| FLOS/open access | split by repository | QueryFabric source is REUSE-clean; the applications checkout's four moved grant files and 24 other documentation files still lack producer-supplied metadata |
| Public verifiable results | not yet achieved | no public QueryFabric tag and no crates.io release were found; source/tests can become milestone evidence only after reachable publication |
| Technical feasibility | internal proof now present, release evidence incomplete | compiler, bounded import, durable receipt/state, and independent alpha/beta migration test pass; accessibility, authority separation, and release audits remain |
| Impact | plausible, not established | portability benefit is designed; adopters, provider demand, and upstream acceptance are applicant/community evidence |
| Value for money | **unknown** | applicant must supply tasks, efforts, rates, dependencies, and requested amount |
| Prior/other funding | **unknown** | applicant/accounting records |
| Patents/IP constraints | **unknown** | applicant and rights holders |
| GenAI application disclosure | **blocked** | native conversation/model/timestamp/prompt/unedited-output record is not present in the repository |

## Grant-Informed MVP Boundary

The compiler remains the security and portability baseline, but a compiler-only
release does not demonstrate the programme's strongest impact. The MVP gate is
therefore:

> A typed SQL/SyQL compiler plus a reproducible NixOS proof that an authorized
> operator can export, transfer, validate, dry-run, and import a tabular
> resource conforming to a published profile between two independently
> configured QueryFabric hosts, preserving declared portable evidence and
> rejecting tampering without partial visibility.

This is a bounded **tabular resource/data portability** claim. It does not
cover arbitrary resources, dynamic schema/DDL, an entire application service,
a provider account, or an arbitrary database. It also is not high availability.

### 1. Compiler/security baseline

- complete identifier and emitted-token safety;
- typed parameters and stable catalog snapshot identity;
- structured diagnostics, capabilities, result schema, and provenance;
- prepared read-only PostgreSQL execution with limits and cancellation; and
- reproducible compiler packages and downstream SynDB proof.

### 2. Neutral portability layer

`queryfabric-portability` retains bundle 1.0 as legacy export-only evidence and
adds import-ready bundle 2.0 plus one normative tabular CSV profile. Version
2.0 uses RFC 8785 JSON canonicalization, typed `blake3-256` digests, a typed
ordered column schema, and cross-language vectors. The import path verifies the
bundle's expected digest and every staged artifact's actual digest, byte/row
count, format, and schema fingerprint before making a host-visible change.
Dynamic DDL/catalog registration and arbitrary resource profiles are outside
the MVP.

The neutral layer carries JSON-LD, citations, licence, data-use restrictions,
and source provenance as origin-attributed evidence. The current bundle carries
neither full `AccessPolicy` nor ownership; source actors never become target
authorization facts. It does not fetch remote JSON-LD contexts or
bundle-provided URIs. `storageUri` is source metadata; the host binds each
manifest to an already staged target object without rewriting the canonical
bundle. The neutral layer reports source identifiers and conflicts; the host
decides predeclared-relation mapping, accepted URI schemes, artifact retrieval,
authorization, trust, storage, and transaction policy.

A BLAKE3 digest proves integrity only relative to a trusted expected digest.
Neither bundle version has a signature. For the MVP, an authenticated operator
must supply the expected digest through a trusted channel. Documentation must
remove any “signed bundle” claim unless a separately reviewed signature and key
trust design is actually implemented.

### 3. Reference-host import

The source host exports a bundle and artifact. An operator transfers them into
the target host's staging area without giving the target source-database or
source-bucket credentials. The target:

1. authenticates and authorizes the import request;
2. performs a bounded dry-run and returns mapping/conflict diagnostics;
3. verifies the expected bundle digest and all staged artifact facts;
4. re-authorizes and revalidates target revision plus immutable staged bytes at
   apply time;
5. applies the supported tabular profile to a predeclared relation, assigns a
   local owner/policy, and records an origin-linked target import event under
   one host transaction/receipt;
6. exposes the new resource only after the durable commit; and
7. treats an identical replay as an idempotent success and a conflicting replay
   as an explicit error.

Object-store staging and PostgreSQL cannot be made one distributed ACID
transaction. “Atomic” therefore means atomic **visibility**: a failed apply
commits no resource, carried source evidence, local policy/owner, target import
event, mapping, or receipt; staged objects remain unreferenced and are eligible
for cleanup.

The current demo always seeds identical data and keeps provenance/ownership in
memory. Phase 04 must separate schema migration from demo-data seeding, start
the target empty, and persist imported rows, receipt, origin-attributed source
evidence, local policy/owner, target import event, and mapping so restart is
meaningful. Separate database principals/pools cover
schema migration, read-only query execution, and narrow import-state writes;
the query path must not be able to reach the writer.

### 4. Reproducible provider-oriented host proof

Retain the current single-host VM test for module regression, and add a
multi-node migration check. Alpha and beta must use independent database and
object-store endpoints, distinct credentials/state/catalog identities, and no
pre-seeded target resource. A scoped transfer node/unit receives source-read
and target-staging-write credentials only. The check covers export -> operator
transfer -> target dry-run -> import -> query -> restart -> query, plus
unauthorized, tampered, unsupported-version, oversized, forbidden-URI,
conflict, replay, and injected-failure paths.

Security, documentation, accessibility, and footprint results are part of the
MVP evidence rather than post-MVP cleanup. Automated accessibility checking is
paired with a published manual keyboard/labels/contrast/scope report; no WCAG
conformance is claimed before that evidence exists. Resource measurements are
run twice from the clean candidate under a tolerance chosen before measuring.
Governance documentation must name only real maintainers and record the actual
decision/release/security/continuity process, including single-maintainer risk
if that is the current state.

## Conditional Fediversity Integration Proof

The official Fediversity repository was inspected at immutable commit
[`0e4ab02db40b188898531ad36b5eb03c6e46a431`](https://git.fediversity.eu/Fediversity/Fediversity/src/commit/0e4ab02db40b188898531ad36b5eb03c6e46a431/README.md).
It says the project is in development and targets hosting providers/operators,
not universal self-hosting. QueryFabric's NixOS demonstration should therefore
be described as a provider-oriented deployability and
tabular-resource-migration conformance fixture, not evidence of use by real
providers.

The current resource shapes are close to QueryFabric's host needs:

- PostgreSQL exposes a secret `urlFile` and separate `sslMode` in the
  [PostgreSQL contract](https://git.fediversity.eu/Fediversity/Fediversity/src/commit/0e4ab02db40b188898531ad36b5eb03c6e46a431/nix/contracts/lib/contracts/definitions/postgresql.nix).
- S3 exposes endpoint, port, bucket, region, `accessKeyIDFile`, and
  `secretAccessKeyFile` in the
  [S3 contract](https://git.fediversity.eu/Fediversity/Fediversity/src/commit/0e4ab02db40b188898531ad36b5eb03c6e46a431/nix/contracts/lib/contracts/definitions/s3.nix).
- QueryFabric already accepts PostgreSQL `database.urlFile`, but its S3 module
  expects one environment-style `store.credentialsFile` and one combined
  endpoint URL.

The candidate integration work is to add separate access-key/secret-key file
options, preserve the combined file as a compatibility shim, preserve
PostgreSQL TLS mode, and build a thin adapter plus Garage/PostgreSQL VM check.
The adapter belongs at the Nix host boundary, not in the stable Rust compiler
API. QueryFabric must not copy Fediversity's contract framework.

This is **not yet an MVP blocker or a conformance claim**. Before it becomes a
release gate, Fediversity maintainers must identify a supported external
application-contract boundary and QueryFabric maintainers must pin an immutable
tag/revision. If that artifact is unavailable, the grant milestone is contract
discovery plus an isolated adapter prototype, not invented compatibility.

## Portfolio-Relative Differentiation

The current [official portfolio](https://nlnet.nl/thema/NGIFediversityFund.html)
contains work on NixOS fleet lifecycle, source packaging, closure distribution,
adaptive placement, verified boot, licence metadata, service catalogs,
self-hosted storage, and ActivityPub integration. None of the listed project
descriptions is a typed SQL/SyQL compiler or analytical-query conformance host.
That supports only this bounded statement:

> Relative to the current Fediversity portfolio, QueryFabric contributes a
> typed, capability-checked query and data-portability boundary; it does not
> duplicate fleet management, service placement, storage products, verified
> boot, or application-level federation.

Do not use “first”, “only”, or “unique” without a separate global comparison.
Keep scheduling/placement outside QueryFabric; [NixEdgeOpt](https://nlnet.nl/project/NixEdgeOpt/)
already occupies that problem. Treat Nix packaging as delivery evidence, not
novelty. Treat the query catalog as a schema abstraction, not a service catalog.
Describe access/export/erase endpoints as tested technical mechanisms, not
proof of GDPR or other legal compliance.

## Public Milestone Evidence

| Engineering result | Objective proof | Claim boundary |
|---|---|---|
| compiler safety baseline | adversarial facade/adapter tests and stable release-tier metadata | portable compilation, not production hosting |
| import contract | public 2.0 schema/RFC 8785 vectors/tabular profile plus validator/planner/tamper/replay tests | one profile into predeclared relations, not arbitrary resources/services |
| independent-host migration | multi-node NixOS export-transfer-import-restart check | one tabular profile/reference topology, not HA |
| security/accessibility/docs | threat-model cases, cargo audit/deny/REUSE gates, accessibility check/manual report, tested examples | scoped evidence, not formal certification |
| reproducibility/efficiency | flake checks and two clean footprint runs under predeclared tolerance | measured reference host, not universal low-resource operation |
| upstream readiness | nixpkgs-conformant module test, generated options, and reviewable patch/handoff | no upstream/adoption claim until an authorized public PR is opened/merged |
| release | reachable source revision/tag, changelog/status page, reproducible commands | registry availability only after actual publication |
| Fediversity adapter, if supported | immutable upstream contract pin, Garage/PostgreSQL check, public upstream issue/PR after maintainer authorization | compatibility with the pinned interface, not endorsement/adoption |

Proposal work packages may be derived from these rows only after the applicant
adds real effort, rate, schedule, dependency, risk, and amount data. Already
completed work and routine maintenance must not be presented as future funded
R&D.

## Work Kept Outside The MVP

- generic Flight migration and the Kubernetes worker/runtime;
- production federation, hub failover, NAT traversal, active-active data plane,
  and adaptive placement;
- SQLite/DataFusion implementation before a measured choice;
- broad CLI/test-rig and optional-crate cleanup;
- whole SynDB domain/query/database extraction;
- formal external audit procurement, unless separately scoped and eligible; and
- routine hosted-service operation.

HA or an embedded backend can become later grant-aligned R&D only through a
separate phase with its own feasibility question, budget, and black-box proof.
Neither is required to make the bounded portability MVP credible.

## Applicant-Owned Inputs Still Required

Before application drafting or budgeting, the applicant must supply and verify:

- applicant/legal/team/contact/location facts and the concrete European
  dimension;
- project history, current contributors, actual users/adopters, and any real
  upstream or provider discussions;
- prior and concurrent funding, patents/IP/third-party rights, and employer or
  ownership boundaries;
- exact future tasks, named responsible people, hours, rates, dependencies,
  dates, overhead if any, and requested amount;
- which public repositories/pages will carry each milestone result; and
- whether and how the Fediversity maintainers support an external application
  contract.

Missing data blocks proposal claims; it does not block the engineering plan.
Do not copy stale applicant facts, budgets, or release claims from the vendored
SynDB QueryFabric history.

## Proposal-Use Provenance

This plan, the associated research dossier, and the Phase 04 revisions were
AI-assisted in a Codex conversation on 2026-07-12. The repository does not
contain the client-displayed model label, per-message timestamps, every prompt,
or every unedited response. Those required application-record facts must not be
reconstructed from memory. A separate exact version is prudent when exposed and
is explicitly relevant to generated project content, but the live application
form asks for the model rather than listing version as a separate field.

The upstream producer is the current conversation platform/session. Before any
text or substantive analysis from this work informs a submission, the
applicant must export or preserve the native full conversation, record the
model label and timestamps shown by the client, retain every prompt and
unedited output, preserve the exact version too if exposed, and add the
disclosure required by the live NLnet form and policy. Validate the record by
comparing it message-for-message with the original session. If the native
record cannot be obtained, do not reuse AI-generated proposal prose; write the
application independently from the linked primary sources and disclose any
remaining substantive AI assistance.

The four grant artifacts now sit in the applications checkout, so they no
longer block QueryFabric's release-tree REUSE gate. The applications checkout
still needs the producer-supplied rights holder and SPDX licence for these
files (and its other unannotated documentation) before it can pass `reuse
lint`. QueryFabric maintainers must not assign those facts on the producer's
behalf.
