# Reviewer Evidence

This page distinguishes what the current repository proves from what remains a
release, deployment, research, or adoption claim. QueryFabric is pre-release;
the commands below are the acceptance authority until public artifacts exist.

## Implemented proof

| Result | Reproducible evidence | Boundary |
|---|---|---|
| Independent-host tabular migration | `nix build .#checks.x86_64-linux.portability-migration --no-link -L` | One `queryfabric.tabular-csv/1` artifact into an exact, predeclared target schema; not arbitrary data or service migration |
| Hardened reference host | `nix build .#checks.x86_64-linux.selfhost --no-link -L` | One NixOS demonstrator with Postgres and S3-compatible storage; the host still owns identity, reverse proxy, TLS, and policy |
| Bundle contract and independent canonicalization | `nix build .#checks.x86_64-linux.bundle-schema .#checks.x86_64-linux.crossLanguage --no-link -L` | Integrity relative to an authenticated expected digest; no signature or key-trust claim |
| Compiler and adapter quality gates | `nix flake check -L --no-update-lock-file` | Repository checks on the candidate revision; not a formal certification |
| Metadata-derived publication tier | `nix develop -c scripts/release.sh plan` | A ten-crate plan; no crates.io publication, tag, or Codeberg release claimed |
| Documentation and combined site | `nix build .#docs .#site --no-link` | Locally reproducible artifacts; a public Pages deployment must be checked separately |

The migration VM uses independent alpha and beta PostgreSQL and Garage
endpoints. It exercises export, scoped operator transfer, target dry-run,
apply, replay, service restart, query-after-restart, tampered-artifact
rejection, transaction rollback, staging cleanup, and a subsequent successful
retry. The detailed implementation record is in
[MVP implementation status](../planning/syndb-generic-extractions-mvp/implementation-status.md).

## Current limitations

- The importer accepts one artifact and requires the target relation schema to
  match exactly. Multi-resource migration sets, column mapping, and safe typed
  conversion are future R&D.
- The demonstrator announces federation identity, while the hub and node actors
  are library substrate and are not wired into the HTTP demo. Hub failover, NAT
  traversal, and conflict-safe schema synchronization are not implemented.
- The NixOS module runs and hardens the service. It does not provision
  PostgreSQL, S3 storage, DNS, a reverse proxy, TLS certificates, or a production
  identity provider.
- No Docker/Podman image or `oci-image` flake output is published.
- The structural accessibility check is automated; a manual WCAG review has not
  been completed, so no conformance level is claimed.
- RustSec currently has explicit, documented upstream dependency exceptions.
  They are tracked in `.cargo/audit.toml`, not treated as resolved findings.
- Project maintenance is currently single-maintainer. No external operator,
  hosting-provider adoption, or Fediversity endorsement is claimed.

## Public-artifact gate

A first public-release claim requires all of the following to resolve against
the same reviewed revision:

1. a clean candidate and green full flake check;
2. two clean footprint runs under a tolerance chosen before measurement;
3. a published manual accessibility review;
4. a signed source tag, ten reachable crates.io package versions, and a
   Codeberg release;
5. the combined project site at `https://queryfabric.tartanoglu.com/` with this
   mdBook under `/docs/`; and
6. an externally reachable demo only if its build revision and current import
   routes can be verified.

The convenience demo at `https://queryfabricdemo.tartanoglu.com/` is not a
substitute for these repository and release checks.
