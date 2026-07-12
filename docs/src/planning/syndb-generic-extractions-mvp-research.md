# SynDB Generic Extractions And QueryFabric MVP Research Dossier

- **Mode:** durable research dossier
- **Observed:** 2026-07-12
- **QueryFabric baseline:** `trunk@c939ce5ed9581f4ad409d7df5d85d62b13cc553c`
- **SynDB baseline:** `rapid@717f557908c7e69a4710403dad3e898615202d66`
- **Status:** discovery and grant-alignment addendum complete; implementation
  not started

## Goal And Trigger

Plan which SynDB components should become generic QueryFabric components and
design the work still required for an honest MVP. The source inventory is the
owned GitHub workspace at
`/data/nvme0/can/canix/projects/repos/owned/github.com`, with SynDB at
`memorycircuits/SynDB`.

This dossier replaces assumption-driven extraction with evidence from both
repositories. It also audits the existing SynDB plan at
`docs/src/planning/queryfabric-upstream/` rather than treating its unchecked
tasks as current truth.

## Current Reality

### Recommended product boundary

Define the MVP as **QueryFabric 0.2: a portable compiler plus a reproducible,
host-to-host tabular-resource migration proof**, not as a production federated
runtime.

The library promise is:

1. parse SQL or SyQL;
2. bind typed parameters against an immutable catalog snapshot;
3. produce result schema, diagnostics, capability analysis, and provenance;
4. emit safe PostgreSQL or ClickHouse SQL; and
5. let a host enforce authorization, choose a backend, execute, limit, and
   cancel the query.

That compiler boundary matches D003 and D004 in `DECISIONS.md` and the documented host
sequence in `docs/src/integration/host-integration.md`. The existing compiler
facade already implements most of the parse → bind → analyze → emit pipeline in
`crates/queryfabric/src/lib.rs`. The remaining MVP work is hardening, completing
the reference-host request/response contract, converging the SynDB consumer,
making release claims reproducible, and closing the export-only portability gap
with bounded validation/import plus a two-host NixOS proof.

Production federation, Arrow Flight migration, isolated Kubernetes execution,
and the one-shot worker are extension tracks. They must not be presented as MVP
features until their missing behavior and end-to-end tests exist.

### Fediversity alignment delta

The supplied grant context (now kept in the canonical applications checkout)
and current official NLnet/Fediversity sources change the impact gate, not the
stable compiler ownership boundary. A
compiler-only release has credible technical value but weak evidence of the
call's central portability/deployability outcomes. The grant-informed MVP adds
a bounded export -> operator transfer -> dry-run -> import -> restart proof for
one tabular resource conforming to a published import profile between
independent PostgreSQL/S3-backed NixOS hosts.

| Theme | Current evidence | MVP consequence |
|---|---|---|
| NixOS reproducibility | named-instance module and two-instance VM test exist | use it as provider deployability/conformance evidence, not universal self-hosting |
| data portability | export bundle, canonical hash, manifest, licence/restriction, citations, and provenance exist; no executable arbitrary-resource schema exists | add a versioned tabular CSV profile, bounded validator/import plan, predeclared target apply/receipt, and real cross-instance transfer |
| service portability | no full service configuration/account/database migration exists | claim only bounded resource/data migration; full service migration stays out of scope |
| high availability | current federation identity/status and process-local registry are not failover | keep HA, NAT traversal, and active-active work post-MVP |
| security | compiler hardening is planned; bundle trust/apply semantics are missing | add expected-hash trust, URI/reference denial, size limits, policy preservation, replay/idempotency, and atomic-visibility tests |
| accessibility | project states that no WCAG audit has occurred | add automated and scoped manual evidence; do not claim conformance in advance |
| public verification | public source/demo exist, but no tag/registry release was found | separate local release-candidate handoff from a later maintainer-authorized public milestone claim |
| sustainability/upstreaming | governance is minimal and no nixpkgs issue/PR evidence exists | document only real roles/continuity risks and prepare an upstreamable module patch; do not claim adoption before public evidence |
| upstream fit | current Fediversity PostgreSQL/S3 contracts roughly match the host | design a thin conditional adapter, but do not claim compatibility until upstream supplies a supported immutable boundary |

Phase 01 regression repair and Phase 03 extraction convergence remain product
prerequisites, not default grant work packages. They are maintenance/correctness
work unless a genuinely new R&D question is isolated. The detailed grant lens
is recorded in
[`grant-alignment.md`](syndb-generic-extractions-mvp/grant-alignment.md).

### Repository and release state

