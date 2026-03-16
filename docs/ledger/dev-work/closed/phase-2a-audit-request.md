---
Audit-Type: Implementation gate
Scope: feat/egress-surface 2a (GW-EFX-3B) — Egress dispatch plumbing
Requested-By: Claude (Structural Auditor) + Sebastian (Architect)
Date: 2026-03-16
Status: CLOSED
---

<!-- markdownlint-disable MD029 MD032 MD040 -->

# Audit Request: 2a Egress Dispatch Plumbing

## What happened

GW-EFX-3B implemented on `feat/egress-surface`. 21 files changed,
+1826 -83 lines. This is the largest single implementation on the
branch. It wires end-to-end egress: config types, TOML parsing,
startup validation, process lifecycle, per-step dispatch, durable-accept
ack protocol, capture enrichment, interruption markers, CLI integration,
and runtime intent emission.

Three decision records constrain this implementation:
- `docs/ledger/decisions/egress-ack-model.md` — durable-accept
- `docs/ledger/decisions/egress-routing-config.md` — hybrid route table
- `docs/ledger/decisions/egress-timing-lifecycle.md` — per-step blocking

Inline fork resolved as Option C: failed dispatch step retained in
capture with explicit interruption marker.

## Critical note: kernel code was modified

This implementation extended `execute.rs` with `execute_with_metadata()`
and added intent emission logic to the kernel runtime. This was NOT in
the original spec. The original `execute()` is preserved as a wrapper.
Verify this extension does not violate the kernel freeze.

## What you must check

### A. Kernel freeze compliance (HIGHEST PRIORITY)

1. **execute.rs — new public function.** `execute_with_metadata()` is a
   new public function in kernel runtime code. Verify:
   - The original `execute()` signature is unchanged and delegates to
     `execute_with_metadata()` with placeholder args
   - No existing execution behavior is modified — same node iteration,
     same effect construction for writes, same R.7 gating
   - The intent emission code is purely additive — it reads
     `manifest.effects.intents` (which is empty for all existing
     actions) and produces `IntentRecord` entries. When intents is
     empty, zero new code paths execute.
   - `should_skip_action()` is untouched

2. **execute.rs — intent emission correctness.** Verify the emission
   path matches the decisions:
   - Fields sourced from `from_input` (via action inputs) or
     `from_param` (via parameters), exactly one per field
   - `derive_intent_id()` called with correct arguments:
     `graph_id`, `event_id`, `node.runtime_id`, `intent_spec.name`,
     `intent_ordinal`
   - `mirror_writes` applied as `EffectWrite` entries (same vec as
     normal writes), sourced from `field_values_by_name`
   - `ActionEffect` constructed with both `writes` and `intents`

3. **supervisor/lib.rs — capture type extensions.** Verify:
   - `CapturedIntentAck` struct has correct fields (intent_id,
     channel, status, acceptance, optional egress_ref)
   - `EpisodeInvocationRecord` gains `intent_acks` and `interruption`
     with `#[serde(default, skip_serializing_if = ...)]` — must not
     break deserialization of existing capture artifacts
   - No existing fields on `EpisodeInvocationRecord` are modified

4. **supervisor/replay.rs — no replay behavior changes.** Verify:
   - `hash_effect()` is unchanged
   - `compare_decisions()` is unchanged
   - New capture fields (intent_acks, interruption) are NOT compared
     during replay — they are metadata, not determinism assertions

5. **adapter/lib.rs and adapter/composition.rs — changes.** Verify:
   - Any changes to adapter types are backward-compatible
   - `RuntimeHandle` changes (if any) for graph_id/event_id plumbing
     do not alter existing behavior
   - Composition validation still works for existing adapters

### B. Decision-code consistency

6. **egress-ack-model.md compliance.** Verify:
   - Host waits for durable-accept ack per intent (not completion)
   - Ack validation: `status == "accepted"` AND `acceptance == "durable"`
   - Mismatched `intent_id` in ack → protocol error
   - Timeout → dispatch failure → interrupted run
   - `egress_ref` is optional, stored in `CapturedIntentAck`

7. **egress-routing-config.md compliance.** Verify:
   - `EgressConfig` uses `BTreeMap` for channels and routes
   - `EgressChannelConfig::Process` uses `Vec<String>` for command
   - Startup validation: route channel exists, kind is adapter-accepted
   - Routed kinds feed into `ensure_handler_coverage` as
     `egress_claimed_kinds`
   - Non-emittable routed kind is warning, not error

8. **egress-timing-lifecycle.md compliance.** Verify:
   - Egress channels start before first ingress event
   - Per-step blocking: mirror writes → dispatch → wait acks → next step
   - Dispatch only in `StepMode::Live`, never `StepMode::Replay`
   - End-of-run: assert no pending acks → write capture → stop egress
   - Capture write BEFORE egress stop

9. **Inline fork (artifact policy).** Verify Option C implemented:
   - Failed dispatch step is retained in capture (not discarded)
   - Interruption marker recorded on the step
   - Partial acks (if any received before failure) are preserved
   - Run returns interrupted outcome

### C. Process protocol correctness

10. **Egress process handshake.** Verify:
    - Process sends `{"type": "ready"}` on stdout
    - Host validates the ready frame before declaring channel ready
    - Timeout on readiness → startup failure → run does not begin
    - Non-ready frame during startup → protocol error

11. **Intent dispatch protocol.** Verify:
    - Host writes JSON to stdin: `{"type": "intent", "intent_id": ...,
      "kind": ..., "fields": {...}}`
    - `fields` is JSON projection of typed `IntentField` values
      (not raw `Value` enum serialization)
    - Host reads ack from stdout, validates structure
    - Shutdown: host sends `{"type": "end"}` on stdin

