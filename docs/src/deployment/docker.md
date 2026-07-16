# Container Deployment Status

QueryFabric does not currently publish or support a Docker/Podman image. The
repository has no maintained `Dockerfile` or `Containerfile`, no Compose
deployment, no container-registry publication workflow, and no `oci-image`
flake output. In particular, there is no supported
`codeberg.org/caniko/queryfabric-demo:latest` image to pull.

The supported self-hosted deployment path today is the
[NixOS module](./self-hosting-nixos.md). It builds the `queryfabric-demo`
package, creates a hardened systemd unit, and passes the service's actual
configuration and credentials without placing secrets in the Nix store.

Operators may build the current binary with:

```console
$ nix build .#queryfabric-demo
```

Packaging that binary into a private container is presently operator-owned:
it is not covered by the repository's deployment contract or VM tests. The
process still requires operator-provided PostgreSQL, an authentication secret,
and optionally an S3-compatible object store; refer to the NixOS deployment
page and `crates/queryfabric-demo/src/config.rs` for the authoritative runtime
contract.

A container quick start should only be added after the repository supplies and
tests the foundational artifacts: a versioned image definition, a smoke-tested
Compose or Podman configuration, and a publication workflow naming an actual
registry artifact. Until then, container commands and environment-variable
tables would imply support that the project does not provide.
