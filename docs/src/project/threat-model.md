# Threat Model

This chapter documents the current QueryFabric attack surface. It is limited to
the compiler, artifact generation, and federation protocol. Per Decision D003
in `DECISIONS.md`, query execution, authentication, authorization, and network
policy stay with the host rather than QueryFabric core.

## System Context and Trust Boundaries

QueryFabric takes untrusted query text and emits parsed queries, bound queries,
backend analyses, SQL artifacts, and portable bundles. The main boundaries are:

- **Query text is untrusted input.** The SQL front end in
  `crates/queryfabric-dialect-sql/src/` must handle malformed and adversarial
  input without panicking.
- **Catalog state is semi-trusted input.** The binder and emitter trust the
  host to provide a coherent catalog, function registry, and relation names via
  `queryfabric_catalog`, but those inputs still influence capability
  classification, result schemas, and emitted backend SQL.
- **Federation peers are untrusted until registration succeeds.** The protocol
  surface in `crates/queryfabric-federation/src/` accepts remote registration
  attempts and subsequent control-plane messages, but validation is delegated
  to `FederationHost::register_cluster` in
  `crates/queryfabric-federation/src/host.rs`.
- **Host execution stays out of scope.** D003 states that QueryFabric emits SQL
  or plans but does not execute queries, manage auth, or own orchestration.
  Host-owned concerns therefore include runtime authn/authz, database
  permissions, TLS termination, and network isolation.

The reportable bug classes listed in `SECURITY.md` sit on these boundaries:

| Reportable class | Boundary |
| --- | --- |
| SQL or artifact generation bugs | QueryFabric-owned emitter and portability surfaces |
| placeholder handling and parameter propagation | QueryFabric-owned parse/bind/emit path |
| incorrect capability classification or unsafe backend emission | QueryFabric-owned analysis/emission, with host impact because the host may trust the classification |
| provenance or schema metadata mismatches | QueryFabric-owned receipts, result schemas, and bundle manifests |

## Assets

The assets worth protecting are:

- **Emission correctness.** Placeholders must stay placeholders, and emitted
  SQL must not splice attacker-controlled values into SQL text.
- **Capability classification soundness.** A misclassified query can become a
  host-level authorization-bypass primitive if the host trusts QueryFabric's
  portable-subset or backend-support decision.
- **Provenance integrity.** Query hashes, catalog snapshot IDs, backend names,
  and artifact identities in `crates/queryfabric-ir/src/diagnostics.rs` must
  describe what was actually analyzed and emitted.
- **Bundle integrity.** Portable bundles and artifact manifests in
  `crates/queryfabric-portability/src/` rely on deterministic BLAKE3 content
  hashes.
- **Federation registry honesty.** Hub and node actors must resist forged
  registrations, unsafe schema pushes, and misleading locality state.

## Threats by Surface

### Parser and lowering

The parser surface is `crates/queryfabric-dialect-sql/src/lib.rs`,
`lower.rs`, `lower/query.rs`, and `lower/expr.rs`. Threats here are malformed
SQL causing panics, excessive CPU work, or silent semantic degradation.

`Lowerer::emit_unsupported` in
`crates/queryfabric-dialect-sql/src/lower.rs` records structured diagnostics
instead of silently accepting unsupported constructs, and the binder later
fails if error diagnostics remain. The parser surface is also fuzzed by
`fuzz/fuzz_targets/parse_sql_no_panic.rs`.

The remaining gap is resource exhaustion: the tree contains panic-oriented
fuzzing, but there are no explicit input-size or complexity limits in the
parser crates today.

### Binder, placeholders, and capability classification

The binder surface is primarily `crates/queryfabric-catalog/src/bind/`.
Threats here include catalog spoofing, unsafe placeholder propagation, and
incorrect capability classification.

Placeholder handling is typed rather than string-based. SQL placeholders are
normalized into `ParameterRef` values in
`crates/queryfabric-dialect-sql/src/helpers.rs`, constrained during binding in
`crates/queryfabric-catalog/src/bind/params.rs`, and preserved as
`ParameterBinding` / `ParameterSchema` values in
`crates/queryfabric-ir/src/types.rs`. The binder rejects unresolved parameter
types and incompatible values instead of guessing.

Capability classification is derived from the bound plan in
`crates/queryfabric-catalog/src/bind/query.rs` and recorded in provenance in
`crates/queryfabric-catalog/src/bind/mod.rs`. That is QueryFabric-owned logic,
but the impact often lands in the host: if a host uses the capability decision
for routing or authorization, a misclassification becomes an authz-bypass
primitive.

The main gap is that catalog names and function mappings are still trust
inputs. QueryFabric validates structure, but it does not independently prove
that host-supplied relation names or backend mappings are safe or honest.

### Backend emission and artifact generation

The emission surface is `crates/queryfabric-catalog/src/render/emit.rs` and
`render/helpers.rs`. Threats here are dialect-specific SQL injection, unsafe
identifier rendering, incorrect placeholder ordering, and provenance or schema
metadata mismatches.

The main mitigation is that parameter values stay out of SQL text. Emission
computes an ordered parameter schema with `ordered_parameters(...)`, assigns
positions in `SqlRenderer`, and renders placeholders instead of concatenating
concrete values into the query text. Provenance is attached via
`.with_backend(...)` and `.with_artifact_identity(...)` in
`crates/queryfabric-catalog/src/render/emit.rs`.

Relation names, aliases, columns, CTEs, mapped function paths, ClickHouse
table targets, and timestamp type arguments now pass through validation and
segment-aware rendering. String values remain parameterized or literal-escaped
by type. The remaining work is a wider adversarial/property matrix and review
of any future adapter token additions; this is no longer an accepted raw
identifier interpolation path.

