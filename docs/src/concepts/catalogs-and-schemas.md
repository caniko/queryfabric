# Catalogs and Schemas

The catalog is the portability layer.

QueryFabric does not assume PostgreSQL catalogs, QueryFabric metadata tables, or any
other product-specific schema store. It only requires a host to present neutral
relation and function information through the catalog contracts.

## MemoryCatalog

`MemoryCatalog` is the easiest place to start.

Use it for:

- examples
- tests
- small embedded hosts
- conformance fixtures

You register `RelationSchema` values directly, including aliases, relation
kind, and column definitions.

## Relation Schemas

A `RelationSchema` describes a table, view, or other relation-like input:

- namespace
- relation name
- aliases
- relation kind
- columns
- free-form metadata

Column metadata stays generic on purpose. Scientific hosts can carry units,
ontology identifiers, modality tags, or provenance hints in metadata without
forcing those concepts into the neutral core IR.

## Snapshot Identity

Set a snapshot identifier on the catalog when reproducibility matters:

```rust
catalog.set_snapshot_id("catalog-2026-04-20");
```

That snapshot id threads into binding, analysis, and emission provenance so a
host can later explain exactly which schema version was used.

## Result Schemas

Result schemas are first-class public output, not a backend afterthought.

They are designed to survive across adapters with enough structure for real
analytical tooling:

- logical data types
- nullability
- field ordering
- field metadata

That matters for notebook integrations, cached artifacts, and downstream
systems that need a stable contract before execution starts.
