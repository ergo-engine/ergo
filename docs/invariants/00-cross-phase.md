---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-26
Owner: Documentation
Scope: Cross-phase invariants and shared execution guards
Change Rule: Operational log
---

## 0. Cross-Phase Invariants

These invariants hold across all phases. Violation at any point is a system-level failure.

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| X.1 | Exactly four ontological primitives exist: Source, Compute, Trigger, Action | ontology.md §2 | `PrimitiveKind` enum | — | — | — |
| X.2 | Wiring matrix is never violated (see ontology.md §3) | ontology.md §3 | — | — | ✓ | ✓ |
| X.3 | All graphs are directed acyclic graphs (DAGs) | execution.md §2 | — | — | ✓ | ✓ |
| X.4 | Determinism: identical inputs, parameters, and explicit state → identical outputs | execution.md §8 | — | — | — | ✓ |
| X.5 | Actions are terminal; Action → * is forbidden | ontology.md §3 | — | — | ✓ | ✓ |
| X.6 | Sources have no inputs | ontology.md §2.1 | — | — | ✓ | ✓ |
| X.7 | Compute primitives have ≥1 input | ontology.md §2.2 | — | — | ✓ | ✓ |
| X.8 | Triggers emit events | ontology.md §2.3 | — | — | ✓ | ✓ |
| X.9 | Authoring constructs compile away before execution | freeze.md §7 | — | ✓ | — | ✓ |
| X.10 | Compute parameter types must not include Series or String | (inferred) | — | — | ✓ | ✓ |
| X.11 | Int→f64 conversion must be exactly representable (\|i\| ≤ 2^53) | (inferred) | — | — | ✓ | ✓ |
| X.12 | Every ValueType has at least one source producer | (inferred) | — | — | — | ✓ |

### Notes

- **X.1:** Enforced by type system. `PrimitiveKind` enum has exactly four variants.
- **X.4:** Determinism is tested but not structurally enforced. Current docs define it over identical inputs, resolved parameters, and any explicit state. Acceptable for v0.
- **X.7:** ✅ **CLOSED.** Enforced in `compute/registry.rs::validate_manifest` (returns `NoInputsDeclared` when `inputs.is_empty()` for Compute manifests). Test: `cmp_4_no_inputs_rejected` in `compute/registry.rs`.
- **X.9:** Structurally enforced at the expansion boundary. `ExpandedGraph` carries only expanded implementation instances, so authored cluster constructs do not survive into executable runtime topology.
- **X.10:** ✅ **CLOSED.** Enforced in `catalog.rs::register_compute()` through `map_compute_param_type()`, which rejects both `ValueType::Series` and `ValueType::String` for compute parameter metadata by returning `ValidationError::UnsupportedParameterType`. Anchor test: `series_parameter_type_rejected` in `catalog.rs`. Prior behavior silently coerced Series to Number(0.0); current registration rejects unsupported parameter types instead.
- **X.11:** ✅ **CLOSED.** Enforced in `execute.rs::map_to_compute_parameter_value()` (returns `None` for Int values where `|i| > 2^53`). Caller produces `ExecError::ParameterOutOfRange { node, parameter, value }`. Tests: `int_parameter_within_f64_exact_range_allowed`, `int_parameter_out_of_range_rejected`. Prior behavior silently converted all Int to f64, losing precision for large values.
- **X.12 / STRING-SOURCE-1:** ✅ **CLOSED.** `string_source` added to complete ValueType surface coverage. Prior state: `ValueType::String` existed in cluster/runtime types but `common::ValueType` lacked String, creating a gap where string outputs could be declared but never originated. Implementation required adding `common::ValueType::String` and `common::Value::String` variants, which triggered exhaustive match cascade across four mapping functions (`map_compute_param_type`, `map_compute_param_value`, `map_common_value_type`, `map_common_value`). Tests: `string_source_emits_configured_value`, `string_source_defaults_to_empty_string`.

---

### LAYER-* — Crate Boundary Invariants

**Status:** Enforced
**Location:** `tools/verify_layer_boundaries.sh`, integrated via `tools/verify_runtime_surface.sh`

| ID | Invariant | Enforcement |
|----|-----------|-------------|
| LAYER-1 | Kernel crates must not depend on `prod/*` or `shared/*` at runtime | CI script: dependency direction check |
| LAYER-2 | `RuleViolation` is kernel-owned; loader and clients must not define or return rule violations | CI script: `RuleViolation` import guard |
| LAYER-3 | Clients must not import loader/parser internals or perform canonical host orchestration directly | CI script: parser-internal + orchestration delegation guards |

**Rationale:**

- LAYER-1 ensures kernel semantics cannot be contaminated by client or tooling concerns. Kernel looks down to nothing; everything looks up to kernel.
- LAYER-2 ensures validation error types remain a kernel-owned contract. Loader errors are transport/decode failures, not rule violations.
- LAYER-3 ensures clients remain thin adapters. If a client needs canonical run, replay, validation, or manual stepping, it goes through the host API; if it needs parser access, it goes through the loader API rather than internal modules.

`shared/*` crates are allowed in kernel `[dev-dependencies]` for test infrastructure. This does not weaken runtime trust boundaries.

---

### NUM-FINITE-1 — Non-Finite Output Guard

**Status:** Enforced
**Location:** `crates/kernel/runtime/src/runtime/execute.rs`

Non-finite numeric values (NaN, inf, -inf) are rejected at the execution boundary before they can propagate to downstream nodes.

**Enforcement:**
**Scope:** Guards Source and Compute outputs only. Action outputs are not guarded because actions are terminal (F.2) and cannot feed downstream nodes. If future actions emit numeric boundary outputs, this scope may need revisiting.

- `ensure_finite()` checks Number and Series values for NaN/inf/-inf
- Guard called after Source outputs and after Compute outputs
- Violation produces `ExecError::NonFiniteOutput { node, port }` (rule_id: `"NUM-FINITE-1"`)

**Rationale:**

- Non-finite values cause counterintuitive trigger behavior (NaN comparisons always false)
- Prevents semantic corruption from reaching actions
- Defense in depth for implementation bugs

---

### B.2 — Divide-by-Zero Semantics

**Status:** Enforced
**Location:** `crates/kernel/runtime/src/compute/implementations/divide/impl.rs`

Division by zero produces `ComputeError::DivisionByZero`, not IEEE 754 inf/NaN.

**Enforcement:**

- In `divide/impl.rs::Divide::compute()`, the zero guard `if b == 0.0` returns `ComputeError::DivisionByZero`.
- In `divide/impl.rs::Divide::compute()`, the finite guard `if !result.is_finite()` returns `ComputeError::NonFiniteResult`.

**Implementations:**

- `divide` (v0.2.0): Math-true. Errors on `b == 0` or non-finite result.
- `safe_divide` (v0.1.0): Requires `fallback` parameter. Returns fallback on zero/non-finite.

**Rationale:**

- "Division by zero is undefined" is math, not policy
- Policy (what to substitute) belongs in `safe_divide`, not `divide`
- Preserves Non-Normative principle (ontology.md §1.4)
