---
Authority: PROJECT
Date: 2026-03-04
Author: Claude Opus 4.5 (Structural Auditor)
Status: CLOSED
Branch: feat/series-stdlib
Tier: 1 (Kernel Completeness)
Depends-On: feat/series-action-types; docs/ledger/gap-work/open/series-window-size-semantics.md (for SS-3)
---

# Series Stdlib Implementations

## Scope

New stdlib implementations that operate on Series values. All domain-neutral math. No domain-specific language or concepts.

Every implementation listed here follows existing patterns: manifest file, impl file, catalog registration, registry validation, tests. No new traits. No new validation rules. No frozen doc changes.

## New Implementations

### Source Implementations

| ID | Implementation | Primitive | Description | Pattern |
|----|---------------|-----------|-------------|---------|
| SS-1 | `context_series_source` | Source | Reads Series from ExecutionContext by parameter-bound key. Returns empty `vec![]` on missing key or type mismatch. | `context_number_source` |

### Compute Implementations

| ID | Implementation | Primitive | Inputs | Output | Description |
|----|---------------|-----------|--------|--------|-------------|
| SS-2 | `append` | Compute | `series: Series`, `value: Number` | `result: Series` | Returns new Series with value appended at end |
| SS-3 | `window` | Compute | `series: Series` + param `size: Int` | `result: Series` | Returns last N elements. If series shorter than N, returns full series. |
| SS-4 | `mean` | Compute | `series: Series` | `result: Number` | Arithmetic mean. Returns 0.0 for empty series. |
| SS-5 | `sum` | Compute | `series: Series` | `result: Number` | Sum of elements. Returns 0.0 for empty series. |
| SS-6 | `len` | Compute | `series: Series` | `result: Number` | Element count as f64. Returns 0.0 for empty series. |

### Action Implementations

| ID | Implementation | Primitive | Description | Pattern |
|----|---------------|-----------|-------------|---------|
| SS-7 | `context_set_series` | Action | Writes Series to context via `set_context` effect. Requires `event` (Event gate) and `value` (Series payload) inputs. Parameter-bound key via `$key` convention. | `context_set_number` |

## Closure Ledger

| ID | Task | Closure Condition | Owner | Status |
|----|------|-------------------|-------|--------|
| SS-1 | `context_series_source` | Manifest + impl + catalog registration + tests: reads from context, default empty, wrong type returns default | Codex | CLOSED |
| SS-2 | `append` | Manifest + impl + catalog registration + tests: appends value, preserves order | Codex | CLOSED |
| SS-3 | `window` | Manifest + impl + catalog registration + tests: trims to size, handles undersized, and matches the approved `size <= 0` behavior from `GW-SS3-1` | Codex | CLOSED |
| SS-4 | `mean` | Manifest + impl + catalog registration + tests: correct arithmetic mean, empty returns 0.0, NUM-FINITE-1 applies | Codex | CLOSED |
| SS-5 | `sum` | Manifest + impl + catalog registration + tests: correct sum, empty returns 0.0, NUM-FINITE-1 applies | Codex | CLOSED |
| SS-6 | `len` | Manifest + impl + catalog registration + tests: correct count as f64, empty returns 0.0 | Codex | CLOSED |
| SS-7 | `context_set_series` | Manifest + impl + catalog registration + tests: effect write with Series value, round-trips through host | Codex | CLOSED |
| SS-8 | CAT-SYNC-1 parity | `registry_catalog_key_parity` test still passes with new implementations | Codex | CLOSED |
| SS-9 | REG-SYNC-1 parity | All new implementations registered in both catalog and registries via shared build path | Codex | CLOSED |
| SS-10 | Invariant count update | INDEX.md tracked invariant count updated if any new invariants introduced | Codex | CLOSED |
| SS-11 | Integration test | End-to-end test: context_series_source reads → append → window → mean → context_set_series writes → next episode reads updated series | Codex | CLOSED |

## Design Constraints

- Empty series default: all Series-consuming implementations must handle empty `vec![]` gracefully. No panics.
- NUM-FINITE-1: `ensure_finite()` already checks Series elements. New Compute implementations producing Number outputs from Series inputs must produce finite results (mean of empty → 0.0, not NaN).
- `window` size parameter is `Int` type, validated by X.11 (exact f64 representability). `size <= 0` behavior is gated by `GW-SS3-1`.
