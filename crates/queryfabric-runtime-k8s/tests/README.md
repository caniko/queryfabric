# syndb-runtime-k8s kind smoke test

`kind_smoke.rs` is a manual, ignored integration test for the Kubernetes
isolated execution path. It creates a disposable `kind` cluster named
`burst-smoke`, loads the locally built burst-worker OCI archive, applies the
burst-worker RBAC/config templates from the `syndb-clickhouse` chart, spawns a
worker through `K8sIsolatedDriver`, and asserts that `SELECT 1` yields one Arrow
`RecordBatch`.

Prerequisites:

- Docker-backed `kind`
- `kubectl`
- `helm`
- Nix
- a prebuilt burst-worker image archive at the repository root:

```sh
nix build .#oci-syndb-burst-worker
```

Run:

```sh
cargo test -p syndb-runtime-k8s --features integration-k8s -- --ignored kind_smoke
```

The test deletes any pre-existing `burst-smoke` cluster before starting. On
success it deletes the cluster. On failure it intentionally leaves the cluster
running for inspection:

```sh
kubectl --context kind-burst-smoke get pods
kubectl --context kind-burst-smoke logs -l app.kubernetes.io/name=syndb-burst-worker
kind delete cluster --name burst-smoke
```

Set `SYNDB_BURST_WORKER_IMAGE_ARCHIVE=/path/to/archive.tar` to load an image
archive from a path other than `./result`.
