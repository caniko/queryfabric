# MVP implementation status

This note records what is implemented in the current working tree after the
grant-aligned Phase 04 work. It is deliberately factual; it is not a release
claim.

## Implemented

- `queryfabric-portability` retains the legacy export-only bundle 1.0 and adds
  import-ready bundle 2.0 sealing with RFC 8785 canonical JSON and typed
  `blake3-256:` digests.
- The bounded `queryfabric.tabular-csv/1` profile validates duplicate-key-free
  JSON, exact typed headers, CRLF CSV, UTF-8/no-BOM input, scalar lexical
  boundaries, byte/row/column/nesting limits, artifact digests, and schema
  fingerprints. It returns a deterministic predeclared-target import plan.
- The demonstrator uses the normative writer and exposes `/imports/dry-run` and
  `/imports/apply`. Apply persists rows, source evidence, local policy/owner,
  mapping, target import event, and an idempotent receipt in PostgreSQL.
- `seedDemoData = false` creates a schema-only target with predeclared station
  relations. Garage replaces the insecure MinIO NixOS fixture.
- `nix/tests/portability-migration.nix` proves independent alpha/beta
  PostgreSQL and Garage endpoints, operator transfer, dry-run, apply, replay,
  restart persistence, and tampered-artifact rejection.
- The demonstrator query endpoint now executes typed positional or named
  parameters through PostgreSQL bind values and returns contract version,
  ordered parameter schema, result schema, query provenance, snapshot ID, and
  explicit row/byte limits, backend selection, snapshot conflicts, and an
  explicit truncation flag. The self-host VM asserts this path.
- The reference-host import path now derives the predeclared target revision
  and local owner, stages artifacts under their content digest, returns a
  dry-run plan/staging identity, and requires both identities on apply. Stale
  plans and changed staged bytes fail with conflict responses; replay checks
  the stored plan, mapping, owner, revision, and byte count before returning a
  receipt.
- Host mutation/import routes resolve verified PASETO bearer credentials and
  required roles. The NixOS module loads the validation secret through
  `LoadCredential`. Database handles and module options now support distinct
  migration, read-only query, and narrow import-writer URLs; the migration VM
  provisions and tests those role boundaries.
- OpenDAL 0.58's S3 feature is wired with its explicit HTTP transport and
  registry initialization. Garage export/import is covered by both VM proofs.
- Cargo metadata now reports exactly the ten-crate stable publish tier. The
  nine tooling/runtime packages outside that closure have `publish = false`,
  their accidental publish workflows are removed, and CI compares the
  metadata-derived tier with the remaining publish workflows. The release
  helper derives dependency order from the same metadata instead of keeping a
  second crate list.
- Nested CTE emission now propagates renderer errors instead of converting a
  failed nested render into an empty SQL fragment. Compiler budgets are
  configurable at the facade boundary and return structured dimension/limit
  diagnostics; identifier, mapped-function, timezone, and ClickHouse table
  tokens use segment-aware validation/rendering. A larger adversarial/property
  matrix remains release evidence work.
- The release smoke gate passes formatting, Clippy, workspace tests, both fuzz
  targets, examples, maturin wheel/develop, the Python smoke test, and pytest;
  the mdBook build and doctest suite also pass.
- SynDB's production Flight server now selects its behavior-complete
  `SyndbFlightService` instead of the unfinished skeleton. Its focused Flight
  package gate passes all unit and internal integration tests, and a new
  non-ignored bound-socket fixture proves that the production constructor
  rejects unauthenticated DoGet requests. The broader DoGet/DoPut policy,
  metadata, and cancellation matrix remains open.

## Still open before an MVP release gate

- Import HTTP authorization is currently a demonstrator-local PASETO secret
  and role mapping; production identity-provider/session integration remains
  open. The NixOS migration fixture now proves separate PostgreSQL role
  permissions, but the complete failure/cleanup and imported-row restart matrix
  remains open.
- Accessibility review, pinned/offline RustSec checks, generated schemas and
  cross-language vectors, and public release evidence remain separate work.
  `cargo deny check bans licenses sources` passes, but `cargo audit` currently
  reports four vulnerabilities: `hickory-proto 0.25.2` through
  `piying 0.1.1 -> libp2p 0.56 -> libp2p-mdns` (no fixed release for one
  advisory), and `quick-xml 0.39.4` through `polars 0.53.0 -> object_store
  0.13.2` (Polars does not yet accept `object_store 0.14`). The remaining
  `bincode`, `paste`, and `proc-macro-error2` findings are unmaintained-crate
  warnings. No advisory is ignored or silently waived.
- The four supplied grant context/template artifacts now live in the canonical
  applications checkout at `docs/grants/`, outside this release tree. QueryFabric
  is REUSE-clean after the move. The applications checkout itself still fails
  `reuse lint` because its 28 documentation files (including these four) lack
  producer-supplied copyright/licence metadata; no maintainer copyright or
  licence is inferred for them.
- The workspace all-targets test gate passes with the repository's existing
  `--exclude queryfabric-python` boundary, and `cargo check -p
  queryfabric-python --locked` passes separately. The Python crate's unit-test
  target still cannot link with PyO3's `extension-module` feature because the
  dev shell does not provide a Python embedding library; Python behavior remains
  covered by the maturin/pytest release path.
