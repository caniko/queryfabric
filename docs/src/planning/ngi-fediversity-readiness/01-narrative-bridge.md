# Phase 01 — Add the self-hosting and data-sovereignty narrative

## Working tree

`/data/nvme0/can/Projects/queryfabric` (this repo, branch off `trunk`).

## Goal

This phase succeeds when README.md, the Zola website landing page, and a new
mdBook chapter present QueryFabric as the data-sovereignty and
query-portability layer for self-hosted and federated services — in addition
to, not instead of, its scientific-platform identity — with every claim backed
by a named crate or file in the repo.

## Why this matters now

The NGI Fediversity grant application (deadline 2026-08-01) funds "the hosting
stack of the future" with *"service portability and data decoupling"* as its
core criterion. QueryFabric has the substance (GDPR traits in
`queryfabric-access`, content-addressed export bundles in
`queryfabric-portability`, tenancy isolation in `queryfabric-tenancy`, libp2p
federation in `queryfabric-federation`, a hardened NixOS module) but its
public framing — README line 3 reads *"QueryFabric is a portable analytical
query compiler for scientific platforms."* — gives a reviewer scanning for
hosting relevance no reason to keep reading. The grant-readiness report
(`docs/grants/ngi-fediversity-application-plan.md` §3) calls this the single
highest-leverage fix. Every other docs phase in this plan set hangs its
framing off this one.

The canonical bridge paragraph (adapt, don't contradict):

> Self-hosting fails at the data layer. Services trap user data in
> backend-specific SQL, schemas, and storage, so "moving instances" means
> lossy exports and manual surgery. QueryFabric is a portable analytical query
> compiler and data-sovereignty toolkit: a verified portable SQL subset that
> compiles to multiple backends (PostgreSQL, ClickHouse today), GDPR-aligned
> access/rectification/erasure as first-class library traits,
> content-addressed export bundles with provenance, and a libp2p federation
> protocol — packaged as a hardened NixOS module with end-to-end VM tests. It
> re-establishes the boundary between content owner and service provider at
> the query layer.

## Out of scope

- Do NOT touch `docs/src/deployment/self-hosting-nixos.md` — phase 07 owns it.
- No REUSE badge in README (phase 02), no ROADMAP/issue templates (phase 03),
  no accessibility statement (phase 03).
- No removal or downgrading of the scientific-platform framing — additive only.
- No new code, no Cargo.toml changes, no website redesign (content edits within
  the existing Zola templates only).
- No claims about unimplemented features (no "import bundles", no "SQLite
  backend" — those are grant-funded future work and belong in ROADMAP, phase 03).

## Plan

1. Branch from latest `trunk`.
2. **README.md**: keep the existing first sentence; immediately after it add a
   second positioning sentence from the bridge paragraph. Add a new section
   `## Why this matters for self-hosting` (after "What Stays Out of Core")
   that maps, one bullet each, to: `crates/queryfabric-access` (GDPR Art.
   15/16/17 traits), `crates/queryfabric-portability` (content-addressed
   export bundles, provenance, DOI), `crates/queryfabric-tenancy`
   (multi-tenant isolation), `crates/queryfabric-federation` +
   `crates/queryfabric-cluster` (wire-stable libp2p federation), and
   `nix/modules/queryfabric.nix` (hardened NixOS deployment, VM-tested).
   Link each bullet to the crate directory.
3. **Website** (`website/content/_index.md`): add a "Data sovereignty for
   self-hosted services" section using the same five capability bullets,
   phrased for a non-Rust operator audience. Verify rendering with
   `nix build .#website` (or `zola build` inside `website/`).
4. **New mdBook chapter** `docs/src/concepts/self-hosting-and-data-sovereignty.md`:
   ~1–2 pages tying together the Phase 05 sovereignty crates and the NixOS
   module. Structure: the lock-in problem → what QueryFabric owns (with crate
   links) → what stays with the host (cite `DECISIONS.md` D003: no execution
   in core — frame as the sovereignty/footprint feature it is) → pointer to
   the deployment chapter.
5. **SUMMARY.md**: add one line under `# Concepts`:
   `- [Self-Hosting and Data Sovereignty](./concepts/self-hosting-and-data-sovereignty.md)`.
   This file is also touched by phases 04/05/06 — rebase before landing.
6. Verify: `mdbook build docs` and the website build both succeed.
7. Commit (plain `git commit`, default signing) and merge per the wave order
   in the plan README (this phase lands first among the docs phases).

## Acceptance criteria

- [ ] README first paragraph contains both the scientific-platform sentence and
      the hosting/sovereignty sentence.
- [ ] README has a `## Why this matters for self-hosting` section with ≥5
      bullets, each linking to a real crate directory or `nix/modules/` path.
- [ ] `website/content/_index.md` contains the new section and `zola build`
      (or `nix build .#website`) exits 0.
- [ ] `docs/src/concepts/self-hosting-and-data-sovereignty.md` exists, cites
      `DECISIONS.md` D003 explicitly, and contains zero claims about
      unimplemented features.
- [ ] `mdbook build docs` exits 0 with the new SUMMARY line.
- [ ] `git grep -n "scientific platforms" README.md` still returns a hit
      (identity preserved).

## Files likely touched

- `README.md`
- `website/content/_index.md`
- `docs/src/concepts/self-hosting-and-data-sovereignty.md` (new)
- `docs/src/SUMMARY.md` (one line; shared with phases 04/05/06 — rebase)

## Pitfalls

- **Replacing instead of adding.** Symptom: scientific framing gone. Cause:
  over-eager rewrite. Recovery: restore the original first sentence; the
  acceptance grep guards this.
- **Unverifiable claims.** Symptom: prose says "migrate any service's data".
  Cause: bridging too hard. Recovery: every capability sentence names the
  crate that implements it; if no crate, it doesn't ship in this phase.
- **SUMMARY merge conflict.** Symptom: mdbook build fails post-merge with a
  missing-file or duplicate-entry error. Cause: phases 04/05/06 landed in
  between. Recovery: rebase, keep all additive lines, rebuild.
- **Zola template assumptions.** Symptom: section renders unstyled or not at
  all. Cause: `_index.md` front-matter/shortcode expectations in
  `website/templates/`. Recovery: mirror the structure of an existing section
  in `_index.md` rather than inventing new shortcodes.

## Reference

- Grant-readiness report §3 (narrative bridge): `docs/grants/ngi-fediversity-application-plan.md`
- Plan set: `docs/src/planning/ngi-fediversity-readiness/README.md`
- Decision log: `DECISIONS.md` (D003, D006)
- Downstream consumers of this framing: phases 02 (README badge), 03 (ROADMAP
  tone), 04/06 (docs chapters reference the sovereignty chapter)
