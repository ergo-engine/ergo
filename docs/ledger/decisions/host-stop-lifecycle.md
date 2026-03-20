---
Authority: PROJECT
Date: 2026-03-19
Decision-Owner: Sebastian (Architect)
Recorder: Claude (Structural Auditor) + Codex (Implementation)
Status: DECIDED
Scope: v1
Parent-Decision: egress-failure-taxonomy.md
Unblocks: feat/host-stop-lifecycle (HSL-1..HSL-8)
---

# Decision: Host-Initiated Run Lifecycle Control

**Status:** Final — Codex compliant (2026-03-19)  
**Codex Verdict:** compliant (after amendments)  
**Author:** Claude (Structural Auditor), with Sebastian  
**Date:** 2026-03-19  
**Affects:** host layer, SDK, loader (production layer only — no kernel changes)

---

## Problem

Ergo has no mechanism for the host to initiate a clean run stop. Run lifecycle is entirely driver-determined:

- **Fixture drivers** exhaust naturally → `Completed`
- **Process drivers** send `{"type":"end"}` or die → `Completed` or `Interrupted(DriverTerminated)`

There is no path for:
1. **Signal handling** — SIGINT/SIGTERM kills the entire process group immediately. No capture is written. No egress channels are shut down. Run data is lost.
2. **Bounded execution** — "run for 15 minutes" or "run for N events" has no engine-level support. Each ingress channel would need to implement its own timeout logic.

Both problems block production use. A live trading system that loses its capture on Ctrl-C is not production-real. An engine that can't bound its own run duration forces lifecycle logic into every channel implementation.

---

## Design Principle

**Run lifecycle is the host's responsibility, not the driver's.**

The driver's job is to deliver events. The host's job is to decide when to stop processing them. Currently, the host delegates this decision entirely to the driver. This decision adds a host-initiated stop path that coexists with the existing driver-initiated paths.

---

## Existing Precedent

`HostStopRequested` is already listed as an interruption reason in `egress-failure-taxonomy.md` (line 67) but was never implemented. The code currently has 6 `InterruptionReason` variants; the taxonomy describes 7.

The `into_capture_bundle` path on `HostedRunner` produces a valid bundle regardless of how many events were processed. The finalization infrastructure exists, with one caveat addressed in Amendment 2.

---

## Approved Design

### 1. `InterruptionReason::HostStopRequested`

Add the missing 7th interruption reason variant. A run interrupted by host stop is `Interrupted(HostStopRequested)`, not `Completed`. The run didn't finish because the driver said so — the host intervened.

Single reason for both signal-stop and duration-expiry. No claim of capture-level cause distinction (see Amendment 3).

### 2. Stop Flag with Wakeup Mechanism

