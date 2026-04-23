# queryfabric-adapter-postgres

PostgreSQL SQL adapter for QueryFabric's verified portable subset.

This crate performs backend capability analysis for PostgreSQL and emits typed
SQL artifacts for the portable relational contract QueryFabric verifies.

It is public for advanced use, but the preferred entry point is the
[`queryfabric`](https://docs.rs/queryfabric) facade crate.
