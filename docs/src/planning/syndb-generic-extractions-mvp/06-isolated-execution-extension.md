# Phase 06: Isolated Execution Extension

## Goal

Build a self-contained QueryFabric-owned isolated execution path with one
versioned worker protocol, a real worker artifact, generic Kubernetes
configuration, and a non-ignored end-to-end test.

This is a post-MVP extension and depends on the graduated Flight contract.
It is outside the default Fediversity application scope. Scheduling,
placement, Kubernetes lifecycle, and worker distribution overlap existing
ecosystem work and need a separate integration/R&D case.

## Starting Gaps

- runtime-k8s serializes the full bound query as its Flight ticket;
- worker interprets ticket bytes as a UTF-8 provenance query hash;
- worker signals shutdown before the returned stream is consumed;
- no worker binary loads configuration or runs a Flight server;
- no concrete QueryFabric test executor, OCI image, or generic Kubernetes
  fixture exists;
- the ignored Kind smoke refers to missing SynDB charts/images; and
- runtime defaults and environment variables still contain `SYNDB_*` and
  `syndb-snapshot-*` names.

## Protocol Design

### Separate job specification from request authorization

Deliver `IsolatedJobSpec` to the worker through one immutable, size-bounded
mechanism selected by the runtime: mounted file/Secret, object-store reference,
or another explicitly tested transport. The Flight ticket must not duplicate
the full bound query.

Define a versioned ticket envelope with at least:

- schema version;
- job identity;
- provenance query hash;
- expiration;
- nonce/replay identity; and
- authenticated proof produced and verified through a generic credential
  seam.

Use one canonical serialization at both ends. Reject unknown versions,
malformed data, job/hash mismatch, expiry, replay, and invalid authentication
before starting backend execution. Keep credential material out of Kubernetes
labels, command arguments, logs, and the Nix store.

### Worker lifecycle

The worker binary:

1. loads and validates generic `QUERYFABRIC_*` configuration;
2. loads exactly one immutable job spec;
3. constructs an injected/concrete backend `QueryExecutor`;
4. binds a Flight listener and becomes ready;
5. accepts the matching authorized ticket;
6. streams every batch/result;
7. propagates cancellation and errors; and
8. signals shutdown only when the stream completes or is dropped.

A guard around the returned stream owns the one-shot shutdown signal. Returning
the stream must not shut the server down immediately.

### Runtime lifecycle

The Kubernetes driver:

1. validates generic config and resource requests;
2. writes the chosen job-spec artifact;
3. creates least-privilege Job/Pod resources with configurable
   namespace/name/labels/image;
4. waits for readiness with timeout and diagnostic events;
5. connects with the Phase 05 authenticated Flight client;
6. drains or cancels the stream;
7. cleans up job/spec/credentials according to policy; and
8. reports stable lifecycle errors.

SynDB compatibility environment names live only in a SynDB migration adapter,
not in the QueryFabric library defaults.

## Test Artifact Design

Add:

- `queryfabric-worker` binary;
- deterministic test executor returning multiple known Arrow batches;
- `oci-queryfabric-test-worker` flake package;
- QueryFabric-owned RBAC and Job fixtures, with a chart only if a real
  non-test consumer requires it;
- local/in-process protocol tests; and
- a non-ignored Kind test that uses only QueryFabric-owned artifacts.

The Kind test covers happy path, malformed/wrong/expired/replayed ticket,
multi-batch drain, cancellation, worker failure, startup timeout, and resource
cleanup.

## Deliverables

- shared versioned ticket codec and authentication seam;
- worker binary and deterministic test executor;
- generic runtime configuration with no SynDB defaults;
- OCI image and Kubernetes fixtures;
- unit, in-process, and Kind end-to-end tests; and
- SynDB burst-worker adapter/cutover plan.

## Acceptance

- [ ] Runtime and worker use the same ticket codec; source search finds no
      competing ticket interpretation.
- [ ] Full `BoundQuery` bytes are not used as an authorization ticket.
- [ ] Worker shutdown occurs after stream completion/drop and is tested for
      multi-batch and cancellation paths.
- [ ] No reusable implementation contains `SYNDB_*`,
      `syndb-snapshot-*`, SynDB namespaces, images, charts, or secret names.
- [ ] Unit/in-process gates pass:

  ```bash
  nix develop -c cargo test -p queryfabric-runtime-k8s --locked
  nix develop -c cargo test -p queryfabric-worker --locked
  ```

- [ ] The worker artifact builds:

  ```bash
  nix build .#oci-queryfabric-test-worker
  ```

- [ ] A non-ignored QueryFabric-owned Kind test passes without a SynDB checkout:

  ```bash
  nix develop -c cargo test -p queryfabric-runtime-k8s \
    --features integration-k8s --test kind_smoke -- --nocapture
  ```

- [ ] Secrets are delivered outside the Nix store and are absent from
      `kubectl get pod -o yaml` fields that are not Kubernetes Secrets.
- [ ] Cancellation and timeout leave no leaked Job, Pod, spec object, or
      credential within the documented cleanup window.

## Non-Goals

- making isolated execution part of the 0.2 host-to-host
  tabular resource-portability MVP;
- embedding SynDB ClickHouse schemas or ETL state;
- requiring Helm when library-generated resources and fixtures suffice;
- production federation fan-out; or
- general multi-job worker scheduling.

## Stop Conditions

If the real worker image, cluster fixture, credential source, or backend
executor is missing, report it as a required artifact with producer, build/setup
workflow, and proof command. Do not substitute a nonexistent SynDB chart or
silently downgrade the test to a unit mock.
