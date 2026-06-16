# NGI Fediversity 12th Call — Application Answers (QueryFabric)

**Deadline:** 2026-08-01 12:00 CEST · **Target submission:** by 2026-07-29
**Applicant:** Can H. Tartanoglu · canhtart@gmail.com
**Premise:** these answers assume the locally evidenced readiness work from
[ngi-fediversity-application-plan.md](ngi-fediversity-application-plan.md) has landed:
release gate green, ROADMAP.md, threat model, footprint benchmarks, HA design doc,
and multi-instance NixOS module. The local release state is documented separately in
`docs/grants/claim-evidence-map.md`; public publication and tagging are not
claimed here, while the live public demo URL is recorded there.

**Submission status:** not ready to submit. Applicant-owned crates.io status, demo
hosting cost, prior NGI/NLnet status, and QueryFabric funding/employment boundary
facts are still missing from the evidence map and must be supplied by the applicant
before 2026-07-29.

---

## 1. Abstract — "Can you explain the whole project and its expected outcome(s)?"

QueryFabric is an Apache-2.0 Rust toolkit for portable analytical data in
self-hosted and federated services. It is embedded as a library rather than run as a
data-owning service. Today it provides a verified portable SQL subset for PostgreSQL
and ClickHouse, a conformance corpus, fuzz targets, GDPR Articles 15/16/17 traits,
content-addressed export bundles with provenance, multi-tenant isolation, libp2p
federation scaffolding, and NixOS deployment wiring.

The problem is service portability at the data layer. NixOS can make a service
declarative, but service data still tends to remain tied to backend-specific SQL
dialects, schemas, and storage formats. QueryFabric gives hosts a stable semantic
boundary so moving analytical data between instances, providers, and database
backends can be verified rather than handled by lossy export scripts.

This grant funds the step from data decoupling to service portability: WP1 adds
import-side bundle ingestion and round-trip export, transfer, import, and verify
flows; WP2 hardens federation and high availability; WP3 adds an embedded backend
and differential conformance testing across three backends; WP4 funds security
hardening, external audit follow-up, nixpkgs upstreaming, and Fediversity
deployment-stack integration.

Expected outcome: a self-hosted operator can migrate analytical data between
instances and backends with verified fidelity, declarative NixOS deployment,
federation, and GDPR data-rights primitives.

---

## 2. "Have you been involved with projects or organisations relevant to this project before?"

I am the author and maintainer of QueryFabric itself. I am a Ph.D. candidate at
DZNE / Charite, where my thesis is SynDB: a federated data platform for
high-resolution microscopy data spanning 100+ TB and 15 facilities. QueryFabric
was extracted from that scientific-data and federation background into a neutral
standalone project: host-specific routing, auth, execution policy, and domain
metadata stay outside the QueryFabric crates, while QueryFabric owns portable
parsing, binding, diagnostics, emission, provenance, portability, tenancy, and
federation primitives.

Relevant prior and ongoing work:

- **NixOS and open source**: I maintain libraries on GitHub and Codeberg and am a
  Nixpkgs contributor using flakes, crane, and fenix. My public profiles are
  GitHub: https://github.com/caniko and Codeberg: https://codeberg.org/caniko.
- **Scientific/data systems**: I built SynDB in Rust (Tokio, Axum, Arrow Flight,
  Kameo), federating 100+ TB across 15 electron-microscopy facilities, and
  designed data-sovereign federation with concurrent Arrow Flight ingestion.
- **NixOS-deployed systems**: my prior project work includes NixOS deployment, and
  QueryFabric carries that operational model into its NixOS module and VM tests.
  Exact fleet-size numbers are not claimed here because they are not present in the
  available evidence.
- **Rust and product systems**: I built ScoreMyCrypto B2B API backends for
  Bitcoin/Solana risk forensics, solo-designed and implemented Pink Raven as a
  rights-aware architecture precedent platform with semantic search, pgvector, and
  NixOS deployment, and built Mnemo as AI-agent memory with Tokio/Axum, SurrealDB,
  MCP/REST, and WASM/IndexedDB.

Applicant-owned facts still required before this answer can be submitted:
exact nixpkgs PR/package examples if the applicant wants to claim them, any
crates.io profile URL or explicit non-publication statement, and any previous
NGI/NLnet funding status.

---

## 3. Requested amount

**€45,000**

---

## 4. "Explain what the requested budget will be used for? Does the project have other funding sources? Breakdown in main tasks with effort, rates explicit."

