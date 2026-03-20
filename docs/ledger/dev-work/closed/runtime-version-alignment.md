---
Authority: PROJECT
Date: 2026-03-19
Author: Codex (Implementation)
Status: CLOSED
Branch: fix/runtime-version-alignment
Landing-Commit: 1ecf81c
Tier: 1 (Correctness Cleanup)
Depends-On: none
---

# Adapter Runtime Version Alignment

## Scope

Replace the adapter crate's placeholder runtime version constant with a
truthful runtime-owned source of version information for ADP-3
compatibility validation.

This is a narrow implementation cleanup. It does not introduce a new
compatibility policy or alter semver comparison semantics.

Doctrine status: mechanical under existing ADP-3 semantics; no new
decision required.

## Current State

Today:

- `crates/kernel/adapter/src/validate.rs` hardcodes
  `RUNTIME_VERSION = "0.1.0"` with a TODO
- ADP-3 validation compares `runtime_compatibility` against that
  placeholder
- the placeholder is accidentally correct at the moment because
  `ergo-runtime` is also `0.1.0`
- the real risk is silent future drift between code and docs when the
  runtime crate version changes

## Closure Ledger

| ID | Task | Closure Condition | Owner | Status |
|----|------|-------------------|-------|--------|
| RVA-1 | Export runtime version truthfully | `ergo-runtime` exposes a canonical runtime version constant or equivalent source that reflects the runtime crate version used by the build. | Codex | CLOSED |
| RVA-2 | Remove adapter placeholder | Adapter ADP-3 validation uses the runtime-owned version source instead of a local hardcoded string. | Codex | CLOSED |
| RVA-3 | Update tests | Adapter validation tests no longer rely on the placeholder comment/assumption and continue to prove incompatible runtime rejection truthfully. | Codex | CLOSED |
| RVA-4 | Preserve behavior | Existing ADP-3 comparison behavior remains semver-based and all relevant tests pass after the source-of-truth swap. | Codex | CLOSED |

## Design Constraints

- No new compatibility semantics.
- No workspace-wide versioning scheme invention.
- Keep the fix local and mechanical.

## What This Branch Enables

After this branch lands, ADP-3 runtime compatibility validation will be
truthful by construction rather than accidentally correct while all
crate versions remain `0.1.0`.
