# NGI Fediversity Application Plan — QueryFabric

**Call:** NGI Fediversity, 12th call (opened 2026-06-01, deadline 2026-08-01 12:00 CEST)
**Grant range:** €5,000 – €50,000 · **Programme runs to:** November 2027
**Status of this document:** internal planning report — not for publication in the mdBook site.

---

## 1. Executive summary

QueryFabric is unusually well positioned for this call, but the application will fail or
succeed on **framing**, not engineering maturity. The call funds the "hosting stack of the
future": NixOS-based, self-hostable, portable, secure services where *"service portability
and data decoupling go hand in hand."* QueryFabric already ships, today:

- a hardened **NixOS module** with a VM-level integration test (`nix/tests/selfhost.nix`) —
  the call is literally built on NixOS and praises declarative, reproducible deployment;
- **data decoupling primitives**: GDPR Art. 15/16/17 trait surface (`queryfabric-access`),
  content-addressed portable export bundles with provenance and DOI minting
  (`queryfabric-portability`), multi-tenant isolation (`queryfabric-tenancy`);
- a **federation substrate** (`queryfabric-cluster`, `queryfabric-federation`) over libp2p
  with a wire-stable protocol;
- the openness criteria pre-met: Apache-2.0, GOVERNANCE.md, SECURITY.md, CONTRIBUTING.md,
  conformance corpus, fuzzing in CI, staged release process.

The gap: the project currently presents itself as *"a portable analytical query compiler
for scientific platforms."* Reviewers scanning for Fediverse/hosting relevance will not
connect that sentence to the call. The single biggest improvement is a **narrative bridge**
— QueryFabric as the *data-sovereignty and query-portability layer* for self-hosted and
federated services — backed by a small number of concrete, visible artifacts (demo
instance, roadmap, REUSE compliance) and a proposal whose work packages extend exactly the
seams the codebase has deliberately kept open (DataFusion/SQLite backends, federation
hardening, **import**-side portability).

Recommended ask: **~€45,000** (near the cap, justified by a 4-WP plan with explicit rates).

---

## 2. What reviewers will score (and where we stand)

NGI open calls (NLnet-style review) weigh roughly: fit to call topic, technical excellence,
value for money, openness/standards, and credibility of the team. Mapped to QueryFabric:

| Criterion | Current state | Verdict |
|---|---|---|
| Fit to call topic | Strong substance (NixOS, portability, federation) hidden behind "scientific platforms" framing | **Fix framing — top priority** |
| Technical excellence | 35 crates, conformance corpus, fuzzing, MSRV gate, VM tests, decision log | Strong — surface it concisely |
| Value for money | Mature base means grant money buys *new* capability, not catch-up | Strong — say this explicitly |
| Openness & standards | Apache-2.0, Arrow Flight, RFC 9457 problem details, PASETO, DataCite/DOI; no REUSE/SPDX yet | Good — add REUSE before submission |
| Team credibility | Single visible contributor, 0.1.x, no public instance | **Weakest axis — mitigate (§4)** |

---

## 3. Narrative bridge (do this first)

One paragraph that every application answer hangs off. Draft:

> Self-hosting fails at the data layer. Services trap user data in backend-specific SQL,
> schemas, and storage, so "moving instances" means lossy exports and manual surgery —
> exactly the lock-in the NGI 2025 study warns about. QueryFabric is a portable analytical
> query compiler and data-sovereignty toolkit: it gives self-hosted services a verified
> portable SQL subset that compiles to multiple backends (PostgreSQL, ClickHouse today),
> GDPR-aligned access/rectification/erasure as first-class library traits,
> content-addressed export bundles with provenance, and a libp2p federation protocol for
> multi-instance deployments — all packaged as a hardened NixOS module with end-to-end VM
> tests. It re-establishes the boundary between content owner and service provider at the
> query layer, so services can be mixed, matched, and migrated.