| Fact | Evidence | Consequence |
|---|---|---|
| QueryFabric has 41 workspace packages, 19 currently publishable | `cargo metadata --no-deps --format-version 1` | README and crate catalog claims of 35 are stale; the publish set is too broad |
| Rust-version sources disagreed at discovery | workspace `Cargo.toml` and pre-commit policy selected 1.94, while compatibility/legacy CI said 1.85 and the current dev shell supplied 1.95 | compatibility docs and CI are now aligned to 1.94; an exact flake-pinned MSRV build remains required before release |
| Simit is not supplied by the QueryFabric flake | current successful resolution is the user-profile binary | pin the workflow producer before regenerating or trusting release metadata |
| SynDB consumes two different QueryFabric revisions while canonical development targets a third lineage | submodule `1ba7f34`, `flake.lock` revision `8f4707d`, canonical trunk target `c939ce5` | no extraction can be considered adopted until both consumers converge on the canonical lineage |
| The SynDB submodule topic branch and canonical QueryFabric trunk have no merge base | `git merge-base c939ce5 a13aca2146ec1c245e8e11de84fc7b32d6ed7161` exits 1 | manually reconcile branch deltas onto a new canonical-trunk branch; do not blindly cherry-pick or merge |
| No public QueryFabric tag was found | `git tag --list` and `git ls-remote --tags origin` | 0.2 has not been released |
| No queried Rust or Python package was found in the public registries | crates.io API/search for the QueryFabric facade/core names; PyPI JSON for `queryfabric` | installation and release-readiness language must remain prospective |
| The public demo answers health, resource, federation-status, and PostgreSQL query requests | direct `curl` checks against `https://queryfabricdemo.tartanoglu.com` | useful reference-host proof, not proof of production federation |

### MVP component design

| Component | Current evidence | MVP completion |
|---|---|---|
| Stable compiler facade | `QueryCompiler` exposes parse, bind/validate, analyze, and emit | keep this as the primary public API and test it through the facade |
| Immutable catalog input | `MemoryCatalog` and `CatalogSnapshotId` exist | require a non-empty snapshot identity at the host boundary and return it with every result |
| Typed parameter input | `QueryParameters`, parameter inspection, and JSON conversion helpers exist | accept exactly one named or positional parameter mode, validate it before execution, and preserve backend parameter order |
| Backend analysis and emission | PostgreSQL and ClickHouse adapters plus portable conformance tests exist | make capability rejection and structured diagnostics part of the vertical-slice response |
| Safe backend-token rendering | emitters append relation/column/alias/CTE names and mapped function paths directly; ClickHouse timezone type arguments are also raw | type, validate, and safely render every catalog-derived identifier, function path, keyword/operator, and type argument |
| Error preservation | CTE emission uses `unwrap_or_default()` in `crates/queryfabric-catalog/src/render/emit.rs` | propagate the underlying error; never turn a failed subquery into empty SQL |
| Runtime seam | `ExecutionRuntime` exists, `InteractiveRuntime` always returns `NotImplemented`, and the optional Flight feature exposes an unfinished skeleton | publish honest traits only; remove the stub and move Flight to registry-unpublished `queryfabric-flight` until Phase 05 |
| Reference host | `queryfabric-demo` executes emitted PostgreSQL SQL, but only accepts `{ sql }` and uses empty parameters | add dialect, typed parameters, catalog identity, structured response data, read-only execution, limits, and cancellation |
| Export bundle | `queryfabric-portability` builds ad-hoc-canonical bundle 1.0 JSON with manifests, provenance, citations, licence/restriction, and a BLAKE3 content address; artifact hash comments allow an unspecified host alternative | keep 1.0 export-only; define import-ready 2.0 with RFC 8785, typed BLAKE3-256 digests, schema/fixtures, and absent-signature honesty |
| Import contract | no validator, executable schema/profile, import plan/report, mapping, artifact transfer, receipt, or target apply exists | add one normative typed CSV profile plus bounded neutral validation/plan/report while keeping authorization, I/O, predeclared-target mapping, and transaction policy in the host |
| Durable target state | demo data is always seeded and provenance/ownership are process-local; bundles do not carry full access policy or ownership | separate schema setup from seed data and persist rows, origin-attributed source evidence, local policy/owner/import event, receipt, and mapping through restart |
| NixOS migration proof | alpha/beta processes share one VM; they have separate databases/buckets but both seed the same data, and the test only exports on alpha | retain the single-host regression test, add separate alpha/beta test nodes plus a scoped transfer path, start beta empty, and prove import/restart/rejection without shared source-host credentials |
| Accessibility/docs | no WCAG audit exists; the portability scenario is now honest about unsigned content-addressed bundles, while critical examples still lack dedicated executable targets | add pinned automation plus scoped manual findings, and require real tested example targets |
| Fediversity host adapter | PostgreSQL `urlFile` matches; current Fediversity S3 uses separate credential files while QueryFabric expects one combined file | prepare a thin Nix adapter/Garage test only after a supported immutable upstream contract boundary is identified |
| Release surface | ten compiler/facade dependency crates form the coherent core; nine unfinished peripheral crates are also publishable | derive tiers from Cargo metadata; publish only the compiler tier, remove unfinished Flight from its feature surface, and set peripheral crates to `publish = false` |
| Downstream proof | SynDB already consumes many QueryFabric crates | converge to one revision and run focused plus workspace gates before release |

The target reference-host flow is:

