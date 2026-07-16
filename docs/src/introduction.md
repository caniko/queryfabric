# QueryFabric

QueryFabric provides a verified data-portability boundary for self-hosted
analytical services and a portable query compiler extracted from scientific
platform work. The reference NixOS proof moves one published tabular profile
between independently configured hosts, verifies its expected digest, rejects
tampering, and persists an idempotent receipt.

The compiler gives hosts a stable semantic boundary between query text and
backend execution:

- parse SQL or downstream dialects such as SyQL into `ParsedQuery`
- bind names, parameters, functions, and types into `BoundQuery`
- analyze backend support before execution
- emit backend-specific artifacts with typed schemas and provenance

The public compatibility boundary is the
[`queryfabric` facade crate](https://codeberg.org/caniko/queryfabric/src/branch/trunk/crates/queryfabric/README.md).
Internal crates stay modular for composition, but they are not the public
promise.

## What QueryFabric Owns

- portable parsing and canonicalization
- typed parameters and result schemas
- catalog and function-registry contracts
- capability analysis and backend diagnostics
- SQL emission for the verified portable subset
- provenance receipts for analysis and emission

## What Stays Out of Core

- host routing and fan-out
- auth and job orchestration
- product-specific metadata resolution
- backend execution itself

That boundary is the point of the project. QueryFabric is useful when a host
needs one trustworthy place to parse, validate, analyze, and emit queries
without dragging runtime policy into the compiler surface.

## Documentation Map

- Start with [Data Portability](./scenarios/data-portability.md) and the
  [Reviewer Evidence](./project/evidence.md) page for the implemented MVP and
  its exact limits.
- Use [Installation](./getting-started/installation.md) if you want a
  local development setup.
- Follow [Quick Start](./getting-started/quick-start.md) for the shortest
  parse-bind-analyze-emit flow.
- Read [Host Integration](./integration/host-integration.md) if you are
  embedding QueryFabric into a larger scientific platform.

Source: <https://codeberg.org/caniko/queryfabric>
