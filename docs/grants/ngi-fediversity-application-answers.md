# NGI Fediversity 12th Call — Application Answers (QueryFabric)

**Deadline:** 2026-08-01 12:00 CEST · **Target submission:** by 2026-07-29
**Applicant:** Can H. Tartanoglu · canhtart@gmail.com
**Premise:** these answers assume all Tier 1 + Tier 2 readiness work from
[ngi-fediversity-application-plan.md](ngi-fediversity-application-plan.md) has landed:
v0.2.0 tagged and on crates.io, REUSE-compliant, public demo instance live, ROADMAP.md,
threat model, footprint benchmarks, HA design doc, and multi-instance NixOS module.

Items in `[square brackets]` are facts only the applicant can supply — replace before
submitting. Everything else is written to be pasted into the form as-is.

---

## 1. Abstract — "Can you explain the whole project and its expected outcome(s)?"

Self-hosting fails at the data layer. Services trap user data in backend-specific SQL
dialects, schemas, and storage formats, so "moving to another instance or provider" means
lossy exports and manual surgery — exactly the lock-in between content owner and service
provider that the NGI 2025 study identifies. Hosting platforms can deploy a service
declaratively with NixOS, but they cannot yet move its *data* with the same confidence.

QueryFabric closes that gap. It is an open-source (Apache-2.0) Rust toolkit — 35 crates,
embedded as a library rather than run as a data-owning service — that gives self-hosted
and federated services a portable data layer:

- a **verified portable SQL subset** that compiles to multiple backends (PostgreSQL and
  ClickHouse today), backed by a published conformance corpus and continuous fuzzing;
- **data-sovereignty primitives**: GDPR Articles 15/16/17 (access, rectification,
  erasure) as first-class library traits, content-addressed export bundles with full
  provenance history, and multi-tenant isolation;
- a **federation protocol** (libp2p) for multi-instance deployments with health
  monitoring and schema synchronisation;
- a **hardened NixOS module** with end-to-end VM tests, multi-instance support, and a
  secrets-never-in-store credential pattern — a live demo instance runs at
  [demo URL], deployed from exactly that module.

This grant funds the step from *data decoupling* to *service portability*: (WP1)
import-side bundle ingestion, so a receiving instance can verify, re-bind, and adopt an
exported dataset — making instance-to-instance migration a verified round trip; (WP2)
federation and high-availability hardening (hub failover, NAT traversal, schema-sync
conflict resolution); (WP3) an embedded backend for small self-hosters plus differential
conformance testing across three backends; (WP4) threat-model-driven security hardening,
an external audit, nixpkgs upstreaming, and integration with the Fediversity hosting
stack.

Expected outcome: an operator of a self-hosted service can migrate analytical data
between instances, providers, and database backends with verified fidelity — deployed
declaratively on NixOS, with federation and GDPR data rights built in, on hardware as
small as a 1-vCPU VPS.

---

## 2. "Have you been involved with projects or organisations relevant to this project before?"