### Import bundles and host apply

The reference host treats bundle JSON and tabular artifact bytes as untrusted.
Threats include duplicate JSON keys, oversized or malformed CSV, digest or
schema confusion, staged-object replacement, stale target revisions, replay
with changed mappings, and partial visibility during a failed transaction.

The host validates bounded bundle/profile facts before mutation, stages bytes
under their artifact digest, requires the dry-run `planDigest` and
`stagedObject` on apply, rechecks the staged bytes, and persists rows and the
receipt in one PostgreSQL transaction. Identical replay returns the original
receipt; a changed plan conflicts. Import, export, erase, DOI, and access
export routes require a verified PASETO bearer subject with the appropriate
role. Content hashes provide integrity against a trusted expected digest; they
are not signatures.

The remaining host gap is external identity-provider/session integration and a
complete injected-failure/staging-cleanup matrix in the VM proof.

### Federation control plane

The federation surface is `crates/queryfabric-federation/src/`. The six remote
message types are `RegisterCluster`, `HealthPing`, `SchemaSync`,
`ResourceAnnouncement`, `CatalogRequest`, and `GetFlightEndpoint`, documented
in `crates/queryfabric-federation/src/lib.rs` and defined across
`messages.rs`, `schema.rs`, `hub_actor.rs`, and `node_actor.rs`.

Threats here include:

- forged or replayed registration attempts against `RegisterCluster`
- malicious `SchemaSync` requests that try to push arbitrary SQL
- dishonest or spammy `ResourceAnnouncement` traffic that pollutes locality
  state
- misleading `CatalogRequest` responses or `GetFlightEndpoint` replies from a
  compromised peer
- resource-exhaustion via repeated pings, announcements, or catalog syncs

Existing mitigations are partial but real. Registration validation is pushed to
`FederationHost::register_cluster(...)`, so peers are not trusted before host
approval. `SchemaSync` is guarded by `ddl_allowed(...)` in
`crates/queryfabric-federation/src/schema.rs`, which accepts only `CREATE` and
`ALTER` migrations before calling the host DDL hook. `ClusterNodeActor` only
handles `HealthPing`, `SchemaSync`, `CatalogRequest`, and
`GetFlightEndpoint`.

The largest gap is authentication hardening after registration. The protocol
returns an API key in `RegisterClusterReply`, but the current crate surface does
not show complete end-to-end verification for every subsequent message, nor any
built-in rate limiting or anti-spam controls.

### Tokens, session helpers, and deployment

The auth-helper surface is `crates/queryfabric-paseto/` and
`crates/queryfabric-session/`. Threats here are short secrets, malformed
tokens, wrong-scope delegation tokens, and cookie flag mistakes.

Existing mitigations include minimum secret-length validation and typed token
validation in `crates/queryfabric-paseto/src/lib.rs` and `typed.rs`, plus
short-lived delegation tokens (`DELEGATION_TTL_SECS = 30`) in `typed.rs`.
Session cookies always include `HttpOnly`, `Path=/`, `SameSite`, and optional
`Secure` in `crates/queryfabric-session/src/lib.rs`.

Deployment hardening exists in `nix/modules/queryfabric.nix`: secrets arrive
through `LoadCredential`, the service runs with `DynamicUser`, `NoNewPrivileges`,
`ProtectSystem=strict`, restricted address families, and a reduced system-call
surface.

### Supply chain and release inputs

QueryFabric also has a supply-chain surface through its Rust dependencies and
Nix inputs. Current mitigations are reproducibility and CI rather than an
in-repo audit policy: the workspace is lockfile-based (`Cargo.lock`,
`flake.lock`), the build/test/fuzz gates are defined in
`.forgejo/workflows/ci.yml`, and the Nix docs/build checks live in `flake.nix`.

## Existing Mitigations Summary

Today the repo already provides:

- panic-focused parser and binder fuzz targets:
  `fuzz/fuzz_targets/parse_sql_no_panic.rs`,
  `fuzz/fuzz_targets/bind_portable_no_panic.rs`, and the CI job in
  `.forgejo/workflows/ci.yml`
- structured diagnostics for unsupported syntax and binding failures:
  `crates/queryfabric-dialect-sql/src/lower.rs`,
  `crates/queryfabric-catalog/src/bind/mod.rs`,
  `crates/queryfabric-catalog/src/render/analysis.rs`
- typed placeholder propagation and schema-checked parameter binding:
  `crates/queryfabric-dialect-sql/src/helpers.rs`,
  `crates/queryfabric-catalog/src/bind/params.rs`,
  `crates/queryfabric-ir/src/types.rs`
- provenance receipts and content-addressed artifacts:
  `crates/queryfabric-ir/src/diagnostics.rs`,
  `crates/queryfabric-catalog/src/render/emit.rs`,
  `crates/queryfabric-portability/src/bundle.rs`,
  `crates/queryfabric-content-hash/src/lib.rs`
- federation registration and DDL narrowing:
  `crates/queryfabric-federation/src/host.rs`,
  `crates/queryfabric-federation/src/schema.rs`
- systemd and secret-handling hardening for the demonstrator:
  `nix/modules/queryfabric.nix`

## Known Gaps and Planned Work

The main gaps are straightforward:

- no explicit parser or federation message size/rate limits
- broader adversarial/property coverage for the identifier/token helpers
- incomplete visible authn story for post-registration federation traffic
- no documented dependency-audit process beyond lockfiles and CI
- no external security review yet

These are WP4 items rather than current guarantees. The engineering work is
tracked in the active MVP plan under
`docs/src/planning/syndb-generic-extractions-mvp/`; application templates and
grant context live in the separate applications checkout. The right next
steps are external audit follow-up, federation hardening, and
threat-model-driven hardening.
