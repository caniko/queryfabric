# Phase 07 — Multi-instance support in the NixOS module

> **Recommended model: gpt-5.3-codex (codex) — effort `medium`**
>
> Routed: `carter route -c complex -r subagent -n coding -p codex`
> → `gpt-5.3-codex` / `medium`
>
> Complex coding: restructuring a hardened NixOS module around an
> `attrsOf submodule` instances pattern while preserving backward
> compatibility, systemd hardening, LoadCredential secret wiring, and the VM
> integration test. A weaker tier typically breaks one of: the legacy
> single-instance alias, per-instance credential namespacing, or firewall
> aggregation — failures that only surface in the VM test.
>
> Dispatch: `codex --model gpt-5.3-codex -c model_reasoning_effort=medium`

## Working tree

`/data/nvme0/can/Projects/queryfabric` (this repo, branch off `trunk`). No
phase prerequisite — `nix/` is disjoint from the docs phases. This phase owns
`docs/src/deployment/self-hosting-nixos.md` (no other phase touches it).

## Goal

This phase succeeds when `services.queryfabric.instances.<name>` deploys N
independent demo services on one host (distinct ports, state, credentials,
node identities), the existing single-instance interface keeps working
unchanged, and the VM test proves both.

## Why this matters now

Fediversity targets *hosting providers* running services for many users —
multi-tenant, multi-instance NixOS deployment is the consortium's core
scenario, and the grant application claims QueryFabric is deployable in that
world. Today `nix/modules/queryfabric.nix` defines exactly one service
(`options.services.queryfabric.{enable,package,listenAddress,port,publicBaseUrl,logLevel,database.{url,urlFile},store.{backend,endpoint,bucket,region,credentialsFile},federation.{enable,nodeName,hubMultiaddrs,flightPort},openFirewall}`).
A hosting provider cannot run two tenants' instances side by side. This is
Tier 2 item 9 in the grant-readiness report and the only coding-heavy phase
in this set.

## Out of scope

- No new service features (no new demo flags, no auth, no tenant routing —
  `queryfabric-tenancy` is library-level; host routing stays out per
  `DECISIONS.md`).
- No nixpkgs upstreaming (roadmap item).
- No NixOps4/Fediversity-stack integration beyond keeping the module plain
  NixOS (note compat in docs only).
- No HA wiring between the instances (phase 06 documents HA; this phase just
  makes N instances deployable).

## Plan

1. Branch from latest `trunk`.
2. Read `nix/modules/queryfabric.nix` fully, plus `nix/tests/selfhost.nix`
   and the flake's checks wiring (`flake.nix` exposes the VM test as a
   Linux-only heavy check).
