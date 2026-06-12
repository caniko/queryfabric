# SyQL and Dialects

QueryFabric treats SyQL as a downstream dialect layered on the neutral compiler
core, not as the public identity of the project.

## Generic SQL First

The generic SQL dialect is the neutral baseline. It is the dialect you should
start with if you are embedding QueryFabric into a new host that does not need
host-specific syntax.

## SyQL Layering

The SyQL dialect exists for hosts that need compatibility with existing SyQL
query text.

Portable SyQL lowers into the same neutral stages as generic SQL:

- `ParsedQuery`
- `BoundQuery`
- `BackendAnalysis`
- `EmitArtifact`

## Directives and Host Policy

SyQL directives such as `SCOPE` and `DOWNLOAD` remain dialect metadata rather
than neutral core semantics.

That boundary is deliberate:

- QueryFabric owns parsing and structured preservation
- the host owns metadata resolution, routing, and execution policy

## Backend-Specific Functions

Backend-only functions should stay namespaced and explicit. A ClickHouse-only
function belongs behind a backend extension rather than pretending to be part
of the neutral portable function registry.

That keeps portability honest and diagnostics precise.
