---
Authority: PROJECT
Date: 2026-03-05
Author: Sebastian (Architect) + Codex (Implementation)
Status: DECISION_PENDING
Gap-ID: GW-SS3-1
Unblocks: feat/series-stdlib (SS-3)
---

# GW-SS3-1: `window` Size <= 0 Semantics

## Question

For `series.window(size: Int)`, what is canonical behavior when `size <= 0`?

Options:

1. Reject as validation/runtime error
2. Return empty series

## Impact

This decision controls SS-3 implementation semantics and test oracle behavior.

## Decision Ledger

| ID | Task | Closure Condition | Owner | Status |
|----|------|-------------------|-------|--------|
| GW-SS3-1 | Choose behavior for `size <= 0` | Decision recorded with exact expected output/error behavior and test requirements | Sebastian | DECISION_PENDING |
