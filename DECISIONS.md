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
- **Cluster health** — `ClusterProbe::probe_node(NodeId) -> ProbeStatus`,
  implemented by `queryfabric-cluster` in Phase 03.

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
