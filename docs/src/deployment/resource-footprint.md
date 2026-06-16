# Resource Footprint

This benchmark measures the `queryfabric-demo` service process from a
release-profile Nix build. The object store is kept in `memory` so the demo
does not need MinIO; the demo still uses a local temp Postgres cluster
because the service requires a database URL to boot.

## Methodology

Run the benchmark from the repository root:

```console
$ scripts/footprint.sh
```

The script:

- builds `queryfabric-demo` with `nix build .#queryfabric-demo`
- measures the packaged release binary size with `du -h`
- measures the Nix closure size with `nix path-info -S`
- starts a temporary Postgres cluster under the current Unix user
- times cold start from spawn to first `GET /healthz`
- warms the service with one representative `/query` call, then samples
  `VmRSS` after a 40-second settle window and reports the median of five
  samples
- drives a simple concurrent `/query` loop and records the peak `VmRSS`

Build provenance:

- measured command: `nix develop -c scripts/footprint.sh`
- release-profile build inside the script: `nix build .#queryfabric-demo`
- package output: `/nix/store/l7w5zchfzwfg2gizr5gd3kf9rhfmizvi-queryfabric-demo-0.2.0`
- workspace version: `0.2.0`
- git identity at measurement time: local `HEAD` `57066d8`, with no local
  `v0.2.0` tag and a dirty worktree
- workspace Rust floor: `rust-version = "1.88"` in `Cargo.toml`

Measurement host:

- host: `atlas`
- kernel: `Linux 7.0.12-cachyos-lto`
- CPU: `AMD Ryzen 9 9950X3D 16-Core Processor`
- logical cores: `32`
- RAM: `60 GiB`

## Results

| Metric | Value | Notes |
| --- | ---: | --- |
| Release binary size | 17M | `du -h` on the packaged release binary |
| Nix closure size | 63MiB | `nix path-info -S` for the package output |
| Cold-start median (5 runs) | 472 ms | spawn to first successful `GET /healthz` |
| Idle RSS median (5 runs) | 2912 KiB | `VmRSS` after warmup and a 40-second settle window |
| Under-load peak RSS | 14664 KiB | peak `VmRSS` during 8 concurrent `POST /query` workers |

## Sizing

The demo process itself is small: its peak under the simple concurrent query
loop is about 14.3 MiB RSS, and its idle footprint settles around 2.9 MiB.
For the service process alone, a `1 vCPU / 128 MiB` VPS is enough with
margin.

This repo’s NixOS module also expects a local Postgres instance. For the full
single-box stack, `1 vCPU / 512 MiB` is the safer floor; `1 vCPU / 1 GiB`
gives comfortable headroom for the demo, Postgres, and the OS.
