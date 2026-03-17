---
Authority: PROJECT
Date: 2026-03-16
Author: Sebastian (Architect) + Codex (Docs)
Status: CLOSED
Gap-ID: GW-EFX
Resolved-By: >-
  ../../decisions/effect-dispatch-and-channel-roles.md,
  ../../decisions/multi-ingress-host-direction.md
Related-Decisions: >-
  ../../decisions/multi-ingress-host-direction.md,
  ../../decisions/v1-external-effect-intent-model.md,
  ../../decisions/intent-payload-shape.md,
  ../../decisions/intent-id-semantics.md,
  ../../decisions/egress-ack-model.md,
  ../../decisions/egress-routing-config.md,
  ../../decisions/egress-timing-lifecycle.md,
  ../../decisions/crash-consistency.md,
  ../../decisions/egress-failure-taxonomy.md,
  ../../decisions/egress-provenance.md
Unblocks: `ergo-init` workspace ergonomics
---

# GW-EFX: Effect Realization Follow-On Gaps

## Closure Summary

This gap file is now fully closed.

- `GW-EFX-1`, `GW-EFX-1A`, `GW-EFX-1B`, and `GW-EFX-1C` closed by
  [effect-dispatch-and-channel-roles.md](../../decisions/effect-dispatch-and-channel-roles.md).
- `GW-EFX-3` and all `GW-EFX-3*` sub-rows closed by the egress
  decision set plus implementation archived in
  [egress-effect-work-plan.md](../../dev-work/closed/egress-effect-work-plan.md).
- `GW-EFX-2` closed by
  [multi-ingress-host-direction.md](../../decisions/multi-ingress-host-direction.md):
  canonical host remains single-ingress; projects needing multiple live
  feeds multiplex upstream into one ingress channel; `ergo-init` may
  promise exactly one ingress source per profile.

## Final Resolution of `GW-EFX-2`

Current canonical host APIs accept one ingress configuration per run
(`DriverConfig` in the current implementation, which is legacy naming
relative to the doctrinal term *ingress channel*).

That is now a deliberate v1 product boundary, not an undecided future
fork:

- one run/request/profile resolves to exactly one ingress source
- fixture and process ingress remain the supported host-owned shapes
- projects that need multiple live feeds compose them upstream inside
  one ingress channel process
- `ergo-init` may standardize one ingress source per profile and may
  document the multiplexer pattern, but it must not promise host-owned
  multi-ingress launch or discovery

## Archived Egress Resolution

The concrete effect/egress architecture is now fully decided by the
following records:

- [v1 external effect intent model](../../decisions/v1-external-effect-intent-model.md)
- [intent payload shape](../../decisions/intent-payload-shape.md)
- [intent ID semantics](../../decisions/intent-id-semantics.md)
- [egress acknowledgment model](../../decisions/egress-ack-model.md)
- [egress routing config](../../decisions/egress-routing-config.md)
- [egress timing and lifecycle](../../decisions/egress-timing-lifecycle.md)
- [crash consistency](../../decisions/crash-consistency.md)
- [egress failure taxonomy](../../decisions/egress-failure-taxonomy.md)
- [egress provenance](../../decisions/egress-provenance.md)

Those decisions, combined with implementation work on the
`feat/egress-surface` branch, closed `GW-EFX-3` and all of its
sub-rows. The archived branch ledger is
[egress-effect-work-plan.md](../../dev-work/closed/egress-effect-work-plan.md).

## Archived Impact

This gap originally mattered because it constrained:

- workspace layouts that imply many concurrent live sources
- `ergo-init` ergonomics for channel discovery beyond the single-ingress
  case

Those constraints are now resolved by keeping the host surface
single-ingress and moving many-source composition into ingress-channel
implementations.

## Non-Goals Of The Final Ruling

The closing decision does not define:

- a required transport between a multiplexer ingress channel and its
  upstream sources
- a built-in Ergo multiplexer implementation
- any host-level multi-ingress request/config surface for v1
- future reconsideration of host multi-ingress; that would require a
  new gap and decision record

