## 8. Replay Phase

**Scope:** Deterministic capture and verification of episode execution.

**Source:** supervisor.md §2.5, crates/kernel/adapter/src/capture.rs, crates/kernel/supervisor/src/replay.rs

**Entry invariants:**

- Capture bundle is well-formed
- All recorded events have valid hashes

### Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| REP-1 | Capture records are self-validating | — | — | — | ✓ | ✓ |
| REP-2 | Rehydration is deterministic | — | — | — | — | ✓ |
| REP-3 | Fault injection keys on EventId only | — | ✓ | — | — | ✓ |
| REP-4 | Capture/runtime type separation | — | ✓ | — | — | — |
| REP-5 | No wall-clock time in supervisor | — | — | — | — | ✓ |
| REP-6 | Stateful trigger state captured for replay | N/A | N/A | N/A | N/A | CLOSED BY CLARIFICATION |
| REP-SCOPE | Canonical replay Scope A: supervisor scheduling + host-owned effect integrity (same-ingestion path) | — | — | — | — | — |
| SOURCE-TRUST | Source determinism is trust-based | — | — | — | — | — |

### Replay Scope Limits

- Cross-ingestion normalization parity is deferred (`INGEST-TIME-1`)
- Internal source/compute payload outputs are not replay-captured as first-class records

### Notes

- **REP-1:** `validate_hash()` in capture.rs uses SHA256 to verify payload integrity. **ENFORCED** at `replay.rs` via `validate_bundle()` (called by `replay_checked()`). Legacy `replay()` panics on invalid bundle; `replay_checked()` returns `Result<_, ReplayError>` for graceful handling.
  - **Anchor tests:** `replay_rejects_corrupted_bundle`, `replay_rejects_unknown_version`
  - **v0.18:** Enforcement strengthened — `rehydrate_checked()` now called at point-of-use in supervisor replay path (`replay_inner()`). See REP-1b in closure register.
- **REP-2:** `rehydrate()` uses only record fields; no external state dependency.
- **REP-3:** `FaultRuntimeHandle` explicitly discards `graph_id` and `ctx.inner()`; keys on `EventId` only.
- **REP-4:** `ExecutionContext` has no serde derives. Capture types (`ExternalEventRecord`, `EpisodeInvocationRecord`) are separate from runtime types (`ExternalEvent`, `DecisionLogEntry`).
- **REP-5:** `replay_harness::no_wall_clock_usage` enforces no `SystemTime::now`/`Instant::now` usage in supervisor sources.
- **REP-6:** CLOSED BY CLARIFICATION (2025-12-28)

**Resolution:** Prior documentation suggesting "triggers may hold internal state" was a
semantic error that conflated execution-local bookkeeping with ontological state.

Triggers are stateless (see TRG-STATE-1). There is no trigger state to capture. Temporal
patterns requiring memory (once, count, latch, debounce) must be implemented as clusters
with explicit state flow through environment (Source reads state, Action writes state).

Replay determinism is preserved by existing adapter capture (REP-1 through REP-5). No
additional capture mechanism is required.

**Authority:** Sebastian (Freeze Authority), 2025-12-28

- **REP-SCOPE:** Canonical replay for D3 is **Scope A (self-consistency)**. It enforces strict capture preflight (version + provenance), rehydrates events with hash checks, re-executes through `ergo-host`, and verifies decision/effect integrity against host-owned captured effects. Runtime provenance uses the format `rpv1:sha256:<hex>`. It still does not guarantee cross-ingestion normalization parity; that is tracked as `INGEST-TIME-1`.
- **SOURCE-TRUST:** Source primitive determinism is trust-based, not enforced. The `SourcePrimitiveManifest` declares `execution.deterministic = true`, but the trait has no compile-time restrictions preventing non-deterministic implementations. Enforcement is by convention and code review. See `source/registry.rs::validate_manifest()`.

### UI-REF-CLIENT-1: Client Authoring is Non-Canonical

**Status:** Documented
**Enforcement:** Convention

Client libraries may demonstrate how to construct and emit `ExpandedGraph` payloads. They are NOT:

- A canonical contract implementation
- An enforcement boundary
- A required dependency for runtime execution

Contract authority remains with Rust types + `ui-runtime.md`; clients delegate canonical run/replay execution to host entrypoints.

---

## Supervisor + Replay Freeze Declaration

**Effective:** 2025-12-27

The Orchestration Phase (§7) and Replay Phase (§8) implementations are frozen at this point. The following constraints are now in force:

1. **CXT-1 through SUP-7** are enforced as specified in supervisor.md
2. **REP-1 through REP-5** are enforced via capture.rs and replay.rs
3. **Capture schema** (`ExternalEventRecord`, `EpisodeInvocationRecord`) is stable
4. **Replay harness API** (`replay()`, `rehydrate()`, `validate_hash()`) is stable

This freeze applies to:

- `crates/kernel/adapter/src/lib.rs` (ExternalEvent, ExecutionContext, RuntimeInvoker, FaultRuntimeHandle)
- `crates/kernel/adapter/src/capture.rs`
- `crates/kernel/supervisor/src/lib.rs` (Supervisor, DecisionLog, DecisionLogEntry)
- `crates/kernel/supervisor/src/replay.rs`

**To unfreeze:** Requires joint escalation per repository collaboration protocol (`.agents/COLLABORATION_PROTOCOLS.md`).
