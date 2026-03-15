---
Authority: PROJECT
Date: 2026-03-15
Author: Sebastian (Architect) + Codex (Docs)
Status: OPEN
Gap-ID: GW-EFX
Resolved-By: ../../decisions/effect-dispatch-and-channel-roles.md
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

### 2. Egress-Channel Lifecycle And Configuration

The doctrine now says egress channels are the correct prod boundary for
truly external effect realization. What remains open is the concrete
contract.

Still undefined:

- how host configures and launches egress channels
- whether one egress channel handles many effect kinds or one target
- what handshake/protocol/lifecycle they use
- what failure and interruption semantics apply
- how backpressure works
- what capture/replay artifacts are required for verification
- what, if anything, becomes manifest-visible at the adapter contract
  layer

## Impact

These remaining gaps block or constrain:

- workspace layouts that imply many concurrent live sources
- any bidirectional process-channel story
- any branch that wants to make egress channels first-class
- future host routing between host-internal handlers and true external
  channel realizations
- `ergo-init` ergonomics for channel discovery beyond the single-ingress
  case

## Non-Goals For This Gap

These follow-on gaps still do **not** pre-select:

- `stdin` as the required transport
- a specific protocol frame shape
- a specific adapter manifest field such as `realized_by`
- a kernel-generalized arbitrary effect-kind model

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
| GW-EFX-3 | Define egress-channel contract | Doctrine + dev work define configuration, lifecycle, routing, failure semantics, and replay/capture posture for egress channels without collapsing the kernel/prod boundary | Claude + Sebastian | OPEN |
<!-- markdownlint-restore -->