Concrete edits this implies (keep the scientific identity, *add* the hosting angle —
don't replace it):

- **README.md**: keep the first line, add a second positioning sentence + a "Why this
  matters for self-hosting" subsection linking portability/access/tenancy crates.
- **Website** (`website/`, Zola): landing page gets a "data sovereignty for self-hosted
  services" section; currently the demo framing is the closest thing.
- **docs**: a new mdBook chapter "Self-hosting and data sovereignty" tying together the
  Phase 05 crates and the NixOS module — this becomes the page the application links to.

---

## 4. Improvement plan, prioritized

### Tier 1 — before submission (high signal, ≤ 1–2 days each)

1. **REUSE compliance** (`reuse lint` clean, SPDX headers, `.reuse/dep5` or
   `REUSE.toml`, badge in README). The NGI/NLnet ecosystem treats REUSE as table stakes;
   currently there are no SPDX markers anywhere in the repo.
2. **Cut and publish a release.** CHANGELOG has unreleased entries and `scripts/release.sh`
   is staged but the workspace sits at 0.1.1. A tagged v0.2.0 on crates.io with the
   documented staged-publication process *executed* (not just documented) converts
   "release process exists" into "project releases."
3. **Public demo instance** of `queryfabric-demo` (federation mode, S3 store) on a small
   VPS, deployed *from the NixOS module* — then the application can say "deployed with one
   NixOS module, here's the URL and here's the exact flake config." Cheapest possible
   proof of the entire pitch. Add an asciinema/screenshot to README and website.
4. **ROADMAP.md** describing the next 18 months, structured so the grant work packages in
   §6 are visibly a subset of it. Reviewers should see the grant accelerating an existing
   trajectory, not inventing one. Source material already exists in DECISIONS.md
   (deferred items D003-adjacent: DataFusion seam, host routing) — make it public-facing.
5. **Community surface**: a public Matrix room (or Codeberg issue tracker policy),
   issue templates, 5–10 curated `good-first-issue`s. This is the cheapest mitigation for
   the single-contributor signal; the grant itself (WP4) funds the rest.
6. **Threat model document** (1–2 pages, extending SECURITY.md): SQL injection surface of
   the compiler, placeholder handling, capability misclassification, federation message
   trust. NGI projects get free security audits (Radically Open Security et al.) —
   stating "we have a threat model and want the audit" is a known-good move.

### Tier 2 — strengthens the proposal if time allows (before or during)

7. **Resource-efficiency numbers.** The call names resource efficiency and e-waste
   explicitly. Measure and publish: demo binary size, RSS at idle and under load, cold
   start time, and a comparison point ("runs on a 1 vCPU / 512 MB VPS"). A
   `docs/src/deployment/footprint.md` chapter with a reproducible benchmark script.
8. **High-availability story.** The call calls HA "a dark art." Document (even as a
   design doc) how hub failover, health monitoring, and stateless demo instances behind
   a load balancer compose — `queryfabric-cluster` already has health monitoring and DHT
   registry; write down what works today vs. what WP2 funds.
9. **NixOS module polish**: multi-instance support (`services.queryfabric.instances.<name>`),
   and a compatibility note for Fediversity's own deployment tooling (NixOps4). Evaluate
   upstreaming the module to nixpkgs once the project stabilizes — mention as roadmap item.
10. **Accessibility note** for the Leptos/web surfaces — NGI asks UI projects about
    accessibility; one honest paragraph beats silence.

### Tier 3 — do NOT pre-build; propose as funded work (§6)

- DataFusion and/or SQLite/embedded backend (seam deliberately open per DECISIONS.md).
- **Import-side portability** — today bundles export; true *service portability* requires
  a receiving instance to ingest a bundle (re-bind catalog, verify content hash, replay
  provenance). This is the strongest single work package for this call.
- Federation hardening: NAT traversal, hub failover, schema-sync conflict resolution.
- Operator UX: guided first-run, web admin for tenancy/exports.
- External security audit follow-up work.

---

## 5. Application answers — drafting guidance

### Abstract
Use the §3 bridge paragraph, then one sentence per work package, then the expected
outcomes: "a self-hosted service operator can migrate analytical data between instances
and backends with verified fidelity, deployed via NixOS, with federation and GDPR rights
built in." Keep under ~300 words; lead with the problem (data lock-in in self-hosted
services), not the technology.

