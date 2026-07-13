# Phase 04: Tabular Import, Reference Host, And Release Proof

## Goal

Complete the portable-import R&D and one secure, typed, reproducible
host-to-host tabular-resource migration vertical slice, then produce a
release-candidate handoff for the MVP.

Passing this phase means all three internal gates are complete:

- **04A, tabular profile/format:** a versioned profile bundle can be validated
  and converted into a neutral import plan;
- **04B, host apply/persistence:** a predeclared target can apply the plan with
  durable evidence and correct authorization/transaction semantics; and
- **04C, independent-host proof:** an operator can export, transfer, dry-run,
  import, query, and restart one profile-conforming tabular resource across two
  isolated NixOS hosts.

The MVP is then ready for an explicitly authorized tag and publication. This
phase does not itself authorize pushing, tagging, publishing, opening an
upstream issue/PR, or deploying.

## Gate 04A: Versioned Tabular Portability Profile

### 1. Publish the bundle schema, tabular profile, and fixtures

Do not retrofit importability onto the current under-specified bundle `1.0`.
Document it as a legacy export-only format and introduce import-ready bundle
`2.0` with profile `queryfabric.tabular-csv/1`. Bundle/profile stability is
separate from Rust API stability. Publish:

- a machine-readable JSON Schema for the exact `2.0` envelope and tabular
  profile;
- canonical valid/minimal/complete fixtures and their expected BLAKE3 digests;
- invalid fixtures for unsupported version, malformed resource identity,
  duplicate/conflicting artifacts, invalid digest/size, forbidden extension
  shape, and resource-budget overflow; and
- a compatibility policy that says which changes require a new major bundle
  version and how unknown versions/fields are handled.

Generate or mechanically check schema/fixtures from the Rust types so prose,
Serde behavior, and test data cannot drift independently. Do not call either
bundle signed: content addressing has no signature or key trust model.

