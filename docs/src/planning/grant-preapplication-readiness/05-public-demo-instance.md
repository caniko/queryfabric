# Phase 05 — Stand Up and Prove the Public Demo

> **Recommended model: gpt-5.3-codex (codex) — effort `medium`**
>
> Routed: `carter route -c complex -r subagent --needs coding`
> → `gpt-5.3-codex` / `medium`
>
> Complex deployment work: the phase crosses NixOS module configuration,
> HTTPS, object-store-backed exports, and public smoke checks. A weaker model
> may hand-deploy the binary and accidentally invalidate the NixOS-module
> proof the grant needs.

## Working tree

`/data/nvme0/can/Projects/queryfabric` plus the deployment repository or host
configuration used by the maintainer. Prerequisites: Phase 01 evidence map,
Phase 03 public narrative validation, and Phase 04 release proof.

## Goal

This phase succeeds when a public HTTPS `queryfabric-demo` instance is deployed
from the NixOS module, smoke checks pass, and the application has a real demo
URL plus deployment evidence.

## Why this matters now

The draft application contains `[demo URL]` and `[URL]` placeholders. The demo
is the most compact proof that NixOS deployment, federation status, resources,
and sovereignty endpoints are real. Without a public URL, those claims must be
removed.

## Out of scope

- No hand-deployed binary as the cited proof.
- No secrets committed to this repository.
- No HA implementation beyond what the NixOS module already supports.
- No new product UI or admin panel.

## Plan

1. Choose the deployment source of truth: canix, host flake, or another NixOS
   repo. Record its path in the evidence map.
2. Deploy `queryfabric-demo` through `services.queryfabric` or
   `services.queryfabric.instances.<name>`.
3. Enable:
   - stable public base URL,
   - HTTPS reverse proxy,
   - federation mode,
   - object-store backed export path if the application cites export bundles.
4. Keep secrets outside the Nix store using the module's credential file
   pattern.
5. Run smoke checks:
   ```bash
   curl --fail https://<demo-host>/healthz
   curl --fail https://<demo-host>/federation/status
   curl --fail https://<demo-host>/resources
   ```
6. Optionally exercise one export endpoint if the application cites a live
   export demo.
7. Add the public URL, deployment config reference, and smoke-check commands to
   `docs/grants/claim-evidence-map.md`.
8. Replace demo placeholders in the application answers or remove demo claims.

## Acceptance criteria

- [ ] Public HTTPS URL exists.
- [ ] `/healthz`, `/federation/status`, and `/resources` return success.
- [ ] Deployment evidence points to NixOS module configuration, not a manual
      process.
- [ ] No secrets are committed.
- [ ] Application answers no longer contain `[demo URL]` or `[URL]` placeholders.

## Files likely touched

- External deployment repo or host configuration.
- `docs/grants/claim-evidence-map.md`
- `docs/grants/ngi-fediversity-application-answers.md`

## Pitfalls

- **Manual deployment invalidates claim.** Symptom: service is live but not
  module-deployed. Cause: faster manual path. Recovery: redeploy from NixOS
  module before citing it.
- **HTTPS works, federation endpoint does not.** Symptom: `/healthz` passes
  but `/federation/status` fails. Cause: federation env/module options missing.
  Recovery: fix module config or remove federation demo claim.
- **Secrets leak risk.** Symptom: deployment snippet contains credentials.
  Cause: copying full config into docs. Recovery: redact and cite the
  credential-file pattern.

## Reference

- `docs/src/deployment/self-hosting-nixos.md`
- `nix/modules/queryfabric.nix`
- `crates/queryfabric-demo/src/http.rs`
- `docs/grants/ideal-project-set.md`, Project 4.
