# QueryFabric Roadmap

QueryFabric is a portable analytical query compiler for scientific platforms.
It also provides a data-sovereignty and query-portability layer for self-hosted
and federated services.

This roadmap describes the public direction of the project without promising
work that has not been delivered yet. The items below are ordered by horizon,
not by implementation status.

## Now

Pre-1.0 work for 2026 H2:

- v0.2.0 release
- REUSE compliance
- threat model refinement
- reproducible footprint benchmarks
- high-availability design documentation
- multi-instance NixOS module support

## Next

2026 H2 to 2027. These items are the subject of an NGI Fediversity grant
application and align with the grant work packages in
[`docs/grants/ngi-fediversity-application-plan.md`](docs/grants/ngi-fediversity-application-plan.md):

- Import-side portable bundles: export, transfer, import, verify round-trip
  delivery for portable query artifacts and their provenance
  - maps to WP1, service portability end to end
- Federation hardening: hub failover, NAT traversal, and schema-sync conflict
  handling for multi-instance deployments
  - maps to WP2, federation and HA hardening
- Embedded backend breadth: SQLite or DataFusion, chosen by footprint, with
  conformance-corpus expansion and differential testing across backends
  - maps to WP3, backend breadth for small hosters
- External security audit follow-up, threat-model-driven hardening, and
  contributor onboarding improvements
  - maps to WP4, security, community, and Fediversity integration
- Nixpkgs module upstreaming for the QueryFabric NixOS module
  - maps to WP4, because it supports community adoption and Fediversity
    integration

## Later

Post-1.0 work:

- 1.0 API stabilization per [`COMPATIBILITY.md`](COMPATIBILITY.md)
- additional backends via the open artifact seam described in
  [`DECISIONS.md`](DECISIONS.md) decision D003

## Principles

- Keep the public API stable once 1.0 is reached.
- Keep host-specific routing, auth, and execution policy outside the
  `queryfabric-*` crates.
- Treat portability and provenance as first-class project goals.