```text
request(query, dialect, typed parameters, requested backend)
  -> host identity, authorization, and immutable catalog snapshot
  -> QueryCompiler::parse
  -> QueryCompiler::bind_and_validate
  -> QueryCompiler::analyze
  -> host backend selection and execution limits
  -> QueryCompiler::emit
  -> host prepared execution with cancellation
  -> response(rows, result schema, provenance, diagnostics, snapshot id)
```

The new portability proof is a separate host flow:

```text
alpha export -> RFC 8785 bundle 2.0 + typed CSV profile + expected digest
  -> authenticated operator transfer into beta staging
  -> bounded parse and digest/artifact verification
  -> dry-run predeclared-target plan bound to target/staging revision
  -> re-authorized/revalidated apply with atomic visibility and durable receipt
  -> beta query/restart/replay verification
```

The host remains responsible for identity, access policy, database credentials,
connection management, row/time/size budgets, and job lifecycle. QueryFabric
must supply enough typed artifacts for the host to do those jobs without
re-parsing SQL or reaching into private compiler crates.

## Evidence Inventory

| ID | Source | Finding |
|---|---|---|
| E01 | user-provided workspace root | SynDB is the extraction source; no missing repository was substituted |
| E02 | `git status` and `git log -1` in both repositories | both audited worktrees were clean at the observed revisions before this untracked plan and the user-supplied `grant/` artifacts appeared |
| E03 | SynDB `docs/src/planning/queryfabric-upstream/{README,01..05}.md` | the previous five-phase extraction plan exists, is unchecked, and is absent from `SUMMARY.md` |
| E04 | QueryFabric `DECISIONS.md` D003/D004 and `docs/src/integration/host-integration.md` | execution, authorization, routing, and domain policy belong to the host |
| E05 | `crates/queryfabric/src/lib.rs` and compiler/conformance/property tests | the compiler slice is substantially implemented and test-backed |
| E06 | `crates/queryfabric-demo/src/http.rs` and `nix/tests/selfhost.nix` | a real PostgreSQL reference host and a two-instance NixOS test exist; the HTTP query contract is still thin |
| E07 | SynDB `crates/services/flight/src/server.rs`, `skeleton.rs`, and `service/flight_impl.rs` | production serves a skeleton whose DoGet/DoPut/ListFlights handlers are unimplemented while the old service still contains working data-plane behavior |
| E08 | QueryFabric `crates/queryfabric-adapter-clickhouse/src/driver.rs` and SynDB `crates/core/syndb-clickhouse/src/dynamic.rs` | `DynamicClient` is duplicated; SynDB retains richer retry behavior/tests |
| E09 | `crates/queryfabric-runtime-k8s/src/stream.rs` and `crates/queryfabric-worker/src/lib.rs` | runtime sends serialized `BoundQuery` while worker expects a UTF-8 provenance hash; shutdown is signaled before stream drain |
| E10 | Cargo metadata, `RELEASE.md`, release helpers, and generated workflows | package counts, publish lists, tag conventions, artifact jobs, Python paths, and MSRV claims conflict |
| E11 | `README.md`, `ROADMAP.md`, `docs/src/project/threat-model.md`, and canonical docs tree | durable grant/application files are maintained in the canonical applications checkout, not in the QueryFabric release tree |
| E12 | QueryFabric `flake.nix` and ancestor `/data/nvme0/can/canix/.cargo/config.toml` | repo-root Cargo inherits nightly-only codegen flags while the dev shell supplies stable Rust |
| E13 | SynDB `nix/rust.nix`, `pyproject.toml`, and `uv.lock` | pure Nix evaluation uses an obsolete absolute QueryFabric path; Python locking expects a missing sibling `../nix-article` |
| E14 | SynDB vendored QueryFabric commits `36a327f`, `626167d`, and `1ba7f34` | generic web redirect/flash changes exist only on the topic lineage; the dependency rename is already represented on canonical trunk |
| E15 | applications checkout `docs/grants/NGI_Fediversity_2026_LLM_Context.{md,json}` and generic template pair | supplied grant digest/template are parseable and useful orientation, but current official sources remain authoritative and applicant facts remain unset |
| E16 | official NLnet call, guide, eligibility, FAQ, form, GenAI policy, and portfolio, verified 2026-07-12 | call/date/range/scoring are current; milestones must be public/verifiable; AI use needs disclosure/provenance; portability/NixOS/security/accessibility are relevant |
| E17 | `queryfabric-portability`, demo sovereignty path, `nix/tests/selfhost.nix`, accessibility/HA/portability docs | discovery-baseline finding: only export existed; the bundle lacked an executable tabular profile/full policy/ownership; CSV was handwritten; alpha/beta seed data was identical; the docs overclaimed signing and used nonexistent APIs (the implementation pass below records the repairs) |
| E18 | official Fediversity repository `0e4ab02db40b188898531ad36b5eb03c6e46a431` | provider-focused project is in development; PostgreSQL exposes `urlFile`/`sslMode`, S3 exposes separate access/secret files; no supported external QueryFabric boundary has been supplied |
| E19 | `nix develop -c reuse lint` and `nix develop -c mdbook test docs` on 2026-07-12 | QueryFabric REUSE passes after the grant move; the applications checkout still fails on 28 unannotated documentation files; QueryFabric's mdBook build and doctest suite now pass after classifying illustrative code, diagrams, and data snippets explicitly |
| E20 | `git config --show-origin --get remote.origin.url`, SSH and HTTPS `git ls-remote` | local origin has a trailing space and SSH fails; canonical HTTPS is reachable and has no public tags |

