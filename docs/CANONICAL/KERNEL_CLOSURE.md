---
Authority: CANONICAL
Version: v1
Last Updated: 2026-01-11
Owner: Claude (Structural Auditor)
Scope: v0 baseline declaration, v1 workstream rules
Change Rule: Operational log
---

# Kernel Closure and v1 Workstream Declaration

**Status:** The v0 kernel is closed. Work may continue, but the kernel's meaning is now a fixed reference point.

---

## What "Kernel" Means

Kernel = runtime + supervisor + adapter + contracts that define:
- Execution semantics (ExpandedGraph → validate → execute)
- Primitive ontology (Source / Compute / Trigger / Action)
- Determinism + replay integrity posture
- Core stdlib atoms (domain-neutral)
- Contract surfaces used by clients (UI contract, capture/replay format)

---

## What "Closed" Means

Closed ≠ abandoned. Closed = semantics-stable reference point.

- We may patch bugs that violate already-declared invariants.
- We may not "improve behavior" by quietly changing meanings.
- Any new semantics obligations require an explicit v1 decision record.

---

## Two Parallel Tracks Going Forward

### 1. v0 Kernel (Closed / Stable Reference)

**Changes allowed:**
- Invariant enforcement fixes
- Clarifications
- Bug fixes consistent with doctrine

**Changes not allowed:**
- Meaning changes
- New coercions
- New hidden defaults
- Domain-loaded naming

### 2. v1 Workstream (Exploration / Expansion)

New semantics are allowed, but must be:
- Explicitly specified
- Phase-bounded (where enforced)
- Regression-tested
- Tagged/recorded as new obligations

---

## Compatibility Posture

- Backward compatibility is required only for explicitly versioned persisted formats (e.g., capture bundles).
- Renames affecting persisted formats require a compatibility plan (serde alias + legacy test).

---

## Reference Clients

`crates/reference-client` is a reference client, not a core component:
- It must reflect contracts accurately
- It must not define runtime semantics
- Any "helpful UI behavior" must be explicit and versioned/documented as UI-only

---

## Release/Tag Discipline

Every kernel-changing merge must correspond to:
- Updated invariants/closure entries (if applicable)
- A tag that anchors the reference point when appropriate

If a change would confuse a new contributor reading the docs, it needs a doctrine update or a separate PR.

---

## v1 Workstream Log

Tracks semantic changes that exceed v0 scope.

| Item | PR  | Tag            | Description                                                                                                                  |
|------|-----|----------------|------------------------------------------------------------------------------------------------------------------------------|
| B.2  | #35 | v1.0.0-alpha.1 | Divide-by-zero semantics: strict divide errors, safe_divide with fallback, NUM-FINITE-1 guard, SemanticError classification |

---

## Declaration

**v0 kernel is now the baseline. v1 begins as a workstream, not a public promise.**
