---
Authority: CANONICAL
Version: v0.37
Owner: Claude (Structural Auditor)
Last Updated: 2026-03-16
Scope: Phase boundaries, enforcement loci, gap tracking
Change Rule: Operational log
---

# Phase Invariants — v0

**Tracked invariants:** 208

This document defines the invariants that must hold at each phase boundary in the system. It is the authoritative reference for what is true, where that truth is enforced, and what happens if it is violated.

**An invariant without an enforcement locus is not an invariant. It is a wish.**

---

## Preamble

### Purpose

This document serves as:

- The constitution of the system's correctness guarantees
- An audit baseline for code review
- A gap-detection tool for implementation work
- A portable reference for future contributors

### Enforcement Locus Definitions

| Locus | Meaning | Strength |
|-------|---------|----------|
| **Spec** | Documented in frozen/stable specification | Declarative only — requires other loci for enforcement |
| **Type** | Impossible to violate due to Rust type system | Strongest — compile-time guarantee |
| **Assertion** | Enforced via `assert!` / `debug_assert!` / panic | Strong — fails loudly at runtime |
| **Validation** | Enforced by validation logic returning `Result::Err` | Strong — recoverable, explicit |
| **Test** | Enforced by test coverage | Weakest — detects regression, does not prevent |

**Rule:** Every invariant must have at least one enforcement locus beyond **Spec**. Spec alone is insufficient.

### Source Documents

This checklist draws from:

- `ontology.md` (frozen)
- `execution.md` (frozen)
- `freeze.md` (frozen)
- `adapter.md` (frozen)
- `supervisor.md` (frozen)
- `concepts.md` (stable)
- `cluster-spec.md` (stable)
- `adapter.md` (stable)
- `source.md` (stable)
- `compute.md` (stable)
- `trigger.md` (stable)
- `action.md` (stable)

---

## Core v0.1 Freeze Declaration

**Effective:** 2025-12-22

Core is frozen at this point. The following constraints are now in force:

1. **No new core implementations** without a vertical proof demonstrating necessity
2. **Any core change** must introduce a new invariant with explicit enforcement locus
3. **Infrastructure actions** (ack, annotate, context_set_*) live in core; **domain-specific capability actions** live in verticals

This freeze applies to:

- `src/source/`
- `src/compute/`
- `src/trigger/`
- `src/action/`
- `src/cluster.rs`
- `src/runtime/`

Doctrine documents retain their existing authority levels.

**To unfreeze:** Requires joint escalation to Sebastian with justification referencing a specific vertical that cannot function without the change.

---

## Golden Spike Tests

The following tests are designated as canonical execution path anchors:

| Test | Proves | Invariants Exercised |
|------|--------|---------------------|
| `hello_world_graph_executes_with_core_catalog_and_registries` | Direct execution path works | R.1–R.7, V.*, X.* |
| `supervisor_with_real_runtime_executes_hello_world` | Orchestrated execution path works | SUP-1, SUP-2, CXT-1, R.* |

These tests are permanent. Failure indicates invariant regression.

**Authority:** Claude (Doctrine Owner), designated 2025-12-28

---

## Canonical Run / Replay Strictness (v3)

| ID | Invariant | Enforcement Locus | Status |
|----|-----------|-------------------|--------|
| RUN-CANON-1 | Canonical graph run requires explicit event source | Host canonical path usecase validation (`ergo_host::run_graph_from_paths`, delegating to `run_graph`, enforces ingress presence before execution) | Enforced |
| RUN-CANON-2 | Adapter binding is mandatory only for adapter-dependent graphs | Host canonical path usecase validation (`ergo_host::run_graph_from_paths`, delegating to `run_graph`, enforces adapter requirement from dependency summary) | Enforced |
| REP-7 | Strict replay requires provenance contract match | `supervisor::replay::replay_checked_strict` | Enforced |
| REP-8 | Strict replay rejects duplicate `events[].event_id` values | `supervisor::replay::validate_bundle_strict` | Enforced |

Notes:

- Adapter-dependent graph detection is based on required source context keys and action effects (writes and declared intents).
- Adapter-independent canonical captures use explicit provenance sentinel `none`.
- Capture bundles are strict v3 (`capture_version: "v3"`):
  `adapter_provenance`, `runtime_provenance`, and `decisions[].effects`
  are required, unknown fields are rejected, and legacy
  `adapter_version` bundles fail deserialization.
- v2 artifacts remain readable as JSON fixtures, but strict replay
  comparability is version-scoped and not guaranteed across the v2→v3
  semantic boundary.
- Strict replay preflight enforces unique `events[].event_id` identities.
- Repo policy: capture bundles and fixtures are ephemeral/regenerated artifacts; backward compatibility across bundle schema revisions is not guaranteed inside this repo.

---

## Phase Files