### Checks completed during discovery

The following passed at the original clean repository baseline:

- QueryFabric `cargo metadata --no-deps --format-version 1`;
- QueryFabric `reuse lint` before the four unlicensed `grant/` files appeared and again after they were moved to the applications checkout;
- QueryFabric `mdbook build docs`;
- Plinth project configuration check;
- a full QueryFabric workspace `cargo build --locked`, Clippy with
  `-D warnings`, and `cargo test --locked` from `/tmp` using
  `--manifest-path` and `target/audit-clean`; and
- focused tests for the extracted/runtime utility crates using the same
  isolated working-directory method.

The two supplied JSON grant artifacts also pass `jq empty`, and their Markdown
counterparts were cross-checked against current official sources. The files now
live in the applications checkout. QueryFabric's `nix develop -c reuse lint`
passes after the move; running the same tool in the applications checkout fails
because its 28 documentation files lack copyright/licensing metadata.
`nix develop -c mdbook build docs` and `nix develop -c mdbook test docs` now
pass. Illustrative Rust, diagrams, trees, YAML, and JSON snippets that are not
standalone doctests are explicitly classified so mdBook does not attempt to
compile them as repository examples. The applications checkout's REUSE failure
remains separate and is still blocked on producer-supplied rights metadata.

The full Rust gate included `queryfabric-python` in build and Clippy, but that
crate disables its Rust test and doctest harnesses. No Python import, wheel, or
pytest gate was run.

The ordinary documented repo-root Cargo workflow did **not** pass: it fails
before compiling project code because the ancestor Cargo configuration enables
nightly `-Z codegen-backend` flags under stable Rust. The `/tmp` result proves
the source tree, not the correctness of the developer workflow; Phase 00 must
repair the workflow rather than institutionalize the bypass.

SynDB workspace gates were not claimable. Its Nix source assembly and Python
path dependency are foundational blockers described below.

## Existing Plan Status

The prior SynDB plan was added at `f1fb57a` and never reconciled with later
implementation. None of its five phase documents is safe to retire as a whole.

| Old phase | Status | Evidence-backed disposition |
|---|---|---|
| 01 cleanup and small upstreams | mostly complete | dead federation modules are gone; ClickHouse types/config and Arrow helpers are consumed. Resolve remaining thin wrappers and duplicate `spawn_traced` implementations opportunistically |
| 02 ClickHouse adapter | partial | `ChType` and configuration landed; `DynamicClient` remains duplicated. Reject whole-`ChQuery` extraction because it is bound to SynDB tables, clusters, filters, and errors |
| 03 Flight refactor | unsafe and incomplete | auth/ticket adapter scaffolding exists, but the production server now selects unimplemented handlers and an allow-all coarse access decision |
| 04 CLI and test infrastructure | partial | generic primitives exist, but Flight timeout/auth arguments are ignored, Kubernetes `500m` parsing truncates to zero, Docker credential input is wrong, and the service rig retains SynDB defaults |
| 05 database/types/changelog/worker/K8s/web | partial and internally inconsistent | several crates are copied but not adopted or tested; runtime/worker protocols disagree; the topic-only web changes still need canonical reconciliation |

The old plan also embeds obsolete `/data/nvme0/can/Projects/...` paths and the
false assumption that Nix automatically consumes the submodule. It should stay
available as historical evidence until the replacement plan records each item
as adopted, deferred, rejected, or superseded.

## Work That Should Survive

### Keep in the MVP

- the facade, neutral IR, SQL/SyQL dialects, catalog/binder, capability
  analysis, PostgreSQL and ClickHouse emitters, optimizer seam, contract traits,
  runtime traits, typed schema/parameters, provenance, and compiler
  diagnostics;
- `ChType`, `SimpleColumnType`, `ClickHouseConfig`, Arrow-safe projection
  helpers, and a single behavior-complete `DynamicClient`; the client is a
  host-integration utility, not part of the compiler-facade promise in D003;
- the generic web `Flash`, query-value, safe-local-redirect, and append-query
  helpers from the vendored topic branch after manual canonical porting;
- generic utilities that already have neutral contracts and consumers, such
  as process/spawn helpers, content hashing, command execution, Flight
  pool/cache, TCP tuning, job queue traits, metrics, and Docker primitives; and
- the PostgreSQL demo and NixOS self-host test as acceptance fixtures, with
  claims limited to behavior they actually exercise.

### Defer until after the host-to-host tabular resource-portability MVP

- the generic Flight skeleton migration and the CLI Flight client; keep the
  pre-graduation server surface in `queryfabric-flight` with
  `publish = false`;
- Kubernetes isolated runtime, worker binary/image, chart/manifests, and Kind
  test;
