# Governance

QueryFabric is currently a single-maintainer project. The Codeberg repository
owner, [`@caniko`](https://codeberg.org/caniko), is the maintainer and release
authority; no larger team or independent adoption is implied. Host integrations
are reference uses, not the definition of the public scope.

## Decisions and contributions

- Design discussion and change review happen in public Codeberg issues and pull
  requests.
- Durable architecture decisions are recorded in [`DECISIONS.md`](DECISIONS.md).
- Compatibility changes are documented in
  [`COMPATIBILITY.md`](COMPATIBILITY.md) and [`MIGRATION.md`](MIGRATION.md).
- Changes must preserve the separation between portable compiler behavior and
  host policy and avoid host-specific public symbols in the neutral core.
- The maintainer makes the final decision when consensus is not reached and
  records material trade-offs in the issue, pull request, or decision log.

## Releases and security

The maintainer controls release signing and registry credentials. The release
workflow and its verification gates are documented in [`RELEASE.md`](RELEASE.md).
Security scope and the current disclosure limitation are documented in
[`SECURITY.md`](SECURITY.md); the project does not yet advertise a separate
confidential reporting channel, so reporters should not place secrets in a
public issue.

## Continuity

The single-maintainer model is a bus-factor risk. The source, decisions,
conformance data, Nix deployment module, and tests are public under Apache-2.0,
so another maintainer can reproduce or fork the work. Adding a second release
maintainer requires a public governance change plus an explicit handover of
review and signing responsibilities; no such handover has happened yet.
