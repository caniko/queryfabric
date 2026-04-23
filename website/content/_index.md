+++
title = "Portable Analytical Query Compiler"

[extra]
tagline = "Stable query compilation for scientific platforms"
subtitle = "QueryFabric gives database hosts one place to parse, bind, analyze, and emit portable analytical queries without dragging product-specific execution policy into the compiler core."
license = "Apache-2.0"
install_note = "During early adoption, consume QueryFabric by local path. The repo is staged for crates.io publication, but the compiler contract is already facade-first and stable in shape."
install_snippet = """
[dependencies]
queryfabric = { path = "../queryfabric/crates/queryfabric" }
"""
hero_code = """
let parsed = compiler.parse(&GenericSqlDialect, sql)?;
let bound = bind_and_validate_query(&parsed, &catalog, &params)?;
let analysis = compiler.analyze(&bound, &ClickHouseAdapter, &catalog);
let artifact = compiler.emit(&bound, &ClickHouseAdapter, &catalog)?;
"""
stages = [
  "ParsedQuery",
  "BoundQuery",
  "BackendAnalysis",
  "EmitArtifact",
]
backends = [
  "ClickHouse SQL",
  "PostgreSQL SQL",
  "Generic SQL dialect",
  "SyQL dialect layering",
  "Python bindings",
  "Provenance receipts",
]

[[extra.features]]
title = "Stable Facade"
body = "Use the `queryfabric` crate as the public contract: parse, bind, analyze, and emit without depending on parser internals."

[[extra.features]]
title = "Portable Subset"
body = "Ship a verified analytical subset with conformance tests instead of promising feature breadth that adapters cannot prove."

[[extra.features]]
title = "Typed Schemas"
body = "Carry typed parameters, result schemas, catalog snapshots, and field metadata through the compiler boundary."

[[extra.features]]
title = "Capability Analysis"
body = "Ask each backend adapter what a query requires, whether it is supported, and which diagnostics or advisories apply."

[[extra.features]]
title = "Dialect Layering"
body = "Treat SyQL as a downstream dialect on top of the neutral core instead of making one host language the public identity."

[[extra.features]]
title = "Scientific Host Fit"
body = "Keep routing, auth, execution, and metadata policy in the host so QueryFabric stays reusable across scientific platforms."
+++

QueryFabric is the portability boundary between analytical query text and
backend execution. It exists for hosts that need reproducibility, capability
checks, typed schemas, and backend diagnostics before they ever send SQL to a
database.