12. **Process lifecycle safety.** Verify:
    - Force-terminate on Drop (no zombie processes)
    - Graceful shutdown with bounded timeout then force kill
    - Stderr captured (not lost)
    - Stdout reader on separate thread (no deadlock from buffering)

### D. Backward compatibility

13. **No-egress runs unchanged.** Verify:
    - `egress_config: None` produces identical behavior to pre-branch
    - No egress processes spawned when config is None
    - Coverage check still works (empty egress_claimed_kinds)
    - All existing tests pass without egress config

14. **Capture artifact backward compat.** Verify:
    - `intent_acks: Vec<CapturedIntentAck>` uses
      `#[serde(default, skip_serializing_if = "Vec::is_empty")]`
    - `interruption: Option<String>` uses
      `#[serde(default, skip_serializing_if = "Option::is_none")]`
    - Existing capture artifacts (no intent_acks, no interruption)
      deserialize correctly
    - Effect hash (`hash_effect()`) is unchanged — only effects
      participate in the hash, not acks or interruptions

15. **CLI backward compat.** Verify:
    - `--egress-config` is optional
    - Omitting it produces pre-branch behavior
    - No existing CLI flags changed or removed

### E. Second/third-order concerns

16. **Intent field JSON projection.** The `fields` object sent to
    egress uses `BTreeMap<String, serde_json::Value>` (deterministic
    key ordering). Verify the `common_value_to_json()` conversion
    handles all `Value` variants: Number, Series, Bool, String.

17. **Per-step blocking and StepMode interaction.** The `start_egress_channels()`
    call is inside `execute_step()` gated by `StepMode::Live`. Verify
    it's idempotent (called every step but only starts once). If not
    idempotent, first step starts channels, subsequent steps must not
    try to re-start.

18. **Error propagation.** Verify egress errors (startup, protocol, IO,
    timeout) are converted to `HostedStepError` variants. Verify these
    map cleanly to `RunOutcome::Interrupted` with appropriate reason.

19. **Multiple intents per step.** If one step produces multiple
    intents routed to different channels, verify they are dispatched
    sequentially (or confirm the dispatch model). If one ack fails
    after others succeed, verify partial acks are preserved.

20. **TOML Duration parsing.** `Duration` doesn't have a standard
    serde representation. Verify the TOML format uses a parseable
    representation (e.g., `"5s"`, `5000` ms, or a custom deserializer).
    Test that the example TOML from the decision record actually parses.

### F. Test coverage

21. **Integration tests with real processes.** Verify tests exist for:
    - Single intent → dispatch → ack → success
    - Ack timeout → dispatch failure
    - Invalid ack (wrong intent_id) → protocol error
    - Egress startup failure (no ready frame)
    - Multiple intents dispatched and acked
    - Replay mode skips egress (no process spawned)

22. **Validation tests.** Verify tests exist for:
    - Valid config passes
    - Route → nonexistent channel → error
    - Route → non-accepted kind → error
    - Missing route for emittable kind → coverage error
    - Non-emittable routed kind → warning
    - Handler + egress both claim kind → ConflictingCoverage

23. **Config parsing tests.** Verify:
    - Example TOML from decision record parses correctly
    - Missing required fields → error
    - BTreeMap ordering is deterministic

## Files to read

Kernel (highest scrutiny — freeze compliance):
- `crates/kernel/runtime/src/runtime/execute.rs`
- `crates/kernel/runtime/src/runtime/mod.rs`
- `crates/kernel/supervisor/src/lib.rs`
- `crates/kernel/supervisor/src/replay.rs`
- `crates/kernel/adapter/src/lib.rs`
- `crates/kernel/adapter/src/composition.rs`
- `crates/kernel/adapter/tests/composition_tests.rs`

Host/prod (decision compliance):
- `crates/prod/core/host/src/egress/mod.rs`
- `crates/prod/core/host/src/egress/config.rs`
- `crates/prod/core/host/src/egress/validation.rs`
- `crates/prod/core/host/src/egress/process.rs`
- `crates/prod/core/host/src/runner.rs`
- `crates/prod/core/host/src/usecases.rs`
- `crates/prod/core/host/src/capture_enrichment.rs`
- `crates/prod/core/host/src/error.rs`
- `crates/prod/core/host/src/lib.rs`
- `crates/prod/core/host/src/replay.rs`
- `crates/prod/core/host/src/demo_fixture_usecase.rs`
- `crates/prod/core/host/Cargo.toml`

CLI:
- `crates/prod/clients/cli/src/graph_yaml.rs`
- `crates/prod/clients/cli/src/output/text.rs`

Reference (read for invariant verification):
- `docs/ledger/decisions/egress-ack-model.md`
- `docs/ledger/decisions/egress-routing-config.md`
- `docs/ledger/decisions/egress-timing-lifecycle.md`
- `docs/FROZEN/execution_model.md`
- `docs/CANONICAL/PHASE_INVARIANTS.md`

## Output format

For each check (1-23), report:

```
[CHECK-N] PASS | FAIL | FLAG
Evidence: <file:line or brief explanation>
Detail: <only if FAIL or FLAG>
```

PASS = correct, no issue.
FAIL = bug, invariant violation, or decision non-compliance. Stop and report.
FLAG = not a bug, but a risk or gap that should be tracked.

At the end, provide:
- Overall verdict: COMPLIANT or VIOLATION or FLAGGED
- If FLAGGED: list items to track
- If VIOLATION: stop, do not proceed

<!-- markdownlint-restore -->