3. Restructure the module:
   - Extract the current per-service option set into a submodule type
     (`instanceModule`), used by a new
     `services.queryfabric.instances = lib.mkOption { type = attrsOf (submodule instanceModule); default = {}; }`.
   - Generate one systemd unit per instance: `queryfabric.service` for the
     legacy path, `queryfabric-<name>.service` per instance. Preserve ALL
     existing hardening directives verbatim per unit.
   - Namespace state and runtime: `StateDirectory=queryfabric-<name>`,
     distinct `DynamicUser` per unit (or per-instance users — match the
     module's current user strategy), per-instance LoadCredential entries for
     `database.urlFile` and `store.credentialsFile`.
   - **Backward compatibility**: keep the existing top-level options working —
     implement them as a virtual default instance (internally mapped to the
     same submodule), so existing configs (and `nix/tests/selfhost.nix`
     pre-edit) evaluate unchanged. Assert that legacy options and
     `instances.default` are not both set.
   - Validation: assertions for port uniqueness across enabled instances
     (listen port and federation flightPort), and for
     `federation.enable → nodeName` uniqueness.
   - `openFirewall`: aggregate allowed ports across instances.
4. Update `nix/tests/selfhost.nix`: keep the existing single-instance
   scenario (legacy path) AND add a second machine—or extend the existing
   machine—running two instances via `instances.{alpha,beta}` with distinct
   ports; test both respond over HTTP and have isolated state directories.
   If VM resource cost forces a choice, convert the test to the instances
   API for both and add a tiny eval-only check (e.g.
   `nixosConfigurations`-style eval in `checks`) proving the legacy alias
   still evaluates.
5. Update `docs/src/deployment/self-hosting-nixos.md`: document
   `instances.<name>`, the legacy single-instance shorthand, a two-tenant
   example, and the port/credential namespacing rules.
6. Verify:
   - `nix flake check` (or at minimum `nix build .#checks.x86_64-linux.selfhost`)
     passes.
   - `nix eval` of a config using only legacy options succeeds with zero
     warnings other than intended ones.
   - `mdbook build docs` exits 0.
7. One CHANGELOG line under Unreleased: "multi-instance support in the
   QueryFabric NixOS module (`services.queryfabric.instances.<name>`)".
8. Commit (plain `git commit`, default signing).

## Acceptance criteria

- [ ] `services.queryfabric.instances.<name>` exists with the full option set
      of the current module (every option listed in "Why this matters now"
      available per instance).
- [ ] A configuration using only the pre-existing top-level options still
      evaluates and produces a working `queryfabric.service` (VM test or
      eval check proves it).
- [ ] VM test (`nix build .#checks.x86_64-linux.selfhost`) passes and
      exercises ≥2 concurrently running instances with distinct ports and
      distinct `StateDirectory` paths.
- [ ] Duplicate ports across instances fail evaluation with a clear assertion
      message (add an eval test or document the manual check performed).
- [ ] Secrets still flow only via LoadCredential — `git grep -n "database.url"`
      shows no path where a secret lands in the Nix store by default.
- [ ] `docs/src/deployment/self-hosting-nixos.md` documents the instances API
      with a two-instance example; `mdbook build docs` exits 0.

## Files likely touched

- `nix/modules/queryfabric.nix` (restructure)
- `nix/tests/selfhost.nix` (extend)
- `flake.nix` (only if check wiring needs it)
- `docs/src/deployment/self-hosting-nixos.md`
- `CHANGELOG.md` (one line)

## Pitfalls

- **Hardening drift.** Symptom: per-instance units lose `ProtectSystem`/
  `DynamicUser`/etc. Cause: rebuilding the unit instead of parameterizing the
  existing one. Recovery: diff the generated unit (`systemctl cat` in the VM
  test, or `nix eval` the unit text) against the pre-change unit; they must
  match modulo names/ports/paths.
- **LoadCredential name collisions.** Symptom: second instance reads the first
  instance's database URL. Cause: credential IDs not namespaced per unit.
  Recovery: credentials are per-unit in systemd, but verify the credential
  *names* used in `ExecStart`/env match the per-instance LoadCredential
  entries.
- **VM test cost blowup.** Symptom: selfhost check times out. Cause: extra
  machines/instances exceed the check's resources. Recovery: prefer extending
  the single machine with a second instance over adding machines; bump VM
  memory in the test if needed.
- **Legacy alias infinite recursion.** Symptom: `infinite recursion
  encountered` during eval. Cause: defining `instances.default` from legacy
  options that themselves read merged instance config. Recovery: map legacy →
  instance config in one direction only (e.g. via `mkMerge` on the internal
  config attrset, never reading back through `config.services.queryfabric.instances`).
- **Firewall aggregation.** Symptom: only the last instance's port opens.
  Cause: list overwrite instead of concat. Recovery: collect ports with
  `lib.concatMap`/`lib.mkMerge` across instances.

## Risk profile

Highest-blast-radius phase in the set: it rewrites the deployment artifact the
grant application showcases. Failure modes are eval-time (recursion,
assertions), unit-generation (hardening/credentials drift), and test-time (VM
flakiness). The legacy path is load-bearing: any external consumer of the
module (canix hosts, the demo deployment) must not need config changes.

## Strategy

Commit ladder (revert cost low → high):
1. Pure refactor: extract `instanceModule`, legacy options mapped through it,
   zero behavior change — VM test green before proceeding.
2. Add `instances.<name>` + assertions + firewall aggregation.
3. Extend the VM test to two instances.
4. Docs.
Each step is independently revertable; if step 2 or 3 stalls, step 1 alone is
still a clean landing.

## Rollback drill

Before starting step 2: `git tag pre-multi-instance && nix build
.#checks.x86_64-linux.selfhost` (record the green run). Rollback at any point:
`git reset --hard pre-multi-instance` and re-run the check — budget 15 minutes
for the rebuild; if the check is not green after rollback, the breakage
predates this phase: stop and report.

## Failure modes and recoveries

- **F1 — VM test red only in CI, green locally.** Cause: KVM availability or
  memory limits on the runner. Recovery: check runner labels (the heavy gate
  is Linux-only by design); reduce instance count in the test or raise VM
  memory; do not mark the check broken.
- **F2 — eval-time infinite recursion.** See Pitfalls; bisect with
  `nix eval --show-trace` on a minimal NixOS eval using the module.
- **F3 — legacy consumer breaks downstream (canix host).** Cause: renamed
  option or changed default. Recovery: the backward-compat acceptance
  criterion failed — restore the legacy surface exactly; downstream config
  changes are not acceptable in this phase.
- **F4 — secrets in store.** Cause: per-instance config serializing
  `database.url` into the unit's environment. Recovery: route inline URLs
  through the same env/credential path the current module uses; the
  acceptance grep guards the default path.

## Reference

- Grant-readiness report §4 (Tier 2 item 9): `docs/grants/ngi-fediversity-application-plan.md`
- Plan set: `docs/src/planning/ngi-fediversity-readiness/README.md`
- Module: `nix/modules/queryfabric.nix`; VM test: `nix/tests/selfhost.nix`
- HA context: phase 06 (`06-ha-design-doc.md`) — documents what multi-instance
  deployment does and doesn't buy in availability terms