- production federation transport, actor startup, authenticated registration,
  fan-out/merge, and registry persistence;
- broad CLI/test-service orchestration;
- `queryfabric-types`, the SeaORM enum macro, changelog fetching, and typed
  PASETO resource scope until their adoption contracts are proven; and
- public Python packaging unless wheel construction, import tests, pytest, and
  the correct package directory are wired.

### Reject as QueryFabric extractions

- the whole SynDB `ChQuery` builder;
- `syndb-query-host` metadata lookup, relation selection, and execution routing;
- SynDB Flight descriptors, access filtering, citations, dataset errors, and
  data-plane behavior;
- SynDB ClickHouse schemas, DDL, materialized views, and table registry;
- `syndb-lakehouse`, `prov-activity`, `datacite-types`, `etl-job-state`, and
  `syndb-preflight` as currently shaped;
- neuro/ETL/GPU/search/UI/database/benchmark/manuscript/MCP domain code; and
- `iso-continent`, `meta-stats`, and `latency-stats`, consistent with D005.

“Reject” means keep the behavior in SynDB or a separate focused crate. It does
not prohibit extracting a smaller primitive later after a real second consumer
demonstrates a neutral contract.

### Extraction graduation contract

An extraction is complete only when all of these are true:

1. a neutral contract is justified by at least two consumers or by the stable
   compiler facade itself;
2. original behavior tests move upstream before the source implementation is
   deleted;
3. no reusable implementation names SynDB domain types, resources, databases,
   namespaces, environment variables, or images;
4. upstream default and all-feature checks pass;
5. SynDB consumes the canonical upstream revision and its focused tests pass;
6. duplicate implementation code is removed in the same phase;
7. public APIs contain no ignored security/timeout inputs or unconditional
   stubs; and
8. the crate is explicitly placed in stable,
   registry-unpublished/experimental, or rejected release tier.

## Blockers And Missing Artifacts

### B01: QueryFabric developer workflow is polluted by ancestor Cargo config

- **Missing/invalid input:** a dev shell with a Cargo configuration compatible
  with its stable Rust 1.95 toolchain.
- **Why required:** the documented repo-root validation command fails before
  project code, so contributors and CI cannot reproduce the successful
  isolated audit command.
- **Upstream producer:** the canix/rs-harbor workflow that generated
  `/data/nvme0/can/canix/.cargo/config.toml`, plus QueryFabric's explicit
  toolchain policy in `flake.nix`.
- **Regeneration workflow:** remove or regenerate the ancestor config so
  nightly-only profile/unstable settings are scoped to the projects that use
  them instead of every descendant repository. An isolated `CARGO_HOME` alone
  is insufficient because Cargo still merges nearer ancestor configuration.
  The concrete preferred canix change is to remove the tracked
  `.cargo/config.toml` and rely on its current
  `nix develop .#configure`/rs-harbor dev shell to generate an ephemeral
  nightly Cargo home. Validate canix in that shell before committing the
  removal. The one-time source change is:

  ```bash
  cd /data/nvme0/can/canix
  git rm .cargo/config.toml
  ```

  After the ancestor is neutral, QueryFabric may add a pinned rs-harbor stable
  config/dev shell or an equivalent local stable config. The other explicit
  option is to align QueryFabric deliberately on nightly. Stable remains the
  recommendation because it matches the MSRV intent.
- **Validation:**

  ```bash
  cd /data/nvme0/can/canix/projects/repos/owned/codeberg.org/caniko/queryfabric
  nix develop -c cargo build --workspace --locked
  nix develop -c cargo clippy --workspace --locked -- -D warnings
  nix develop -c cargo test --workspace --locked
  ```

  The upstream canix proof for the preferred repair is:

  ```bash
  cd /data/nvme0/can/canix
  test ! -e .cargo/config.toml
  nix develop .#configure -c cargo check --manifest-path cli/Cargo.toml --locked
  ```

### B02: SynDB Nix source assembly points at an obsolete absolute path

- **Missing/invalid input:** pure QueryFabric source selection in
  `SynDB/nix/rust.nix`.
- **Why required:** `nix flake check --no-build` cannot evaluate the downstream
  consumer, so no extraction can be proven adopted.
- **Upstream producer:** SynDB Nix source assembly.
- **Regeneration workflow:** replace
  `/data/nvme0/can/Projects/SynDB/vendor/queryfabric` with one pure,
  lock-controlled source: the submodule or the QueryFabric flake input. Use the
  same canonical commit for Cargo and Nix.
- **Validation:**

  ```bash
  cd /data/nvme0/can/canix/projects/repos/owned/github.com/memorycircuits/SynDB
  nix flake check --no-build
  ```

### B03: SynDB Python lock expects a missing sibling checkout

- **Missing/invalid input:** reproducible `anx-plot` source. `pyproject.toml` and
  `uv.lock` point to `../nix-article`, while the actual owned checkout is not at
  that path.
- **Why required:** the SynDB dev shell cannot prove its locked Python
  environment.
