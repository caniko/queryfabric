# queryfabric-adapter-clickhouse

ClickHouse SQL adapter for QueryFabric's verified portable subset.

This crate analyzes bound portable queries against ClickHouse capabilities and
emits typed SQL artifacts with QueryFabric provenance metadata.

It is published for composition, but most users should depend on the
[`queryfabric`](https://docs.rs/queryfabric) facade crate and use the adapter
through that public API.
