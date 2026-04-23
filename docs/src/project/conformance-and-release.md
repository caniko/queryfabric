# Conformance and Release

QueryFabric is intentionally narrower than "generic SQL everywhere."

The release standard is built around a verified portable subset and a public
conformance story.

## Conformance Inputs

The repo includes two public machine-readable assets:

- `conformance/portable-subset.json`
- `capabilities/builtin-capability-manifest.json`

Together they describe:

- which query patterns are in scope
- which capabilities those patterns require
- how built-in adapters advertise support

## Fuzzing

Parser and binder robustness are part of release quality, not an afterthought.

Fuzz harnesses live under `fuzz/`:

```bash
cd fuzz && cargo fuzz build parse_sql_no_panic
cd fuzz && cargo fuzz build bind_portable_no_panic
```

The seed corpora come from the public conformance corpus and SyQL differential
samples so fuzzing starts from real shapes rather than random junk alone.

## Release Flow

The release helper is `scripts/release.sh`.

Use it to:

- run the full non-publishing release gate
- publish crates in dependency order
- resume a partial publish from a specific crate
- tag a successful release locally

Read
[`RELEASE.md`](https://codeberg.org/caniko/queryfabric/src/branch/trunk/RELEASE.md)
and
[`COMPATIBILITY.md`](https://codeberg.org/caniko/queryfabric/src/branch/trunk/COMPATIBILITY.md)
for the exact policy around MSRV, CI gates, and staged publication order.