**AMENDED (Codex audit #1):** An `Arc<AtomicBool>` alone is insufficient. After the hello handshake, `run_process_driver` blocks indefinitely on `stdout_rx.recv()` (usecases.rs line ~1023). A flag checked "between steps" cannot wake a blocked receive.

**Revised design:** Replace the unbounded `recv()` in the post-hello event loop with `recv_timeout()` using a bounded poll interval (e.g., 100ms). On each timeout, check the stop flag. This matches the existing pattern used for the pre-hello startup grace period.

**IMPLEMENTATION NOTE (Codex final audit):** The stop flag must be checked in TWO places per loop iteration, not just on timeout:
1. On `recv_timeout()` timeout — catches idle ingress streams
2. At the top of each loop iteration / after each completed step — catches hot ingress streams that never hit the timeout branch

Without both checks, a continuously streaming ingress (like a live OANDA feed) could keep returning `Ok(...)` and starve stop requests indefinitely.

```rust
// Post-hello event loop (revised)
loop {
    // CHECK 1: always check at loop top (catches hot streams)
    if stop_flag.load(Ordering::Relaxed) {
        let _detail = abort_process_child(&mut child, stderr_handle.take());
        return Ok(DriverExecution { /* HostStopRequested */ });
    }
    // Also check max_duration / max_events at loop top

    match recv_process_stream_observation(&stdout_rx, Some(event_recv_timeout)) {
        Ok(observation) => { /* process event as before */ }
        Err(ProcessDriverReceiveFailure::Timeout) => {
            // CHECK 2: on timeout (catches idle streams)
            // stop_flag already checked at loop top; continue loops back
            continue;
        }
        Err(ProcessDriverReceiveFailure::Disconnected) => { /* existing path */ }
    }

    // ... existing message processing ...
}
```

Two distinct intervals serve two different loops:
```rust
struct ProcessDriverPolicy {
    startup_grace: Duration,
    termination_grace: Duration,
    poll_interval: Duration,         // 10ms — process-exit / termination-grace polling
    event_recv_timeout: Duration,    // ~100ms — main loop recv_timeout, idle stop-flag wakeup bound
}
```

`poll_interval` (10ms) is for tight child-exit polling during shutdown. `event_recv_timeout` (~100ms) is for the main event loop's `recv_timeout()` — it controls idle stop-flag wakeup granularity without 100 wakeups/sec noise.

When stop is triggered:
1. Stop polling the process driver stdout
2. Kill the process driver subprocess via `abort_process_child`
3. Call `ensure_no_pending_egress_acks()`
4. Call `stop_egress_channels()`
5. Write capture via `into_capture_bundle()`
6. Return `Interrupted(HostStopRequested)`

### 3. Signal Handler — SDK Opt-In Surface

**AMENDED (Codex design recommendation + final audit):** Do not unconditionally register a global signal handler inside `run_profile()`. Expose a synchronous controlled run surface:

```rust
// SDK public API

/// Opaque handle for requesting a clean run stop.
#[derive(Clone)]
pub struct StopHandle {
    flag: Arc<AtomicBool>,
}

impl StopHandle {
    pub fn new() -> Self {
        Self { flag: Arc::new(AtomicBool::new(false)) }
    }

    pub fn stop(&self) {
        self.flag.store(true, Ordering::Release);
    }
}

impl Ergo {
    /// Existing simple blocking API — unchanged.
    pub fn run_profile(self, profile_name: &str) -> Result<RunOutcome, ErgoRunError>;

    /// Blocking run that checks the stop handle between steps.
    /// The caller creates the StopHandle and wires it to signal
    /// handlers or other stop sources before calling this.
    pub fn run_profile_with_stop(
        self,
        profile_name: &str,
        stop: StopHandle,
    ) -> Result<RunOutcome, ErgoRunError>;

    /// Explicit-config equivalent.
    pub fn run_with_stop(
        self,
        config: RunConfig,
        stop: StopHandle,
    ) -> Result<RunOutcome, ErgoRunError>;
}
```

**Rationale for synchronous shape (Codex):**
- Preserves the current blocking mental model of Ergo
- Avoids inventing a new SDK threading model and `RunHandle::join()` semantics
- The signal handler already provides the out-of-band thread needed to set the atomic flag
- Simpler failure and ownership semantics
- Easier to scaffold and test

The scaffold generated by `ergo init` wires the default Ctrl-C handling:

```rust
// In scaffolded main.rs
let stop = StopHandle::new();
let stop_clone = stop.clone();
ctrlc::set_handler(move || stop_clone.stop())?;
let outcome = build_ergo()?.run_profile_with_stop(profile, stop)?;
```

This keeps the SDK non-opinionated about signal policy while giving scaffold projects production-correct defaults.

### 4. Run Bounds in Profile Config

New optional fields in `ergo.toml` profile configuration:

```toml
[profiles.live]
graph = "graphs/strategy.yaml"
adapter = "adapters/sample.yaml"
egress = "egress/paper.toml"
capture_output = "captures/live.capture.json"
max_duration = "15m"    # optional: stop after this wall-clock duration
max_events = 1000       # optional: stop after processing N events
```

These are checked at the top of every loop iteration in the run loop (alongside the stop flag). When either bound is reached, the stop flag is set. Same stop path as signal handling.

**Clarification (Codex final audit):** `max_events` counts committed events processed (events where `runner.step()` returned successfully), not raw protocol frames observed.

Profile config resolves into `RunConfig`, same pattern as existing profile resolution (loader `project.rs` → SDK `lib.rs`):

```rust
pub struct RunConfig {
    // ... existing fields ...
    pub max_duration: Option<Duration>,
    pub max_events: Option<u64>,
}
```

### 5. Process Driver Lifecycle on Host Stop

The host does NOT need the driver's cooperation to stop. The driver is killed via `abort_process_child` (existing function). This is consistent with:

- Ingress v0 is unidirectional; stdin is explicitly unused (`ingress-channel-guide.md` line 210)
- Host lifecycle ownership is established by the driver/channel role separation
- No protocol change required

**Future consideration:** A bidirectional driver protocol (host→driver stdin messaging) could allow graceful driver shutdown. This is explicitly deferred to a future protocol version (v1).

---

## Zero-Event Stop Policy

**AMENDMENT (Codex audit #2):** `finalize_run_summary()` explicitly rejects zero-event runs (usecases.rs line ~1491). The original proposal overclaimed that "the infrastructure for stop at any point exists."

**Policy:** Host stop before first committed event remains a `HostRunError`, not an `Interrupted` outcome. No capture is written. This is consistent with the existing invariant that a capture bundle must contain at least one committed event.

Rationale: A run that is stopped before processing any events has no decision truth to preserve. The operator gets an error message; they do not get an empty artifact.

---

## Capture Format

**AMENDMENT (Codex audit #3):** Current capture bundles do not have a run-level interruption-reason field. Per-decision `interruption: Option<String>` exists but a host stop between steps would not land there.

**Policy:** This decision does NOT claim capture-level cause distinction between signal-stop and duration-expiry. `HostStopRequested` is a single `InterruptionReason` variant. The `RunOutcome::Interrupted` return value carries the reason to the caller, but it is not persisted in the capture bundle.

If run-level stop metadata is needed in captures later, that is a separate product-layer proposal requiring its own capture format amendment.

---

## What This Does NOT Change

- **Kernel layer:** No changes. The kernel is frozen at v0.28-kernel-closed.
- **Driver protocol:** Still unidirectional, still `ergo-driver.v0`. No new frame types.
- **Egress protocol:** No changes. Egress channels are shut down by the existing `stop_egress_channels` path.
- **Capture format:** No structural changes. `HostStopRequested` is an in-memory interruption reason, not a new capture field.
- **Replay:** No changes needed. Interrupted captures are already replayable.

---

## Scope Estimate

- `InterruptionReason::HostStopRequested` variant: ~10 lines (host layer)
- `StopHandle` + stop flag plumbing: ~30 lines (SDK layer)
- `recv_timeout` poll loop revision + dual stop checks: ~60 lines (host layer)
- `run_profile_with_stop` + `run_with_stop`: ~50 lines (SDK layer)
- `max_duration` / `max_events` in RunConfig + profile resolution: ~60 lines (SDK + loader)
- `ergo.toml` schema extension: ~20 lines (loader)
- Scaffold update for default Ctrl-C: ~15 lines (ergo init templates)
- Tests: ~200 lines
- **Total: ~450-500 lines**

---

## Verification Criteria

1. `cargo run -- run live` with Ctrl-C produces a valid capture file (when ≥1 event committed)
2. Ctrl-C before first committed event returns an error, no capture written
3. That capture replays successfully
4. `max_duration = "15m"` in profile config stops the run after 15 minutes
5. `max_events = 100` in profile config stops the run after 100 events
6. Egress channels are shut down cleanly on host stop (no orphaned processes)
7. All existing tests continue to pass
8. `RunOutcome::Interrupted(HostStopRequested)` is returned, not `Completed`
9. Idle recv wakeup latency is bounded by `event_recv_timeout` (no indefinite blocking on driver stdout). For hot streams, stop may be observed at the loop-top check without any timeout. Total stop latency includes either idle recv wakeup OR current step completion (including egress ack settlement per `egress-timing-lifecycle.md`), plus finalization.

---

## Codex Audit Answers (incorporated)

1. **HostStopRequested conflict?** — No conflict. Listed in `egress-failure-taxonomy.md` line 67. Production-layer only.
2. **Host kills driver without asking?** — Consistent. Ingress v0 stdin is unused. Host lifecycle ownership is established.
3. **Profile-level or RunConfig?** — Both. Profile resolves into RunConfig, same as existing pattern.
4. **Kernel implication?** — None. Purely host/loader/SDK.
5. **Stop-between-steps invariants?** — Preserved. Per-step blocking + ack settlement guarantees no pending egress work once `runner.step()` returns (`egress-timing-lifecycle.md` line 66). Stop flag is only checked between completed steps.

---

## Implementation Order

1. `InterruptionReason::HostStopRequested` variant in host layer
2. `StopHandle` + `Arc<AtomicBool>` plumbing in SDK
3. `recv_timeout` poll loop revision with dual stop checks in `run_process_driver`
4. `run_profile_with_stop` + `run_with_stop` synchronous APIs in SDK
5. `max_duration` / `max_events` in loader profile schema + RunConfig
6. Scaffold template update for default Ctrl-C wiring
7. Tests for all paths
8. *(Optional follow-up, outside Ergo repo)* Update `my-new-app/src/main.rs` to use `run_profile_with_stop` as downstream smoke-test

**Codex verdict: compliant. Ready for implementation dispatch.**

---

## Implementation Decisions (Codex, 2026-03-19)

Resolved before dispatch to avoid rework:

### Stop flag threading

Do NOT put `Option<Arc<AtomicBool>>` on `RunGraphRequest` or `RunGraphFromPathsRequest`. That leaks mechanism into the public host API.

- Keep existing request structs unchanged
- Thread stop control as a **separate parameter** through `run_graph_from_paths_internal` → `run_graph_with_policy` → `run_fixture_driver` / `run_process_driver`
- If a public host surface is needed, use an opaque host-owned type (e.g., `RunStopToken`), not raw `Arc<AtomicBool>`
- Non-stop paths pass `None`

### `max_duration` / `max_events` ownership

Host owns both. No SDK timer threads.

- Loader profile config resolves into SDK `RunConfig`
- SDK passes them through into host as `RunBounds { max_duration, max_events }`
- Host checks both at the top of each loop iteration, same place as the stop flag
- `max_events` counts committed events (`runner.step()` returned successfully), not raw protocol frames
- `max_duration` uses host-side elapsed wall clock (`Instant::now()` vs run start)

Rationale: lifecycle bounds are host policy. Only the host can truthfully count committed events. Duration belongs next to the same loop that observes events and interruption state.

### Fixture driver path

Yes, thread stop/bounds through `run_fixture_driver()` too.

- Check stop flag before consuming the next fixture event
- Check `max_events` after each committed step
- `max_duration` uses the same host-side elapsed wall clock
- Stop before first committed event → `HostRunError`, no capture
- Stop after ≥1 committed event → `Interrupted(HostStopRequested)` with capture

### Required tests

1. **Process driver, zero-event stop:** driver says hello then sleeps forever. Stop flag set before first event. Expect `HostRunError`, no capture.
2. **Process driver, normal stop after committed events:** driver streams events indefinitely. Stop after N committed events. Expect `Interrupted(HostStopRequested)` and valid capture. Replay that capture successfully.
3. **Hot-stream stop:** driver floods events quickly. Prove stop still works without waiting for timeout (loop-top check).
4. **Fixture stop:** long fixture, stop after some events. Expect interrupted capture.
5. **Egress shutdown on host stop:** egress test script writes a sentinel when it receives `{"type":"end"}`. Assert sentinel exists to prove shutdown happened cleanly.

---

## Impacted Ledger Files

- [host-stop-lifecycle.md](../dev-work/open/host-stop-lifecycle.md)