### Prior involvement
Answer factually (fill in personal history: NixOS/nixpkgs contributions, prior NGI/NLnet
grants if any, relevant open-source maintenance). The call itself notes many deployed
projects were bootstrapped on these grants — if any prior NGI-funded project was used or
built upon, name it. Do not pad; reviewers read hundreds of these.

### Requested amount & budget breakdown
- Make the **hourly rate explicit** as the form demands. NLnet-ecosystem norms for
  independent developers are roughly €50–100/h; pick one defensible number (e.g. €75/h)
  and use it consistently.
- Suggested shape: **€45,000 = 600 h @ €75/h**, broken down per work package (§6).
- State explicitly that hardware/hosting costs are negligible (NixOS on commodity VPS) —
  this doubles as a resource-efficiency argument.

### Other funding sources
Answer honestly (presumably: none / self-funded to date). If SynDB or an employer funded
adjacent work, disclose the boundary: QueryFabric was extracted as a neutral, standalone
project (GOVERNANCE.md already states the neutrality stance — cite it).

### Comparison with existing efforts
Position against, honestly and specifically:

| Effort | What it is | Why QueryFabric differs |
|---|---|---|
| Apache Calcite | JVM query framework/optimizer | JVM-centric, no sovereignty layer, heavyweight for self-hosters |
| DataFusion / Substrait | Rust execution engine / IR interchange | Execution-focused; QueryFabric is the compile-time semantic boundary *above* them — Substrait/DataFusion are integration targets, not competitors |
| PRQL / sqlglot | Query language / SQL transpiler | Syntax-level transpilation without catalog binding, capability verification, or conformance guarantees |
| Trino/Presto | Federated query *execution* | Heavy multi-node executor; QueryFabric deliberately does not execute (DECISIONS D003) and targets small self-hosted footprints |
| PostgREST/Hasura | API-over-database | Single-backend, no portability/export semantics |

The unique combination: verified portable subset (conformance corpus) + sovereignty
primitives (GDPR traits, content-addressed bundles, DOI) + federation protocol + NixOS-
first deployment, in a library that hosts embed rather than a service that owns the data.

### Significant technical challenges
Real ones to name (reviewers reward honesty about hard parts):
1. **Semantic fidelity across dialects** — proving the portable subset means the same
   thing on PostgreSQL, ClickHouse, and a new embedded backend (conformance corpus
   expansion, differential testing, fuzzing across backends).
2. **Import-side portability** — ingesting a bundle into a *differently-shaped* catalog:
   schema matching, capability re-verification, provenance chain continuity.
3. **Federation without central trust** — schema sync conflict resolution and hub
   failover over libp2p while keeping the wire protocol stable.
4. **Secrets-free reproducible deployment** — keeping the systemd LoadCredential pattern
   airtight as the module grows multi-instance and HA options.

### Ecosystem & engagement
- **Fediversity consortium itself**: align the NixOS module with their deployment stack;
  offer QueryFabric as the analytical/portability layer for their hosted-services catalog.
- **NixOS community**: upstream module to nixpkgs; talk proposals at NixCon / FOSDEM
  (Nix and distributions devrooms).
- **Fediverse developers**: a worked integration example with one real Fediverse service's
  analytics/export needs (e.g. instance statistics, GDPR export for a Mastodon-like
  service) — even as a design study in WP3.
- **Research-data world**: DataCite/DOI integration is already real; this is a second,
  distinct user community and worth one sentence.
