---
Authority: PROJECT
Date: 2026-03-16
Decision-Owner: Sebastian (Architect)
Participants: Claude (Structural Auditor), Codex (Ontology Guardian)
Status: DECIDED
Scope: v1
Parent-Decision: v1-external-effect-intent-model.md
Resolves: GW-EFX-3H
---

# Decision: Egress Acknowledgment and Result Semantics

## Context

The v1 external effect intent model establishes that actions emit
external intents (e.g., `place_order`) which are dispatched to egress
processes after mirror writes are applied. This decision defines what
the host waits for from the egress process after dispatching an intent,
and how completion truth eventually returns to the system.

This is the keystone Phase 2 decision. It shapes:
- GW-EFX-3E (egress timing / lifecycle)
- GW-EFX-3I (crash consistency / delivery model)
- GW-EFX-3B (dispatch plumbing, particularly timeout and interruption)
- GW-EFX-3J (failure taxonomy)

---

## The Fork

### Option A — Received ("got it")

Host sends intent record to egress. Egress responds "received." Host
moves on.

- Step latency: minimal (local process round-trip)
- Capture: records intent + received-ack
- Replay: verifies same intent produced, same ack received
- Risk: "received" says nothing about whether the intent will survive
  an egress process crash. The intent could be silently lost between
  receipt and execution.

**Rejected.** Receipt without durability is a false promise. The user
thinks the intent is safe when it isn't.

### Option B — Completed ("done, here's the result")

Host sends intent to egress. Egress executes the external action,
waits for the result, and returns it. Host blocks until completion
or timeout.

- Step latency: unbounded (depends on external system)
- Capture: records intent + full completion result
- Replay: completion result is non-deterministic (broker may return
  different fill price, timing, partial fill). Replay comparison
  becomes ambiguous.
- Risk: step pipeline blocks on external latency. Multiple events
  queue behind a slow external call.

**Rejected.** Unbounded step latency violates the design principle
that graph execution is fast and deterministic. External completion
is observational truth that belongs in future events, not in the
dispatch path.

### Option C — Fire-and-forget ("sent, don't wait")

Host sends intent to egress. Doesn't wait for any response. Moves
on immediately. Result arrives later as an ingress event.

- Step latency: zero
- Capture: records intent only. No ack.
- Replay: verifies intent produced. Cannot verify delivery.
- Risk: if egress process crashed or pipe broke, intent is silently
  lost with no indication to the host or user.

**Rejected.** No delivery signal at all is unacceptable. The user
cannot distinguish "intent dispatched" from "intent lost" without
building external monitoring infrastructure.

### Option D — Durable-accept ("queued, I won't lose it") — CHOSEN

Host sends intent to egress. Egress confirms the intent is durably
accepted — meaning the egress process has taken responsibility for
the intent such that a process crash after acknowledgment will not
silently drop it. Host moves on. Completion truth returns later as
an ingress event keyed by `intent_id`.

- Step latency: bounded (egress confirms local acceptance, not
  external completion)
- Capture: records intent + durable-accept ack
- Replay: verifies same intent produced. Does not re-contact egress.
- User experience: two-phase — immediate "accepted" + eventual
  "filled/rejected" as future observation

---

## Ruling

**Option D — durable-accept.**

### Why it fits the ontology

The two-correlated-projections model already separates decision-state
(mirror write) from external action (intent dispatch). Adding a third
category — completion truth — is the natural extension:

1. **Mirror write** = what the system decided (applied before dispatch)
2. **Durable-accept** = the outside world received the decision
   (bounded-latency confirmation)
3. **Completion** = what actually happened (arrives later as
   observation via ingress)

The host's responsibility ends at (2). The graph observes (3) as
future data, same as any other external event.

### What "durable" means

"Durable" is a contracted acceptance level, not merely "stdout ack
seen." It means: the egress process has accepted responsibility for
the intent such that a process crash after acknowledgment will not
silently drop the intent.

