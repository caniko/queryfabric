# Self-Hosting and Data Sovereignty

Self-hosted services usually fail in the same place: the data layer. Once
service policy, host-specific schema design, and migration logic are fused
together, "moving instances" turns into a lossy export, a manual rewrite, or
both. QueryFabric exists to keep that boundary explicit.

The project does that by owning the portability and sovereignty primitives at
the query layer while leaving the actual service runtime with the host.

## What QueryFabric owns

- [`queryfabric-access`](https://codeberg.org/caniko/queryfabric/src/branch/trunk/crates/queryfabric-access): GDPR-aligned
  access, rectification, and erasure traits over generic `ResourceRef`
  values.
- [`queryfabric-portability`](https://codeberg.org/caniko/queryfabric/src/branch/trunk/crates/queryfabric-portability):
  content-addressed export bundles, provenance records, citation metadata, and
  DOI minting.
- [`queryfabric-tenancy`](https://codeberg.org/caniko/queryfabric/src/branch/trunk/crates/queryfabric-tenancy): the generic
  account, collection, and group model that keeps multi-tenant ownership
  separate from the compiler core.
- [`queryfabric-federation`](https://codeberg.org/caniko/queryfabric/src/branch/trunk/crates/queryfabric-federation) and
  [`queryfabric-cluster`](https://codeberg.org/caniko/queryfabric/src/branch/trunk/crates/queryfabric-cluster): the
  federation substrate, resource locality, routing, and stable cluster
  messaging used to connect nodes.
- [`nix/modules/queryfabric.nix`](https://codeberg.org/caniko/queryfabric/src/branch/trunk/nix/modules/queryfabric.nix): the
  hardened NixOS module that packages the demonstrator service, keeps secrets
  out of the store, and wires the runtime with systemd credentials.

## What stays with the host

Decision [D003](https://codeberg.org/caniko/queryfabric/src/branch/trunk/DECISIONS.md#d003-keep-host-execution-outside-queryfabric)
keeps execution outside QueryFabric: the host still runs the SQL, owns
authentication, and controls orchestration. That separation is not a gap in the
design; it is the feature that keeps the core small, reviewable, and portable
across different deployment environments.

The same boundary also keeps the operational footprint clear. QueryFabric can
describe what a query needs, how a bundle is represented, or how a federation
message moves across nodes, but the host decides where execution happens and
how its own policy is enforced.

## Deployment path

For a concrete single-host setup, see
[Self-hosting on NixOS](../deployment/self-hosting-nixos.md). That chapter
shows how the module, database, object store, secrets handling, and VM test fit
together in practice.
