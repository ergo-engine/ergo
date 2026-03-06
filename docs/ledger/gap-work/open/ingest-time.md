# INGEST-TIME-1 Ledger

Authority: CANONICAL  
Owner: ChatGPT (Build Orchestrator)  
Status Rule: OPEN until a doctrine-approved normalization contract is specified and tested.

This ledger tracks a deferred non-goal for D3 closure work.

| ID | Gap | Scope | Closure Condition | Owner | Status | Evidence |
|----|-----|-------|-------------------|-------|--------|----------|
| INGEST-TIME-1 | Cross-ingestion normalization parity is not guaranteed across all event-ingress paths | Out of scope for D3 Scope A (self-consistency replay) | Canonical normalization rules are specified and enforced across ingestion modes with invariant-linked tests | ChatGPT + Claude | OPEN | `docs/ledger/gap-work/closed/sup2-alignment.md` |

## Notes

- D3 closure is scoped to self-consistency replay (Scope A).
- Cross-ingestion parity remains explicitly deferred under `INGEST-TIME-1`.
