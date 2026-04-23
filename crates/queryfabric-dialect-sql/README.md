# queryfabric-dialect-sql

Generic SQL frontend for QueryFabric.

This crate parses portable SQL into QueryFabric's neutral parsed-query
contracts. It is the default SQL entrypoint underneath the facade and examples.

It is published for custom integrations, but most users should consume it
through the [`queryfabric`](https://docs.rs/queryfabric) facade crate.
