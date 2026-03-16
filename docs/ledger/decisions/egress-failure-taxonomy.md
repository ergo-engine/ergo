---
Authority: PROJECT
Date: 2026-03-16
Decision-Owner: Sebastian (Architect)
Participants: Claude (Structural Auditor), Codex A + Codex B (Ontology Guardian)
Status: DECIDED
Scope: v1
Parent-Decision: egress-ack-model.md, crash-consistency.md
Resolves: GW-EFX-3J
---

# Decision: Egress Failure Taxonomy and Partial-Apply Semantics

## Context

Egress dispatch failures currently map to
`HostedStepError::EgressDispatchFailure { detail: String }`, which
produces `RunOutcome::Interrupted` with a reason string. Users cannot
programmatically distinguish timeout from protocol violation from IO
failure. The crash consistency decision (2e) establishes that crash-
recovery states are outside the runtime taxonomy. The remediation pass
hardened the ack lifecycle with channel quiescence on all failure paths.

This decision defines the public failure taxonomy for egress.

---

## Ruling

### Separation of failure phases

Not all egress failures are in-run interruptions. The taxonomy
distinguishes three failure phases:

**1. Pre-run failures (HostRunError, not InterruptionReason):**
- Egress config validation failures
- Egress channel startup failures (process won't spawn, no ready frame)
- Handshake capability mismatches

These occur before the first event is processed. The run has not
begun. They are typed `HostRunError` variants, not interruption
reasons.

**2. In-run dispatch failures (InterruptionReason):**
- Ack timeout
- Protocol violation (invalid ack, unexpected frame)
- IO failure (broken pipe, channel crash mid-run)

These occur during step execution. The run was in progress. They
produce `RunOutcome::Interrupted` with a typed reason.

**3. Post-run finalization failures (HostRunError or logged):**
- Egress shutdown timeout
- Channel won't terminate

These occur during finalization, after all steps complete but before
capture write. They do not affect capture truth because egress is
quiesced/stopped before capture finalization (per the timing decision
and remediation ordering: assert no pending acks → stop egress →
build capture bundle → write capture).

### InterruptionReason variants

Flat, consistent with existing ingress variants:

```rust
pub enum InterruptionReason {
    // Existing ingress variants
    HostStopRequested,
    DriverTerminated,
    ProtocolViolation,
    DriverIo,

    // New egress variants
    EgressAckTimeout { channel: String, intent_id: String },
    EgressProtocolViolation { channel: String },
    EgressIo { channel: String },
}
```

Design choices:

- **Flat, not nested.** Matches existing ingress style. Keeps user
  pattern matching simple. Only three meaningful in-run egress
  categories.
- **`channel` on all egress variants.** Operationally essential in
  multi-channel route tables. Users need to know which channel failed.
- **`intent_id` only on `EgressAckTimeout`.** Timeout is intent-
  specific (waiting for a particular ack). Protocol and IO are
  channel-level breakages — the specific intent is less stable as
  public API.

### Typed dispatch failure

`HostedStepError::EgressDispatchFailure { detail: String }` becomes
a typed enum so the usecase layer can map to `InterruptionReason`
without string parsing:

```rust
pub enum EgressDispatchFailure {
    AckTimeout { channel: String, intent_id: String },
    ProtocolViolation { channel: String, detail: String },
    Io { channel: String, detail: String },
}
```

`detail` is preserved on protocol and IO for diagnostic logging but
is not part of the public `InterruptionReason`.

### Partial delivery semantics

- **Stop on first failure.** The host does not attempt remaining
  intents after a dispatch failure.
- **Preserve prior durable acks.** Intents that were successfully
  acked before the failure are recorded in the capture artifact.
- **Partial delivery is accepted as a fact of the model.** It is
  not hidden, retried, or rolled back.

This aligns with the crash consistency decision: no retries, no
all-or-nothing illusion, no optimistic continued dispatch after one
channel is dead.

### Quiescence in the reason

- **Not encoded in `InterruptionReason`.** The reason represents the
  initiating failure only.
- Quiescence outcome is secondary operational detail: logged,
  optionally captured as interruption context, but not part of the
  public failure category.

---

## What This Does NOT Decide

- **Specific `HostRunError` variant names** for pre-run and post-run
  failures. These follow existing patterns in `usecases.rs`.
- **Retry policy.** There is no retry. The crash consistency decision
  is authoritative.
- **Multi-channel partial delivery recovery.** Out of scope for v0.

---

## Impacted Files

- `InterruptionReason` enum (new variants)
- `HostedStepError` / `EgressDispatchFailure` (typed enum)
- `runner.rs` (map `EgressProcessError` to typed dispatch failure)
- `usecases.rs` (map typed dispatch failure to `InterruptionReason`)
