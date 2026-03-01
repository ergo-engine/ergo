# SUP-2 RunResult Alignment Ledger

Authority: CANONICAL  
Owner: ChatGPT (Build Orchestrator)  
Status Rule: A row is closed only when code + tests + docs all match.

This ledger tracks the Supervisor boundary realignment to frozen SUP-2 semantics.
Do not remove rows; transition status instead.

| ID | Gap | Target | Closure Condition | Owner | Status | Evidence |
|----|-----|--------|-------------------|-------|--------|----------|
| D1 | Supervisor-facing invoker returned `RunResult` in host path drafts | `RuntimeInvoker::run(...) -> RunTermination` at Supervisor boundary | Trait signature is termination-only, and Supervisor compiles without `RunResult` dependency | Codex | CLOSED | `crates/adapter/src/lib.rs`, `crates/supervisor/src/lib.rs` |
| D2 | Supervisor consumed effects from invocation result | Supervisor consumes termination only; effects handled outside Supervisor | `invoke_with_retries` returns termination-only; `DecisionLogEntry.effects` not sourced from runtime result in canonical path | Codex | CLOSED | `crates/supervisor/src/lib.rs` |
| D3 | Effect capture authority split between supervisor decision log and host enrichment | Host capture enrichment is authoritative for canonical effect records | Strict replay path for effect-aware captures is fully aligned with host-owned effect records end-to-end | ChatGPT + Claude | OPEN | `crates/ergo-host/src/capture_enrichment.rs`, `crates/supervisor/src/replay.rs` |
| D4 | SUP-2 invariant note drift risk between code and docs | PHASE invariants explicitly map to termination-only boundary and host loop invariants | PHASE invariants contain explicit SUP-2/HST mapping and remain in sync with implementation | Codex | CLOSED | `docs/CANONICAL/PHASE_INVARIANTS.md` |

## Closure Notes

- `D3` remains intentionally open until replay effect integrity verification is fully anchored to host-owned effect records in doctrine and tests.
- `DOC-GATE-1` blocks "canonical complete" claims while any row in this ledger is not `CLOSED`.
