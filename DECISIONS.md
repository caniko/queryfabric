# Decisions

## D001: Use `sqlparser` AST as the initial neutral query tree

The first extraction cut reuses `sqlparser`'s AST instead of creating a second
custom SQL AST. This keeps the new surface neutral and reduces migration risk.

## D002: Keep execution hints outside relational semantics

`SCOPE`, `DOWNLOAD`, tracing IDs, and similar host directives are represented as
`ExecutionHints`, not as relational operators.

## D003: Keep host execution in SynDB

QueryFabric emits SQL or a DataFusion `LogicalPlan`. It does not execute queries,
manage auth, or own job orchestration.

## D004: Domain types enter only via trait impls in the host

`queryfabric-contract` is the single seam between QueryFabric and its host
application. No crate in this workspace names a host domain concept; the host
injects domain knowledge exclusively by implementing the contract traits.

The four contract traits (plus their carrier types):

- **Provenance** — `DomainActivity` extends the core `Activity` enum
  (`Created`, `Deleted`, `Accessed`, `Modified`, `OwnershipTransferred`,
  `ContentHashRecorded`, `FederationFlow`, `BackupAnchor`; `#[non_exhaustive]`)
  with host-specific activities serialized opaquely.
- **Access control** — `AccessDecision::evaluate(&Subject, &AccessPolicy) ->
  AccessOutcome`. `AccessPolicy` is `Open | Registered | Restricted`
  (`#[non_exhaustive]`); GA4GH data-use restrictions ride along as opaque
  strings.
- **Cost statistics** — `StatisticsSource::stats_for(ResourceRef) ->
  Option<RelationStats>` lets the host inject live row/byte estimates into the
  catalog cost model (wired in Phase 04 via
  `queryfabric_catalog::relation_statistics_from_source`).
- **Cluster health** — `ClusterProbe<C, H>::probe(node, handle) ->
  ProbeResult<Output>` over the wire-stable `Health` vocabulary
  (`healthy | degraded | unreachable | unknown`), plus an
  `on_successful_sweep` hook. Driven by the `HealthMonitorActor` in
  `queryfabric-cluster` (Phase 03); the host supplies the probe impl.

Identity is carried by two distinct UUID newtypes — `ResourceRef { namespace,
id }` for queryable resources and `NodeId` for federation nodes — so the two
kinds of identifier cannot be mixed.

Crates scaffolded for the extraction (bodies land in the noted phases):

- Phase 02 (utility): `queryfabric-paseto`, `queryfabric-session`,
  `queryfabric-problem-details`, `queryfabric-namespace-uuid`,
  `queryfabric-flight-pool`, `queryfabric-flight-cache`,
  `queryfabric-tcp-tuned`, `queryfabric-job-queue`, `queryfabric-prom`,
  `queryfabric-content-hash`, `queryfabric-fetch`, `queryfabric-cli-toolbelt`,
  `queryfabric-cmd-runner`, `queryfabric-test-rig`.
- Phase 03 (federation): `queryfabric-cluster`, `queryfabric-federation`.
- Phase 05 (sovereignty): `queryfabric-provenance`, `queryfabric-access`,
  `queryfabric-portability`, `queryfabric-tenancy`, `queryfabric-store`.

## D005: Utility crates deliberately left in SynDB

Phase 02 moved the domain-neutral utility crates out of SynDB's
`crates/owned/` but deliberately skipped `meta-stats`, `iso-continent`, and
`latency-stats` (too niche or domain-flavoured to justify a neutral home).
`prov-activity` and `datacite-types` were also not moved here: they feed the
sovereignty layer and are generalised — not relocated — in Phase 05.
SynDB's `priority-job-runner` was folded into `queryfabric-job-queue` as the
`priority` module rather than becoming its own crate (it is a thin,
single-consumer companion of the job queue).

## D006: The demonstrator is a host, not a library feature

Phase 06's `queryfabric-demo` exercises the extraction end-to-end by playing
the *host* role the contract assigns (D003/D004): it executes the SQL the
Postgres adapter emits, owns its catalog and identity data, and supplies the
clock for provenance timestamps. Three demonstrator-level choices follow:

- **Query execution lives in the demo binary** (per-request
  `tokio-postgres` connections), not in any queryfabric crate.
- **DOI minting is offline by default**: `LocalDoiProvider` fabricates
  records under the DataCite test prefix (`10.5072`) so the demonstrator
  needs no registrar account; the real `DataCiteProvider` remains the
  production path.
- **`queryfabric-store` gained the typed `S3Config` constructor** so hosts
  configure S3-compatible backends without depending on OpenDAL directly —
  the seam the NixOS module's `store.*` options map onto.
