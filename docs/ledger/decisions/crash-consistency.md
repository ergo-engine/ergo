---
Authority: PROJECT
Date: 2026-03-16
Decision-Owner: Sebastian (Architect)
Participants: Claude (Structural Auditor), Codex (Ontology Guardian)
Status: DECIDED
Scope: v1
Parent-Decision: egress-ack-model.md, egress-timing-lifecycle.md
Resolves: GW-EFX-3I
---

# Decision: Crash Consistency and Delivery Guarantees

## Context

The durable-accept ack model (GW-EFX-3H) and per-step blocking
lifecycle (GW-EFX-3E) together define the normal-path behavior: host
dispatches intents, waits for acks, proceeds to next step, and
finalizes a capture bundle at end of run. If a higher-level path wants
a capture artifact on disk, that file write happens after host
finalization produces the bundle and before that path
returns. This decision defines what the system promises when things go
wrong — specifically, when the host crashes at various points in the
lifecycle.

---

## Crash Window Analysis

Under per-step blocking, the host lifecycle within a run is:

```
Run begins
  ├─ Start egress channels
  ├─ Start ingress
  ├─ Step 1:
  │   ├─ Graph executes, effects produced
  │   ├─ Mirror writes applied (ContextStore, in-memory)
  │   ├─ Intents dispatched to egress
  │   └─ Durable-accept acks received
  ├─ Step 2: (same)
  ├─ ...
  ├─ Step N: (same)
  ├─ Assert no pending acks
  ├─ Stop egress channels
  ├─ Produce capture bundle         ← crash before here: all evidence lost
  ├─ Optional write capture artifact
  └─ Return final result to caller
```

There are three crash categories:

### 1. Crash during step dispatch (between intent send and ack receive)

The host wrote the intent to the egress process's stdin. The egress
process may or may not have read it:

- **If egress read and durably queued it:** The external effect will
  happen. The host has no record. The ack was in flight when the host
  died.
- **If egress did not read it (pipe buffer):** The intent is lost.
  No external effect occurs.

The host cannot distinguish these cases after a crash.

### 2. Crash after all step acks but before capture finalization

All intents across all steps were dispatched and durably accepted by
egress. The external effects will happen (or already happened). But
the capture bundle was never produced, so no capture artifact could be
written either. There is no Ergo record of what the graph decided or
what was dispatched. The egress processes hold the only evidence.

This is the **recording gap**: delivery succeeded but evidence was
not persisted. This is a run-wide concern, not a per-step concern,
because capture is finalized once at end-of-run.

### 3. Crash before or after step execution (no dispatch in flight)

Standard incomplete-run behavior. No delivery ambiguity. Either the
step hadn't started (no effects) or it completed fully (effects
recorded in host state, awaiting capture finalization — which falls
into category 2).

---

## Ruling

### Guarantee: at-most-once host dispatch

The host attempts each intent dispatch exactly once per live
execution step. There is no automatic retry, redelivery, or
reconciliation on failure or crash. If the host crashes or dispatch
fails, the run is interrupted and the intent is not re-sent.

This is scoped to **host dispatch attempts**. It is not an end-to-end
delivery guarantee. What the egress process does with the intent
after receiving it is egress-owned.

Reference: `runner.rs` dispatch loop in `execute_step()` runs once
per intent, gated by `StepMode::Live`.

### Post-ack ownership: egress is responsible

Once the egress process sends a valid durable-accept ack, the intent
is the egress process's responsibility. The host's job — making the
decision and delivering it — is complete. Host crash after ack does
not affect delivery.

"Durable-accept" is a **contract assertion**: the host validates the
ack shape (`status: "accepted"`, `acceptance: "durable"`) but cannot
independently verify that the egress process actually persisted the
intent. The host trusts the contract. This is acceptable because:

- The egress process is user-authored code. The user is responsible
  for implementing the durability they claim.
- The host's role is graph execution and effect dispatch, not
  external persistence verification.
- Verifying external durability would require a two-phase protocol
  that is out of scope for v0.

### Crash before ack: delivery status unknown

If the host crashes between dispatching an intent and receiving the
durable-accept ack, the delivery status is **unknown**. The intent
may have been durably accepted by egress (external effect will
happen) or may have been lost in the pipe buffer (no external
effect). The system makes no promise about which occurred.

### Crash before capture write: recording gap

