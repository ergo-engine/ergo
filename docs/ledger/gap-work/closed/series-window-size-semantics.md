---
Authority: PROJECT
Date: 2026-03-05
Author: Sebastian (Architect) + Codex (Implementation)
Status: CLOSED
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

This decision controls SS-3 implementation semantics and test oracle
behavior.

## Decision Ledger

<!-- markdownlint-disable MD013 -->
| ID | Task | Closure Condition | Owner | Status |
| ---- | ---- | ----------------- | ----- | ------ |
| GW-SS3-1 | Choose behavior for `size <= 0` | Decision recorded with exact expected output/error behavior and test requirements | Sebastian | CLOSED |
<!-- markdownlint-restore -->

## Decision Record

- Date: 2026-03-05
- Decision owner: Sebastian
- Selected behavior: Reject
- Canonical behavior: `window(size <= 0)` returns
  `ComputeError::InvalidParameter { parameter: "size", reason: "size
  must be a positive integer" }`.
- Test requirements:
  - `window_rejects_non_positive_size` enforces runtime behavior.
  - No silent coercion to empty series is permitted.
