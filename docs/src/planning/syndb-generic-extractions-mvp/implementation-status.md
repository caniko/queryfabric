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
  restart persistence, tampered-artifact rejection, and an injected transaction
  failure that leaves both imported rows and receipts unchanged, removes the
  failed plan's unreferenced staging object, and then succeeds after a fresh
  dry-run.
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
- The portability schemas and machine-readable fixtures are checked in the
  `bundle-schema` gate. The fixture suite covers the RFC 8785 canonicalization
  vector, duplicate-key rejection, schema shape, and a valid bundle with
  independently verified artifact and schema digests. The separate
  `crossLanguage` flake check runs the same vector through Nixpkgs' Python
  RFC 8785 and BLAKE3 implementations.
- The release checks include a Rust 1.94 full-workspace compile gate, an
  offline RustSec audit using the pinned advisory database, `cargo-deny` bans /
  licence / source checks, and a generated-document structural accessibility
  smoke gate. These are deterministic repository checks; they do not replace
  a manual WCAG review or public release evidence.
- The NixOS module has a self-contained `checks.module` VM contract test that
  starts two named instances with a fake package and verifies the generated
  units retain the service hardening defaults and independent instance
  topology.
- The release smoke gate passes formatting, Clippy, workspace tests, the four
  `queryfabric-python` Rust unit tests, both fuzz targets, examples, maturin
  wheel/develop, the Python smoke test, and pytest;
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
  permissions, injected transaction rollback, plan-specific staging cleanup,
  replay, and restart recovery; the broader imported-row restart matrix
  remains open.
- The structural accessibility, pinned/offline RustSec, `cargo-deny`, MSRV,
  and bundle-schema gates now run in the flake. The audit passes with four
  explicit upstream-pin exceptions in `.cargo/audit.toml`: hickory-proto via
  libp2p-mdns (`RUSTSEC-2026-0118` and `RUSTSEC-2026-0119`) and quick-xml via
  Polars/object_store (`RUSTSEC-2026-0194` and `RUSTSEC-2026-0195`). These are
  tracked producer blockers, not silent waivers; remove each entry when the
  upstream producer publishes a compatible dependency update. The audit also
  reports the existing unmaintained-crate warnings for bincode, paste, and
  proc-macro-error2. A manual WCAG audit and public release evidence remain
  open.
- The four supplied grant context/template artifacts now live in the canonical
  applications checkout at `docs/grants/`, outside this release tree. QueryFabric
  is REUSE-clean after the move. The applications checkout itself still fails
  `reuse lint` because its 28 documentation files (including these four) lack
  producer-supplied copyright/licence metadata; no maintainer copyright or
  licence is inferred for them. The producer/rights holder must add approved
  SPDX headers or `.reuse/dep5` entries (using `reuse annotate` only with those
  approved values). Validate from that checkout with:

  ```bash
  nix develop /data/can/canix/projects/repos/owned/codeberg.org/caniko/queryfabric -c reuse lint
  ```
- The Python crate now keeps PyO3's `extension-module` feature behind the
  maturin-only `extension-module` feature, so its four Rust unit tests run in
  the normal dev shell. Maturin explicitly enables that feature for wheel and
  editable builds; the Python package's maturin/pytest path remains in place.
