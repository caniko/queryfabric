# High Availability

This chapter separates **today's behavior in the repository** from
**planned WP2 work** for the NGI Fediversity application. Every section uses
that same boundary on purpose: QueryFabric already has useful HA substrate,
but it does **not** yet deliver a complete fault-tolerant federated service.

## Scope and current boundary

Today there are two distinct pieces to reason about:

1. The **demo service** (`crates/queryfabric-demo`), which is the runnable
   self-hosted application documented in
   [Self-hosting on NixOS](./self-hosting-nixos.md).
2. The **federation substrate** (`crates/queryfabric-cluster`,
   `crates/queryfabric-federation`), which provides health probing, resource
   locality, schema sync, and a hub/node actor protocol.

Those pieces are related, but they are not yet wired into one end-to-end HA
product. In particular, the current demo exposes federation identity facts at
`GET /federation/status`, but it does not yet instantiate the hub or cluster
actors shown in the federation crates
(`crates/queryfabric-demo/src/http.rs`,
`crates/queryfabric-federation/src/hub_actor.rs`,
`crates/queryfabric-federation/src/node_actor.rs`).

## Deployment topologies

### Topology 1: single demo node

**Today**

This is the status quo supported by the current NixOS chapter and module:
one `queryfabric-demo` process, one Postgres database, and either an S3
backend or the non-durable in-memory object store
(`docs/src/deployment/self-hosting-nixos.md`,
`nix/modules/queryfabric.nix`,
`crates/queryfabric-demo/src/main.rs`,
`crates/queryfabric-demo/src/config.rs`).

What survives a demo-process restart:

- Postgres-backed dataset state survives because the demo reconnects per
  operation and seeds idempotently with `CREATE TABLE IF NOT EXISTS` and
  `ON CONFLICT DO NOTHING`
  (`crates/queryfabric-demo/src/db.rs`).
- S3-backed export bundles survive because they live outside the process
  (`crates/queryfabric-demo/src/main.rs`,
  `crates/queryfabric-demo/src/sovereignty.rs`).
- systemd restarts the service on failure with `Restart=on-failure` and
  `RestartSec=2` (`nix/modules/queryfabric.nix`).

What does not survive a demo-process restart:

- Provenance history is currently held in an in-process
  `VecProvenanceStore` (`crates/queryfabric-demo/src/main.rs`,
  `crates/queryfabric-demo/src/http.rs`).
- Resource ownership is currently held in an in-process
  `InMemoryOwnership` map (`crates/queryfabric-demo/src/main.rs`,
  `crates/queryfabric-demo/src/http.rs`).

So the current demo is **restartable**, but not fully stateless.

### Topology 2: multiple demo instances behind a host load balancer

**Today**

This topology is only partially safe today. The demo has no HTTP session
state, and its queryable dataset lives in shared Postgres while export
artifacts can live in shared S3
(`crates/queryfabric-demo/src/http.rs`,
`crates/queryfabric-demo/src/db.rs`,
`crates/queryfabric-demo/src/main.rs`).

However, the demo is **not** fully stateless:

- provenance is node-local (`VecProvenanceStore`);
- ownership is node-local (`InMemoryOwnership`);
- the default object store can be non-durable `memory`, which is explicitly
  warned against for production (`crates/queryfabric-demo/src/main.rs`).

The practical consequence is that active-active replicas can front the demo's
read/query surface, but an operator should not promise durable,
cross-replica-consistent provenance or audit behavior yet. This is why the
multi-instance deployment work is tracked separately in the Phase 07 plan,
not claimed here as already delivered.

### Topology 3: federated multi-node deployment with one hub

**Today**

The federation substrate supports a single hub actor plus multiple cluster
node actors. Registration, resource announcements, health probing, endpoint
discovery, and schema-sync broadcasts all exist today
(`crates/queryfabric-federation/src/lib.rs`,
`crates/queryfabric-federation/src/hub_actor.rs`,
`crates/queryfabric-federation/src/node_actor.rs`,
`crates/queryfabric-federation/src/transport.rs`,
`crates/queryfabric-federation/tests/federation_two_node.rs`).

What survives normal node failure:

- a failing cluster is marked `Unreachable` by the health monitor, and
  delegatable routing stops using it
  (`crates/queryfabric-cluster/src/health_monitor.rs`,
  `crates/queryfabric-cluster/src/health.rs`,
  `crates/queryfabric-cluster/src/routing.rs`).
- the hub can continue routing other healthy clusters as long as the hub
  process itself stays up.

What does not survive hub failure:

