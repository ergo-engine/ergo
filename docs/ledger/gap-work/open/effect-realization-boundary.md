---
Authority: PROJECT
Date: 2026-03-15
Author: Sebastian (Architect) + Codex (Docs)
Status: OPEN
Gap-ID: GW-EFX
Resolved-By: ../../decisions/effect-dispatch-and-channel-roles.md
Related-Decisions: ../../decisions/v1-external-effect-intent-model.md, ../../decisions/intent-payload-shape.md, ../../decisions/intent-id-semantics.md
Unblocks: future multi-ingress host work; future egress-channel design; `ergo-init` workspace ergonomics
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

Three follow-on design decisions further resolve the concrete
architecture:

- [v1 external effect intent model](../../decisions/v1-external-effect-intent-model.md)
  — first-class intents, two-correlated-projections, manifest
  `effects.intents`, mirror writes, dispatch ordering, route-table
  ownership with ready-handshake capability attestation.
- [intent payload shape](../../decisions/intent-payload-shape.md) —
  typed fields (`Vec<IntentField>`), JSON projection at egress boundary.
- [intent ID semantics](../../decisions/intent-id-semantics.md) —
  deterministic SHA-256 derivation (`eid1:sha256:hex`).

These decisions, combined with implementation work on the
`feat/egress-surface` branch, fully close `GW-EFX-3A`, `GW-EFX-3F`,
and `GW-EFX-3G` (Phase 1 of the
[egress work plan](../../dev-work/open/egress-effect-work-plan.md)).

The remaining rows below are the Phase 2 and Phase 3 questions that
these decisions constrain but do not fully settle.

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

### 2. Replay Effect-Path Split (Prerequisite) — RESOLVED

`StepMode` enum (`Live` | `Replay`) added to `runner.rs`.
`step()` passes `Live`, `replay_step()` passes `Replay`,
`execute_step()` accepts the mode parameter. When external dispatch
is added, the gate is a single `match mode` branch.
See `GW-EFX-3A` (CLOSED) in the ledger below.

### 3. Egress-Channel Lifecycle And Configuration

The doctrine now says egress channels are the correct prod boundary for
truly external effect realization. What remains open is the concrete
contract.

#### Prerequisite — SATISFIED

Phase 1 of the
[egress work plan](../../dev-work/open/egress-effect-work-plan.md) is
complete. `GW-EFX-3A` (replay split), `GW-EFX-3F` (coverage), and
`GW-EFX-3G` (architecture + types) are all CLOSED. The foundational
code and decisions are in place. The items below are Phase 2/3 work.

Phase 3 remediation closed six critical implementation gaps in one pass:

- canonical effect stream emission + ownership routing
- metadata truth guard for intent IDs (`execute`/`run` reject
  intent-emitting graphs without metadata)
- mirror-write coverage/composition alignment
- startup intent-schema compatibility checks (fail-closed)
- ack lifecycle integrity (real pending-ack invariant + quiesce on
  timeout/protocol/I/O failures)
- ready-handshake capability attestation (`protocol` +
  `handled_kinds`)

Still undefined:

- **GW-EFX-3C (open):** egress provenance in capture/replay contract
  (identity granularity, storage shape, replay strictness).
- **GW-EFX-3J (open):** user-facing egress failure taxonomy and
  partial-apply semantics beyond current detail-string interruption
  surface.

## Impact

These remaining gaps block or constrain:

- workspace layouts that imply many concurrent live sources
- replay safety when external effect handlers are introduced
  (`GW-EFX-3A`)
- any bidirectional process-channel story
- any branch that wants to make egress channels first-class
- future host routing between host-internal handlers and true external
  channel realizations
- any replay-safe external effect story beyond `set_context`
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
| GW-EFX-3 | Define egress-channel contract | `GW-EFX-3A` through `GW-EFX-3J` are closed, with `GW-EFX-3A` resolved before delivery work and `GW-EFX-3F` through `GW-EFX-3G` resolved before routing/coverage closure | Claude + Sebastian | OPEN |
| GW-EFX-3A | Replay effect-path split (prerequisite) | `execute_step` gates effect application by live-versus-replay mode; external handlers are never invoked during replay | Claude + Sebastian | CLOSED |
| GW-EFX-3B | Step-outcome-to-egress dispatch plumbing | Full pipeline implemented and hardened: canonical effect ownership routing, per-step blocking durable-accept ack, Option C artifact policy, quiesce-before-capture integrity, ready capability attestation, pending-ack invariant enforcement. | Claude + Sebastian | CLOSED |
| GW-EFX-3C | Egress channel provenance in capture | Capture bundle includes egress channel identity; replay validates it | Claude + Sebastian | OPEN |
| GW-EFX-3D | Effect-kind-to-egress routing configuration | Hybrid model decided. `EgressConfig` canonical, TOML file surface for v0. See `decisions/egress-routing-config.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3E | Egress run-phase timing | Start before first event, per-step blocking dispatch+ack, quiesce/stop egress before capture finalization to prevent post-capture ack drift. See `decisions/egress-timing-lifecycle.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3F | Effect-kind classification and coverage model | Classification decided by route-table ownership with ready-handshake capability attestation. Coverage widened (`egress_claimed_kinds` param). Conflict guard: both claim same kind → `ConflictingCoverage` at startup. Inline fork resolved. | Claude + Sebastian | CLOSED |
| GW-EFX-3G | External-effect dispatch architecture and intent model | Architecture decided. Payload shape decided. Intent ID decided. Types implemented. `mirror_writes[].from_field` validation implemented (ACT-32/ACT-33). See `decisions/intent-payload-shape.md`, `decisions/intent-id-semantics.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3H | Egress acknowledgment and result semantics | Durable-accept model decided. Host waits for durably-queued ack, not completion. Completion returns via ingress. See `decisions/egress-ack-model.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3I | Crash consistency and artifact policy | At-most-once host dispatch, egress-owned post-ack, recording gap documented, v2 exactness path scoped. See `decisions/crash-consistency.md`. | Claude + Sebastian | CLOSED |
| GW-EFX-3J | Egress failure and partial-apply semantics | User-visible failure taxonomy and partial-delivery/atomicity rules are defined for egress execution and drain phases | Claude + Sebastian | OPEN |
<!-- markdownlint-restore -->
