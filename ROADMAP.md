# QueryFabric Roadmap

QueryFabric combines a portable analytical query compiler with a bounded
data-portability layer for self-hosted services. This roadmap separates
delivered repository evidence from future research; it is not a funding or
release claim.

## Delivered baseline

The current `0.2.0` workspace implements and tests:

- bundle `2.0` with RFC 8785 canonical JSON, typed BLAKE3 digests, and the
  `queryfabric.tabular-csv/1` profile;
- authenticated export plus operator-mediated transfer, dry-run, apply, replay,
  restart, rollback, and tamper rejection for one predeclared tabular resource;
- durable PostgreSQL import receipts and independent alpha/beta Postgres and
  Garage endpoints in a two-host NixOS VM proof;
- typed query compilation for the verified PostgreSQL/ClickHouse subset;
- a hardened single-node NixOS service and named multi-instance module wiring;
- structural accessibility, MSRV, RustSec, dependency-policy, schema-vector,
  fuzz-build, Python-binding, and documentation checks.

These are repository results, not evidence of a public crate release, a
production federation deployment, or adoption by an external operator. The
[reviewer evidence matrix](docs/src/project/evidence.md) gives the exact proof
commands and limitations.

## Release-readiness work

Before the first public release:

- publish the combined project site and mdBook at the documented custom domain;
- run two clean-candidate footprint measurements under a predeclared tolerance;
- complete and publish a manual keyboard, labels, contrast, and scope review;
- reconcile and exercise the metadata-derived ten-crate publication path;
- create a signed tag, crates.io publications, and a Codeberg release, then
  verify every public URL; and
- obtain an external technical read-through and record real feedback without
  implying adoption.

## Candidate funded R&D

The following is proposed work. Effort, cost, schedule, and eligibility must be
re-estimated from the final work breakdown; already delivered baseline work is
not part of the future scope.

### WP1: versioned migration sets and typed schema rebinding

Move from one exact-schema artifact to a coordinated set of related resources:

- publish a `migration-set/1` schema and canonical vectors for multiple related
  `queryfabric.tabular-csv/1` artifacts;
- map explicit column renames and reordering into predeclared target relations;
- define a narrow safe-conversion lattice and structured diagnostics for every
  rejected or ambiguous mapping, without dynamic DDL;
- provide operator commands to inspect, plan, apply, verify, and resume supplied
  migration files;
- make multi-resource application atomic in visibility or explicitly
  checkpointed, with durable provenance and idempotent recovery; and
- prove differently named/shaped targets, interruption and resume, tampering,
  mapping conflicts, and absence of partial visibility in a two-host NixOS test.

### WP2: federation and high-availability hardening

- persist enough registry state for bounded hub failover and re-registration;
- design and test a concrete NAT traversal/fallback path;
- reject divergent schema-sync histories and make replay ordering explicit; and
- wire the federation substrate into an end-to-end reference deployment before
  making a production federation claim.

### WP3: one measured embedded backend

- select one embedded backend using published footprint, semantic-coverage, and
  maintenance criteria;
- implement its adapter against the existing artifact seam; and
- execute an expanded differential conformance corpus across PostgreSQL,
  ClickHouse, and the selected backend.

### WP4: security, community, and conditional ecosystem integration

- harden the new migration and federation surfaces from the threat model;
- triage every delivered external-audit finding and remediate agreed in-scope
  findings, conditional on an audit actually being supplied;
- prepare a reviewable nixpkgs module contribution and operator documentation;
- improve issue curation and contributor walkthroughs; and
- discover the supported Fediversity application-contract boundary and build an
  isolated adapter proof only if upstream provides an immutable interface.

## Later

- 1.0 API stabilization under [`COMPATIBILITY.md`](COMPATIBILITY.md);
- additional backends through the artifact seam in
  [`DECISIONS.md`](DECISIONS.md), decision D003; and
- broader resource profiles only after each gains its own schema, limits,
  conformance vectors, and host proof.

## Principles

- Keep delivered evidence distinct from planned work.
- Keep host routing, authorization, execution, and deployment policy outside
  the stable compiler crates.
- Treat integrity, provenance, portability, and explicit claim boundaries as
  first-class project requirements.
- Do not turn conditional upstream cooperation into a promised deliverable.