## Decision Ledger

<!-- markdownlint-disable MD013 -->
| ID | Task | Closure Condition | Owner | Status |
| ---- | ---- | ----------------- | ----- | ------ |
| GW-EFX-1 | Define canonical meaning of graph-produced effects | Closed by [effect-dispatch-and-channel-roles.md](../../decisions/effect-dispatch-and-channel-roles.md) | Claude + Sebastian | CLOSED |
| GW-EFX-1A | Define realization boundary | Closed by [effect-dispatch-and-channel-roles.md](../../decisions/effect-dispatch-and-channel-roles.md) | Claude + Sebastian | CLOSED |
| GW-EFX-1B | Define replay contract | Closed by [effect-dispatch-and-channel-roles.md](../../decisions/effect-dispatch-and-channel-roles.md) | Claude + Sebastian | CLOSED |
| GW-EFX-1C | Define end-user extension story | Closed by [effect-dispatch-and-channel-roles.md](../../decisions/effect-dispatch-and-channel-roles.md) | Sebastian | CLOSED |
| GW-EFX-2 | Define multi-ingress host direction | Closed by [multi-ingress-host-direction.md](../../decisions/multi-ingress-host-direction.md): canonical host remains single-ingress; projects needing multiple live feeds multiplex upstream into one ingress channel; `ergo-init` may promise exactly one ingress source per profile. | Sebastian | CLOSED |
| GW-EFX-3 | Define egress-channel contract | Closed by 9 decision records, 1 remediation pass, final implementation for 3J + 3C, and archived work plan `../../dev-work/closed/egress-effect-work-plan.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3A | Replay effect-path split (prerequisite) | `execute_step` gates effect application by live-versus-replay mode; external handlers are never invoked during replay | Claude + Sebastian | CLOSED |
| GW-EFX-3B | Step-outcome-to-egress dispatch plumbing | Full pipeline implemented and hardened: canonical effect ownership routing, per-step blocking durable-accept ack, Option C artifact policy, quiesce-before-capture integrity, ready capability attestation, pending-ack invariant enforcement. | Claude + Sebastian | CLOSED |
| GW-EFX-3C | Egress channel provenance in capture | `egress_provenance: Option<String>` (`epv1:sha256:hex`). Full normalized config including timeouts. Audit-only for replay. See `decisions/egress-provenance.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3D | Effect-kind-to-egress routing configuration | Hybrid model decided. `EgressConfig` canonical, TOML file surface for v0. See `decisions/egress-routing-config.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3E | Egress run-phase timing | Start before first event, per-step blocking dispatch+ack, quiesce/stop egress before capture finalization to prevent post-capture ack drift. See `decisions/egress-timing-lifecycle.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3F | Effect-kind classification and coverage model | Classification decided by route-table ownership with ready-handshake capability attestation. Coverage widened (`egress_claimed_kinds` param). Conflict guard: both claim same kind -> `ConflictingCoverage` at startup. Inline fork resolved. | Claude + Sebastian | CLOSED |
| GW-EFX-3G | External-effect dispatch architecture and intent model | Architecture decided. Payload shape decided. Intent ID decided. Types implemented. `mirror_writes[].from_field` validation implemented (ACT-32/ACT-33). See `decisions/intent-payload-shape.md`, `decisions/intent-id-semantics.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3H | Egress acknowledgment and result semantics | Durable-accept model decided. Host waits for durably-queued ack, not completion. Completion returns via ingress. See `decisions/egress-ack-model.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3I | Crash consistency and artifact policy | At-most-once host dispatch, egress-owned post-ack, recording gap documented, v2 exactness path scoped. See `decisions/crash-consistency.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3J | Egress failure and partial-apply semantics | Flat InterruptionReason variants (EgressAckTimeout, EgressProtocolViolation, EgressIo). Stop on first failure, partial acks preserved. See `decisions/egress-failure-taxonomy.md`. | Claude + Sebastian | CLOSED |
<!-- markdownlint-restore -->