- **Upstream producer:** SynDB `pyproject.toml` and UV lock workflow.
- **Regeneration workflow:** choose a reproducible workspace or Git source for
  `anx-plot`, update `pyproject.toml`, then regenerate without entering the
  currently broken shell hook using
  `nix shell --inputs-from . nixpkgs#uv -c uv lock`.
- **Validation:**

  ```bash
  cd /data/nvme0/can/canix/projects/repos/owned/github.com/memorycircuits/SynDB
  nix develop . -c uv sync --locked
  ```

### B04: QueryFabric revision lineage is split

- **Missing/invalid input:** one canonical commit containing the required topic
  branch behavior.
- **Why required:** SynDB's submodule, Nix lock, and canonical QueryFabric
  checkout disagree, and the topic lineage cannot be safely merged.
- **Upstream producer:** QueryFabric maintainer branch reconciliation followed
  by SynDB dependency updates.
- **Regeneration workflow:** branch from canonical `trunk`, manually port and
  test the neutral deltas from `36a327f` and `1ba7f34`, verify `626167d` is
  already equivalent, then stop for explicit operator review/push/merge. Only
  after the candidate is reachable from the approved canonical remote ref may
  SynDB update both the submodule and `flake.lock` to that commit.
- **Validation:**

  ```bash
  git -C /data/nvme0/can/canix/projects/repos/owned/github.com/memorycircuits/SynDB \
    submodule status vendor/queryfabric
  git -C /data/nvme0/can/canix/projects/repos/owned/github.com/memorycircuits/SynDB \
    show HEAD:flake.lock | jq -r '.nodes.queryfabric.locked.rev'
  ```

  Both commands must name the same canonical revision. Fetch the named
  canonical ref and prove ancestry rather than using `ls-remote` with a SHA:

  ```bash
  cd /data/nvme0/can/canix/projects/repos/owned/github.com/memorycircuits/SynDB
  revision="$(jq -r '.nodes.queryfabric.locked.rev' flake.lock)"
  git -C vendor/queryfabric fetch origin trunk
  git -C vendor/queryfabric merge-base --is-ancestor "$revision" origin/trunk
  ```

### B05: SynDB production Flight selects incomplete handlers

- **Missing/invalid input:** a production-path Flight service with implemented
  data-plane operations and real access evaluation.
- **Why required:** this is a live correctness regression independent of the
  future extraction design.
- **Upstream producer:** SynDB Flight server wiring.
- **Recovery workflow:** immediately serve the existing
  `SyndbFlightService` again; add a non-ignored test that boots
  `start_flight_server`. Do not select the generic skeleton again until Phase
  05's parity contract passes.
- **Validation:**

  ```bash
  cd /data/nvme0/can/canix/projects/repos/owned/github.com/memorycircuits/SynDB
  nix develop . -c cargo test -p flight --locked
  ```

### B06: QueryFabric-owned isolated execution fixtures do not exist

- **Missing/invalid input:** a QueryFabric worker binary, OCI image, generic
  manifests/chart, and self-contained Kind fixture.
- **Why required:** the ignored smoke test references
  `infrastructure/helm/syndb-clickhouse`, `.#oci-syndb-burst-worker`, and SynDB
  environment/secrets that canonical QueryFabric cannot generate.
- **Upstream producer:** a deliberate extraction from the SynDB burst-worker
  and deployment path, after the worker protocol is redesigned.
- **Regeneration workflow:** implement Phase 06, then build
  `.#oci-queryfabric-test-worker` and run the non-ignored QueryFabric-owned
  Kind test.
- **Validation:**

  ```bash
  nix build .#oci-queryfabric-test-worker
  nix develop -c cargo test -p queryfabric-runtime-k8s \
    --features integration-k8s --test kind_smoke -- --nocapture
  ```

The exact final test target may be named during implementation, but it must be
owned by QueryFabric and must not require an untracked SynDB chart or image.

### B07: Grant context artifacts lack rights metadata

- **Missing/invalid input:** producer-supplied copyright holder and SPDX licence
  for the four grant context/template files now held under the applications
  checkout's `docs/grants/` (and the other unannotated application documents).
- **Why required:** their presence makes the repository fail REUSE and no
  maintainer may silently relicense third-party/generated context material.
- **Upstream producer:** the person/toolchain that created and supplied the
  grant context/template artifacts, plus the rights holder who can license
  them.
- **Regeneration workflow:** obtain the actual holder/licence in writing. Then
  either add those exact facts as a dedicated `REUSE.toml` annotation covering
  the applications checkout's grant files, or run `reuse annotate` there with
  the supplied values and `--force-dot-license`. If the producer cannot license
  the files, keep them outside any release repository; do not substitute
  QueryFabric maintainer ownership.
- **Validation:**

  ```bash
  nix develop /data/nvme0/can/canix/projects/repos/owned/codeberg.org/caniko/queryfabric \
    -c bash -lc 'cd /data/nvme0/can/canix/projects/personal/professional/applications && reuse lint'
  ```

### B08: Proposal-use GenAI provenance is not preserved in the repository