- the hub registry is in-memory shared state (`HubRegistryState`) backed by
  `papaya` maps, not persisted storage
  (`crates/queryfabric-cluster/src/registry.rs`).
- resource locality announcements are therefore lost with the hub process
  unless hosts add persistence outside the current skeleton.

So today's federation story is **single-hub, best-effort resilience**, not
hub failover.

## What works today

### Health monitoring and degraded routing

**Today**

The generic `HealthMonitorActor` periodically probes every registered
cluster, caches the last health result, and applies a simple circuit breaker:
three consecutive failures open the circuit for a 30-second cooldown by
default (`crates/queryfabric-cluster/src/health_monitor.rs`).

The health result matters operationally:

- `Healthy` and `Degraded` are delegatable;
- `Unknown` and `Unreachable` are not
  (`crates/queryfabric-cluster/src/health.rs`).

The transport-backed probe also refreshes the cached Flight endpoint after a
successful health ping (`crates/queryfabric-federation/src/transport.rs`).

### Resource registry and locality index

**Today**

The hub maintains:

- a `cluster_refs` map keyed by cluster id;
- a `resource_index` map keyed by resource id
  (`crates/queryfabric-cluster/src/registry.rs`).

Cluster registration inserts or replaces a cluster handle; resource
announcements insert, replace, or remove locality entries
(`crates/queryfabric-federation/src/hub_actor.rs`).

Routing then groups requested resources by remote cluster and treats unknown
resources as local (`crates/queryfabric-cluster/src/routing.rs`).

The important limitation is that this is an **in-memory last-writer-wins
index**. There is no persistent event log, lease, epoch, or conflict
resolver in the current implementation.

### Restart behavior under systemd

**Today**

The current deployment module gives the demo service a reasonable single-node
restart posture: `Restart=on-failure`, a short restart delay, and secrets
re-injected through `LoadCredential` on each start
(`nix/modules/queryfabric.nix`).

That helps with process crashes, but it does not turn process-local demo
state into durable shared state. Operators should read the restart policy as
"the service will come back quickly," not "all semantics are preserved across
replicas."

## Single points of failure today

### The hub is a current single point of failure

**Today**

The current federation design has one hub actor owning registration and
resource-announcement handling, and its registry is backed by in-memory maps
(`crates/queryfabric-federation/src/hub_actor.rs`,
`crates/queryfabric-cluster/src/registry.rs`).

That means the hub is a real current single point of failure:

- if the hub process dies, new `RegisterCluster` calls fail;
- previously announced resource locality is lost with that process;
- schema-sync broadcast coordination is lost with that process.

Nothing in the current code disproves that; the current code confirms it.

### The demo still has process-local state

**Today**

The demo keeps provenance and ownership in memory
(`crates/queryfabric-demo/src/main.rs`,
`crates/queryfabric-demo/src/http.rs`), so multi-instance deployment cannot
yet promise consistent audit history or shared authorization metadata.

### NAT traversal is not implemented

**Today**

The swarm bootstrap code accepts explicit listen addresses, optional
bootstrap multiaddrs, and optional mDNS for local discovery
(`crates/queryfabric-cluster/src/swarm.rs`), but this repository does not
yet implement a NAT traversal story for the federation substrate. Operators
should therefore assume reachable, operator-managed addresses today.

## Federation message failure analysis

The current wire-visible federation message set is documented in
`crates/queryfabric-federation/src/lib.rs`. Each message behaves as follows
today.

### `RegisterCluster`

**Today**

The hub validates registration through the host's `register_cluster` hook,
then inserts a cluster handle with no Flight endpoint yet and returns schema
facts plus an assigned `cluster_id`
(`crates/queryfabric-federation/src/hub_actor.rs`,
`crates/queryfabric-federation/src/host.rs`).

Failure behavior today:

- if the hub is down, registration cannot complete;
- if the host rejects the password or identity, registration fails cleanly;
- if the hub later restarts, the in-memory registration state is gone and
  clusters must re-register.

### `HealthPing`

**Today**

The node answers `HealthPing` from its own `storage_ok`, `resource_count`,
schema version, uptime, and host revision
(`crates/queryfabric-federation/src/node_actor.rs`).

Failure behavior today:

- a timed-out or failed probe becomes `Unreachable`;
- after repeated failures the circuit breaker suppresses further probes until
  cooldown expires;
- an unreachable node stops being eligible for delegated routing
  (`crates/queryfabric-cluster/src/health_monitor.rs`,
  `crates/queryfabric-cluster/src/health.rs`).

### `SchemaSync`

**Today**