If the host crashes after successfully completing all steps and acks
but before writing the capture artifact, the external effects
happened but no Ergo evidence exists. This is a **recording gap**,
not a delivery guarantee failure. The egress processes and external
systems hold the ground truth.

This gap is run-wide: capture is written once at end-of-run, so a
crash at this point loses evidence for all steps, not just the last
one.

### Recovery contract: deterministic intent_id enables reconciliation

Because `intent_id` is deterministically derived from
`(graph_id, event_id, node_runtime_id, intent_kind, intent_ordinal)`,
the user can reconstruct what intent_ids would have been produced for
a given run and check the egress system (broker, queue, etc.) for
those IDs.

This is a **manual, operational reconciliation path** — not an
automated recovery mechanism. The user must:

1. Determine which events were processed (from ingress logs or
   external data source)
2. Compute the expected `intent_id` values for those events
3. Query the egress system for those IDs
4. Reconcile any discrepancies

This is documented as expected operational practice for v0, not as
a system guarantee.

---

## What is explicitly out of scope for v0

The following capabilities would be required for stronger guarantees
and are not implemented:

- **Host-side WAL / outbox.** Persist intent before dispatch so the
  host can recover and re-send on restart. Required for at-least-once.
- **Restart recovery scanner.** On host restart, read WAL and
  reconcile with egress state. Required for automatic recovery.
- **Idempotent egress apply with durable dedup store.** Egress
  process maintains a durable set of processed `intent_id` values to
  prevent duplicate execution on redelivery. Required for
  exactly-once with at-least-once dispatch.
- **Incremental capture checkpointing.** Write partial capture
  artifacts per-step or per-batch rather than once at end-of-run.
  Would narrow the recording gap from run-wide to step-wide.

These are documented here as the v2 exactness path, not as deferred
work items. They represent a fundamentally different reliability
tier that requires host-side persistence infrastructure.

---

## Interaction with other decisions

### Mirror-write divergence on crash

If the host crashes during step dispatch (category 1), mirror writes
for that step are lost (ContextStore is in-memory). But the intent
may have arrived at egress. On restart or replay, the graph does not
know the intent was dispatched — the mirror write
(e.g., `last_order_symbol`) was never persisted.

This is acceptable for v0. The operational mitigation is
reconciliation from external truth: the user should ingest a status
or position snapshot from the external system (keyed by `intent_id`)
via ingress before making new decisions.

### Failure taxonomy (3a)

Crash-consistency states are NOT runtime failure taxonomy entries.
The failure taxonomy (3a) covers detectable live failures: timeout,
protocol violation, IO error. These produce `RunOutcome::Interrupted`
with specific reasons.

Crash recovery is out-of-band: there is no running process, no
`RunOutcome`, no capture artifact. The crash-consistency semantics
documented here are operational guidance, not runtime error handling.

### Capture finalization ordering (2d)

The timing decision specifies: all acks settled → stop egress → build
capture bundle → optional artifact write. A crash between "all acks
settled" and "build capture bundle" is category 2 (recording gap).
This is internally consistent — the delivery guarantee is not
conditional on capture success. Delivery happened. Evidence didn't get
recorded.

---

## Summary

| Scenario | Delivery | Evidence | User action |
| --- | --- | --- | --- |
| Normal completion | All intents durably accepted | Finalized capture bundle; artifact written if caller requests it | None |
| Crash during dispatch | Unknown (may or may not have arrived) | No capture for interrupted step | Reconcile via intent_id |
| Crash after all acks, before capture finalization | All intents durably accepted | No capture bundle or artifact | Reconcile via intent_id |
| Ack timeout (no crash) | Intent not confirmed | Capture bundle/artifact with interruption marker | Retry run or reconcile |
| Replay | No dispatch (StepMode gate) | Verified against original capture | None |

---

## Impacted Files

This decision is now reflected in the host finalization path and the
caller-owned capture write path:

- Host finalization (`ensure_no_pending_egress_acks` → `stop_egress_channels` → `into_capture_bundle`)
- SDK/manual-runner artifact write path (optional file write from the finalized bundle)

Future v2 work (WAL, recovery, checkpointing) would additionally impact:
- Host startup path (WAL recovery)
- `runner.rs` (pre-dispatch persistence)
- Capture pipeline (incremental checkpointing)
- New egress dedup infrastructure
