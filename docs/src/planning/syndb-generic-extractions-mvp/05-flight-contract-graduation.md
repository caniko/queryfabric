# Phase 05: Flight Contract Graduation

## Goal

Extend QueryFabric's Arrow Flight frame until it can represent the behavior
SynDB already needs, prove it independently, and only then migrate SynDB off
its direct service.

This phase starts after the host-to-host tabular resource-portability MVP.
Phase 01's direct production service remains in place throughout development.
Phase 02 has moved the current
skeleton into registry-unpublished `queryfabric-flight`; this phase works there
and decides whether it is publishable only after parity.

It is outside the default Fediversity application scope. It must not displace
portable-import or independent-host evidence merely because SynDB already uses
Flight.

## Starting Gaps

The current `FlightHandlers` contract and `FlightSkeleton` cannot represent:

- handler-provided response metadata/trailers;
- `get_schema`;
- `do_action` and `list_actions`;
- streamed Put results without first collecting a `Vec`;
- descriptor authorization before DoPut handling;
- SynDB's partial dataset allow/deny/citation/delegation metadata; or
- an explicit parity manifest for every supported RPC.

The current CLI Flight builder also ignores connect timeout and bearer token
arguments and implements only a narrow DoGet path.

## Contract Design

### Authentication and authorization order

Keep authentication as a host-supplied metadata-to-`Subject` trait. Separate:

1. ticket/descriptor decoding into generic target references;
2. coarse policy evaluation;
3. host data-plane authorization that may return partial access and opaque
   domain decision metadata; and
4. handler execution.

The generic layer owns ordering and gRPC status mapping. SynDB owns dataset
policy, descriptor meaning, citation values, and delegation errors.

### Handler responses

Each supported handler returns a structured response containing:

- body or stream;
- initial response metadata;
- optional trailers/final status; and
- cancellation behavior.

DoGet stays streaming. DoPut returns a stream of `PutResult` values rather than
collecting all results. Upload authorization occurs after decoding the first
descriptor-bearing frame but before handing payload batches to the host.

Add explicit handler seams for `get_schema`, `do_action`, and `list_actions`.
RPCs neither QueryFabric nor SynDB supports remain explicitly
`Unimplemented` and are not advertised.

### Client builder

Replace the current thin helper with a builder that actually applies:

- TLS mode and trust roots;
- connect timeout;
- per-request timeout;
- bearer metadata with secret-safe debug output;
- maximum encoding/decoding frame sizes; and
- generic opaque ticket/descriptor/action bytes.

Expose streaming DoGet, DoPut, and action operations required by the server
contract. Domain descriptor constructors remain in SynDB.

## Work

1. Convert Phase 01's RPC parity manifest into QueryFabric contract tests.
2. Extend traits without importing SynDB types or policy.
3. Add an in-process tonic server/client fixture covering every supported RPC,
   metadata, authorization stage, multi-batch streaming, cancellation, and
   errors.
4. Implement SynDB adapters over the new traits while the direct service
   remains production-selected.
5. Run the same black-box suite once against the direct service and once
   against the skeleton adapter; compare status, metadata, schema, and batches.
6. Select the skeleton in production only after parity passes.
7. Remove the direct implementation only after a release cycle or an explicit
   rollback decision confirms the new path.

## Deliverables

- behavior-complete generic Flight traits and skeleton;
- real configurable Flight client builder;
- QueryFabric-owned in-process end-to-end fixture;
- SynDB adapters with real access evaluation;
- black-box direct-versus-skeleton parity evidence; and
- controlled SynDB cutover with rollback path.

## Acceptance

- [ ] No authentication, timeout, frame-limit, or TLS builder argument is
      ignored.
- [ ] DoGet and DoPut remain streaming under multi-batch load.
- [ ] Shutdown/cancellation drops the stream and backend work promptly.
- [ ] Descriptor/ticket authorization occurs before protected data reaches a
      handler.
- [ ] Metadata/citations and partial-access information survive the generic
      frame without QueryFabric knowing their domain schema.
- [ ] QueryFabric tests pass:

  ```bash
  nix develop -c cargo test -p queryfabric-flight --all-features --locked
  nix develop -c cargo test -p queryfabric-cli-toolbelt --all-features --locked
  ```

- [ ] SynDB's non-ignored production-server suite from Phase 01 passes against
      the skeleton.
- [ ] `rg -n 'allow all' crates/services/flight/src` finds no production access
      evaluator.
- [ ] The direct service is not deleted in the same commit that first selects
      the skeleton unless the parity/rollback evidence is independently
      reviewable.

## Non-Goals

- encoding SynDB dataset descriptors in QueryFabric;
- moving SynDB database/data-plane queries upstream;
- implementing every Arrow Flight RPC;
- coupling Flight to Kubernetes; or
- claiming federation from a Flight transport.

## Stop Conditions

If a SynDB behavior cannot fit without placing a domain type in QueryFabric,
extend the opaque metadata/adapter seam or keep that behavior in SynDB. If
tonic/Arrow versions prevent a shared streaming contract, record the exact
version/API incompatibility and its upstream producer before altering behavior
or buffering entire streams.
