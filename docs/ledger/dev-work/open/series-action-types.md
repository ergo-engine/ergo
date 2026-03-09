---
Authority: PROJECT
Date: 2026-03-05
Author: Sebastian (Architect) + Codex (Implementation)
Status: CLOSED
Branch: feat/series-action-types
Tier: 1 (Kernel Completeness)
Depends-On: docs/ledger/gap-work/open/s0-series-action-types-authority.md (decision S-0)
---

# Series Action Types (Delivery Plan)

## Scope

Implement Series payload support for Actions after S-0 decision is recorded.

This is a kernel type-coverage change, not a causal-model change. Trigger gating (R.7/V.5), Action terminality, and DAG constraints remain unchanged.

## Start Gate

- S-0 must be decided in `docs/ledger/gap-work/open/s0-series-action-types-authority.md`.

## Delivery Changes

1. Add `Series` variant to `ActionValueType` and `ActionValue`.
2. Extend `wiring_allowed_for_edge()` to allow `ValueType::Series` for non-Event Action payload ports.
3. Extend ACT-23 write type compatibility for Series.
4. Update `runtime/execute.rs` mapping functions for Series round-trip (`RuntimeValue <-> ActionValue`).

## Closure Ledger

| ID | Task | Closure Condition | Owner | Status |
|----|------|-------------------|-------|--------|
| S-1 | Add `ActionValueType::Series` and `ActionValue::Series` | Code compiles, exhaustive matching satisfied, no dead variants introduced | Codex | CLOSED |
| S-2 | Update `wiring_allowed_for_edge()` | Series-typed Source/Compute -> Action scalar payload edge validates successfully | Codex | CLOSED |
| S-3 | Update ACT-23 write type compatibility | Action manifest with Series write spec passes registration checks | Codex | CLOSED |
| S-4 | Update execute mappings | Series maps correctly in both directions with no lossy conversion | Codex | CLOSED |
| S-5 | Validation test coverage | Test proves Series payload wiring into Action is accepted only on scalar payload ports | Codex | CLOSED |
| S-6 | Registration test coverage | Test proves Series write spec is accepted by ACT-23 path | Codex | CLOSED |
| S-7 | Replay/effect integrity test | Host capture/replay test proves Series effect integrity end-to-end | Codex | CLOSED |
| S-8 | Doc alignment | Invariant docs and relevant primitive docs reflect Series Action payload support accurately | Codex | CLOSED |

## Design Constraints

- Do not weaken Trigger-gated Action execution (R.7, V.5).
- Do not introduce Action-internal state.
- Do not change ontology/freeze text in this branch; if required, route through the S-0 decision ledger.