I am the author and maintainer of QueryFabric itself, which I extracted from
[SynDB — one sentence: what it is, e.g. "a self-hosted scientific data platform I
build and operate"] into a neutral, standalone project with its own governance
(see GOVERNANCE.md) precisely so that other hosts could adopt it without inheriting
host-specific policy.

Relevant prior and ongoing work:

- **NixOS / nixpkgs**: contributor to nixpkgs ([name 1–3 packages/PRs, e.g. the
  onnxruntime package work]); I maintain a multi-host NixOS fleet configuration
  covering [N] machines with declarative deployment, agenix secrets, and a self-hosted
  binary cache (Attic) — the operational experience the QueryFabric NixOS module and its
  VM tests are built on.
- **Self-hosted infrastructure**: I run my own git forge (Forgejo), CI runners, binary
  cache, and services on NixOS — I am the target user of the Fediversity stack, not an
  outside observer.
- **Rust open source**: author/maintainer of [list 2–4 published crates or tools with
  one-phrase descriptions].
- **[Scientific/data background if applicable: degree, research-software work, or
  employment relevant to the analytical-query domain.]**
- **[Any prior NGI/NLnet involvement — grants received, projects built on NGI-funded
  software. If none: state "I have not previously received NGI funding; QueryFabric has
  been self-funded to date." Note that QueryFabric builds on NGI-adjacent foundations
  such as libp2p.]**

All of this work is public: [Codeberg/forge profile URL], [crates.io profile URL].

---

## 3. Requested amount

**€45,000**

---

## 4. "Explain what the requested budget will be used for? Does the project have other funding sources? Breakdown in main tasks with effort, rates explicit."

The entire budget funds development labour at a single explicit rate of **€75/hour**,
600 hours total, performed by me as an independent developer. There are no hardware,
travel, or subcontracting costs: the project deliberately runs on commodity hardware
(the public demo runs on a [1 vCPU / 1 GB] VPS at [€X]/month, self-funded), and all
infrastructure (CI, forge, binary cache) is already self-hosted.

**Other funding sources:** none, past or present. QueryFabric has been self-funded
volunteer work since its inception. It was extracted from [SynDB], which is likewise
self-funded; the extraction boundary is documented in GOVERNANCE.md and the public
decision log (DECISIONS.md), and no employer or third party holds rights over the
codebase. [Adjust if any employer/other funding context exists — disclose it here.]

### Task breakdown

| # | Work package | Main tasks | Hours | Cost |
|---|---|---|---|---|
| WP1 | Service portability, end to end | Import-side bundle ingestion: content-hash verification, catalog re-binding against a differently-shaped target schema, provenance-chain continuity; round-trip CLI (`export → transfer → import → verify`); operator documentation | 180 | €13,500 |
| WP2 | Federation & high availability | Hub failover and re-election; NAT traversal for instances behind home connections; schema-sync conflict resolution; HA deployment guide promoting the existing design doc from "planned" to "implemented"; multi-node VM test | 150 | €11,250 |
| WP3 | Backend breadth for small hosters | Embedded backend (SQLite or DataFusion, selected by measured footprint); conformance-corpus expansion; differential testing of the portable subset across all three backends in CI | 160 | €12,000 |
| WP4 | Security, community & Fediversity integration | Threat-model-driven hardening; remediation of external security-audit findings; nixpkgs module upstreaming; integration with the Fediversity deployment stack; operator docs for non-Rust audiences; contributor onboarding (issue curation, architecture walkthroughs) | 110 | €8,250 |
| | **Total** | | **600** | **€45,000** |

Each work package concludes with a tagged release (continuing the project's existing
staged release process, RELEASE.md) and CHANGELOG entries, giving natural, verifiable
payment milestones. Effort estimates derive from the project's own history: comparable
phases (federation extraction, sovereignty crates, NixOS module + VM test) each landed
in 120–180 focused hours, with the codebase and decision log to show for it.

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
   partial failure.
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
   ([X] MB binary, [Y] MB RSS idle, runs on 1 vCPU / [Z] MB — see the published
   benchmark chapter). Resource efficiency is a stated project constraint, not an
   afterthought; the embedded-backend choice in WP3 is explicitly decided by footprint.

---

## 7. "Describe the ecosystem of the project, and how you will engage with relevant actors and promote the outcomes?"

QueryFabric sits between three communities, and the engagement plan addresses each:

**The Fediversity / NixOS hosting ecosystem.** The NixOS module is the project's
front door: WP4 upstreams it to nixpkgs and aligns it with the Fediversity deployment
stack so that hosting providers in the pilot can offer QueryFabric-backed data
portability as a catalog service. I will engage directly with the consortium during the
project (module compatibility, NixOps4 deployment review) and present the work at
NixCon and FOSDEM (Nix and distributions devrooms) — communities I am already active in
as a nixpkgs contributor and NixOS fleet operator.

**Fediverse and self-hosted service developers.** The promise of WP1 — verified
instance-to-instance data migration — is demonstrated, not just documented: the public
demo instance ([URL]) runs in federation mode, and WP1 ships a worked end-to-end
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

**Promotion of outcomes.** Every work package ends in a tagged release with an
announcement post; the mdBook documentation site and Zola website are published from the
same repository; the conformance corpus is released as a standalone, reusable artifact
that other query tools can test against — a small standards contribution in itself. All
development happens in the open on [forge URL] under Apache-2.0, REUSE-compliant, with
curated entry-level issues and a public roadmap (ROADMAP.md) whose near-term items are
exactly the work packages above — so anyone can verify that grant work lands where the
proposal said it would.

---

## Pre-submission checklist for these answers

- [ ] Replace every `[bracketed]` placeholder with real facts/URLs/numbers
- [ ] Verify the demo instance is up and the URL resolves over HTTPS
- [ ] Re-measure footprint numbers on the release build actually tagged as v0.2.0
- [ ] Confirm word/character limits on the live form and trim per-field if needed
      (the abstract above is ~330 words — check against the form's limit first)
- [ ] Every factual claim has a public URL behind it (repo file, docs page, release tag)
- [ ] Peer read-through done
- [ ] Submit by 2026-07-29
