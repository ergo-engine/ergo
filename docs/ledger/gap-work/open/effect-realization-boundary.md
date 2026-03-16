---
Authority: PROJECT
Date: 2026-03-15
Author: Sebastian (Architect) + Codex (Docs)
Status: OPEN
Gap-ID: GW-EFX
Resolved-By: ../../decisions/effect-dispatch-and-channel-roles.md
Related-Decisions: ../../decisions/v1-external-effect-intent-model.md
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

A follow-on design decision — the
[v1 external effect intent model](../../decisions/v1-external-effect-intent-model.md)
— further resolves the concrete
architecture for external effect intents. It establishes:

- External intents (e.g. `place_order`) are first-class effect kinds,
  not context writes.
- One action attempt emits two correlated projections sharing an
  `intent_id`: an optional internal mirror write (via `set_context`)
  and an external intent record (forwarded to egress).
- Action manifests gain an `effects.intents` section; emission is
  manifest-derived, not implementation-emitted.
- Mirror writes are applied first; egress dispatch follows. Mirror
  failure blocks dispatch.
- Classification of internal vs external is resolved by the egress
  process handshake: if an egress channel claims an effect kind, it is
  external.

This partially resolves `GW-EFX-3F` and `GW-EFX-3G` and constrains the
design space for `GW-EFX-3B`, `GW-EFX-3D`, and `GW-EFX-3H`.

The remaining rows below are the product, host, and kernel-adjacent
follow-on questions that doctrine and the intent model decision do not
fully settle.

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

### 2. Replay Effect-Path Split (Prerequisite)

The decision record says host-internal effects may be replay-realized,
but truly external effects must not be re-executed against live systems.
Current host code does not yet enforce that split.

Today, live and replay both pass through the same
[`HostedRunner::execute_step()`](../../../../crates/prod/core/host/src/runner.rs)
path:

- [`HostedRunner::step()`](../../../../crates/prod/core/host/src/runner.rs)
  delegates to `execute_step()`
- [`HostedRunner::replay_step()`](../../../../crates/prod/core/host/src/runner.rs)
  delegates to `execute_step()`
- [`HostedRunner::execute_step()`](../../../../crates/prod/core/host/src/runner.rs)
  drains effects and applies registered handlers identically in both
  modes

Today this is safe because
[`HostedRunner::new()`](../../../../crates/prod/core/host/src/runner.rs)
only registers `SetContextHandler`, which is host-internal and safe to
replay.

The moment a true external egress handler is registered, replay will
invoke it too unless the shared step path is explicitly gated by
live-versus-replay effect-application mode.

This is a prerequisite for any egress-channel work. It must be resolved
before, or at the same time as, external egress support.

### 3. Egress-Channel Lifecycle And Configuration

The doctrine now says egress channels are the correct prod boundary for
truly external effect realization. What remains open is the concrete
contract.

#### Prerequisite

`GW-EFX-3A` blocks all egress-channel delivery work. `GW-EFX-3F` and
`GW-EFX-3G` are also foundational — the v1 external effect intent model
decision resolves their architectural direction (egress
handshake for classification, two-correlated-projections for dispatch),
but implementation and the sub-decisions listed in that record
(payload shape, correlation semantics, route-table schema) remain open.

Still undefined:

- how host configures and launches egress channels
- whether one egress channel handles many effect kinds or one target
- what handshake/protocol/lifecycle they use
- what failure and interruption semantics apply
- how backpressure works
- what capture/replay artifacts are required for verification
- what, if anything, becomes manifest-visible at the adapter contract
  layer
- step-outcome-to-egress dispatch plumbing: the v1 intent model
  decision defines the mechanism: host applies mirror writes
  via `SetContextHandler`, then forwards external intent records to
  egress. **Still open:** the current host run loops in
  [`usecases.rs`](../../../../crates/prod/core/host/src/usecases.rs)
  do not implement this plumbing. No wiring exists yet to turn
  post-step intent output into egress-channel input.
- egress channel provenance in capture artifacts: capture bundles track
  adapter provenance and runtime provenance today, but no egress channel
  identity/provenance. Replay has no mechanism to verify that the same
  egress contract was in place.
- effect-kind-to-egress-channel routing configuration: no user-facing
  surface exists to declare which effect kinds route to which egress
  channels. This mapping is not currently defined in the adapter
  manifest, the host run request, or thin-client flags.
