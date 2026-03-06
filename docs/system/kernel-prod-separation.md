---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-04
Owner: Claude (Structural Auditor)
Scope: Kernel/prod boundary and host intent
Change Rule: Operational log
---

# Kernel/Prod Separation and Host Intent

This document defines the operational boundary between `kernel/*` and `prod/*`.
Its purpose is to prevent semantic bleed: kernel owns meaning, prod owns composition and product entrypoints.

---

## 1. Boundary Contract

### Kernel (`crates/kernel/*`) owns semantics

Kernel is the semantic authority for:

- Ontology and wiring laws (Source / Compute / Trigger / Action)
- Expansion, validation, execution semantics, and deterministic runtime behavior
- Replay primitives and strict replay preflight APIs
- Rule identity and rejection (`RuleViolation` ownership)
- Supervisor scheduling semantics and retry policy categories

Kernel must remain domain-neutral and must not depend on `prod/*` runtime crates.

### Prod (`crates/prod/*`) owns composition and delivery

Prod is the product composition layer for:

- Transport/decode/discovery (`prod/core/loader`)
- Canonical host orchestration (`prod/core/host`)
- Thin client entrypoints (`prod/clients/*`) that delegate to host

Prod may compose kernel APIs, but must not redefine kernel meaning.

---

## 2. Host Intent (What Host Is For)

`prod/core/host` is the canonical execution host for product callers.

Host responsibilities:

- Provide canonical entrypoints for run/replay (`run_graph_from_paths`, `replay_graph_from_paths`)
- Own loader + kernel composition for client paths
- Own adapter dependency scan and adapter composition setup for canonical runs
- Own host boundary effect lifecycle (buffer drain/apply/enrich capture)
- Enforce host lifecycle integrity guarantees (for example duplicate `event_id` rejection at host step boundary)

Host non-responsibilities:

- Defining or changing ontology, wiring matrix, or primitive semantics
- Defining new kernel rule IDs or owning `RuleViolation` surfaces
- Replacing supervisor scheduling semantics
- Becoming an alternate semantic runtime

Host is a composition boundary, not a semantic authority.

---

## 3. Invariant Ownership Map

| Scope | Invariant Families | Primary Locus |
|------|---------------------|---------------|
| Kernel semantics | `X.*`, `D.*`, `I.*`, `E.*`, `V.*`, `R.*`, `TRG-STATE-*` | `kernel/runtime`, `kernel/adapter`, `kernel/supervisor` |
| Kernel replay primitives | `REP-1` through `REP-5`, `REP-7`, `REP-8` | `kernel/adapter` + `kernel/supervisor/replay.rs` |
| Kernel orchestration core | `SUP-*`, `CXT-*`, `RTHANDLE-*` | `kernel/supervisor` + `kernel/adapter` |
| Host boundary orchestration | `HST-*`, `RUN-CANON-*`, `SDK-CANON-*` | `prod/core/host` |
| Cross-scope replay posture | `REP-SCOPE` | kernel scheduling + host-owned effect integrity |

Interpretation rule:

- If an invariant changes primitive meaning or rule identity, it is kernel scope.
- If an invariant changes canonical orchestration shape without changing semantic meaning, it is host scope.

---

## 4. Bleed Detection Rules

Treat the following as boundary violations unless explicitly escalated:

1. Prod introduces new semantic rule meanings, rule IDs, or `RuleViolation` ownership.
2. Clients bypass host canonical entrypoints and perform canonical orchestration directly.
3. Kernel crates take runtime dependencies on `prod/*` or `shared/*`.
4. Host/clients reinterpret primitive ontology or wiring legality.
5. Loader performs semantic enforcement that belongs in kernel validation/expansion.

---

## 5. Review Checklist

Use this checklist for boundary-sensitive PRs:

1. Did this PR change meaning or only composition?
2. If meaning changed, are kernel docs/invariants and kernel tests updated?
3. If composition changed, is the change isolated to host/loader/clients?
4. Do CLI/SDK remain thin adapters over host?
5. Do boundary guards still pass (`verify_layer_boundaries.sh`)?

If any answer is ambiguous, escalate before merge.

---

## 6. References

- [Kernel Closure and v1 Workstream Declaration](kernel.md)
- [Orchestration Phase Invariants](../invariants/07-orchestration.md)
- [Replay Phase Invariants](../invariants/08-replay.md)
- [Rule Registry](../invariants/rule-registry.md)
