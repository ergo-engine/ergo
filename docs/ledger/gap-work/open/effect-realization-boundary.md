---
Authority: PROJECT
Date: 2026-03-16
Author: Sebastian (Architect) + Codex (Docs)
Status: OPEN
Gap-ID: GW-EFX
Resolved-By: ../../decisions/effect-dispatch-and-channel-roles.md
Related-Decisions: >-
  ../../decisions/v1-external-effect-intent-model.md,
  ../../decisions/intent-payload-shape.md,
  ../../decisions/intent-id-semantics.md,
  ../../decisions/egress-ack-model.md,
  ../../decisions/egress-routing-config.md,
  ../../decisions/egress-timing-lifecycle.md,
  ../../decisions/crash-consistency.md,
  ../../decisions/egress-failure-taxonomy.md,
  ../../decisions/egress-provenance.md
Unblocks: future multi-ingress host work; `ergo-init` workspace ergonomics
---

# GW-EFX: Effect Realization Follow-On Gaps

## Decision Landed

The doctrinal boundary questions that originally motivated this gap are
now decided by
[effect-dispatch-and-channel-roles.md](../../decisions/effect-dispatch-and-channel-roles.md).

That decision records that:

- Actions emit effect intent.
- Adapters declare the accepted effect contract.
- Host owns post-episode effect dispatch.
- Ingress and egress channels realize boundary I/O.
- Replay may re-realize host-internal effects for determinism, while
  truly external effects are re-derived and verified rather than
  re-executed against live systems.

The decision closes doctrine.

Nine follow-on decisions further resolve the concrete
architecture:

- [v1 external effect intent model](../../decisions/v1-external-effect-intent-model.md)
  — first-class intents, two-correlated-projections, manifest
  `effects.intents`, mirror writes, dispatch ordering, route-table
  ownership with ready-handshake capability attestation.
- [intent payload shape](../../decisions/intent-payload-shape.md) —
  typed fields (`Vec<IntentField>`), JSON projection at egress boundary.
- [intent ID semantics](../../decisions/intent-id-semantics.md) —
  deterministic SHA-256 derivation (`eid1:sha256:hex`).
- [egress acknowledgment model](../../decisions/egress-ack-model.md) —
  durable-accept, not completion.
- [egress routing config](../../decisions/egress-routing-config.md) —
  `EgressConfig` as canonical routing model.
- [egress timing and lifecycle](../../decisions/egress-timing-lifecycle.md)
  — start-before-run, per-step blocking, quiesce before capture
  finalization.
- [crash consistency](../../decisions/crash-consistency.md) —
  at-most-once host dispatch and recording-gap doctrine.
- [egress failure taxonomy](../../decisions/egress-failure-taxonomy.md)
  — typed interruption surface for in-run egress failures.
- [egress provenance](../../decisions/egress-provenance.md) —
  run-level `egress_provenance` hash, audit-only for replay.

These decisions, combined with implementation work on the
`feat/egress-surface` branch, now fully close `GW-EFX-3` and all of
its sub-rows. The archived branch ledger is
[egress-effect-work-plan.md](../../dev-work/closed/egress-effect-work-plan.md).

The only remaining open row in this gap file is `GW-EFX-2`.

## Remaining Open Gaps

### 1. Multi-Ingress Host Surface

Current canonical host APIs accept one ingress configuration per run
(`DriverConfig` in the current implementation, which is legacy naming
relative to the new doctrinal term *ingress channel*).

If a project needs multiple live feeds today, it must either:

- multiplex them upstream into one ingress channel, or
- wait for future host support for multiple ingress channel configs.

The doctrine decision intentionally does not choose between those two
host evolutions.

### 2. Egress Work Status — CLOSED

The entire `GW-EFX-3` lane is now closed:

- replay/live effect-path split
- effect classification and coverage
- external intent architecture and payload model
- dispatch plumbing
- acknowledgment contract
- routing configuration
- timing and lifecycle
- crash consistency doctrine
- failure taxonomy
- egress provenance

Those decisions and implementation passes are archived in the closed
[egress work plan](../../dev-work/closed/egress-effect-work-plan.md).

## Impact

The remaining gap blocks or constrains:

- workspace layouts that imply many concurrent live sources
- `ergo-init` ergonomics for channel discovery beyond the single-ingress
  case

## Non-Goals For This Gap

