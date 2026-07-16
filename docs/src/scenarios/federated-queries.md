# Scenario: Federated Queries

Federated query execution is not an end-to-end demonstrator capability yet.
This page records the current boundary so that deployment configuration is not
mistaken for a working scatter/gather service.

## What the repository has today

The `queryfabric-cluster` and `queryfabric-federation` crates contain reusable
federation substrate: hub and node actors, registration messages, resource
announcements, health probing, endpoint discovery, and schema-sync messages.
Their behavior is exercised by crate-level tests.

The runnable `queryfabric-demo` service does **not** instantiate those hub or
node actors. Enabling its federation configuration only makes the configured
identity facts visible at `GET /federation/status`:

```nix
services.queryfabric = {
  enable = true;
  database.urlFile = "/run/secrets/queryfabric-db-url";
  auth.secretFile = "/run/secrets/queryfabric-auth-secret";

  federation = {
    enable = true;
    nodeName = "node-a";
    hubMultiaddrs = [ "/dns4/hub.example.org/tcp/4001" ];
    flightPort = 50051;
  };
};
```

```console
$ curl --fail http://127.0.0.1:8780/federation/status
```

The response reports whether federation identity is enabled, the configured
identity, and the configured hub multiaddresses. The demo does not connect to
those addresses, register itself with a hub, or open an Arrow Flight server as
a result of this option alone.

## What is not available today

The current demo has no:

- HTTP route for registering federation nodes;
- `SCOPE federation` query syntax;
- scatter/gather planner or aggregate decomposition exposed through the demo;
- Arrow Flight dispatch from the demo to configured peers; or
- merged multi-node result stream.

Consequently there is no supported curl or SyQL walkthrough for a federated
query. A single demo instance only executes `POST /query` against its configured
PostgreSQL backend and fixed catalog.

## Planned boundary

End-to-end multi-node execution and failure evidence remain planned work. A
future operational scenario must be backed by a runnable hub/node deployment,
public configuration contracts, and tests that exercise registration,
dispatch, partial failure, and result gathering. Until those artifacts land,
the federation crates are integration substrate rather than a deployed
federated-query product.

See [High Availability](../deployment/high-availability.md) for the detailed
inventory of current health, routing, persistence, and single-point-of-failure
behavior.
