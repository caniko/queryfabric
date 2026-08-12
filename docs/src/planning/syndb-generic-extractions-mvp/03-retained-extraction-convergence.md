# Phase 03: Retained Extraction Convergence

## Goal

Finish adoption of the generic work worth keeping, remove its proven
duplicates, and leave domain-bound SynDB behavior in SynDB.

All SynDB paths below are relative to
`/data/can/canix/projects/repos/owned/github.com/memorycircuits/SynDB`.

This is a non-grant product prerequisite by default. Branch reconciliation,
duplicate removal, and consumer cutover are maintenance/adoption work, not the
public portability R&D milestone.

## Workstream A: One ClickHouse DynamicClient

### Preserve behavior first

Compare QueryFabric
`crates/queryfabric-adapter-clickhouse/src/driver.rs` with SynDB
`crates/core/syndb-clickhouse/src/dynamic.rs`. Port the original SynDB tests
and any missing neutral semantics before changing consumers:

- primary failure followed by fallback success;
- retry classification by transport versus HTTP/server error;
- execute paths that must not retry;
- no retry after a streaming response begins;
- endpoint/config validation; and
- error context sufficient for SynDB to map into domain errors.

Keep the QueryFabric error type neutral. SynDB may retain a thin conversion
adapter, but not a second HTTP client implementation.

Treat the client as an adapter-level host integration, preferably behind an
explicit feature. SynDB imports it from the adapter crate; the stable
`queryfabric` compiler facade does not re-export it or imply that QueryFabric
owns host execution.

### Make identifiers safe

Adopt Phase 02's typed qualified ClickHouse identifier for every DynamicClient
table operation. Do not document raw `database.table` strings as caller-trusted
input.

### Cut consumers over

Switch every SynDB DynamicClient consumer to
`queryfabric_adapter_clickhouse::DynamicClient`, run focused integration tests,
then delete the local implementation and duplicate neutral error variants.

Do not extract the whole `ChQuery` builder. It remains coupled to
`SyndbTable`, `TABLE_COLUMNS`, `FederatedCluster`, dataset filters, and SynDB
error policy. Prefer QueryFabric's compiler; extract a smaller renderer only if
a second host demonstrates the need.

## Workstream B: Canonical web utilities

Using the Phase 00 reviewed port of `36a327f`/`1ba7f34`:

- test `Flash` serialization/lifecycle;
- test `next_query_value` and `append_query` with existing, empty, repeated, and
  encoded query values;
- test `safe_local_redirect` against absolute URLs, scheme-relative URLs,
  backslashes, encoded separators, fragments, and valid local paths; and
- switch SynDB UI/SSR consumers from the vendored topic behavior to canonical
  QueryFabric.

The route table and `RouteDecision` stay in SynDB unless a second host requires
the same policy.

## Workstream C: Small extraction cleanup

Review, one at a time:

- update SynDB's experimental skeleton imports from the removed
  `queryfabric-runtime` Flight feature to registry-unpublished
  `queryfabric-flight`, without changing Phase 01's production service
  selection;
- remaining private `spawn_traced` copies in QueryFabric adapters/cluster/job
  queue/test rig;
- the `_syndb_arrow_safe` alias emitted by QueryFabric's ClickHouse Arrow
  helper, replacing it with a neutral collision-resistant alias;
- the SynDB `build_query_parameters` Arrow wrapper;
- dead SynDB source files left behind a runtime-k8s re-export; and
- duplicate local copies of already identical neutral crates.

Consolidate only where behavior and error types remain clear. A thin domain
error adapter is acceptable; a complete duplicate implementation is not.

Do not fold `queryfabric-types`, the SeaORM active-enum macro, changelog
fetching, or generic test-service stacks into this phase. They require the
separate disposition in Phase 07.

## Deliverables

- one tested, safely addressed `DynamicClient`;
- canonical and adopted generic web utilities;
- removal of proven dead/duplicate implementation files;
- explicit retained SynDB adapters; and
- an updated extraction matrix marking each old-plan item adopted, deferred,
  or rejected.

## Acceptance

- [ ] DynamicClient behavior tests pass upstream:

  ```bash
  nix develop -c cargo test -p queryfabric-adapter-clickhouse --all-features --locked
  ```

- [ ] No SynDB implementation duplicate remains:

  ```bash
  rg -n 'struct DynamicClient|impl DynamicClient' crates
  ```

  Expected matches are imports/re-exports or a documented domain adapter, not a
  second client.

- [ ] QueryFabric web and relevant SynDB consumers pass focused tests.
- [ ] Safe-redirect adversarial cases cannot leave the local origin.
- [ ] QueryFabric implementation code passes the domain-neutrality audit:

  ```bash
  rg -n 'syndb|SyndbTable|neurometa|GraphTrainingSet|SYNDB_' \
    /data/can/canix/projects/repos/owned/codeberg.org/caniko/queryfabric/crates
  ```

  Every remaining match is documented as migration compatibility, test data,
  or a defect assigned to a later phase.

- [ ] `cargo tree -d` and source search show which duplicates are dependency
      versions versus duplicated local behavior.
- [ ] SynDB focused ClickHouse and UI tests pass against the same canonical
      QueryFabric revision selected in Phase 00.

## Non-Goals

- extracting host query construction or database schemas;
- migrating SynDB Flight;
- expanding utility crates without a consumer;
- deleting domain-specific error mapping; or
- declaring copied but unused crates complete.

## Stop Conditions

If SynDB's local DynamicClient has a behavior that cannot be represented
without a SynDB domain type, keep that behavior in a thin injected policy or
adapter and document it. If original tests depend on unavailable live services
or fixtures, report their producer/setup/proof contract before deleting the
local implementation.
