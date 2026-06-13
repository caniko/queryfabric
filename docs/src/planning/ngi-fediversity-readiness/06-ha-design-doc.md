# Phase 06 — Write the high-availability design document

> **Recommended model: gpt-5.4 (codex) — effort `medium`**
>
> Routed: `carter route -c complex -r subagent -n writing -p codex`
> → `gpt-5.4` / `medium`
>
> Complex: the document must reason architecturally about failure modes of a
> hub/cluster federation substrate it has to first understand from source
> (`queryfabric-cluster` health monitoring, DHT registry; `queryfabric-
> federation` message semantics), and must draw a defensible line between
> what composes today and what is grant-funded future work. A weaker tier
> would hand-wave "run two instances behind a load balancer" without
> addressing hub statefulness or schema-sync.
>
> Dispatch: `codex --model gpt-5.4 -c model_reasoning_effort=medium`

## Working tree

`/data/nvme0/can/Projects/queryfabric` (this repo, branch off `trunk`). No
phase prerequisite; `docs/src/SUMMARY.md` is shared with phases 01/04/05 —
rebase before landing.

## Goal

This phase succeeds when an HA design chapter exists that (a) documents how
stateless demo instances, health monitoring, and the federation registry
compose into a resilient deployment *today*, and (b) specifies hub failover,
NAT traversal, and schema-sync conflict handling as designed-but-unbuilt work
mapped to the grant's WP2 — with the today/planned line unambiguous in every
section.

## Why this matters now

The NGI Fediversity call text singles out high availability: *"Achieving high
availability scenario's is even more of a dark art."* A credible HA story is
therefore directly responsive to the call, and WP2 of the proposed grant
budget (federation & HA hardening, €11,250) needs a design artifact showing
the work is thought through, not invented for the application
(grant-readiness report §4 Tier 2 item 8, §6 WP2). Nothing in the repo
currently documents HA posture.

## Out of scope

- No code changes — this is a design document. Gaps found in
  `queryfabric-cluster`/`queryfabric-federation` are recorded, not fixed.
- No multi-instance NixOS module implementation (phase 07) — but DO
  cross-reference it as the deployment mechanism, phrased so the doc is
  correct whether or not 07 has landed yet ("the NixOS module supports / is
  gaining multi-instance deployment" → settle wording at landing time).
- No Kubernetes/other-orchestrator story; NixOS-first per project direction.
- No benchmarking or chaos testing.

## Plan

1. Branch from latest `trunk`.
2. Read the substrate: `crates/queryfabric-cluster/src/` (actor model, swarm
   bootstrap, DHT registry, health monitoring, resource routing),
   `crates/queryfabric-federation/src/` (Register, HealthPing, SchemaSync,
   ResourceAnnouncement, CatalogRequest, GetFlightEndpoint; hub/cluster actor
   skeletons; InMemoryTransport), `crates/queryfabric-demo` (FederationHost
   impl, statefulness — what state lives in-process vs Postgres vs S3), and
   `nix/modules/queryfabric.nix` (federation options: nodeName,
   hubMultiaddrs, flightPort).
3. Establish the factual baseline (verify in code, don't assume): which
   components are stateless; where the hub keeps registry state; what happens
   today when the hub dies, when a node misses HealthPings, when two nodes
   announce the same resource, when SchemaSync messages conflict or arrive
   out of order.
4. Write `docs/src/deployment/high-availability.md`:
   - **Deployment topologies**: single node (status quo); N stateless demo
     instances + shared Postgres/S3 behind a host-provided load balancer;
     federated multi-node with one hub. For each: what fails, what survives.
   - **What works today** (each claim cites crate/file): health monitoring,
     DHT registry, instance statelessness (if verified), restart behavior
     under systemd.
   - **Single points of failure today**: the hub; anything found in step 3.
     Name them plainly.
   - **Planned (WP2, grant-funded)**: hub failover strategy (e.g. standby hub
     + re-registration, or DHT-based hub election — present the considered
     options and the leading candidate with rationale), NAT traversal,
     schema-sync conflict resolution. Mark every item "planned, subject of an
     NGI Fediversity grant application".
   - **Operator guidance**: what an operator can deploy safely today and what
     SLO they should not promise yet.
5. Add the SUMMARY line under `# Deployment`:
   `- [High Availability](./deployment/high-availability.md)` (rebase —
   shared file).
6. Verify `mdbook build docs` exits 0.
7. One CHANGELOG line under Unreleased: "high-availability design
   documentation".
8. Commit (plain `git commit`, default signing).

## Acceptance criteria

- [ ] `docs/src/deployment/high-availability.md` exists; every "works today"
      claim cites a crate or file path; every unbuilt item is explicitly
      marked planned/WP2.
- [ ] The hub is explicitly identified as a current single point of failure
      (or, if code review disproves this, the actual behavior is documented
      with the code reference that proves it).
- [ ] All six federation message types are accounted for in the failure
      analysis.
- [ ] The doc cross-references the NixOS deployment chapter and the footprint
      chapter (if landed) without depending on them.
- [ ] `mdbook build docs` exits 0.

## Files likely touched

- `docs/src/deployment/high-availability.md` (new)
- `docs/src/SUMMARY.md` (one line; shared with 01/04/05 — rebase)
- `CHANGELOG.md` (one line)

## Pitfalls

- **Blurring today vs planned.** Symptom: a reviewer can't tell which HA
  properties exist. Cause: aspirational prose. Recovery: per-section "Today /
  Planned" labels; the grant application depends on this honesty (report §8
  risk 4 — claims will be checked against the repo).
- **Assuming statelessness.** Symptom: doc claims instances are stateless;
  demo actually holds in-memory session or job-queue state
  (`queryfabric-session`, `queryfabric-job-queue` exist — check whether the
  demo uses them in-process). Recovery: verify in step 3; if stateful,
  document the implication (sticky sessions or shared store) instead.
- **Designing too much.** Symptom: 10 pages of consensus-protocol design.
  Cause: solving WP2 instead of scoping it. Recovery: options + leading
  candidate + rationale, one page max for the planned section.
- **SUMMARY conflict.** Recovery: rebase, keep all additive lines.

## Reference

- Grant-readiness report §4 (Tier 2 item 8), §6 (WP2):
  `docs/grants/ngi-fediversity-application-plan.md`
- Plan set: `docs/src/planning/ngi-fediversity-readiness/README.md`
- Substrate code: `crates/queryfabric-cluster`, `crates/queryfabric-federation`
- Deployment mechanism: phase 07 (`07-multi-instance-nixos-module.md`)
