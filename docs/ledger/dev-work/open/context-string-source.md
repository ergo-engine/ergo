---
Authority: PROJECT
Date: 2026-03-19
Author: Codex (Implementation)
Status: OPEN
Branch: feat/context-string-source
Tier: 1 (Stdlib Completeness)
Depends-On: >-
  docs/ledger/closure-register.md
  (CONTEXT-NUMBER-SOURCE-1 established the context-source pattern);
  docs/ledger/dev-work/closed/series-stdlib.md
  (series stdlib completed the remaining typed context source/action family
  except string source)
---

# Context String Source

## Scope

Implement the missing `context_string_source` stdlib source primitive so
the typed context-source family is complete across Number, Bool,
Series, and String.

This work covers:

- manifest
- implementation
- catalog/registry admission
- runtime/source tests
- documentation alignment

This branch does not add new ontology, new adapter rules, or new
replay semantics.

Doctrine status: authorized family extension under the shipped Phase 8
context-source pattern; no new decision required.

## Current State

Today:

- `context_number_source`, `context_bool_source`, and
  `context_series_source` are shipped
- `context_set_number`, `context_set_bool`, `context_set_string`, and
  `context_set_series` are shipped
- `context_string_source` is still called out as deferred in
  `docs/contracts/extension-roadmap.md`
- users can write string values to context but cannot consume them
  through the same first-class stdlib source family

## Closure Ledger

| ID | Task | Closure Condition | Owner | Status |
|----|------|-------------------|-------|--------|
| CSS-1 | Add manifest + implementation | `context_string_source@0.1.0` exists under `crates/kernel/runtime/src/source/implementations/context_string/` and follows the existing context-source pattern. | Codex | OPEN |
| CSS-2 | Register in core stdlib | The source is admitted through the shared catalog/registry build path and available in runtime tests and downstream surfaces. | Codex | OPEN |
| CSS-3 | Implement deterministic defaults | Missing key and wrong-type reads deterministically return empty string, matching the existing typed context-source family design. | Codex | OPEN |
| CSS-4 | Validate `$key` contract | Manifest uses the same parameter-bound `$key` requirement pattern and passes existing source registration/composition rules. | Codex | OPEN |
| CSS-5 | Add tests | Source tests and runtime tests prove default-key read, custom-key read, missing-key fallback, wrong-type fallback, and manifest validation. | Codex | OPEN |
| CSS-6 | Align docs | Runtime README and any remaining roadmap/reference docs reflect `context_string_source` as shipped once code lands. | Codex | OPEN |

## Design Constraints

- Follow the existing context-source family pattern exactly.
- No runtime error path; fallback is deterministic value emission.
- No special casing beyond String-typed context access.

## What This Branch Enables

After this branch lands, the typed context-source family will be
complete and symmetrical with the shipped context-set action family.