- **Outcome promotion**: release announcements, the demo instance, mdBook docs site,
  conformance corpus published as a reusable artifact other projects can test against.

---

## 6. Proposed work packages (the funded plan)

| WP | Title | Content | Effort (h) | Cost @ €75/h |
|---|---|---|---|---|
| WP1 | Service portability, end to end | Import-side bundle ingestion, catalog re-binding, round-trip verification (export → transfer → import → verify), CLI + docs | 180 | €13,500 |
| WP2 | Federation & HA hardening | Hub failover, NAT traversal, schema-sync conflict handling, HA deployment guide, multi-instance NixOS module | 150 | €11,250 |
| WP3 | Backend breadth for small hosters | Embedded backend (SQLite or DataFusion — decide by footprint), conformance corpus expansion, differential testing across 3 backends | 160 | €12,000 |
| WP4 | Security, community & Fediversity integration | Threat-model-driven hardening, external audit follow-up, nixpkgs upstreaming, Fediversity stack integration, docs for non-Rust operators, contributor onboarding | 110 | €8,250 |
| | **Total** | | **600** | **€45,000** |

Each WP ends in a tagged release with CHANGELOG entries — matches the existing
RELEASE.md/COMPATIBILITY.md discipline and gives NLnet-style payment-per-milestone a
natural structure.

---

## 7. Timeline to deadline (today: 2026-06-13 → 2026-08-01)

| Week | Dates | Work |
|---|---|---|
| 1 | Jun 15–21 | Narrative bridge: README/website/docs chapter (§3). REUSE compliance. |
| 2 | Jun 22–28 | Cut v0.2.0, publish to crates.io. ROADMAP.md. Issue templates + good-first-issues. |
| 3 | Jun 29–Jul 5 | Public demo instance up; screenshots/asciinema; footprint numbers (Tier 2 #7). |
| 4 | Jul 6–12 | Threat model doc. HA design doc. First full application draft. |
| 5 | Jul 13–19 | Budget table finalized; comparison section sourced; external read-through by a peer. |
| 6 | Jul 20–26 | Revise; trim abstract; verify every claim in the application has a public URL behind it. |
| 7 | Jul 27–31 | Submit **early** (by Jul 29) — never at the noon deadline. |

---

## 8. Risks and honest weaknesses (address, don't hide)

1. **Single contributor / bus factor.** Mitigation: GOVERNANCE.md exists; WP4 explicitly
   funds contributor onboarding; documentation depth (mdBook, DECISIONS.md, conformance
   corpus) makes the project legible to newcomers. Say this in the application.
2. **"Scientific platforms" vs Fediverse fit.** Mitigated by §3; do not pretend the
   project was always Fediverse-targeted — frame it as the same portability problem in a
   new domain, which is true.
3. **0.x maturity.** Fine for this call (it funds R&D), and COMPATIBILITY.md already
   documents the semver policy. Frame 1.0 stabilization as a post-grant outcome.
4. **No execution engine** could be misread as "incomplete." Pre-empt: D003 is a
   deliberate boundary that keeps the footprint small and the host in control — this *is*
   the resource-efficiency and sovereignty story.
5. **Scope creep in the proposal.** Resist promising ActivityPub or a full hosting panel;
   stay on the data/query layer where the project is credible.

---

## 9. Checklist (pre-submission gate)

- [ ] README + website carry the hosting/sovereignty narrative
- [ ] `reuse lint` passes; badge added
- [ ] v0.2.0 tagged and on crates.io
- [ ] Demo instance live at a public URL, deployed via the NixOS module
- [ ] ROADMAP.md published; WPs in application are a visible subset of it
- [ ] Threat model doc merged
- [ ] Footprint/efficiency numbers in docs
- [ ] Every factual claim in the application has a clickable public link
- [ ] Budget: explicit rate, per-WP hours, milestone = release mapping
- [ ] Peer read-through completed
- [ ] Submitted ≥ 2 days before the 2026-08-01 noon CEST deadline