- run-phase timing: host lifecycle does not yet define when an egress
  process starts, whether it is long-lived for the run, or whether host
  must wait for queued egress work to drain before writing the capture
  bundle after ingress completion.
- effect-kind classification: doctrine distinguishes host-internal
  versus truly external effects. The v1 intent model decision
  resolves the classification mechanism: the egress process handshake
  declares which effect kinds it handles, and that declaration is the
  classification. **Still open:** coverage validation must be updated
  to treat egress-claimed kinds as satisfied without requiring an
  in-process `EffectHandler` registration, and the system must enforce
  a startup guarantee that no run begins if an emittable intent kind
  lacks egress coverage.
- external-effect dispatch architecture and intent model: the v1 intent
  model decision establishes the architecture: action manifests
  declare `effects.intents`, runtime emits intent records from manifest
  declarations, host applies optional `mirror_writes` via
  `SetContextHandler` then forwards the external intent to egress.
  The existing `EffectHandler` trait stays for internal writes.
  **Still open:** `ActionEffect` data model extension (payload for
  non-write intents), `intent_id` correlation semantics, and
  `mirror_writes[].from_field` registration-time validation (every
  `from_field` must reference a declared intent field in the same
  `intents` entry).
- egress acknowledgment and result semantics: the host has no defined
  contract for what an egress channel must acknowledge, whether the
  host waits for delivery acceptance versus completed external work, or
  whether downstream confirmations return later as ordinary ingress
  events.
- crash consistency, artifact policy, and partial-apply semantics: the
  host currently hard-fails on step errors and explicitly does no
  rollback on effect application. Egress design still needs a doctrine
  and product rule for retry/idempotency, duplicate-delivery windows,
  capture finalization on dispatch failure, and whether partial egress
  delivery is acceptable.
- egress failure taxonomy: current interruption reasons are ingress
  centric. No outcome vocabulary yet distinguishes egress launch,
  protocol, I/O, delivery, or drain failures in a way users can rely
  on.

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
| GW-EFX-3A | Replay effect-path split (prerequisite) | `execute_step` gates effect application by live-versus-replay mode; external handlers are never invoked during replay | Claude + Sebastian | OPEN |
| GW-EFX-3B | Step-outcome-to-egress dispatch plumbing | Host applies mirror writes via `SetContextHandler`, then forwards external intent records to egress process(es); mirror failure blocks dispatch; dispatch failure behavior defined per artifact policy (inline fork, decided during implementation) | Claude + Sebastian | OPEN |
| GW-EFX-3C | Egress channel provenance in capture | Capture bundle includes egress channel identity; replay validates it | Claude + Sebastian | OPEN |
| GW-EFX-3D | Effect-kind-to-egress routing configuration | User-facing surface exists to declare effect-kind to egress-channel mapping | Claude + Sebastian | OPEN |
| GW-EFX-3E | Egress run-phase timing | Host lifecycle defines egress start/stop/drain ordering relative to run completion and capture write | Claude + Sebastian | OPEN |
| GW-EFX-3F | Effect-kind classification and coverage model | Classification mechanism decided (egress handshake declares external kinds). Remaining: coverage validation accepts egress-claimed kinds as covered without in-process `EffectHandler`; startup invariant enforced (no run starts if emittable intent kind lacks egress coverage) | Claude + Sebastian | OPEN |
| GW-EFX-3G | External-effect dispatch architecture and intent model | Architecture decided (two-correlated-projections, manifest `effects.intents`, `mirror_writes`, `SetContextHandler` for internal, separate dispatch for egress). Remaining: `ActionEffect` data model extension, `intent_id` correlation semantics, `mirror_writes[].from_field` registration-time validation | Claude + Sebastian | OPEN |
| GW-EFX-3H | Egress acknowledgment and result semantics | Host/egress contract defines what gets acknowledged, what counts as delivery versus completion, and whether later confirmations return via ingress | Claude + Sebastian | OPEN |
| GW-EFX-3I | Crash consistency and artifact policy | Delivery guarantee, retry/idempotency stance, and capture-finalization behavior are defined for host crash or dispatch failure windows | Claude + Sebastian | OPEN |
| GW-EFX-3J | Egress failure and partial-apply semantics | User-visible failure taxonomy and partial-delivery/atomicity rules are defined for egress execution and drain phases | Claude + Sebastian | OPEN |
<!-- markdownlint-restore -->
