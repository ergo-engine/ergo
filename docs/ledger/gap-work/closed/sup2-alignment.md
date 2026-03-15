---
Authority: CANONICAL
Date: 2026-03-15
Owner: ChatGPT (Build Orchestrator)
Status: CLOSED
Gap-ID: SUP-2
---

# SUP-2 RunResult Alignment Ledger

This ledger tracks the Supervisor boundary realignment to frozen SUP-2
semantics.

Status rule at closure time: a row closes only when code + tests + docs
all match.

<!-- markdownlint-disable MD013 -->
| ID | Gap | Target | Closure Condition | Owner | Status | Evidence |
| ---- | ---- | ------ | ----------------- | ----- | ------ | -------- |
| D1 | Supervisor-facing invoker returned `RunResult` in host path drafts | `RuntimeInvoker::run(...) -> RunTermination` at Supervisor boundary | Trait signature is termination-only, and Supervisor compiles without `RunResult` dependency | Codex | CLOSED | `crates/kernel/adapter/src/lib.rs`, `crates/kernel/supervisor/src/lib.rs` |
| D2 | Supervisor consumed effects from invocation result | Supervisor consumes termination only; effects handled outside Supervisor | `invoke_with_retries` returns termination-only; `DecisionLogEntry.effects` not sourced from runtime result in canonical path | Codex | CLOSED | `crates/kernel/supervisor/src/lib.rs` |
| D3 | Effect capture authority split between supervisor decision log and host enrichment | Host capture enrichment is authoritative for canonical effect records | Canonical replay runs through host strict preflight + host re-execution + effect-integrity comparison | ChatGPT + Claude | CLOSED | `crates/prod/core/host/src/replay.rs`, `crates/prod/clients/cli/src/main.rs`, `crates/prod/core/host/src/capture_enrichment.rs`, `crates/kernel/supervisor/src/replay.rs` |
| D4 | SUP-2 invariant note drift risk between code and docs | PHASE invariants explicitly map to termination-only boundary and host loop invariants | PHASE invariants contain explicit SUP-2/HST mapping and remain in sync with implementation | Codex | CLOSED | `docs/invariants/INDEX.md` |
<!-- markdownlint-restore -->

## Closure Notes

- Cross-ingestion normalization parity is explicitly deferred under
  `INGEST-TIME-1`
  (`docs/ledger/gap-work/open/ingest-time.md`).
- `DOC-GATE-1` blocks "canonical complete" claims while any row in
  this ledger is not `CLOSED`.