The entire budget funds development labour at a single explicit rate of **€75/hour**,
600 hours total, performed by me as an independent developer. There are no hardware,
travel, or subcontracting costs. The project deliberately targets commodity hardware;
local footprint measurements show a 17M release binary, 2.9 MiB idle RSS, and a
service-only sizing floor of 1 vCPU / 128 MiB. The full single-box stack with local
Postgres should use at least 1 vCPU / 512 MiB. The actual public-demo hosting plan
and monthly cost still require applicant confirmation before submission.

**Other funding sources:** applicant confirmation is still required before submission.
Do not submit this answer until the applicant has confirmed QueryFabric's past and
present funding, employment, ownership, and rights boundary. The repository evidence
supports the QueryFabric governance and technical boundary, and CourseOfLife confirms
current and prior roles, but neither source proves the project's external funding or
employer-rights situation.

### Task breakdown

| # | Work package | Main tasks | Hours | Cost |
|---|---|---|---|---|
| WP1 | Service portability, end to end | Import-side bundle ingestion: content-hash verification, catalog re-binding against a differently-shaped target schema, provenance-chain continuity; round-trip CLI (`export → transfer → import → verify`); operator documentation | 180 | €13,500 |
| WP2 | Federation & high availability | Hub failover and re-election; NAT traversal for instances behind home connections; schema-sync conflict resolution; HA deployment guide promoting the existing design doc from "planned" to "implemented"; multi-node VM test | 150 | €11,250 |
| WP3 | Backend breadth for small hosters | Embedded backend (SQLite or DataFusion, selected by measured footprint); conformance-corpus expansion; differential testing of the portable subset across all three backends in CI | 160 | €12,000 |
| WP4 | Security, community & Fediversity integration | Threat-model-driven hardening; remediation of external security-audit findings; nixpkgs module upstreaming; integration with the Fediversity deployment stack; operator docs for non-Rust audiences; contributor onboarding (issue curation, architecture walkthroughs) | 110 | €8,250 |
| | **Total** | | **600** | **€45,000** |

Each work package concludes with a release checkpoint in the project's staged
release process (`RELEASE.md`) and CHANGELOG entries, giving natural, verifiable
payment milestones. Effort estimates derive from the project's own history:
comparable phases (federation extraction, sovereignty crates, NixOS module + VM test)
each landed in 120–180 focused hours, with the codebase and decision log to show for it.

---

## 5. "Compare your own project with existing or historical efforts."

The closest efforts each solve a different slice of the problem; none combines verified
query portability with data-sovereignty primitives in an embeddable, self-hosting-sized
package:

- **Apache Calcite** is the historical reference for pluggable query compilation, but it
  is a JVM framework aimed at database builders — heavyweight for self-hosted services,
  with no data-portability or GDPR layer.
- **Apache DataFusion and Substrait** focus on query *execution* and plan interchange.
  QueryFabric deliberately does not execute queries (a documented design boundary,
  DECISIONS.md D003): it is the compile-time semantic layer *above* an executor.
  DataFusion is an integration target in WP3, not a competitor; Substrait plan emission
  is on the public roadmap.
- **PRQL and sqlglot** transpile query syntax between dialects, but without catalog
  binding, capability verification, or a conformance corpus — they translate text, they
  cannot tell an operator *whether the result means the same thing* on the target
  backend. QueryFabric's verified portable subset is exactly that guarantee.
- **Trino/Presto** federate query execution across sources, but as a heavy multi-node
  JVM deployment aimed at data warehouses, with the engine owning the data path —
  the opposite of the small-footprint, host-in-control model this call asks for.
- **PostgREST and Hasura** expose a single database as an API; they neither abstract
  over backends nor provide export/import semantics.
- On the sovereignty side, **Solid** re-establishes user data ownership at the personal
  data-store level; QueryFabric addresses the complementary layer — analytical data held
  by services — and could interoperate with such stores as export targets.

Historically, "write once, run on any database" efforts (ODBC, JDBC, SQL standards
compliance) failed at the semantic edges: identical syntax, divergent behaviour.
QueryFabric's answer is to verify a smaller subset rather than promise the whole
language — a conformance corpus and differential fuzzing across backends instead of a
specification nobody implements identically.

---

## 6. "What are significant technical challenges you expect to solve during the project?"