These follow-on gaps still do **not** pre-select:

- `stdin` as the required transport
- a specific protocol frame shape
- a specific adapter manifest field such as `realized_by`
- a fully generic user-defined effect dispatch runtime (v1 adds
  manifest-derived intents, not arbitrary plugin-style effect handlers)
- that egress must reuse the current `EffectHandler` trait unchanged

Those remain candidate solutions. The semantics decision is closed; the
concrete host/product surfaces are what remain open.

## Decision Ledger

<!-- markdownlint-disable MD013 -->
| ID | Task | Closure Condition | Owner | Status |
| ---- | ---- | ----------------- | ----- | ------ |
| GW-EFX-1 | Define canonical meaning of graph-produced effects | Closed by [effect-dispatch-and-channel-roles.md](../../decisions/effect-dispatch-and-channel-roles.md) | Claude + Sebastian | CLOSED |
| GW-EFX-1A | Define realization boundary | Closed by [effect-dispatch-and-channel-roles.md](../../decisions/effect-dispatch-and-channel-roles.md) | Claude + Sebastian | CLOSED |
| GW-EFX-1B | Define replay contract | Closed by [effect-dispatch-and-channel-roles.md](../../decisions/effect-dispatch-and-channel-roles.md) | Claude + Sebastian | CLOSED |
| GW-EFX-1C | Define end-user extension story | Closed by [effect-dispatch-and-channel-roles.md](../../decisions/effect-dispatch-and-channel-roles.md) | Sebastian | CLOSED |
| GW-EFX-2 | Define multi-ingress host direction | Decision or design record states whether canonical host remains single-ingress plus multiplexer, or gains multi-ingress request/config support, and what `ergo-init` may promise in the meantime | Sebastian | OPEN |
| GW-EFX-3 | Define egress-channel contract | Closed by 9 decision records, 1 remediation pass, final implementation for 3J + 3C, and archived work plan `../../dev-work/closed/egress-effect-work-plan.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3A | Replay effect-path split (prerequisite) | `execute_step` gates effect application by live-versus-replay mode; external handlers are never invoked during replay | Claude + Sebastian | CLOSED |
| GW-EFX-3B | Step-outcome-to-egress dispatch plumbing | Full pipeline implemented and hardened: canonical effect ownership routing, per-step blocking durable-accept ack, Option C artifact policy, quiesce-before-capture integrity, ready capability attestation, pending-ack invariant enforcement. | Claude + Sebastian | CLOSED |
| GW-EFX-3C | Egress channel provenance in capture | `egress_provenance: Option<String>` (`epv1:sha256:hex`). Full normalized config including timeouts. Audit-only for replay. See `decisions/egress-provenance.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3D | Effect-kind-to-egress routing configuration | Hybrid model decided. `EgressConfig` canonical, TOML file surface for v0. See `decisions/egress-routing-config.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3E | Egress run-phase timing | Start before first event, per-step blocking dispatch+ack, quiesce/stop egress before capture finalization to prevent post-capture ack drift. See `decisions/egress-timing-lifecycle.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3F | Effect-kind classification and coverage model | Classification decided by route-table ownership with ready-handshake capability attestation. Coverage widened (`egress_claimed_kinds` param). Conflict guard: both claim same kind → `ConflictingCoverage` at startup. Inline fork resolved. | Claude + Sebastian | CLOSED |
| GW-EFX-3G | External-effect dispatch architecture and intent model | Architecture decided. Payload shape decided. Intent ID decided. Types implemented. `mirror_writes[].from_field` validation implemented (ACT-32/ACT-33). See `decisions/intent-payload-shape.md`, `decisions/intent-id-semantics.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3H | Egress acknowledgment and result semantics | Durable-accept model decided. Host waits for durably-queued ack, not completion. Completion returns via ingress. See `decisions/egress-ack-model.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3I | Crash consistency and artifact policy | At-most-once host dispatch, egress-owned post-ack, recording gap documented, v2 exactness path scoped. See `decisions/crash-consistency.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3J | Egress failure and partial-apply semantics | Flat InterruptionReason variants (EgressAckTimeout, EgressProtocolViolation, EgressIo). Stop on first failure, partial acks preserved. See `decisions/egress-failure-taxonomy.md`. | Claude + Sebastian | CLOSED |
<!-- markdownlint-restore -->
