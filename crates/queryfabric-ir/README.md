# queryfabric-ir

Backend-neutral syntax and bound-query contracts for QueryFabric.

This crate carries the parsed and bound IR, diagnostics, logical types,
parameters, and provenance receipts used by dialects, binders, optimizers, and
emitters.

It is published for advanced composition, but most users should depend on the
[`queryfabric`](https://docs.rs/queryfabric) facade crate instead of building on
this crate directly.