For v0, this means the egress process must have persisted or durably
queued the intent before sending the ack. Reading from stdin and
printing an ack is NOT sufficient — that's Option A (received), not
Option D (durable-accept).

The specific durability mechanism is egress-owned (WAL, queue,
database, etc.). The host does not prescribe the implementation —
it prescribes the contract: after ack, the intent survives process
restart.

### Ack protocol

The egress process sends a JSON ack over its communication channel:

```json
{
  "type": "intent_ack",
  "intent_id": "eid1:sha256:...",
  "status": "accepted",
  "acceptance": "durable"
}
```

Required fields:
- `type` — must be `"intent_ack"`
- `intent_id` — must match the dispatched intent's `intent_id`
- `status` — must be `"accepted"`
- `acceptance` — must be `"durable"`

Optional fields:
- `egress_ref` — external correlation ID (e.g., broker order ID).
  Opaque to the host. Stored in capture artifact for provenance.

Timestamps are optional and non-normative for replay purposes.

### Timeout semantics

If the egress process does not send a valid `intent_ack` within the
configured `ack_timeout`:

1. The dispatch is considered failed.
2. The run is interrupted.
3. The interruption reason maps to existing `InterruptionReason`
   taxonomy short-term (likely `DriverIo` or a new
   `EgressAckTimeout` variant — resolved in GW-EFX-3J).
4. Capture artifact behavior (partial write, full write, no write)
   is resolved by the 2a inline fork (artifact policy on dispatch
   failure).

The `ack_timeout` is a configuration value, not a protocol constant.
It belongs in the egress routing configuration (2c).

### Replay behavior

During replay (`StepMode::Replay`):

1. The host does NOT contact the egress process.
2. The host verifies that the same `IntentRecord` was produced
   (via `hash_effect()` on the full `ActionEffect`, which includes
   the `intents` field).
3. The captured durable-accept ack is trusted — it is not re-verified
   against a live egress process.
4. This is consistent with the replay doctrine: external effects are
   verified for determinism, not re-executed.

The `StepMode` gate (Phase 1, GW-EFX-3A) is the enforcement point.

### Completion feedback path

Completion truth (order filled, order rejected, etc.) returns to the
system as a future ingress event. The ingress event carries the
`intent_id` as a correlation key, allowing the graph to match the
completion to the original intent.

This is not a new mechanism — it's a standard ingress event processed
by the graph like any other external data. The graph author decides
how to handle it (update state, trigger further actions, etc.).

The completion feedback path is NOT part of this decision. It is
standard ingress behavior that already exists. This decision only
establishes that completion does NOT arrive via the egress ack.

---

## What This Decides

- Host waits for durable-accept ack from egress, not completion.
- "Durable" means intent survives egress process crash after ack.
- Ack payload: `type`, `intent_id`, `status`, `acceptance`, optional
  `egress_ref`.
- Timeout → dispatch failure → interrupted run.
- Replay skips egress entirely; verifies intent record determinism.
- Completion returns via ingress, not egress ack.

## What This Does NOT Decide

- **Ack timeout value or configuration surface.** That's 2c (routing
  config).
- **Capture artifact policy on dispatch failure.** That's the 2a
  inline fork.
- **Specific interruption reason for ack timeout.** That's 3a
  (failure taxonomy).
- **Crash consistency model.** That's 2e — but this decision
  constrains it: the crash window is between intent dispatch and
  durable-accept receipt.
- **Egress process lifecycle.** That's 2d — but this decision
  constrains it: egress must be ready to receive intents and return
  acks within the timeout window.

---

## Impacted Files

- `runner.rs` — live dispatch path and durable-accept ack handling in
  `execute_step()`
- Egress protocol definition — ack message schema
- Capture bundle / decision ack records — durable-accept ack stored
  alongside the dispatched intent record
- Egress configuration — `ack_timeout` field
- Interruption mapping — timeout / protocol / IO failures surfaced
  through the egress failure taxonomy
