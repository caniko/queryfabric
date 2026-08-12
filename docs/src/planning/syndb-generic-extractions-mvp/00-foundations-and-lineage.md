# Phase 00: Foundations And Canonical Lineage

## Goal

Make the normal validation workflows reproducible and establish one canonical
QueryFabric revision before migrating or deleting any behavior.

This phase is a hard prerequisite. A successful `/tmp --manifest-path` test is
useful source evidence, but it is not permission to retain broken repo-root or
pure-Nix workflows.

## Starting Evidence

- QueryFabric canonical checkout: `trunk@c939ce5`.
- SynDB checkout: `rapid@717f557`.
- SynDB submodule: `workspace-tidy-20260711@1ba7f34`.
- SynDB `flake.lock` QueryFabric revision: `8f4707d`.
- The vendored topic branch is based on `a13aca2`, which has no merge base with
  canonical `c939ce5`.
- Topic-only neutral deltas are `36a327f` and `1ba7f34`. The `626167d`
  thespis-to-piying migration is already represented on canonical trunk.
- QueryFabric repo-root Cargo inherits nightly codegen settings from
  `/data/nvme0/can/canix/.cargo/config.toml` while its Nix dev shell supplies
  stable Rust.
- SynDB pure evaluation imports
  `/data/nvme0/can/Projects/SynDB/vendor/queryfabric`.
- SynDB UV metadata expects the missing sibling `../nix-article`.
- At the initial observation the four new `grant/*.{md,json}` artifacts had no
  copyright/licensing metadata and made QueryFabric's REUSE check fail. They
  were subsequently moved to the canonical applications checkout; QueryFabric
  is now REUSE-clean, while the applications checkout still needs producer
  metadata.
- QueryFabric's local `remote.origin.url` ends in a literal space; SSH remote
  operations fail with `Forgejo: Invalid repo name` even though the canonical
  HTTPS repository is reachable.

## Work

### 1. Isolate QueryFabric's stable developer toolchain

First remove or regenerate `/data/nvme0/can/canix/.cargo/config.toml` so its
nightly-only profile and unstable settings are scoped to the projects that
actually use them. Cargo merges ancestor configuration even when `CARGO_HOME`
is isolated, so a child dev-shell config cannot neutralize this by itself. The
upstream producer is the canix/rs-harbor workflow that wrote the ancestor file.
The preferred upstream change removes the tracked parent file and relies on
canix's current `nix develop .#configure` dev shell to install its nightly
configuration into an ephemeral Cargo home.

Apply that one-time canix source change with:

```bash
cd /data/nvme0/can/canix
git rm .cargo/config.toml
```

After that repair, QueryFabric may add a pinned rs-harbor input, generate a
stable `mkCargoConfig`, and use its `mkDevShell` isolation. If that dependency
is not accepted, implement equivalent stable configuration in the existing
`pkgs.mkShell`. Preserve the workspace's declared MSRV policy; do not switch to
nightly merely to accommodate an unrelated ancestor config unless the project
explicitly changes its compiler policy.

Add a regression check that enters the dev shell from the repository root and
shows no inherited profile or `-Z codegen-backend` setting.

### 2. Repair SynDB's pure source inputs

Replace the absolute QueryFabric source in `SynDB/nix/rust.nix` with one
lock-controlled source. Choose either the submodule or flake input and use that
choice consistently for Nix and Cargo.

Choose a reproducible `anx-plot` source, update `pyproject.toml`, and regenerate
`uv.lock` with:

```bash
nix shell --inputs-from . nixpkgs#uv -c uv lock
```

This uses SynDB's pinned nixpkgs input without running the dev-shell hook that
currently invokes `uv sync` against the invalid lock source.

Do not point the lock at a convenient untracked sibling checkout.

### 3. Reconcile the QueryFabric topic lineage

First repair the local canonical remote URL exactly, without changing its host,
owner, repository, or protocol:

```bash
git remote set-url origin 'ssh://git@codeberg.org/caniko/queryfabric.git'
```

The malformed value is stored in this checkout's `.git/config`. If the
workspace clone/registry workflow recreates it, fix that producer as well; do
not carry a one-off local correction as reproducibility evidence.

Create a new implementation branch from canonical `trunk`. Review the
`36a327f` and `1ba7f34` diffs file by file:

- port generic `queryfabric-web` Flash, query-string, append-query, and
  safe-local-redirect behavior;
- adapt compiler dependencies/features to canonical Cargo structure;
- re-evaluate the ClickHouse rewrite/scope hunks against canonical code rather
  than replaying them automatically; and
- skip `626167d` where canonical behavior is already equivalent.

Run focused QueryFabric and SynDB UI tests before updating dependencies. Then
stop at an operator checkpoint: review the branch and obtain explicit authority
to push it and merge it into the canonical remote ref. Only after that commit
is remotely reachable may SynDB update both its submodule and QueryFabric flake
lock entry. If push/merge is not authorized, hand off the tested branch and
leave Phase 00 incomplete rather than pinning an unreachable commit.