Use [RFC 8785 JSON Canonicalization Scheme](https://www.rfc-editor.org/rfc/rfc8785)
for bundle `2.0`, including its I-JSON constraints, rather than the current
language-specific “sort keys plus
`serde_json` formatting” implementation. Publish cross-language vectors for
Unicode ordering/escaping, numbers, nested objects/arrays, duplicate-key
rejection, and unknown fields. Declare typed `blake3-256` digests for bundle,
artifact, and schema fingerprints. Version `1.0` retains its historical
QueryFabric canonicalizer and is never silently rehashed as `2.0`.

The tabular profile is deliberately narrow:

- UTF-8 without BOM; comma delimiter; double-quote quoting with doubled quotes;
  CRLF records; no comments; and one required header whose decoded names/order
  exactly match the typed schema;
- non-nullable columns only in profile 1, avoiding an ambiguous CSV null token;
  supported types are Boolean, signed Int64, finite Float64, UTF-8 string,
  UUID, and RFC 3339 UTC timestamp ending in `Z`; invalid/overflow/non-finite
  values, duplicate headers, or missing/extra fields reject the artifact;
- lowercase `true`/`false`; JSON-number grammar for integers/floats with no
  leading plus; lowercase hyphenated UUID; and normative lexical/test vectors
  for every type, including CSV escaping and Unicode;
- a schema fingerprint over the RFC 8785 canonical typed schema, not the
  current ad-hoc colon/comma string;
- one `table_export` artifact and explicit decoded row/byte limits; and
- mapping only to a host-predeclared relation adapter whose accepted profile
  and portable-column schema fingerprint match.

The mapping is explicit by column name/type, never implicit physical position.
It may declare host-injected target columns such as the mapped resource key and
a deterministic row identifier; those columns and their derivation are part of
the versioned host adapter and import plan, not falsely included in the portable
artifact schema.

Replace the demonstrator's handwritten CSV concatenation with the normative
writer. Dynamic relation registration, DDL, arbitrary profile discovery, and
automatic schema evolution are non-goals. The reference host may derive local
row identities deterministically from the target resource plus canonical row
ordinal/content, but that rule is a versioned host-adapter contract and must be
tested for uniqueness and replay.

### 2. Add bounded validation and import artifacts

Add neutral types along these lines, with final names chosen during API review:

- `ImportLimits`: maximum bundle bytes, JSON nesting, string length, artifact
  count, aggregate artifact bytes, rows, and extension bytes;
- `ValidatedBundle`: supported version, canonical bytes/digest, validated
  manifests, carried licence/restriction/source-provenance/citations/metadata,
  and source resource;
- `ImportPlan`: target mapping proposal, required artifact actions, conflicts,
  warnings, target catalog/state revision, immutable staged-object identities,
  and a deterministic digest over a versioned RFC 8785 plan document; and
- `ImportReport`: expected/actual bundle digest, artifact verification facts,
  target resource, idempotency outcome, persisted receipt identity, and
  structured diagnostics.

Validation must complete before any host-visible mutation. It must:

1. reject unsupported versions and inputs over every configured bound;
2. canonicalize the parsed document and verify a caller-supplied expected
   bundle digest using constant-time digest comparison where applicable;
3. validate manifest kind/format, digest syntax, byte/row counts, schema
   fingerprint, and duplicate/conflicting entries;
4. preserve licence and data-use restrictions as mandatory local policy input
   rather than informational text;
5. preserve citations and source provenance as origin-attributed evidence,
   without asserting that the target created the source history or that source
   actor IDs authorize anything locally;
6. report explicitly that the current bundle carries neither full
   `AccessPolicy` nor ownership, so neither can be “preserved”; and
7. never dereference a storage URI, JSON-LD context, or other remote reference.

The expected digest is meaningful only when it arrives through a trusted
channel. In the reference host that channel is an authenticated operator
request. A future signature scheme needs its own key-distribution, revocation,
algorithm-agility, and threat-model design; it is not implied by this phase.

Treat each `storageUri` as source metadata, never as a fetch instruction. The
dry-run request binds every manifest index/digest to an already staged target
object. The host streams those bytes under limits, checks actual byte count and
digest, decodes the supported format, recomputes schema fingerprint and row
count, and rejects any declared/actual mismatch before apply. The canonical
source bundle is preserved; target storage locations belong in the import plan
and receipt rather than by rewriting the bundle.

The host assigns a real local target owner through its authorization policy and
records a new target-side `Imported` provenance event containing the importer,
source resource, target resource, bundle digest, and source-provenance digest.
Source ownership and actors remain evidence only. The durable mapping and
receipt must distinguish carried source facts from local authorization state.

### 3. Keep import policy and I/O at the host boundary

The portability crate returns facts and a plan. Host adapters own:

- import authorization and actor identity;
- accepted artifact kind/format and storage-URI schemes;
- transfer/staging and target object naming;
- source-to-target resource and catalog mapping;
- conflict, replacement, and data-use policy;
- format decoding and target schema/application logic;
- transaction, rollback, receipt persistence, and staged-object cleanup; and
- durable ownership/provenance stores.

An import API must support a dry-run that performs the same parsing,
verification, mapping, and conflict checks as apply. Apply must bind the
dry-run plan digest, target catalog/state revision, and immutable staged-object
identities so a changed plan or target cannot be committed silently. Use
content-addressed immutable staging where possible. Apply re-authenticates the
actor, rechecks target conflicts/revision, and revalidates artifact digest,
size, decoded schema, and actual row count immediately before the database
transaction; a prior dry-run is never authorization or freshness by itself.

### 4. Define replay and failure semantics

Use the verified bundle digest plus target mapping as the idempotency identity.
Re-applying an already committed identical import returns the original receipt;
the same source with a different mapping or content is a structured conflict.

PostgreSQL and object storage cannot provide one distributed ACID transaction.
The host therefore stages and verifies objects first, commits rows/metadata,
carried source evidence, local policy/owner, target import event,
source-to-target mapping, and receipt together in PostgreSQL, and exposes the
resource only after commit. A failed commit leaves no visible resource and no
receipt; unreferenced staging objects are safe to garbage-collect. Tests must
inject failure before and during commit and prove this visibility contract.

## Gate 04B: Reference Host Apply And Persistence

### 1. Make an actually fresh target possible

Split database schema migration from demonstration-data seeding. Add an
explicit seed policy whose production-safe default and demo behavior are
documented. The beta fixture must start with schema only and must not seed the
resource that alpha later transfers.

Replace process-local imported state with a durable reference implementation
for import receipts, origin-attributed source provenance, local target policy
decisions/ownership, target import events, and source-to-target mapping.
Restarting beta must not silently reconstruct those facts from the hard-coded
demo dataset.

### 2. Implement the bounded host apply contract

Support only `queryfabric.tabular-csv/1` into a predeclared target relation.
Dry-run chooses the target resource/relation, validates the profile/schema,
assigns the proposed local owner and applicable licence/restriction policy,
decodes every row under limits, and returns conflicts plus the bound target
state revision. Apply repeats all security/freshness checks and commits target
resource metadata, deterministic row identities/data, local policy/owner,
origin-attributed source evidence, the new target import event, mapping, and
receipt under the atomic-visibility contract.

### 3. Separate database authority

Use distinct credentials/principals and connection pools for:

- schema migration administration, available only to the migration unit;
- read-only compiled-query execution; and
- narrowly writable import/state persistence, limited to the predeclared
  target and import-state tables with no DDL/role authority.

The query HTTP path must hold only the read-only pool and must not be able to
reach the import writer through a generic database handle. Import endpoints are
separately authorized and use only the narrow writer. Test database grants and
application wiring, not merely role names.

## Gate 04C: Multi-Node Migration And Release Evidence

### 1. Prove operator-mediated transfer

Keep `nix/tests/selfhost.nix` as the single-host/module regression test and add
a separate multi-node `nix/tests/portability-migration.nix` (final name may be
adjusted consistently) in which alpha and beta use distinct:

- PostgreSQL roles and databases;
- S3-compatible buckets and credential files;
- state directories, ports, and catalog snapshot identities; and
- authorization identities.

Run alpha and beta as separate NixOS test nodes with independent database and
object-store endpoints. A third transfer node or tightly scoped transfer unit
gets source-read and target-staging-write credentials only. It exports on
alpha, copies the sealed bundle and declared artifacts into beta's staging
namespace, calls beta dry-run with the independently obtained expected digest,
applies the bound plan, and queries the imported rows through the typed query
endpoint. Beta must never receive alpha's database URL or any source-bucket
credential, and alpha must never receive beta's database or ordinary bucket
credential.

Restart beta and prove imported rows, carried source evidence, local
policy/owner, target import event, mapping, and receipt remain. Then cover
tampered bundle/artifact, unsupported version, oversize, forbidden URI/path,
mapping conflict, replay, unauthorized request, stale target revision/staging
identity, and injected-apply failure; none may expose partial state.

This proves one tabular resource conforming to the published profile. It does
not prove arbitrary resource import, dynamic schemas, full application service
migration, federation, failover, or HA.

### 2. Add accessibility and documentation evidence

- Add a pinned flake `checks.accessibility` check for the actual demo page and
  documentation output.
- Publish a scoped manual review covering keyboard operation, focus, labels,
  headings, errors/status, zoom/reflow, and contrast. Record findings and
  remediation instead of claiming WCAG conformance in advance.
- [x] Repair `docs/src/scenarios/data-portability.md`: it now describes
  unsigned content-addressed bundles and the implemented HTTP import API.
- [x] Classify every documentation fence as compilable Rust, `text`, shell, or
  explicitly illustrative; `mdbook test docs` and `cargo test --doc --workspace`
  now pass. Critical examples still need dedicated Cargo integration
  test/example targets before they can be treated as executable documentation.
- Verify Phase 02's durable roadmap/threat-model grant-link repair and the
  bounded MVP claims; no absent application-plan link may survive release.
- Expand `GOVERNANCE.md` only with verified maintainer/release/security
  ownership, decision and succession procedures. If the project is currently a
  single-maintainer project, state that and its continuity risk rather than
  inventing a team.

### 3. Produce security and reproducibility evidence

Update the threat model for untrusted bundles, expected-hash delivery, remote
reference denial, decompression/JSON/resource exhaustion, path/URI injection,
schema confusion, policy downgrade, replay, staged-object cleanup, partial
commit, provenance origin, and authorization.

Retain REUSE, `cargo audit`, and `cargo deny` evidence plus the responsible
disclosure route. Pin the RustSec advisory database as an immutable flake input
and expose offline-capable `checks.audit` and `checks.deny`; replace generated
CI's mutable shallow clone with those checks. Measure the footprint twice from
the clean candidate using a tolerance and machine/toolchain context declared
before measurement. Existing dirty/stale footprint output is not release
evidence.

The four supplied grant artifacts were moved to the canonical applications
checkout, outside QueryFabric's release tree. The applications repository
still fails REUSE for those files and its other unannotated documentation. Its
producer must supply the rights holder and SPDX licence, after which an
authorized maintainer can add an explicit annotation or per-file metadata and
rerun `reuse lint`. Do not assign those legal facts to QueryFabric maintainers
without authority.

### 4. Prepare, but do not fabricate, upstream adoption

Make the NixOS package/module conform to the pinned nixpkgs module/package and
test conventions, with generated option documentation and a self-contained
module test suitable for upstream review. Produce an upstreamable patch and a
maintainer handoff that names remaining policy questions.

Opening an issue/PR or contacting maintainers is an external action and needs
explicit authority. Third-party merge acceptance is not an MVP gate. Do not
claim nixpkgs upstreaming until a public issue/PR exists, or adoption until it
is actually merged.

## Typed Query And Release Baseline (Gate 04C)

### Query API Contract

Introduce a versioned request contract with:

- `query` text;
- `dialect` (`sql` or `syql`);
- typed `parameters` represented as exactly one mode: a named map or a
  positional list (absence means no parameters);
- optional `expectedCatalogSnapshotId` for optimistic consistency; and
- optional requested backend as an advisory, with the host retaining final
  selection policy.

The reference host may support only PostgreSQL execution in 0.2, but it must
report that choice explicitly. It must reject an unavailable requested backend
through a structured capability diagnostic rather than silently changing
semantics.

The response contract includes:

- contract version;
- chosen backend and emitted dialect;
- catalog snapshot identity;
- result schema;
- ordered parameter schema without secret values;
- provenance receipt/query hash;
- structured diagnostics and capability decision;
- rows, row count, and a truncation indicator; and
- emitted SQL only in an explicitly documented demo/debug field.

Do not expose credentials, bearer tokens, raw secret parameters, or internal
database errors.

### Work

#### 1. Bind and execute typed parameters

Reuse the facade's parameter inspection/JSON conversion helpers. Validate the
selected named or positional mode against the compiler's `ParameterSchema`,
preserve backend ordering, and bind values through the PostgreSQL driver rather
than interpolating or always passing `[]`. Reject any wire representation that
tries to supply both modes.

Test nullability, integers at range boundaries, floating point, booleans,
strings, UUIDs, timestamps if supported, repeated references, missing values,
extra values, and mixed named/positional errors.

#### 2. Make the catalog snapshot explicit

The host constructs the immutable catalog and chooses its authoritative
snapshot ID. Return that ID in every response. If a request supplies
`expectedCatalogSnapshotId` and it does not match, reject before execution with
a stable conflict diagnostic.

#### 3. Enforce host execution policy

Run emitted SQL using a read-only PostgreSQL role or read-only transaction.
This query path receives only the query pool from Gate 04B; it cannot borrow or
downcast to the import/state writer. Schema migration uses neither runtime pool.
Apply configurable:

- compile timeout;
- execution timeout;
- maximum returned rows;
- maximum response bytes; and
- cancellation on client disconnect or server shutdown.

Phase 02 compiler budgets apply before execution. Truncation must be explicit;
timeouts and cancellations must not appear as successful empty results.

Add host HTTP middleware that resolves a bearer/session credential into the
generic contract `Subject`; the concrete identity provider remains outside
QueryFabric. Require that subject for endpoints that mutate ownership,
provenance, exports, or deletion state. If the public demonstrator retains an
anonymous/local mode, disable mutation there or label and isolate it as
non-production. The current implicit default operator is not a production
authorization mechanism.

#### 4. Extend vertical-slice tests

Add HTTP integration tests and extend `nix/tests/selfhost.nix` to cover:

- one named or positional parameterized query;
- the returned result schema, provenance, and snapshot ID;
- invalid parameter type/count;
- malicious identifier/catalog input;
- stale expected snapshot;
- unavailable backend/capability;
- compile and execution budget rejection;
- missing and invalid credential rejection on mutation endpoints;
- anonymous/local-mode mutation denial;
- authenticated mutation attribution to the resolved `Subject`;
- cancellation; and
- read-only database enforcement.

The two-instance VM remains useful deployment proof, but federation status
alone must not be described as data-plane federation.

#### 5. Make release automation single-source

- Use exact SemVer tags without a `v` prefix, matching generated workflow
  validation.
- Retain `scripts/release.sh check` as the local non-publishing gate, but remove
  or subordinate its legacy `v${version}` tagging/publishing path and the
  competing release-tool path. Simit owns versioning, ordering, and tag
  creation.
- Generate publish order from Cargo metadata through simit.
- Use the Phase 02 flake-pinned simit package to regenerate CI. The user-profile
  `/etc/profiles/per-user/can/bin/simit` is not reproducible release evidence.
- Regenerate/remove publish workflows so no `publish-crate-*.yaml` targets a
  `publish = false` package.
- Remove the release artifact job when there are no artifacts, or configure
  real build commands and checksums. An unconditional `exit 1` is not a gate.
- Align `COMPATIBILITY.md`, workspace `rust-version`, CI MSRV, and the Nix
  toolchain.
- Add a Nix `checks.msrv` derivation using the exact selected compiler (1.94 at
  this baseline unless the project deliberately changes it). Run the stable
  ten-crate feature matrix under that compiler as well as current stable.
- Make Python a documented preview unless Phase 04 also builds a wheel from
  `packages/queryfabric`, installs it into a clean environment, imports it, and
  runs Python tests. Do not claim a PyPI release that does not exist.
- Update README/install docs to distinguish source checkout, crates.io, and
  PyPI availability.

#### 6. Prove packages and downstream compatibility

Generate the ten-crate dependency order with:

```bash
nix develop -c simit --version
nix develop -c simit release plan --workspace
```

Run `cargo package`/`cargo publish --dry-run` wherever the first-release
dependency constraint permits. For crates whose same-version dependencies do
not yet exist on crates.io, record that exact constraint rather than claiming a
successful all-workspace dry run. Optionally prove ordered publication against
a disposable local registry; public publication remains a staged operator
action.

Converge SynDB on the release-candidate commit and run its focused and workspace
gates under the repaired Phase 00 environment.

## Deliverables

- legacy/export-only 1.0 disposition plus bundle 2.0 JSON Schema, RFC 8785
  cross-language vectors, typed digest contract, tabular CSV profile, and
  compatibility policy;
- bounded validator plus neutral import plan/report with integrity, portable
  source evidence, conflict, freshness, replay, and failure semantics;
- authenticated dry-run/apply host API with predeclared-target mapping, local
  policy/owner/import event, durable receipt, separated database authority, and
  atomic visibility;
- independent alpha -> beta transfer/import/restart proof with no shared
  database or ordinary bucket credential;
- versioned typed query request/response;
- prepared PostgreSQL execution with catalog identity, limits, and
  cancellation;
- security/rejection coverage in unit, HTTP, and NixOS tests;
- repaired portability documentation, accessibility evidence, current threat
  model, honest governance/sustainability guidance, and two-run footprint
  evidence;
- coherent exact-SemVer release automation;
- honest Rust/Python/package documentation;
- generated ten-crate publish plan; and
- SynDB downstream proof against one canonical commit.

## Acceptance

- [ ] QueryFabric gates pass from the repository root:

  ```bash
  nix develop -c cargo fmt --all -- --check
  nix develop -c cargo fmt --manifest-path fuzz/Cargo.toml --all -- --check
  nix develop -c cargo clippy --workspace --all-targets --locked -- -D warnings
  nix develop -c cargo test --workspace --all-targets --exclude queryfabric-python --locked
  nix develop -c cargo test -p queryfabric-python --locked
  nix develop -c cargo clippy --workspace --all-targets --all-features \
    --locked -- -D warnings
  nix develop -c cargo test --workspace --all-targets --all-features \
    --exclude queryfabric-python --locked
  nix develop -c cargo test --doc --workspace --locked
  nix develop -c cargo test -p queryfabric-portability --all-features --locked
  nix flake check -L
  nix build .#checks.x86_64-linux.audit -L
  nix build .#checks.x86_64-linux.deny -L
  nix develop -c reuse lint
  nix develop -c mdbook build docs
  nix develop -c mdbook test docs
  nix develop -c scripts/release.sh check
  nix develop -c sh -c \
    'case "$(command -v simit)" in /nix/store/*) ;; *) exit 1 ;; esac'
  nix develop -c simit release plan --workspace
  ```

- [x] `nix build .#checks.x86_64-linux.selfhost -L` passes on the target CI
      architecture.
- [x] `nix build .#checks.x86_64-linux.portability-migration -L` boots separate
      alpha/beta nodes plus the scoped transfer path and passes the complete
      export-transfer-import-restart/rejection sequence.
- [ ] `nix build .#checks.x86_64-linux.bundle-schema -L` regenerates/checks the
      2.0 schema/profile, RFC 8785 cross-language vectors, typed-schema
      fingerprints, and canonical fixture digests without drift.
- [ ] `nix build .#checks.x86_64-linux.accessibility -L` passes, and the scoped
      manual review records browser/assistive-technology coverage, findings,
      and remediation.
- [ ] `nix build .#checks.x86_64-linux.msrv -L` passes with the exact declared
      MSRV and compiles/tests every stable crate's supported feature matrix.
- [ ] `checks.audit` uses the flake-locked RustSec database with no network
      fetch, and `checks.deny` runs `cargo deny check bans licenses sources`;
      generated CI calls the same checks.
- [ ] Valid import fixtures produce deterministic plan/report facts; invalid,
      unsupported, oversized, forbidden-reference, tampered, duplicate, and
      conflicting fixtures fail before host-visible mutation.
- [ ] Profile tests cover commas/quotes/CRLF/Unicode, exact headers, every
      supported scalar boundary, invalid UTF-8, empty string (valid only for a
      string column), absent null representation, overflow/non-finite values,
      schema mismatch, and decoded row/field/aggregate limits.
- [ ] dry-run/apply is bound to the target state/catalog revision and immutable
      staged objects; apply re-authorizes and revalidates bytes, decoded schema,
      row count, mapping, and conflicts so stale plans fail closed.
- [ ] Alpha exports a `queryfabric.tabular-csv/1` resource, the operator
      transfers its exact bundle/artifact into beta staging, and initially empty
      beta dry-runs/imports it into a matching predeclared relation without
      source database or ordinary source-bucket credentials.
- [ ] Beta returns imported rows with origin-attributed source evidence, a real
      local owner/policy and target import event, records the mapping/receipt,
      survives restart, and treats an identical replay idempotently.
- [ ] Injected failures before/during commit leave no visible resource, carried
      source evidence, local owner/policy, target import event, mapping, or
      receipt; unreferenced staging cleanup is tested.
- [x] A parameterized HTTP query reaches PostgreSQL with prepared values and
      returns schema, provenance, diagnostics, and snapshot ID.
- [ ] Every listed security/rejection case has a non-ignored test.
- [ ] HTTP and NixOS tests assert missing/invalid credentials and anonymous
      mutation are denied, while an authorized mutation records the resolved
      subject identity.
- [ ] The database role cannot mutate schema or data through the query path.
- [ ] migration-admin, read-only query, and narrow import/state writer
      principals are distinct; tests prove the query path cannot reach the
      writer and the writer cannot perform DDL or role administration.
- [ ] The Cargo-metadata-derived publishable set equals the ten stable compiler
      crates; registry-unpublished workspace crates may still appear in
      metadata.
- [ ] A metadata-to-workflow check proves no publish workflow targets a
      registry-unpublished crate.
- [ ] Release and PyPI workflows contain no guaranteed failure and use the
      correct package working directory.
- [ ] `nix develop -c simit --version` reports the flake-locked tool, and
      regenerated-workflow drift checks pass.
- [ ] Exact SemVer is used consistently by simit, tag validation, changelog,
      release helpers, and documentation.
- [ ] QueryFabric's release tree excludes the grant application packet; the
      canonical applications checkout has producer-supplied REUSE metadata (or
      the packet remains outside any release tree); no maintainer-assigned
      copyright/licence is fabricated.
- [x] the portability chapter contains no “signed” claim unless a signature
      and key-trust design was implemented and tested, and all shown APIs exist.
- [ ] roadmap/threat-model links resolve without relying on a nonexistent grant
      application file.
- [ ] the NixOS package/module has a self-contained upstream-style test,
      generated option documentation, and a reviewable patch/handoff; public PR
      or merge claims appear only after those external events actually occur.
- [ ] governance documentation names only real maintainers/roles and records
      the actual decision, release, security, and continuity process.
- [ ] the footprint report names the clean candidate commit, toolchain and
      machine context, contains two runs, and stays within the predeclared
      tolerance or explains/remediates the variance.
- [ ] SynDB passes:

  ```bash
  cd /data/nvme0/can/canix/projects/repos/owned/github.com/memorycircuits/SynDB
  nix flake check --no-build
  nix develop . -c uv sync --locked
  nix develop . -c cargo build --workspace --locked
  nix develop . -c cargo clippy --workspace --locked -- -D warnings
  nix develop . -c cargo test --workspace --locked
  ```

- [ ] Both worktrees are clean after gates, and dependency revisions still
      identify the tested commit.

The local phase may end at a release-candidate handoff. The stronger public or
grant milestone claim requires a maintainer-authorized, reachable source
revision/tag plus a public changelog/status page linking the exact commands and
evidence above. Registry publication is claimed only after the registry can be
queried successfully.

## MVP Exit Statement

After acceptance, documentation may claim:

> QueryFabric 0.2 is a portable SQL/SyQL compiler for PostgreSQL and ClickHouse,
> with typed catalog binding, capabilities, result schema, provenance, safe
> emission, and a reproducible NixOS reference proof that exports, transfers,
> validates, and imports one tabular resource conforming to the published
> QueryFabric profile between independent PostgreSQL/S3-backed hosts with
> durable import receipts.

It may not claim production Flight migration, Kubernetes isolated execution,
worker images, full service/provider migration, high availability, data-plane
federation, Fediversity integration/adoption, general durable demo state,
crates.io/PyPI availability before publication, production-ready
authentication, low-resource operation beyond the measured fixture, or WCAG
conformance beyond the completed audit evidence. It also may not generalize the
proof to arbitrary resources, dynamic DDL/catalog creation, or other tabular
profiles.

## Stop Conditions

Stop release-candidate handoff if any stable crate packages code with raw
untrusted identifiers, any request error becomes a successful empty response,
the NixOS migration slice is ignored/unbuildable, an import can expose partial
state, documentation regresses to nonexistent signing/APIs, the applications
checkout's grant files lack producer-supplied rights metadata, or SynDB cannot
consume the same canonical revision. Report the exact missing fixture, secret-free setup,
producer, regeneration workflow, and validation command.

Missing GenAI provenance blocks use of this planning work in an application,
not the engineering release. A missing supported Fediversity contract blocks
only the adapter/conformance claim; it must not cause an internal interface to
be invented or make the host-to-host tabular resource-portability release
dependent on mutable upstream `main`.

## Non-Blocking Fediversity Discovery Track

This track is outside Gates 04A-04C and cannot delay the MVP. At official
Fediversity commit `0e4ab02db40b188898531ad36b5eb03c6e46a431`, PostgreSQL
supplies `urlFile` plus `sslMode`, while S3 supplies endpoint/port,
bucket/region, `accessKeyIDFile`, and `secretAccessKeyFile`. QueryFabric already
accepts the database URL file but currently combines S3 credentials.

After Fediversity maintainers identify a supported external application
boundary, QueryFabric may pin its immutable tag/revision, prepare separate S3
access-key and secret-key file options (retaining the combined file as a
compatibility shim), and build a thin Nix adapter that preserves TLS policy,
combines endpoint/port, keeps secrets out of the store, and exercises
PostgreSQL plus Garage. Do not copy the Fediversity contract system or put the
adapter in the stable Rust facade.

Until that upstream artifact exists, the only honest grant deliverable is
contract discovery/prototype evidence. There is no Phase 04 acceptance item
and no Fediversity compatibility, adoption, conformance, or endorsement claim.
