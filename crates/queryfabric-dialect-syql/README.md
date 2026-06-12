# queryfabric-dialect-syql

SyQL dialect frontend layered on QueryFabric's neutral contracts.

This crate preserves QueryFabric-compatible SyQL parsing while keeping QueryFabric host
policy out of the neutral core.

It is a downstream dialect crate. General-purpose consumers should start with
the [`queryfabric`](https://docs.rs/queryfabric) facade crate unless they
specifically need SyQL support.