1. **Semantic fidelity across backends.** The hard part of portability is not syntax but
   meaning: NULL ordering, collation, integer division, timezone arithmetic and
   overflow behaviour all diverge between PostgreSQL, ClickHouse, and embedded engines.
   WP3 turns the existing conformance corpus into a differential-testing harness that
   executes the corpus on all three backends and treats any divergence as a bug in the
   portable-subset definition — expanding the subset only as fast as it can be verified.
2. **Import into a differently-shaped world (WP1).** Export is easy; import is the
   research problem. A receiving instance has its own catalog, type mappings, and
   capability profile. The importer must re-bind a bundle against that catalog, verify
   the content hash and provenance chain end-to-end, surface semantic mismatches as
   structured diagnostics rather than silent coercions, and remain idempotent under
   interrupted operation retries.
3. **Federation without central trust (WP2).** Hub failover and schema-sync conflict
   resolution must work over libp2p between instances run by different operators on
   residential connections — NAT traversal, partition tolerance, and a wire protocol
   that stays stable while the implementation hardens. The protocol is already
   wire-versioned; keeping it so under these changes is the discipline challenge.
4. **Security at the SQL boundary (WP4).** A query compiler is an injection surface by
   definition. The threat model (published, docs/src) drives this: placeholder handling,
   capability misclassification (a query believed "read-only" that is not), and trust
   in federation messages. Fuzzing already gates CI; the external audit and its
   remediation extend that to the new import and federation surfaces.
5. **Staying small.** Every work package must preserve the measured footprint
   (17M release binary, 2.9 MiB RSS idle, and a service-only floor of 1 vCPU /
   128 MiB; the full single-box stack with local Postgres should use at least
   1 vCPU / 512 MiB — see the benchmark chapter). Resource efficiency is a
   stated project constraint, not an afterthought; the embedded-backend choice
   in WP3 is explicitly decided by footprint.

---

## 7. "Describe the ecosystem of the project, and how you will engage with relevant actors and promote the outcomes?"

QueryFabric sits between three communities, and the engagement plan addresses each:

**The Fediversity / NixOS hosting ecosystem.** The NixOS module is the project's
front door: WP4 upstreams it to nixpkgs and aligns it with the Fediversity deployment
stack so that hosting providers in the pilot can offer QueryFabric-backed data
portability as a catalog service. I will engage directly with the consortium during the
project (module compatibility, NixOps4 deployment review) and present the work at
NixCon and FOSDEM (Nix and distributions devrooms) — communities adjacent to my
Nixpkgs contribution and NixOS-deployed project work.

**Fediverse and self-hosted service developers.** The promise of WP1 — verified
instance-to-instance data migration — is expressed through a worked end-to-end
migration example between two instances of the reference host (`queryfabric-demo`),
plus a design study applying the same pattern to a typical Fediverse service's
GDPR-export and instance-statistics needs. Integration is deliberately low-friction:
QueryFabric is an embeddable library with C-free pure-Rust core and Python bindings,
not another service to operate.

**The research-data community.** The portability layer already mints DataCite DOIs and
emits citation metadata — analytical exports double as citable research artifacts. This
second user community (scientific platforms, the project's origin) is a deliberate
bridge: the same bundle format serves a lab moving data between institutions and a
Fediverse instance honouring a GDPR request.

**Promotion of outcomes.** Every work package ends in a release checkpoint and an
announcement post; the mdBook documentation site and Zola website are published from
the same repository; the conformance corpus is released as a standalone, reusable
artifact that other query tools can test against — a small standards contribution in
itself. The repository is Apache-2.0, has a public roadmap (ROADMAP.md), and keeps
the grant work packages visible as roadmap items so reviewers can verify that grant
work lands where the proposal said it would. The public repository is
https://codeberg.org/caniko/queryfabric. Any live published documentation or website
URL still requires applicant confirmation before submission.

---

## Pre-submission checklist for these answers

- [x] No unresolved bracket placeholders remain, except ordinary Markdown links and
      Markdown task-list checkboxes.
- [ ] Applicant-owned crates.io status, prior NGI/NLnet status, demo hosting cost,
      and QueryFabric funding/employment/rights boundary supplied and verified.
- [x] Public demo instance is up and the URL resolves over HTTPS.
- [x] Footprint numbers re-measured on the local release-profile build cited here.
- [x] Live form fields checked on 2026-06-14; abstract is limited to 1500
      characters and the answer above is under that limit.
- [ ] Every retained claim has a public URL behind it, or a local validation command
      where the release is intentionally still unpublished.
- [ ] Peer read-through done.
- [ ] Submit by 2026-07-29.