The default review ref and proof, executed only after push authority is given,
are:

```bash
candidate="$(git rev-parse HEAD)"
git push -u origin HEAD:refs/heads/syndb-generic-extractions-mvp-foundations
git fetch origin refs/heads/syndb-generic-extractions-mvp-foundations
git merge-base --is-ancestor "$candidate" FETCH_HEAD
```

Merge that named ref through the repository's normal review workflow. The
post-merge `origin/trunk` proof below is the gate for changing SynDB pins.

### 4. Record a machine-readable baseline

Capture, in a committed or CI-produced report:

- both repository revisions and cleanliness;
- Cargo workspace package and publishable counts;
- the chosen Rust/Cargo/Clippy versions;
- submodule and flake-input QueryFabric revisions;
- focused and workspace gate commands; and
- known unavailable external publication/deployment credentials.

The report may be regenerated, but its producer command must be documented.

### 5. Resolve grant-context provenance before treating it as repository input

Obtain the actual rights holder and SPDX licence from the producer of the four
grant context/template files. If the producer authorizes repository inclusion,
add those exact facts through a dedicated `REUSE.toml` annotation or per-file
metadata. If not, keep the artifacts outside the release repository. Do not
assign QueryFabric maintainer ownership or Apache-2.0 by inference.

The grant pack is research orientation, not a source of applicant facts. Keep
the verified technical conclusions in this plan, but do not copy stale
identity, budget, rate, adopter, patent, prior-funding, or release claims from
the vendored topic lineage.

## Deliverables

- QueryFabric dev shell with stable, isolated Cargo configuration.
- Pure SynDB QueryFabric and `anx-plot` sources.
- New canonical-trunk QueryFabric commit containing reviewed topic-only web
  behavior.
- SynDB submodule and flake lock converged on that commit.
- Reproducible baseline evidence for later phases.
- REUSE-clean treatment of the grant context, based on producer-supplied
  rights metadata or exclusion from the release tree.

## Acceptance

- [ ] These commands work from the QueryFabric repository root without a
      `/tmp` bypass:

  ```bash
  nix develop -c cargo build --workspace --locked
  nix develop -c cargo clippy --workspace --locked -- -D warnings
  nix develop -c cargo test --workspace --locked
  ```

- [ ] The chosen ancestor-config repair is explicit and proven. Stable mode
      must not inherit `profile.dev.codegen-backend`, `[unstable]` codegen
      settings, or generated `-Z codegen-backend` flags from the canix parent.
      For the preferred removal, canix itself passes:

  ```bash
  cd /data/nvme0/can/canix
  test ! -e .cargo/config.toml
  nix develop .#configure -c cargo check --manifest-path cli/Cargo.toml --locked
  ```

- [ ] SynDB pure and Python inputs validate:

  ```bash
  cd /data/can/canix/projects/repos/owned/github.com/memorycircuits/SynDB
  nix flake check --no-build
  nix develop . -c uv sync --locked
  ```

- [ ] Submodule and lock metadata identify one commit:

  ```bash
  git submodule status vendor/queryfabric
  jq -r '.nodes.queryfabric.locked.rev' flake.lock
  ```

- [ ] Before merge, the candidate is reviewable against its named remote
      implementation ref. An explicitly authorized review/merge/push checkpoint
      then makes it reachable from canonical `origin/trunk`.
- [ ] The canonical remote has no trailing whitespace and is reachable:

  ```bash
  test "$(git remote get-url origin)" = \
    'ssh://git@codeberg.org/caniko/queryfabric.git'
  git ls-remote --exit-code origin HEAD
  ```
- [ ] After that checkpoint, the selected SynDB revision is an ancestor of the
      canonical integration branch:

  ```bash
  git -C vendor/queryfabric fetch origin trunk
  git -C vendor/queryfabric merge-base --is-ancestor \
    "$(jq -r '.nodes.queryfabric.locked.rev' flake.lock)" origin/trunk
  ```

- [ ] `nix develop -c cargo test -p queryfabric-web --all-features --locked`
      passes upstream, followed by the focused SynDB UI/server tests that
      consume the helpers.
- [ ] `nix develop -c reuse lint` passes without fabricated ownership/licence
      metadata.
- [ ] A diff review confirms no topic-branch grant claims or stale generated
      artifacts were silently copied.

## Stop Conditions

Stop and report the missing-artifact contract if:

- the canix/rs-harbor producer cannot regenerate or relocate the ancestor
  Cargo config without breaking its intended nightly consumers;
- the `anx-plot` upstream source or required revision cannot be identified;
- the topic-only web behavior depends on code absent from canonical trunk; or
- review/push/merge authority for the canonical candidate is not available; or
- the corrected canonical remote is not reachable, or the workspace producer
  keeps regenerating the malformed URL; or
- SynDB's Nix and Cargo consumers cannot be made to use one revision; or
- the applications checkout retains grant artifacts without producer-supplied
  rights metadata and cannot pass its REUSE gate.

Do not begin source deletion or public release work while any condition is
unresolved.
