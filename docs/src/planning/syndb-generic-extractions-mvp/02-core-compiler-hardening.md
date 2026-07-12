# Phase 02: Core Compiler Hardening

## Goal

Turn the substantially implemented compiler into a safe, coherent, and honest
0.2 public surface.

This is the technical/security baseline for the grant-informed MVP, not a
sufficient Fediversity impact outcome on its own. Only genuinely new scoped R&D
from this phase belongs in a future application; package cleanup and release
catch-up do not become research by relabeling them.

## Starting Evidence

- `QueryCompiler` already exposes parse, bind/validate, analyze, and emit.
- SQL/SyQL, catalog binding, result schema, capabilities, provenance, and both
  backend emitters have broad tests.
- `crates/queryfabric-catalog/src/render/emit.rs` appends logical identifiers directly
  in several relation, column, alias, and CTE paths.
- the same emitter renders catalog-controlled backend function mappings through
  raw `display_name()` text; `crates/queryfabric-catalog/src/render/helpers.rs`
  also places ClickHouse timezone text into a quoted type argument without
  escaping.
- the same emitter uses `unwrap_or_default()` when rendering a CTE subquery.
- `InteractiveRuntime` is publicly re-exported but every execution returns
  `RuntimeError::NotImplemented`.
- `queryfabric-runtime` exposes its incomplete Flight skeleton through a public
  optional feature even though Flight graduation is post-MVP.
- Cargo metadata reports 19 publishable crates, while only ten form the
  coherent compiler/facade dependency closure.

## Work

### 1. Define one backend-token model

Inventory every catalog/IR-derived string that can reach emitted backend text
and classify it as an identifier, an allowlisted keyword/operator, or a typed
literal/type argument. No catalog-controlled string may bypass that model.

Catalog and IR names remain logical, unquoted identifier segments. Each
backend adapter renders those segments through one implementation that:

- rejects empty, control-character, or otherwise invalid logical names;
- escapes the backend quote character correctly;
- always quotes untrusted relation, column, alias, and CTE segments;
- renders qualified names segment by segment rather than quoting a dotted
  string; and
- does not accept “already quoted” input as an escape hatch.

Apply equivalent typed handling to other raw backend tokens already present:

- render `BackendFunctionMapping::{namespace,name}` as validated backend
  function-path segments rather than raw `display_name()` text;
- represent ClickHouse timestamp timezones as validated timezone values and
  escape them as type arguments rather than inserting raw quoted text; and
- keep keywords, operators, type constructors, and function capabilities in
  closed adapter-owned enums/registries.

Apply the same rule to ClickHouse `DynamicClient` table targets. Replace raw
fully-qualified table interpolation with a parsed/typed qualified identifier.
Literal values continue through parameters, never identifier rendering.

Add adversarial tests for quote characters, dots, whitespace, Unicode, reserved
words, comment markers, statement separators, alias/CTE nesting, mapped
function namespace/name values, and timezone/type arguments on both PostgreSQL
and ClickHouse.

### 2. Preserve all compiler errors

Replace CTE `unwrap_or_default()` with error propagation and an error context
that identifies the failing CTE without discarding the original diagnostic and
source span.

Audit compiler/adapter emission for other `unwrap_or_default`, ignored results,
panic paths, or fallback-to-empty behavior. Fix only paths that can hide
invalid compiler output; record unrelated cleanup separately.

### 3. Add configurable compilation budgets

Introduce a host-configurable budget checked during parse/bind for at least:

- input bytes;
- parameter count;
- syntax/plan node count;
- nesting depth; and
- join/CTE count.

Defaults must be conservative enough for normal examples and explicit in
documentation. Exceeding a budget returns a stable structured diagnostic, not
a panic or generic internal error. Execution time, row count, and byte limits
remain host responsibilities and are implemented in Phase 04.

### 4. Make the runtime surface truthful

Keep `ExecutionRuntime` and cancellation-aware stream types as contracts.
Remove `InteractiveRuntime` from the stable facade, remove the type entirely,
or give it a real implementation. An unconditional `NotImplemented` concrete
type is not part of the stable MVP.

Likewise, stop re-exporting `DynamicClient` and other execution-only
ClickHouse configuration/errors from the compiler facade. They may remain in a
documented adapter feature for host integrations, but D003 does not make them
compiler behavior.

Document ClickHouse interactive execution as a host-provided transport seam.
Keep batch/isolated modes explicitly experimental until Phase 06.

