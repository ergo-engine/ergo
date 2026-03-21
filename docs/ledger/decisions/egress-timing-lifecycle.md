---
Authority: PROJECT
Date: 2026-03-16
Decision-Owner: Sebastian (Architect)
Participants: Claude (Structural Auditor), Codex (Ontology Guardian)
Status: DECIDED
Scope: v1
Parent-Decision: egress-ack-model.md
Resolves: GW-EFX-3E
---

# Decision: Egress Run-Phase Timing and Lifecycle

## Context

The durable-accept ack model (GW-EFX-3H) establishes that the host
waits for a bounded-latency durability acknowledgment from egress
after dispatching each intent. This decision defines when egress
processes start, when they stop, and how dispatch timing relates to
the step pipeline, capture-bundle finalization, and any follow-on
capture artifact write.

---

## The Fork

### Option (i) — Start at run start, stop after drain (chosen)

Egress processes launch when the run begins, before the first ingress
event. After the last step, host verifies no pending acks remain,
quiesces/stops egress with bounded shutdown, then finalizes a capture
bundle. If a higher-level path wants a capture artifact on disk, that
file is written from the finalized bundle afterward.

### Option (ii) — Lazy start on first intent (rejected)

Egress launches when the first intent is dispatched. Lower overhead
for intent-free runs. But first-intent latency includes process
startup, and startup failure occurs mid-run rather than at the
beginning.

**Rejected.** Startup failures should be caught before any events
are processed, not mid-run when state has already been committed.
The process startup cost is negligible (child process, same as
ingress).

### Option (iii) — Start at run start, no drain (rejected)

Egress launches at run start. After the last event, un-acked intents
are recorded as incomplete. No drain wait.

**Rejected.** Under per-step blocking (see below), there are no
un-acked intents at run end — each step waits for its acks. But even
structurally, "no drain" would mean the finalized capture result can't
guarantee all intents were durably accepted, which contradicts the
ack model.

---

## Ruling

### Lifecycle phases

**1. Startup (before first ingress event)**

All egress channels declared in the `EgressConfig` route table are
launched and handshaked before the first ingress event is processed.
Startup means:

- Spawn the egress process (per `EgressChannelConfig::Process`)
- Receive a readiness signal (protocol TBD — may mirror ingress
  `hello` message, or be implicit on first successful ack)
- If any egress channel fails to start, the run does not begin.
  Interruption reason: `DriverIo` with egress-specific detail string
  (explicit taxonomy deferred to 3a/GW-EFX-3J).

Ordering relative to ingress: egress channels start before or
simultaneously with ingress. No ordering dependency between them —
both must be ready before the first event.

**2. Per-step dispatch (during run)**

For each step (event processed by the graph):

1. Graph executes. Action emits effects.
2. Mirror writes applied via `SetContextHandler` (existing path).
3. External intent records dispatched to egress channel(s) per route
   table.
4. Host waits for durable-accept ack for each dispatched intent,
   bounded by `ack_timeout`.
5. Only after all acks for this step are received does the host
   proceed to the next step.

This is **per-step blocking.** The host does not process step N+1
until all intents from step N are durably accepted.

Why per-step blocking:

- **Causal clarity.** The capture artifact for step N includes its
  ack results. No ambiguity about which step an ack belongs to.
- **No backlog.** There is never a queue of un-acked intents building
  up across steps. The system is always in a known state.
- **Localized timeout.** If an ack times out, the interruption is
  attributed to a specific step and event, not to a batch.
- **Simpler implementation.** No async ack tracking, no ack-to-step
  correlation after the fact.

If ack timeout is reached for any intent in a step:

- The dispatch is considered failed.
- The run is interrupted.
- Capture artifact behavior is determined by the 2a inline fork
  (artifact policy on dispatch failure).

**3. End-of-run (after last ingress event)**

Under per-step blocking, no pending acks can exist at run end — each
step's acks were resolved before the next step. End-of-run is:

1. **Invariant check:** Assert zero pending acks. (This should always
   be true under per-step blocking. If violated, it's a bug.)
2. **Stop/quiesce egress channels.** Send shutdown signal (protocol TBD —
   may mirror ingress `end` message). Bounded graceful shutdown with
   timeout. If egress doesn't exit within the shutdown timeout, force
   kill.
3. **Finalize capture bundle.** All steps, all effects, and all ack
   records are finalized after egress is quiesced, so late channel
   frames cannot mutate truth after bundle finalization.
4. **Optional artifact write.** If the caller configured
   `capture_output`, write the capture artifact from that finalized
   bundle after host finalization returns.

Capture-bundle finalization happens AFTER egress quiesce/stop. This
freezes external channel activity before artifact finalization and
prevents post-capture ack drift.

### Replay behavior

During replay (`StepMode::Replay`):

- Egress channels are NOT launched.
- No intent dispatch occurs.
- No acks are expected.
- The host verifies intent records via `hash_effect()` on the
  `ActionEffect` (which includes `intents` field).
- Captured ack records are trusted, not re-verified.

This is already enforced by the `StepMode` gate from Phase 1
(GW-EFX-3A).

---

## What This Decides

- Egress starts before first ingress event; capture finalization occurs
  only after egress is quiesced/stopped.
- Per-step blocking: dispatch + wait for all acks before next step.
- No pending acks at run end (invariant under per-step blocking).
- Capture finalization order: all acks settled → quiesce/stop egress →
  finalize capture bundle → optional artifact write.
- Startup failure → run does not begin.
- Ack timeout → dispatch failure → interrupted run.
- Replay never contacts egress.

## What This Does NOT Decide

- **Egress process protocol details** (hello/end messages, readiness
  signal). Protocol design is part of 2a implementation.
- **Specific interruption reason variants.** Uses `DriverIo` with
  detail strings for now. Explicit taxonomy is 3a (GW-EFX-3J).
- **Shutdown timeout value.** Configuration detail for 2a.
- **Crash consistency model.** That's 2e — but this decision
  constrains it: the crash window is within a single step's
  dispatch-and-ack cycle.

---

## Impacted Files

- `runner.rs` — per-step dispatch + ack wait in `execute_step()`
  (live mode only, gated by `StepMode`)
- Host run/manual-finalization paths — egress startup before first
  event, quiesce/stop before capture-bundle finalization
- `usecases.rs` — egress lifecycle integration with `RunOutcome` and
  hosted-runner finalization
- Capture artifact write path — optional file write from the finalized
  bundle; ack records stored per-step alongside effects