| File | Phase | Invariant IDs |
|------|-------|---------------|
| [00-cross-phase.md](00-cross-phase.md) | Cross-Phase | X.1–X.12, NUM-FINITE-1, B.2, LAYER-1–3 |
| [01-definition.md](01-definition.md) | Definition | D.1–D.11 |
| [02-instantiation.md](02-instantiation.md) | Instantiation | I.1–I.7 |
| [03-expansion.md](03-expansion.md) | Expansion | E.1–E.9 |
| [04-inference.md](04-inference.md) | Inference | F.1–F.6 |
| [05-validation.md](05-validation.md) | Validation | V.1–V.8 |
| [06-execution.md](06-execution.md) | Execution | R.1–R.7, TRG-STATE-1 |
| [07-orchestration.md](07-orchestration.md) | Orchestration | CXT-1, SUP-*, HST-*, RTHANDLE-*, DOC-GATE-1, SDK-CANON-* |
| [08-replay.md](08-replay.md) | Replay | REP-1–REP-6, REP-SCOPE, SOURCE-TRUST |
| [09-adapter-registration.md](09-adapter-registration.md) | Adapter Registration | ADP-1–ADP-19 |
| [10-adapter-composition.md](10-adapter-composition.md) | Adapter Composition | COMP-1–3, COMP-16 |
| [11-source-registration.md](11-source-registration.md) | Source Registration | SRC-1–SRC-17 |
| [12-compute-registration.md](12-compute-registration.md) | Compute Registration | CMP-1–CMP-20 |
| [13-trigger-registration.md](13-trigger-registration.md) | Trigger Registration | TRG-1–TRG-14 |
| [14-action-registration.md](14-action-registration.md) | Action Registration | ACT-1–ACT-33 |
| [15-action-composition.md](15-action-composition.md) | Action Composition | COMP-9–COMP-15, COMP-17–COMP-19 |
| [rule-registry.md](rule-registry.md) | Rule Registry | Generated rule index |

---

# Stage D Verification (stress test)

No implementation required. State is already fully externalized and governed by existing invariants (CXT-1, SUP-*, REP-*). Stage D consists of stress-testing replay determinism and orchestration boundaries; any failures indicate invariant regression and require escalation.

---

# Appendix A: Gap Summary

| ID | Invariant | Issue | Priority | Status |
|----|-----------|-------|----------|--------|
| ~~F.1~~ | ~~Input ports never wireable~~ | ~~Code violation~~ | ~~BLOCKER~~ | ✅ CLOSED |
| ~~E.3~~ | ~~ExternalInput not as sink~~ | ~~No assertion~~ | ~~HIGH~~ | ✅ CLOSED |
| ~~E.7~~ | ~~Boundary ports for inference only~~ | ~~No doc comment~~ | ~~MEDIUM~~ | ✅ CLOSED |
| ~~D.11~~ | ~~Declared wireability ≤ inferred~~ | ~~Validation missing~~ | ~~MEDIUM~~ | ✅ CLOSED |
| ~~X.9~~ | ~~Authoring compiles away~~ | ~~Structurally enforced — type system~~ | ~~MEDIUM~~ | ✅ CLOSED |
| ~~F.6~~ | ~~Inference depends only on graph + catalog~~ | ~~Documented~~ | ~~LOW~~ | ✅ CLOSED |
| ~~R.3~~ | ~~No same-pass action observation~~ | ~~Compositionally enforced via F.2, X.5~~ | ~~LOW~~ | ✅ CLOSED |
| ~~X.7~~ | ~~Compute inputs ≥1~~ | ~~Validation missing~~ | ~~HIGH~~ | ✅ CLOSED |
| ~~R.4~~ | ~~Action failure aborts subsequent actions~~ | ~~Closed by design — Result::Err propagation~~ | ~~LOW~~ | ✅ CLOSED |
| ~~R.7~~ | ~~Actions execute only when trigger emitted~~ | ~~Runtime gating missing~~ | ~~BLOCKER~~ | ✅ CLOSED |
| ~~REP-6~~ | ~~Stateful trigger state captured~~ | ~~Closed — triggers are stateless by design~~ | ~~N/A~~ | ✅ CLOSED |

---

## Appendix B: Code Review Protocol

When reviewing any PR, ask:

1. **Which invariants does this code touch?**
2. **For each touched invariant, is enforcement preserved or strengthened?**
3. **Does this PR introduce any new implicit assumptions?**
4. **If an invariant is weakened, is the weakening explicitly documented and justified?**

A PR that cannot answer these questions is incomplete.

---

## Authority

This document is canonical for v0.

It joins the frozen doctrine set:

- `ontology.md`
- `execution.md`
- `freeze.md`
- `adapter.md`
- `supervisor.md`

And the stable specification set:

- `concepts.md`
- `cluster-spec.md`

Changes to this document require the same review bar as changes to frozen specs.