Move the current Flight module/feature into a dedicated
`queryfabric-flight` crate with `publish = false`. SynDB may consume that
registry-unpublished workspace crate by path while developing parity. The published
`queryfabric-runtime` must not expose a feature that freezes the
default-`Unimplemented` Flight API before graduation.

### 5. Establish release tiers

Keep only this dependency closure publishable:

- `queryfabric-contract`;
- `queryfabric-ir`;
- `queryfabric-catalog`;
- `queryfabric-dialect-sql`;
- `queryfabric-dialect-syql`;
- `queryfabric-runtime`;
- `queryfabric-adapter-postgres`;
- `queryfabric-adapter-clickhouse`;
- `queryfabric-opt`; and
- `queryfabric`.

Set the other nine currently publishable crates to `publish = false` until
their later graduation criteria pass. Generate the publish plan from
`cargo metadata`; do not maintain conflicting hard-coded lists in release
scripts, tools, prose, and workflows.

Add a flake input for `git+https://codeberg.org/caniko/simit.git`, consume
`simit.packages.${system}.default` in the QueryFabric dev shell, and commit its
locked revision before regenerating release/CI metadata. A user-profile binary
is not a reproducible producer.

Regenerate simit CI metadata and remove every `publish-crate-*.yaml` workflow
for a crate that is now `publish = false`. A stale tag-triggered workflow is
still an accidental publication path.

Update README, crate catalog, compatibility/MSRV, threat-model status, and API
examples to match the metadata-derived workspace count (41 at this baseline)
and stable tier. Resolve the broken grant-plan links either by intentionally
porting reviewed durable content or removing the links.

## Deliverables

- centralized safe identifier rendering for both stable adapters;
- propagated CTE and nested-emission failures;
- compiler-budget contract and diagnostics;
- no misleading concrete runtime in the stable facade;
- no execution client presented as part of the compiler-facade contract;
- no unfinished Flight feature in a publishable crate;
- ten-crate stable release tier and registry-unpublished experimental crates;
- flake-pinned simit producer with metadata-consistent publish workflows; and
- accurate package/MSRV/security documentation.

## Acceptance

- [ ] Portable conformance, property, result-schema, capability, and adapter
      tests pass.
- [ ] Adversarial tests cover identifiers, mapped function paths, and
      timezone/type arguments and demonstrate no token or statement injection.
- [ ] A deliberately invalid nested CTE returns its original structured error.
- [ ] Budget tests reject every configured dimension at its boundary and accept
      a normal query immediately below it.
- [ ] `rg -n 'unwrap_or_default' crates/queryfabric-catalog/src/render` has no
      error-swallowing emission path.
- [ ] No stable facade constructor returns `NotImplemented` for every call.
- [ ] No feature of a publishable crate exposes the pre-graduation Flight
      skeleton.
- [ ] Cargo metadata reports exactly the approved publish tier:

  ```bash
  cargo metadata --no-deps --format-version 1 |
    jq -r '.packages[] | select(.publish != []) | .name' | sort
  ```

- [ ] Normal repository gates pass:

  ```bash
  nix develop -c cargo fmt --all -- --check
  nix develop -c cargo clippy --workspace --all-targets --locked -- -D warnings
  nix develop -c cargo test --workspace --all-targets --exclude queryfabric-python --locked
  nix develop -c cargo check -p queryfabric-python --locked
  nix develop -c cargo clippy --workspace --all-targets --all-features \
    --locked -- -D warnings
  nix develop -c cargo test --workspace --all-targets --all-features \
    --exclude queryfabric-python --locked
  nix flake check -L
  ```

- [ ] No `publish-crate-*.yaml` workflow targets a `publish = false` package;
      a metadata-to-workflow consistency check enforces this in CI.

- [ ] `mdbook build docs` and link checking find no missing canonical grant or
      planning target.

## Non-Goals

- owning host authentication, authorization, execution, routing, or queues;
- implementing production federation;
- making the optimizer's default identity pipeline a blocker;
- publishing Python bindings; or
- graduating Flight/K8s/worker crates.

## Stop Conditions

If a logical name or other catalog-controlled token reaches an emitter without
a typed boundary, stop and repair the catalog/IR boundary rather than guessing
how to quote it. If making a crate registry-unpublished breaks the core dependency closure,
record the exact dependency edge and either graduate that dependency with
tests or remove the edge; do not silently publish an unfinished crate.
