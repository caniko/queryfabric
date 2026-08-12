# Phase 01: Restore SynDB Flight Safety

## Goal

Restore the production Arrow Flight path to its last behavior-complete SynDB
service and prove that behavior through the actual server constructor.

This is a correctness recovery, not a generic extraction. It prevents the
unfinished skeleton migration from remaining on the MVP integration path.

It is also a non-grant product prerequisite by default. Restoring a regressed
downstream path is maintenance/correctness evidence, not one of the proposed
Fediversity R&D outcomes.

## Starting Evidence

- `crates/services/flight/src/server.rs:118` constructs
  `crate::skeleton::build_skeleton(state)`.
- `skeleton.rs` returns `Unimplemented` for DoGet, DoPut, and ListFlights.
- `SynDbAccessDecision` currently allows all requests at its coarse gate.
- `service/flight_impl.rs` still contains the working direct
  `SyndbFlightService` DoGet/DoPut and related behavior.
- Most existing tests instantiate the direct service rather than starting the
  production server, so they did not catch the wiring regression.

All source paths in this phase are relative to the SynDB repository.

## Work

### 1. Inventory supported pre-migration behavior

Before editing, list every Flight RPC the direct service intentionally supports
and the response metadata, access filtering, citations, delegation errors, and
stream semantics associated with it. Keep intentionally unsupported RPCs
explicit; this phase does not fabricate implementations for them.

### 2. Restore the direct production service

Change `start_flight_server` to serve `SyndbFlightService` again. Keep the
generic skeleton adapters behind an experimental/test-only path if useful for
Phase 05, but do not route production traffic through them.

Remove or clearly mark comments that claim the data-plane migration is
complete. Do not weaken authentication or dataset-level policy to fit the
smaller generic interface.

### 3. Add production-path tests

Add a non-ignored integration fixture that:

1. constructs real `FlightState` with deterministic test adapters;
2. starts `start_flight_server` on an ephemeral listener;
3. connects through an Arrow Flight client;
4. verifies missing/invalid authentication is rejected;
5. verifies a supported DoGet returns expected schema, batches, and response
   metadata;
6. verifies dataset-level denial and partial-access behavior;
7. verifies supported DoPut behavior and its streamed results; and
8. exercises every other RPC claimed by the production service.

Testing `SyndbFlightService` directly remains useful unit coverage but is not
the acceptance gate.

### 4. Preserve a migration parity manifest

Create a small test table or module-level document enumerating each RPC and the
behavior that Phase 05's generic skeleton must preserve. Include:

- authentication and authorization stage;
- ticket/descriptor interpretation;
- response headers/trailers;
- streaming and cancellation behavior;
- error code mapping; and
- supported versus intentionally unsupported status.

## Deliverables

- production server restored to `SyndbFlightService`;
- non-ignored server-path integration tests;
- explicit RPC parity manifest for the later generic contract; and
- no claim that skeleton migration is complete.

## Acceptance

- [ ] No production path calls `build_skeleton`.
- [ ] `rg -n 'allow all|use SyndbFlightService directly until'` has no
      production-wiring match that contradicts actual behavior.
- [ ] Focused tests pass:

  ```bash
  cd /data/can/canix/projects/repos/owned/github.com/memorycircuits/SynDB
  nix develop . -c cargo test -p flight --locked
  ```

- [ ] At least one test reaches DoGet and DoPut through the bound server socket,
      not through a direct trait call.
- [ ] Authentication denial, dataset-policy denial, metadata, stream contents,
      and cancellation are asserted.
- [ ] SynDB build and Clippy pass for `flight` and its immediate dependents.

## Non-Goals

- extending QueryFabric's generic Flight traits;
- deleting `SyndbFlightService`;
- replacing SynDB descriptors or policies with generic placeholders;
- making intentionally unsupported Flight RPCs appear implemented; or
- claiming the generic skeleton is production-ready.

## Stop Conditions

If the test fixture cannot build `FlightState` without missing generated data,
secrets, migrations, or services, stop and record each required artifact, its
producer, setup command, and proof command. Do not replace the real state path
with a no-op service that would miss the regression.
