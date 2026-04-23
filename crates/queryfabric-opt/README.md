# queryfabric-opt

Conservative normalization and advisory pipeline for QueryFabric.

This crate hosts the backend-neutral optimization pass interface and the small
default normalization pipeline used by the facade.

It is a secondary public crate for advanced composition. Most consumers should
start with the [`queryfabric`](https://docs.rs/queryfabric) facade crate.