- **Missing/invalid input:** the native record of model label, per-message
  dates/times, every prompt, and every unedited output for the AI-assisted
  planning used here. Preserve an exact version too if the client exposes it;
  version is prudent application provenance and explicitly relevant to
  generated project content, but is not a separate live-form application field.
- **Why required:** NLnet policy/form disclosure cannot be satisfied by a
  reconstructed summary. The missing record blocks use of this work in an
  application, not engineering implementation.
- **Upstream producer:** the current Codex conversation platform/session and
  the applicant retaining its native export.
- **Regeneration workflow:** before proposal use, export/preserve the full
  current conversation from the client, record the model label and timestamps
  shown there, preserve the version if exposed, and keep prompts and outputs
  unedited. If the native history cannot be recovered, do not reuse generated
  proposal prose; write the application independently from the cited primary
  sources and disclose any remaining substantive assistance. Do not recreate
  missing timestamps or model identifiers from memory.
- **Validation:** compare the retained record message-for-message with the live
  session and verify every field required by the current
  [NLnet GenAI policy](https://nlnet.nl/foundation/policies/generativeAI/) and
  [proposal form](https://nlnet.nl/propose/) before submission. There is no
  honest repository command that can regenerate missing platform metadata.

### B09: Documentation examples lack an honest executable gate

- **Missing/invalid input:** real Cargo test/example targets for critical public
  examples. `mdbook test docs` now passes after fence classification, but that
  only proves the documentation snippets are not accidentally compiled; it
  does not prove the public examples execute against the current API.
- **Why required:** `mdbook build` alone only renders prose; it does not prove
  public API or migration instructions, so it is insufficient milestone/release
  evidence.
- **Upstream producer:** QueryFabric documentation/API maintainers.
- **Regeneration workflow:** inventory every fence; mark diagrams and command
  output `text`, shell examples with their real language, and partial snippets
  explicitly non-testable. Move critical Rust examples into workspace examples,
  crate doctests, or integration tests that link the actual crates. Replace the
  portability page with the implemented schema/import API and remove signing
  language unless a trust design exists.
- **Validation:**

  ```bash
  nix develop -c mdbook build docs
  nix develop -c mdbook test docs
  nix develop -c cargo test --doc --workspace --locked
  ```

### B10: Fediversity has not supplied a supported external contract pin

- **Missing/invalid input:** a versioned or explicitly supported immutable
  Fediversity application-resource contract boundary for external consumers.
  Current `main` says the project is in development and its internal contract
  system is edited in place.
- **Why required:** without this artifact QueryFabric cannot honestly claim
  Fediversity compatibility, adoption, conformance, or a stable integration
  gate. It does not block the host-to-host tabular resource-portability MVP.
- **Upstream producer:** Fediversity maintainers, followed by QueryFabric
  maintainers accepting and locking the boundary.
- **Regeneration workflow:** contact the upstream maintainers, identify the
  supported PostgreSQL/S3 application interface, record the public response,
  and pin an immutable tag/revision in an isolated Nix integration input. Do
  not copy the framework or pin mutable `main` as a stable API.
- **Validation:** the future `fediversity-contract` check must name the pinned
  revision and pass its PostgreSQL TLS plus Garage/separate-credential fixture:

  ```bash
  nix build .#checks.x86_64-linux.fediversity-contract -L
  ```

  Until that check and upstream support evidence exist, the correct validation
  is that product/application docs contain no Fediversity compatibility or
  endorsement claim.

### B11: QueryFabric's canonical SSH remote contains trailing whitespace

- **Missing/invalid input:** a syntactically valid `remote.origin.url`. The
  current `.git/config` value is
  `ssh://git@codeberg.org/caniko/queryfabric.git ` with a trailing space.
- **Why required:** `git ls-remote origin` fails with `Forgejo: Invalid repo
  name`, so Phase 00 ancestry, review-ref, tag, fetch, and later release proofs
  cannot use the configured canonical remote. The equivalent HTTPS URL is
  reachable and reports no tags, so this is local configuration corruption,
  not evidence that the public repository is absent.
- **Upstream producer:** this checkout's clone/remote-registration workflow. If
  no repeatable producer owns it, the immediate producer is local `.git/config`.
- **Regeneration workflow:** repair only the malformed value, then fix any
  workspace registry/bootstrap source that recreates it:

  ```bash
  git remote set-url origin 'ssh://git@codeberg.org/caniko/queryfabric.git'
  ```

- **Validation:**

  ```bash
  test "$(git remote get-url origin)" = \
    'ssh://git@codeberg.org/caniko/queryfabric.git'
  git ls-remote --exit-code origin HEAD
  git ls-remote --tags origin
  ```

## Risks And Constraints

- Moving code before fixing lineage could strand fixes on the rewritten topic
  branch or silently drop its web behavior.
- Deleting a SynDB duplicate before moving its tests can regress retry,
  authorization, metadata, or lifecycle behavior while still compiling.
- A generic name does not make a contract generic. `SYNDB_*` variables,
  `syndb-snapshot-*` names, domain descriptors, and allow-all policies are
  evidence that extraction is incomplete.
- Publishing all 19 currently publishable crates would freeze incomplete CLI,
  worker, K8s, changelog, type, and test-rig APIs.
- Raw identifier emission is a release-blocking injection risk even when
  literal values are parameterized.
- Bundle content hashes provide integrity only relative to a trusted expected
  hash; describing them as signatures would create a false authenticity claim.
- The current key-sorting/`serde_json` canonicalizer, ad-hoc schema fingerprint,
  and “BLAKE3 unless host says otherwise” comment are not a language-neutral
  import contract. Import-ready 2.0 needs RFC 8785 and normative typed digests.
- Bundles do not carry full access policy or ownership. Source actor/resource
  identifiers must remain origin-attributed evidence; mapping them into target
  authorization would be a privilege-confusion defect.
- A dry-run plan without target-state/staging identity is vulnerable to TOCTOU;
  apply must re-authorize and revalidate immediately before commit.
- Automatic dereferencing of bundle artifact URIs or JSON-LD contexts would
  create SSRF/path/resource-exhaustion surfaces. The neutral crate must never
  fetch them.
- A multi-store import can expose partial state unless staging, PostgreSQL
  transaction boundaries, durable receipts, idempotency, and cleanup semantics
  are explicit and failure-injected.
- The live demo uses process-local provenance/ownership stores and a local DOI
  provider. It can support an MVP demonstration claim, not a production
  durability claim.
- The current federation endpoint reports identity/configuration, not query
  fan-out. Documentation must not infer data-plane federation from a healthy
  status response.
- Existing canonical docs link to missing grant planning artifacts. Durable
  content must be intentionally ported or the links removed; topic-branch files
  must not be silently treated as canonical.
- The official Fediversity portfolio already covers fleet management,
  placement/HA, Nix packaging, service catalogs, storage, verified boot, and
  application federation. Expanding QueryFabric into those areas would weaken
  differentiation and inflate the MVP.
- Applicant facts, upstream interest, budgets, rates, patents, prior funding,
  European dimension, and AI provenance are not repository facts. Guessing any
  of them would invalidate grant-facing evidence.

## Candidate Next Steps

Execute the phase plan in
`docs/src/planning/syndb-generic-extractions-mvp/README.md`:

1. repair validation inputs and canonicalize QueryFabric lineage;
2. restore the SynDB Flight production path;
3. harden the compiler and define the stable release tier;
4. converge the retained SynDB extractions;
5. publish bundle 2.0 with RFC 8785 and the typed CSV profile, then implement
   bounded validation plus a neutral import plan/report;
6. prove authenticated export-transfer-import-restart between isolated NixOS
   instances, with security/accessibility/docs/footprint evidence;
7. produce the release-candidate and authorized public milestone handoff;
8. graduate Flight only after MVP;
9. implement isolated execution only after the Flight contract is sound; and
10. disposition the remaining utility backlog and retire obsolete plans.

Phases 01 and 02 can run in parallel after Phase 00. Phase 03 follows both:
its Flight-skeleton cleanup preserves Phase 01's restored production selection,
and its DynamicClient cutover consumes Phase 02's typed identifier contract. The
MVP gate is the end of Phase 04C after the separate 04A tabular-format and 04B
host-apply/persistence gates. Phases 05–07 are not allowed to delay or inflate
that release unless the product boundary is explicitly changed.

## Open Decisions For The User

The plan uses these defaults unless explicitly changed:

1. **MVP boundary:** portable compiler plus one versioned tabular-profile
   export-transfer-import between independent PostgreSQL/S3-backed NixOS
   reference hosts;
   production federation, HA, a second embedded backend, and isolated execution
   are post-MVP.
2. **Release tag:** exact SemVer without a `v` prefix, matching generated
   workflows.
3. **Stable Rust:** remove/relocate the globally inherited nightly ancestor
   config, then give QueryFabric an explicit stable configuration instead of
   adopting nightly-only developer defaults.
4. **Publish tier:** publish the ten-crate compiler dependency closure only;
   mark the other nine currently publishable crates registry-unpublished until
   graduated. Keep the portability schema/fixtures/source public even if its
   Rust crates remain registry-unpublished in 0.2.
5. **Flight safety:** restore the direct SynDB service now, then migrate to the
   skeleton only after server-path parity tests.
6. **Grant-facing evidence:** port verified technical boundaries into durable
   docs and remove broken application-plan links in Phase 02, before Phase 04;
   do not copy stale applicant claims, budget/rate data, or AI-generated
   proposal prose.
7. **Fediversity adapter:** treat current upstream contracts as research
   evidence only. Create a separate non-MVP compatibility gate only after
   maintainers supply and QueryFabric pins a supported immutable external
   boundary.

Expanding the first decision to full service migration, production HA,
federation transport, a real worker, or a second embedded backend would add
persistent registry/state, provider orchestration, OCI/cluster artifacts, and
several independent end-to-end programmes to the critical path. That is a
different MVP, not a small phase adjustment.