The hub can broadcast a `SchemaSync` to every registered cluster via the
local `SyncAllSchemas` admin message
(`crates/queryfabric-federation/src/hub_actor.rs`).

The node-side application logic is intentionally simple:

- migrations with versions at or below the current version are skipped;
- only `CREATE` and `ALTER` DDL bodies are accepted;
- pending migrations run in the order supplied;
- execution stops at the first rejection or failure
  (`crates/queryfabric-federation/src/schema.rs`).

Failure behavior today:

- there is no conflict detection for two different migrations carrying the
  same version;
- there is no protection against an out-of-order migration list beyond the
  numeric `version <= applied_version` check;
- there is no hub failover mechanism coordinating competing schema-sync
  senders.

### `ResourceAnnouncement`

**Today**

`Added` and `Updated` both call `upsert_resource`; `Removed` deletes the
resource entry; host side effects run only as a best-effort hook
(`crates/queryfabric-federation/src/hub_actor.rs`).

Failure behavior today:

- if the hub is unavailable, the announcement is lost unless the host retries;
- if two nodes announce the same `resource_id`, the later processed
  announcement overwrites the earlier one;
- there is no merge, quorum, lease, or tombstone ordering mechanism in the
  current registry.

### `CatalogRequest`

**Today**

The node can answer a `CatalogRequest` by returning `host.catalog(since)`
with an `as_of` timestamp
(`crates/queryfabric-federation/src/node_actor.rs`).

Failure behavior today:

- transport failure is surfaced to the caller as a request failure;
- the current hub skeleton does not itself issue `CatalogRequest`, so there
  is no built-in retry, reconciliation, or persistence path in the current
  repository.

### `GetFlightEndpoint`

**Today**

The node returns its configured Flight endpoint and TLS bit
(`crates/queryfabric-federation/src/node_actor.rs`).

Failure behavior today:

- the transport-backed health probe treats endpoint refresh as opportunistic;
  a successful `HealthPing` can still leave the cached endpoint unchanged if
  `GetFlightEndpoint` fails
  (`crates/queryfabric-federation/src/transport.rs`);
- a healthy cluster without a cached endpoint still cannot be used for
  delegated Flight routing.

## Planned WP2 work

Everything in this section is **planned, not implemented**, and is explicitly
the subject of the NGI Fediversity WP2 grant application.

### Planned: hub failover

Two options are plausible from the current code structure:

1. **Standby hub plus re-registration.**
2. **DHT-based hub election with one active writer.**

The leading candidate is **standby hub plus re-registration**.

Rationale:

- today's protocol already has explicit `RegisterCluster` and
  `ResourceAnnouncement` flows that can be replayed after hub loss;
- today's registry is in-memory and last-writer-wins, so adding a warm
  standby plus deterministic replay is a smaller change than introducing a
  consensus-style election protocol;
- this keeps the protocol operationally simple for NixOS-first deployments.

Planned WP2 work would therefore persist enough registration and locality
state for a standby hub to take over, then require clusters to re-register
and re-announce on failover.

### Planned: NAT traversal

Planned, subject of an NGI Fediversity grant application.

The current code assumes operator-managed reachability. WP2 would add a real
story for nodes behind NAT, most likely by combining relay-assisted peer
discovery with explicit fallback behavior when direct reachability cannot be
established. The exact transport mechanism is still design work, not a
current feature claim.

### Planned: schema-sync conflict handling

Planned, subject of an NGI Fediversity grant application.

The current `SchemaSync` path lacks conflict detection. WP2 should add:

- a durable schema plan identifier or digest per migration set;
- rejection of divergent migrations reusing the same version number;
- replay-safe ordering rules across hub restart or failover.

That is intentionally scoped as a design target. The repository today only
enforces "ordered list, DDL-only, stop on first failure."

## Operator guidance

### Safe promises today

- A single-node NixOS deployment is the supported operational baseline.
- Process crashes on that node are handled reasonably by systemd restart
  policy.
- Shared Postgres and S3 can externalize the demo's main data path and export
  artifacts.
- The federation substrate can monitor cluster health and avoid delegating to
  unreachable nodes when run with one hub and reachable nodes.

### Promises not yet safe

- Do not promise hub failover.
- Do not promise NAT-transparent federation across arbitrary networks.
- Do not promise conflict-free schema synchronization across independent hub
  writers.
- Do not describe the current demo as fully stateless or fully active-active:
  provenance and ownership are still process-local.

For deployment wiring, use the current NixOS chapter
([Self-hosting on NixOS](./self-hosting-nixos.md)). For sizing guidance, use
the separate resource-footprint chapter if it is present in your checkout;
this HA chapter does not depend on it.
