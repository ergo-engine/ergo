---
Authority: CANONICAL
Version: v0.32
Owner: Claude (Structural Auditor)
Last Updated: 2026-02-17
Scope: Phase boundaries, enforcement loci, gap tracking
Change Rule: Operational log
---

# Phase Invariants — v0

**Tracked invariants:** 165

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
- `execution_model.md` (frozen)
- `V0_FREEZE.md` (frozen)
- `adapter_contract.md` (frozen)
- `SUPERVISOR.md` (frozen)
- `AUTHORING_LAYER.md` (stable)
- `CLUSTER_SPEC.md` (stable)
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
3. **Action implementations in core = zero** by design; capability atoms live in verticals

This freeze applies to:
- `src/source/`
- `src/compute/`
- `src/trigger/`
- `src/action/`
- `src/cluster.rs`
- `src/runtime/`

Doctrine documents (FROZEN/, STABLE/, CANONICAL/) retain their existing authority levels.

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

## Canonical Run / Replay Strictness (v1)

| ID | Invariant | Enforcement Locus | Status |
|----|-----------|-------------------|--------|
| RUN-CANON-1 | Canonical graph run requires explicit event source | CLI validation (`ergo run <graph.yaml>` requires `--fixture` unless `--direct`) | Enforced |
| RUN-CANON-2 | Adapter binding is mandatory only for adapter-dependent graphs | CLI dependency scan over expanded source/action manifests | Enforced |
| REP-7 | Strict replay requires provenance contract match | `supervisor::replay::replay_checked_strict` | Enforced |

Notes:
- Adapter-dependent graph detection is based on required source context keys and action writes.
- Adapter-independent canonical captures use explicit provenance sentinel `none`.
- Capture bundles are strict v1 (`capture_version: "v1"`): `adapter_provenance` is required, unknown fields are rejected, and legacy `adapter_version` bundles fail deserialization.

---

## 0. Cross-Phase Invariants

These invariants hold across all phases. Violation at any point is a system-level failure.

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| X.1 | Exactly four ontological primitives exist: Source, Compute, Trigger, Action | ontology.md §2 | `PrimitiveKind` enum | — | — | — |
| X.2 | Wiring matrix is never violated (see ontology.md §3) | ontology.md §3 | — | — | ✓ | ✓ |
| X.3 | All graphs are directed acyclic graphs (DAGs) | execution_model.md §2 | — | — | ✓ | ✓ |
| X.4 | Determinism: identical inputs + identical state → identical outputs | execution_model.md §8 | — | — | — | ✓ |
| X.5 | Actions are terminal; Action → * is forbidden | ontology.md §3 | — | — | ✓ | ✓ |
| X.6 | Sources have no inputs | ontology.md §2.1 | — | — | ✓ | ✓ |
| X.7 | Compute primitives have ≥1 input | ontology.md §2.2 | — | — | ✓ | ✓ |
| X.8 | Triggers emit events | ontology.md §2.3 | — | — | ✓ | ✓ |
| X.9 | Authoring constructs compile away before execution | V0_FREEZE.md §7 | — | ✓ | — | ✓ |
| X.10 | Compute parameter types must not include Series | (inferred) | — | — | ✓ | ✓ |
| X.11 | Int→f64 conversion must be exactly representable (\|i\| ≤ 2^53) | (inferred) | — | — | ✓ | ✓ |
| X.12 | Every ValueType has at least one source producer | (inferred) | — | — | — | ✓ |

### Notes

- **X.1:** Enforced by type system. `PrimitiveKind` enum has exactly four variants.
- **X.4:** Determinism is tested but not structurally enforced. Acceptable for v0.
- **X.7:** ✅ **CLOSED.** Enforced in `compute/registry.rs::validate_manifest` (returns `NoInputsDeclared` when `inputs.is_empty()` for Compute manifests). Test: `compute_with_zero_inputs_rejected` in `compute/registry.rs`.
- **X.9:** Requires assertion at execution entry that no `ClusterDefinition` or `NodeKind::Cluster` survives.
- **X.10:** ✅ **CLOSED.** Enforced in `catalog.rs::register_compute()` (returns `ValidationError::UnsupportedParameterType` when parameter has `ValueType::Series`). Test: `series_parameter_type_rejected` in `catalog.rs`. Prior behavior silently coerced Series to Number(0.0); now rejects at registration time.
- **X.11:** ✅ **CLOSED.** Enforced in `execute.rs::map_to_compute_parameter_value()` (returns `None` for Int values where `|i| > 2^53`). Caller produces `ExecError::ParameterOutOfRange { node, parameter, value }`. Tests: `int_parameter_within_f64_exact_range_allowed`, `int_parameter_out_of_range_rejected`. Prior behavior silently converted all Int to f64, losing precision for large values.
- **X.12 / STRING-SOURCE-1:** ✅ **CLOSED.** `string_source` added to complete ValueType surface coverage. Prior state: `ValueType::String` existed in cluster/runtime types but `common::ValueType` lacked String, creating a gap where string outputs could be declared but never originated. Implementation required adding `common::ValueType::String` and `common::Value::String` variants, which triggered exhaustive match cascade across four mapping functions (`map_compute_param_type`, `map_compute_param_value`, `map_common_value_type`, `map_common_value`). Tests: `string_source_emits_configured_value`, `string_source_defaults_to_empty_string`.

---

### NUM-FINITE-1 — Non-Finite Output Guard

**Status:** Enforced  
**Location:** `crates/runtime/src/runtime/execute.rs`

Non-finite numeric values (NaN, inf, -inf) are rejected at the execution boundary before they can propagate to downstream nodes.

**Enforcement:**
**Scope:** Guards Source and Compute outputs only. Action outputs are not guarded because actions are terminal (F.2) and cannot feed downstream nodes. If future actions emit numeric boundary outputs, this scope may need revisiting.
- `ensure_finite()` defined at line 296
- Number check at line 302
- Series check at line 308
- Guard called after Source outputs (line 143)
- Guard called after Compute outputs (line 201)
- Violation produces `ExecError::NonFiniteOutput { node, port }` (types.rs line 127)

**Rationale:**
- Non-finite values cause counterintuitive trigger behavior (NaN comparisons always false)
- Prevents semantic corruption from reaching actions
- Defense in depth for implementation bugs

---

### B.2 — Divide-by-Zero Semantics

**Status:** Enforced  
**Location:** `crates/runtime/src/compute/implementations/divide/impl.rs`

Division by zero produces `ComputeError::DivisionByZero`, not IEEE 754 inf/NaN.

**Enforcement:**
- Zero check at line 55
- Returns `DivisionByZero` at line 56
- Finite check at line 60
- Returns `NonFiniteResult` at line 61

**Implementations:**
- `divide` (v0.2.0): Math-true. Errors on `b == 0` or non-finite result.
- `safe_divide` (v0.1.0): Requires `fallback` parameter. Returns fallback on zero/non-finite.

**Rationale:**
- "Division by zero is undefined" is math, not policy
- Policy (what to substitute) belongs in `safe_divide`, not `divide`
- Preserves Non-Normative principle (ontology.md §1.4)

---

## 1. Definition Phase

**Scope:** When a cluster is authored and saved.

**Entry invariants:** None (this is the origin point for authoring).

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| D.1 | Cluster contains ≥1 node | CLUSTER_SPEC.md §6.1 | — | — | ✓ | ✓ |
| D.2 | All edges reference existing nodes and ports | CLUSTER_SPEC.md §6.1 | — | — | ✓ | ✓ |
| D.3 | All edges satisfy wiring matrix | CLUSTER_SPEC.md §6.1 | — | — | ✓ | ✓ |
| D.4 | Every output port references a valid internal node output | CLUSTER_SPEC.md §6.1 | — | — | ✓ | ✓ |
| D.5 | Every input port has a unique name | CLUSTER_SPEC.md §6.1 | — | — | ✓ | ✓ |
| D.6 | Every output port has a unique name | CLUSTER_SPEC.md §6.1 | — | — | ✓ | ✓ |
| D.7 | All parameters have valid types | CLUSTER_SPEC.md §6.1 | — | — | ✓ | ✓ |
| D.8 | Parameter defaults are type-compatible | CLUSTER_SPEC.md §6.1 | — | — | ✓ | ✓ |
| D.9 | No duplicate parameter names | CLUSTER_SPEC.md §6.1 | — | — | ✓ | ✓ |
| D.10 | If declared signature exists, it is compatible with inferred | CLUSTER_SPEC.md §4 | — | — | ✓ | ✓ |
| D.11 | Declared wireability cannot exceed inferred wireability | CLUSTER_SPEC.md §4, §6 | — | — | ✓ | ✓ |

### Notes

- **D.5–D.9:** Enforced in `cluster.rs::validate_cluster_definition` (returns `ExpandError::DuplicateInputPort|DuplicateOutputPort|DuplicateParameter|ParameterDefaultTypeMismatch`). Tests: `duplicate_input_ports_rejected`, `duplicate_output_ports_rejected`, `duplicate_parameters_rejected`, `parameter_default_type_mismatch_rejected`.
- **D.10–D.11:** Enforced during `expand()` via `infer_signature` + `validate_declared_signature` (`ExpandError::DeclaredSignatureInvalid`). Test: `declared_wireability_cannot_exceed_inferred`.

---

## 2. Instantiation Phase

**Scope:** When a cluster is placed in a parent context.

**Entry invariants:**
- Parent context exists and is valid
- Cluster definition passes Definition phase validation

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| I.1 | Wiring from parent edge source to cluster boundary kind is legal | CLUSTER_SPEC.md §6.2 | — | — | ✓ | ✓ |
| I.2 | Port types match at connection points | CLUSTER_SPEC.md §6.2 | — | — | ✓ | ✓ |
| I.3 | All required parameters are either bound or exposed | CLUSTER_SPEC.md §6.2 | — | — | ✓ | ✓ |
| I.4 | Bound parameter values are type-compatible | CLUSTER_SPEC.md §6.2 | — | — | ✓ | ✓ |
| I.5 | Exposed parameters reference parameters that exist in parent context | CLUSTER_SPEC.md §6.2 | — | — | ✓ | ✓ |
| I.6 | Version constraints are satisfied | CLUSTER_SPEC.md §6.2 | — | — | — | — |
| I.7 | Parameter bindings reference only declared parameters | CLUSTER_SPEC.md §6.2 | — | — | ✓ | ✓ |

### Notes

- **I.3–I.5:** Enforced in `cluster.rs::expand_with_context` during nested cluster processing via `validate_parameter_bindings()`. Errors: `MissingRequiredParameter`, `ParameterBindingTypeMismatch`, `ExposedParameterNotFound`, `ExposedParameterTypeMismatch`. Tests: `required_parameter_missing_rejected`, `parameter_binding_type_mismatch_rejected`, `exposed_parameter_not_in_parent_rejected`, `exposed_parameter_type_mismatch_rejected`. Note: I.4 is enforced symmetrically for both Literal and Exposed bindings.
  - **Strengthened (2025-01-05):** Exposed bindings now propagate through nested cluster hierarchies via `resolve_bindings_with_context()` and `build_resolved_params()`. Prior behavior only validated at immediate cluster boundary; multi-level nesting (Parent → Middle → Inner → Leaf) now correctly receives propagated values. Added `ExpandError::UnresolvedExposedBinding { node_id, parameter, referenced }` for primitives with dangling Exposed bindings. Tests: `exposed_binding_propagates_to_leaf_primitive`, `unresolved_exposed_binding_rejected`. Location: `cluster.rs:expand_with_context()`.
  - **Default application (2025-01-05):** Parameters with `default: Some(value)` in either `ParameterMetadata` (primitives) or `ParameterSpec` (clusters) are automatically applied during expansion when no binding is provided. Enforced by `resolve_impl_parameters()` (primitives, cluster.rs:988-1028) and `build_resolved_params()` (clusters, cluster.rs:1034-1074). Tests: `defaulted_parameter_propagates_to_leaf`, `explicit_binding_overrides_default`, `missing_required_param_no_default_rejected`, `cluster_parameter_default_propagates_to_nested`.
- **I.7:** Enforced in `cluster.rs` across three functions: `resolve_impl_parameters()` (primitive nodes), `build_resolved_params()` (nested cluster instantiation), and `validate_parameter_bindings()` (nested cluster pre-validation). Each builds a `HashSet` of declared parameter names from the target's spec and rejects any binding key absent from that set. Error: `ExpandError::UndeclaredParameter { node_id, parameter }`. Tests: `undeclared_primitive_parameter_binding_rejected`, `undeclared_cluster_parameter_binding_rejected`. Prior to this fix, undeclared bindings were silently dropped — a typo in a parameter name would cause the primitive to receive its default value with no error. See `ESCALATION_PARAM_SILENT_DROP.md` for the full finding.
- **I.6:** Version constraint validation **NOT IMPLEMENTED**. Cluster expansion performs exact-match lookup via `ClusterLoader::load()` and `PrimitiveCatalog::get()`. Constraint syntax (e.g., `>=1.0.0, <2.0.0`) is not parsed. See `TODO(I.6)` markers at `NodeKind::Impl` and `NodeKind::Cluster` resolution sites in `crates/runtime/src/cluster.rs`. Tracked in issue #6, deferred to future `version-constraints` branch.

---

## 3. Expansion Phase

**Scope:** Recursive flattening of clusters to primitives.

**Entry invariants:**
- All referenced clusters are loadable
- All parameters are concretely bound (no unresolved `Exposed` bindings at root)

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| E.1 | Output contains only primitives (no `NodeKind::Cluster` survives) | CLUSTER_SPEC.md §7 | — | ✓ | — | ✓ |
| E.2 | All placeholder edges are rewritten to node-to-node edges | CLUSTER_SPEC.md §7 | — | ✓ | — | ✓ |
| E.3 | `ExternalInput` does not appear as edge target (sink) | (inferred) | — | ✓ | — | — |
| E.4 | Authoring path is preserved for each expanded node | CLUSTER_SPEC.md §7.2 | — | — | — | ✓ |
| E.5 | Empty clusters are rejected | CLUSTER_SPEC.md §6.1 | — | — | ✓ | ✓ |
| E.6 | Original cluster definitions are not mutated | (inferred) | — | — | — | — |
| E.7 | `ExpandedGraph` carries boundary ports for inference only | (inferred) | — | — | — | — |
| E.8 | Runtime ID assignment is deterministic for identical definitions | (inferred) | — | — | ✓ | ✓ |
| E.9 | Referenced nested clusters exist | CLUSTER_SPEC.md §6.2 | — | — | ✓ | ✓ |

### Notes

- **E.3:** Requires assertion. Silent assumption is unacceptable.
- **E.6:** True by clone semantics but not explicitly enforced.
- **E.7:** Requires doc comment on `ExpandedGraph` to make contract explicit:

```rust
/// Expansion output. Contains only topology, primitive identity, and authoring trace.
/// `boundary_inputs` and `boundary_outputs` are retained for signature inference only
/// and must not influence runtime execution.
```

- **E.2:** ✅ Strengthened (2025-01-05). Boundary output mapping (`map_boundary_outputs`) and nested output mapping now return typed errors instead of silent fallback. Errors: `ExpandError::UnmappedBoundaryOutput { port_name, node_id }`, `ExpandError::UnmappedNestedOutput { cluster_id, port_name }`. Tests: `unmapped_boundary_output_rejected`, `nested_output_mapping_failure_rejected`.
- **E.8:** ✅ **CLOSED.** Enforced via sorted-key iteration in `expand_with_context` (cluster.rs:694-698). Test: `expansion_runtime_ids_deterministic`.
- **E.9:** Enforced in `cluster.rs::expand_with_context()` when resolving `NodeKind::Cluster` via `ClusterLoader::load`. Missing references return `ExpandError::MissingCluster { id, version }`. Test: `missing_nested_cluster_rejected`.

---

## 4. Inference Phase

**Scope:** Deriving signature from expanded graph.

**Entry invariants:**
- Expanded graph is complete (E.1–E.5 hold)
- `PrimitiveCatalog` is canonical and version-consistent

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| F.1 | Input ports are never wireable | CLUSTER_SPEC.md §3.2 | — | ✓ | — | ✓ |
| F.2 | Output wireability is determined by source node kind (Action → non-wireable) | CLUSTER_SPEC.md §3.2 | — | — | — | ✓ |
| F.3 | `BoundaryKind` inference follows precedence: ActionLike → SourceLike → TriggerLike → ComputeLike | CLUSTER_SPEC.md §3.4 | — | — | — | ✓ |
| F.4 | `has_side_effects` is true iff any expanded node is Action | CLUSTER_SPEC.md §3.3 | — | — | — | ✓ |
| F.5 | `is_origin` is true iff no inputs AND all roots are Sources | CLUSTER_SPEC.md §3.3 | — | — | — | ✓ |
| F.6 | Signature inference depends only on expanded graph + catalog (no other state) | (inferred) | — | — | — | — |

### Notes

- **F.1:** ✅ **CLOSED.** Fixed in cluster.rs. Enforcement:
  - Assertion: `debug_assert!` at cluster.rs:258
  - Test: `input_ports_are_never_wireable` at cluster.rs:1106
  - Merged.

- **F.6:** True by construction. Document on `infer_signature`:

```rust
/// Signature inference assumes a canonical, version-consistent PrimitiveCatalog.
/// Providing a catalog with different or incomplete primitive metadata will produce
/// undefined or incorrect signatures.
```

---

## 5. Validation Phase

**Scope:** Validating the unified DAG before execution.

**Entry invariants:**
- Graph is fully expanded (no clusters remain)
- Signature inference is complete

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| V.1 | No cycles exist in the graph | execution_model.md §2 | — | — | ✓ | ✓ |
| V.2 | All edges satisfy wiring matrix | ontology.md §3 | — | — | ✓ | ✓ |
| V.3 | All required inputs are connected | execution_model.md §2 | — | — | ✓ | ✓ |
| V.4 | All type constraints are satisfied at edges | CLUSTER_SPEC.md §6.3 | — | — | ✓ | ✓ |
| V.5 | All action nodes are gated by trigger events | ontology.md §3 | — | — | ✓ | ✓ |
| V.6 | All nodes pass validation before any action executes | execution_model.md §7 | — | — | ✓ | ✓ |
| V.7 | Each input port receives at most one inbound edge | (inferred) | — | — | ✓ | ✓ |
| V.8 | Referenced primitive implementations exist in catalog | CLUSTER_SPEC.md §6.3 | — | — | ✓ | ✓ |

### Notes

- Validation phase is well-covered by existing executor tests.
- **V.5:** Validation confirms structural wiring (Action has Trigger input). Runtime enforcement (R.7) additionally gates execution on `TriggerEvent::Emitted`. Both validation and runtime enforcement are now complete.
- **V.7:** ✅ **CLOSED.** Enforced in `runtime/validate.rs::enforce_single_edge_per_input()`. Returns `ValidationError::MultipleInboundEdges { node, input }` when multiple edges target same input port. Test: `validate_rejects_multiple_edges_to_same_input`.
  - **Prior behavior:** `execute.rs` used `HashMap::insert` for input collection; multiple edges to same input caused silent last-write-wins data loss.
  - **Rationale:** Silent data loss is truth-destroying. Aggregation semantics (Cardinality::Multiple) remain schema-placeholder only; if ever needed, require explicit v1 decision.
- **V.8:** Enforced at validation entry in `runtime/validate.rs`: each expanded node must resolve through `PrimitiveCatalog::get`, else `ValidationError::MissingPrimitive { id, version }`. Test: `validate_rejects_missing_primitive_metadata`.

---

## 6. Execution Phase

**Scope:** Running the validated graph.

**Entry invariants:**
- All V.* invariants hold
- State is initialized per lifecycle rules

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| R.1 | Each node executes at most once per pass | execution_model.md §1 | — | — | — | ✓ |
| R.2 | Nodes execute in topological order | execution_model.md §3 | — | — | — | ✓ |
| R.3 | No node observes effects from actions in same pass | execution_model.md §1 | — | — | ✓ | ✓ |
| R.4 | Action failure aborts subsequent actions in same pass | execution_model.md §7 | — | — | — | ✓ |
| R.5 | Triggers are stateless (TRG-STATE-1) | execution_model.md §5 | — | — | ✓ | ✓ |
| R.6 | Outputs are deterministic given inputs + state | execution_model.md §8 | — | — | — | ✓ |
| R.7 | Actions execute only when trigger event emitted | execution_model.md §7 | — | — | — | ✓ |

### Notes

- **R.3:** ✅ **CLOSED.** Compositionally enforced by existing invariants:
  - F.2: Action outputs are non-wireable (`cluster.rs:324: wireable = meta.kind != PrimitiveKind::Action`)
  - X.5: "Actions are terminal; Action → * is forbidden" (validated at D.3, V.2)
  - Since no edge can originate from an Action, no node can observe action effects.
  - No separate test needed — enforcement is structural via wiring matrix validation.
- **R.4:** ✅ **CLOSED (by design).** `Result::Err` propagation via `?` is sufficient. `ActionOutcome::Failed` is data, not control flow — structural halt must be expressed via Trigger gating/wiring, not implicit runtime payload semantics.
- **R.5 / TRG-STATE-1:** ✅ **CLOSED.** Triggers are ontologically stateless.

### TRG-STATE-1: Triggers are stateless

| Aspect | Specification |
|--------|---------------|
| **Invariant** | Trigger implementations must not use observable, preservable, or causally meaningful state |
| **Enforcement** | Manifest: `state: StateSpec { allowed: false }` required for all triggers |
| **Locus** | Registry validation at registration time; manifest schema |
| **Violation** | Trigger with `allowed: true` rejected by registry |

**Rationale:** Triggers are ontologically stateless. A Trigger gates whether an Action
may attempt to affect the external world. It does not store information, accumulate
history, or own temporal memory. Execution-local bookkeeping (ephemeral scratch data
during evaluation) is permitted but does not constitute state — it is not observable,
serializable, or preserved across evaluations.

**Canonical Boundary Rule:** Execution may use memory. The system may never observe,
preserve, or depend on that memory.

**Temporal patterns** (once, count, latch, debounce) requiring cross-evaluation memory
must be implemented as clusters with explicit state flow through environment.

**Authority:** Sebastian (Freeze Authority), 2025-12-28

- **Enforcement locus confirmed (2025-01-05):** Statelessness is enforced at two levels:
  1. **Type system:** `TriggerPrimitive::evaluate()` signature takes `&self` (not `&mut self`), no state parameter, no `PrimitiveState` argument. State cannot be smuggled through trait API.
  2. **Registry validation:** `TriggerRegistry::validate_manifest()` rejects any trigger with `state.allowed = true` (returns `StatefulTriggerNotAllowed`). Test: `trg_state_1_stateful_trigger_rejected`.

- **R.7:** ✅ **CLOSED.** Runtime gates Action execution on `TriggerEvent::Emitted`. Implementation:
  - `should_skip_action()` in execute.rs checks for any `TriggerEvent::NotEmitted` input (AND semantics)
  - Skipped actions return `ActionOutcome::Skipped` for Event outputs
  - Test: `r7_action_skipped_when_trigger_not_emitted` verifies enforcement
  - **Strengthened (2025-01-05):** `map_to_action_value()` now uses explicit pattern matching on `TriggerEvent::Emitted` and `TriggerEvent::NotEmitted` rather than wildcard. NotEmitted case includes `unreachable!("R.7 violation: NotEmitted must be caught by should_skip_action")` to prevent silent acceptance of future TriggerEvent variants. Location: `execute.rs:345-351`.

---

## 7. Orchestration Phase

**Scope:** Supervisor scheduling of episodes.

**Source:** SUPERVISOR.md (frozen)

**Entry invariants:**
- Graph is validated (all V.* invariants hold)
- Adapter is available and compliant

### Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| CXT-1 | ExecutionContext is adapter-only | SUPERVISOR.md §3 | ✓ | — | — | ✓ |
| SUP-1 | Supervisor is graph-identity fixed | SUPERVISOR.md §3 | ✓ | — | — | — |
| SUP-2 | Supervisor is strategy-neutral | SUPERVISOR.md §3 | ✓ | — | — | ✓ |
| SUP-3 | Supervisor decisions are replayable | SUPERVISOR.md §3 | — | — | — | ✓ |
| SUP-4 | Retries only on mechanical failure | SUPERVISOR.md §3 | ✓ | — | — | ✓ |
| SUP-5 | ErrKind is mechanical only | SUPERVISOR.md §3 | ✓ | — | — | — |
| SUP-6 | Episode atomicity is invocation-scoped | SUPERVISOR.md §3 | — | — | — | — |
| SUP-7 | DecisionLog is write-only | SUPERVISOR.md §3 | ✓ | — | — | ✓ |
| SUP-TICK-1 | Tick events use deferred-retry scheduling | — | — | — | — | ✓ |
| RTHANDLE-ID-1 | RuntimeHandle discards graph_id and event_id | — | ✓ | — | — | ✓ |
| RTHANDLE-ERRKIND-1 | Pre-execution failures map to ValidationFailed, not RuntimeError or SemanticError | SUPERVISOR.md §2.4 | — | — | ✓ | ✓ |

### Notes

- **CXT-1:** `pub(crate)` constructor; compile_fail doctests verify no external construction.
- **SUP-1:** Private `graph_id` field with no setters; set only at construction.
- **SUP-2:** `RuntimeInvoker::run()` returns `RunTermination` only; no `RunResult` exposure.
- **SUP-4:** `should_retry()` matches only `NetworkTimeout|AdapterUnavailable|RuntimeError|TimedOut`.
- **SUP-5:** `ErrKind` enum contains only mechanical variants; no domain-flavored errors.
- **SUP-7:** `DecisionLog` trait has only `fn log()`; `records()` is on concrete impl, not trait.
- **SUP-TICK-1:** Tick events have special deferred-retry behavior distinct from Command events. Test: `replay_harness.rs` uses Command (not Tick) to avoid interference.
- **RTHANDLE-ID-1:** `RuntimeHandle::run()` explicitly discards `graph_id` and `event_id` parameters (adapter/lib.rs:234-235). Only `ctx.inner()` is passed to underlying runtime. This ensures replay determinism — fault injection keys on EventId only (REP-3).
- **RTHANDLE-ERRKIND-1:** ✅ **CLOSED (2026-02-06).** `RuntimeHandle::run()` maps pre-execution failures to `ErrKind::ValidationFailed`, not `RuntimeError` or `SemanticError`.
  - **Prior bug (runtime_validate path):** `runtime_validate()` errors mapped to `ErrKind::RuntimeError`. Since `should_retry()` treats `RuntimeError` as retryable, this caused **pathological retries** of structurally invalid graphs — a graph that fails validation will fail identically on every retry.
  - **Prior bug (validate_composition path):** `validate_composition()` errors mapped to `ErrKind::SemanticError`. Non-retryable (correct behavior), but **wrong category** — `SemanticError` is for runtime deterministic failures (DivisionByZero, NonFiniteOutput per B.2), not validation-time COMP-* checks.
  - **Fix:** Both paths now return `ErrKind::ValidationFailed`, which is non-retryable (`should_retry` returns `false`) and categorically correct per SUPERVISOR.md §2.4.
  - **Note:** `ErrKind::ValidationFailed` was defined since v0 but never instantiated until this fix. Both error paths should have used it from the start.
  - **Test:** `runtime_handle_rejects_required_context_when_provides_empty` updated to assert `ValidationFailed`.

---

## 8. Replay Phase

**Scope:** Deterministic capture and verification of episode execution.

**Source:** SUPERVISOR.md §2.5, crates/adapter/src/capture.rs, crates/supervisor/src/replay.rs

**Entry invariants:**
- Capture bundle is well-formed
- All recorded events have valid hashes

### Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| REP-1 | Capture records are self-validating | — | — | — | ✓ | ✓ |
| REP-2 | Rehydration is deterministic | — | — | — | — | ✓ |
| REP-3 | Fault injection keys on EventId only | — | ✓ | — | — | ✓ |
| REP-4 | Capture/runtime type separation | — | ✓ | — | — | — |
| REP-5 | No wall-clock time in supervisor | — | — | — | — | ✓ |
| REP-6 | Stateful trigger state captured for replay | N/A | N/A | N/A | N/A | ✅ CLOSED BY CLARIFICATION |
| REP-SCOPE | Replay covers supervisor scheduling only | — | — | — | — | — |
| SOURCE-TRUST | Source determinism is trust-based | — | — | — | — | — |

### Notes

- **REP-1:** `validate_hash()` in capture.rs uses SHA256 to verify payload integrity. **ENFORCED** at `replay.rs` via `validate_bundle()` (called by `replay_checked()`). Legacy `replay()` panics on invalid bundle; `replay_checked()` returns `Result<_, ReplayError>` for graceful handling.
  - **Anchor tests:** `replay_rejects_corrupted_bundle`, `replay_rejects_unknown_version`
  - **v0.18:** Enforcement strengthened — `rehydrate_checked()` now called at point-of-use in supervisor replay path (`replay_inner()`). See REP-1b in closure register.
- **REP-2:** `rehydrate()` uses only record fields; no external state dependency.
- **REP-3:** `FaultRuntimeHandle` explicitly discards `graph_id` and `ctx.inner()`; keys on `EventId` only.
- **REP-4:** `ExecutionContext` has no serde derives. Capture types (`ExternalEventRecord`, `EpisodeInvocationRecord`) are separate from runtime types (`ExternalEvent`, `DecisionLogEntry`).
- **REP-5:** Test at `replay_harness.rs:150-157` enforces no `SystemTime` usage in supervisor.
- **REP-6:** ✅ **CLOSED BY CLARIFICATION (2025-12-28)**

**Resolution:** Prior documentation suggesting "triggers may hold internal state" was a
semantic error that conflated execution-local bookkeeping with ontological state.

Triggers are stateless (see TRG-STATE-1). There is no trigger state to capture. Temporal
patterns requiring memory (once, count, latch, debounce) must be implemented as clusters
with explicit state flow through environment (Source reads state, Action writes state).

Replay determinism is preserved by existing adapter capture (REP-1 through REP-5). No
additional capture mechanism is required.

**Authority:** Sebastian (Freeze Authority), 2025-12-28

- **REP-SCOPE:** Replay determinism covers supervisor scheduling decisions only. It does not capture or replay the internal execution of the runtime graph. Source outputs, compute results, and action side effects are not recorded. Replay verifies that given the same external events, the supervisor makes identical scheduling decisions.
- **SOURCE-TRUST:** Source primitive determinism is trust-based, not enforced. The `SourcePrimitiveManifest` declares `execution.deterministic = true`, but the trait has no compile-time restrictions preventing non-deterministic implementations. Enforcement is by convention and code review. See `source/registry.rs::validate_manifest()`.

### UI-REF-CLIENT-1: UI Authoring is Non-Canonical

**Status:** Documented
**Enforcement:** Convention

The `crates/reference-client` crate is a **reference client** demonstrating how to construct and emit `ExpandedGraph` payloads. It is NOT:

- A canonical contract implementation
- An enforcement boundary
- A required dependency for runtime execution

Contract authority remains with Rust types + `UI_RUNTIME_CONTRACT.md`. TypeScript types are best-effort mirrors.

---

## Supervisor + Replay Freeze Declaration

**Effective:** 2025-12-27

The Orchestration Phase (§7) and Replay Phase (§8) implementations are frozen at this point. The following constraints are now in force:

1. **CXT-1 through SUP-7** are enforced as specified in SUPERVISOR.md
2. **REP-1 through REP-5** are enforced via capture.rs and replay.rs
3. **Capture schema** (`ExternalEventRecord`, `EpisodeInvocationRecord`) is stable
4. **Replay harness API** (`replay()`, `rehydrate()`, `validate_hash()`) is stable

This freeze applies to:
- `crates/adapter/src/lib.rs` (ExternalEvent, ExecutionContext, RuntimeInvoker, FaultRuntimeHandle)
- `crates/adapter/src/capture.rs`
- `crates/supervisor/src/lib.rs` (Supervisor, DecisionLog, DecisionLogEntry)
- `crates/supervisor/src/replay.rs`

**To unfreeze:** Requires joint escalation per AGENT_CONTRACT.md v1.1.

---

## 9. Adapter Registration Phase

**Scope:** When an adapter manifest is registered with the system.

**Source:** EXTENSION_CONTRACTS_ROADMAP.md Phase 1, adapter.md (stable)

**Entry invariants:**
- Manifest is parseable YAML/JSON
- Required fields are present (serde validation)

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| ADP-1 | ID format valid | adapter.md #ADP-1 | — | — | ✓ | adp_1_invalid_id_rejected |
| ADP-2 | Version valid semver | adapter.md #ADP-2 | — | — | ✓ | adp_2_invalid_version_rejected |
| ADP-3 | Runtime compatibility satisfied | adapter.md #ADP-3 | — | — | ✓ | adp_3_incompatible_runtime_rejected |
| ADP-4 | Provides something | adapter.md #ADP-4 | — | — | ✓ | adp_4_empty_adapter_rejected |
| ADP-5 | Context key names unique | adapter.md #ADP-5 | — | — | ✓ | adp_5_duplicate_context_key_rejected |
| ADP-6 | Context key types valid | adapter.md #ADP-6 | — | — | ✓ | adp_6_invalid_context_type_rejected |
| ADP-7 | Event kind names unique | adapter.md #ADP-7 | — | — | ✓ | adp_7_duplicate_event_kind_rejected |
| ADP-8 | Event schemas valid JSON Schema | adapter.md #ADP-8 | — | — | ✓ | adp_8_invalid_schema_rejected |
| ADP-9 | Capture format version present | adapter.md #ADP-9 | — | — | ✓ | adp_9_no_capture_format_rejected |
| ADP-10 | Capture fields referentially valid | adapter.md #ADP-10 | — | — | ✓ | adp_10_invalid_capture_field_rejected |
| ADP-11 | Writable flag must be present | adapter.md #ADP-11 | — | — | ✓ | adp_11_missing_writable_flag_rejected |
| ADP-12 | Effect names unique | adapter.md #ADP-12 | — | — | ✓ | adp_12_duplicate_effect_name_rejected |
| ADP-13 | Effect schemas valid | adapter.md #ADP-13 | — | — | ✓ | adp_13_invalid_effect_schema_rejected |
| ADP-14 | Writable implies set_context accepted | adapter.md #ADP-14 | — | — | ✓ | adp_14_writable_without_set_context_rejected |
| ADP-15 | Writable keys must be capturable | adapter.md #ADP-15 | — | — | — | — |
| ADP-16 | Write effect must be capturable | adapter.md #ADP-16 | — | — | — | — |
| ADP-17 | Writable keys cannot be required | adapter.md #ADP-17 | — | — | ✓ | adp_17_writable_key_required_rejected |

### Notes

- **ADP-15/ADP-16:** Deferred until REP-SCOPE expansion to include context/effect capture fields.
- **Enforcement location:** `crates/adapter/src/validate.rs`
- **Test location:** `crates/adapter/tests/validation.rs`

---

## 10. Adapter Composition Phase

**Scope:** When sources or actions are composed with an adapter.

**Source:** EXTENSION_CONTRACTS_ROADMAP.md Phase 1, adapter.md (stable)

**Entry invariants:**
- Adapter passes registration validation (ADP-* rules)
- Source/action manifests pass their registration validation

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| COMP-1 | Source context requirements satisfied | adapter.md #COMP-1 | — | — | ✓ | comp_1_missing_context_key_rejected |
| COMP-2 | Source context types match | adapter.md #COMP-2 | — | — | ✓ | comp_2_context_type_mismatch_rejected |
| COMP-3 | Capture format version supported | adapter.md #COMP-3 | — | — | ✓ | comp_3_unsupported_capture_format_rejected |

### Notes

- **COMP-1, COMP-2:** Only keys with `required: true` in source requirements must exist in adapter provides.
- **Enforcement location:** `crates/adapter/src/composition.rs`
- **Test location:** `crates/adapter/tests/composition_tests.rs`

---

## 11. Source Registration Phase

**Scope:** When a source manifest is registered with the system.

**Source:** EXTENSION_CONTRACTS_ROADMAP.md Phase 2, source.md (stable)

**Entry invariants:**
- Manifest is parseable
- Required fields are present

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| SRC-1 | ID format valid | source.md #SRC-1 | — | — | ✓ | src_1_invalid_id_rejected |
| SRC-2 | Version valid semver | source.md #SRC-2 | — | — | ✓ | src_2_invalid_version_rejected |
| SRC-3 | Kind is "source" | source.md #SRC-3 | — | — | ✓ | src_3_kind_source_accepted |
| SRC-4 | No inputs declared | source.md #SRC-4 | — | — | ✓ | src_4_source_has_inputs_rejected |
| SRC-5 | At least one output | source.md #SRC-5 | — | — | ✓ | src_5_no_outputs_rejected |
| SRC-6 | Output names unique | source.md #SRC-6 | — | — | ✓ | src_6_duplicate_output_rejected |
| SRC-7 | Output types valid | source.md #SRC-7 | — | — | ✓ | src_7_output_types_valid |
| SRC-8 | State not allowed | source.md #SRC-8 | — | — | ✓ | src_8_source_has_state_rejected |
| SRC-9 | Side effects not allowed | source.md #SRC-9 | — | — | ✓ | src_9_source_has_side_effects_rejected |
| SRC-10 | Required context keys exist in adapter | source.md #SRC-10 | — | — | ✓ | src_10_missing_context_key_rejected |
| SRC-11 | Required context types match adapter | source.md #SRC-11 | — | — | ✓ | src_11_context_type_mismatch_rejected |
| SRC-12 | Execution deterministic | source.md #SRC-12 | — | — | ✓ | src_12_non_deterministic_execution_rejected |
| SRC-13 | Cadence is continuous | source.md #SRC-13 | — | — | ✓ | (structurally enforced) |
| SRC-14 | ID unique in registry | source.md #SRC-14 | — | — | ✓ | src_14_duplicate_id_rejected |
| SRC-15 | Parameter default type matches declared type | source.md #SRC-15 | — | — | ✓ | src_15_invalid_parameter_type_default_rejected |

### Notes

- **SRC-1 through SRC-9, SRC-12, SRC-14, SRC-15:** Registration-time manifest validation.
- **SRC-13:** Structurally enforced — `Cadence` enum only has `Continuous` variant. Enforcement code at `registry.rs:77-78` will be exercised when cadence variants expand.
- **SRC-10/SRC-11:** Composition-time validation. Same predicate and enforcement as COMP-1/COMP-2 (§10). Alias tests provide source-contract traceability.
- **Registration enforcement location:** `crates/runtime/src/source/registry.rs`
- **Registration test location:** `crates/runtime/src/source/tests.rs`
- **Composition enforcement location:** `crates/adapter/src/composition.rs` (invoked by `ergo_adapter::RuntimeHandle::run` after graph validation, before execution)
- **Composition test location:** `crates/adapter/tests/composition_tests.rs`

---

## 12. Compute Registration Phase

**Scope:** When a compute manifest is registered with the system.

**Source:** EXTENSION_CONTRACTS_ROADMAP.md Phase 3, compute.md (stable)

**Entry invariants:**
- Manifest is parseable
- Required fields are present

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| CMP-1 | ID format valid | compute.md #CMP-1 | — | — | ✓ | cmp_1_invalid_id_rejected |
| CMP-2 | Version valid semver | compute.md #CMP-2 | — | — | ✓ | cmp_2_invalid_version_rejected |
| CMP-3 | Kind is \"compute\" | compute.md #CMP-3 | ✓ | — | — | cmp_3_kind_compute_accepted |
| CMP-4 | At least one input | compute.md #CMP-4 | — | — | ✓ | cmp_4_no_inputs_rejected |
| CMP-5 | Input names unique | compute.md #CMP-5 | — | — | ✓ | cmp_5_duplicate_inputs_rejected |
| CMP-6 | At least one output | compute.md #CMP-6 | — | — | ✓ | cmp_6_no_outputs_rejected |
| CMP-7 | Output names unique | compute.md #CMP-7 | — | — | ✓ | cmp_7_duplicate_outputs_rejected |
| CMP-8 | Side effects not allowed | compute.md #CMP-8 | — | — | ✓ | cmp_8_side_effects_rejected |
| CMP-9 | State resettable if allowed | compute.md #CMP-9 | — | — | ✓ | cmp_9_state_not_resettable_rejected |
| CMP-10 | Errors deterministic | compute.md #CMP-10 | — | — | ✓ | cmp_10_non_deterministic_errors_rejected |
| CMP-11 | All outputs produced on success | compute.md #CMP-11 | — | — | ✓ | cmp_11_missing_output_fails |
| CMP-12 | No outputs produced on error | compute.md #CMP-12 | ✓ | — | — | cmp_12_compute_error_fails |
| CMP-13 | Input types valid | compute.md #CMP-13 | — | — | ✓ | cmp_13_invalid_input_type_rejected |
| CMP-14 | Input cardinality single | compute.md #CMP-14 | — | — | ✓ | cmp_14_invalid_input_cardinality_rejected |
| CMP-15 | Parameter types valid | compute.md #CMP-15 | — | — | ✓ | cmp_15_invalid_parameter_type_rejected |
| CMP-16 | Cadence is continuous | compute.md #CMP-16 | — | — | ✓ | cmp_16_invalid_cadence_rejected |
| CMP-17 | Execution deterministic | compute.md #CMP-17 | — | — | ✓ | cmp_17_non_deterministic_execution_rejected |
| CMP-18 | ID unique in registry | compute.md #CMP-18 | — | — | ✓ | cmp_18_duplicate_id_rejected |
| CMP-19 | Parameter default type matches declared type | compute.md #CMP-19 | — | — | ✓ | cmp_19_invalid_parameter_type_default_rejected |
| CMP-20 | Output types valid | compute.md #CMP-20 | ✓ | — | — | cmp_20_output_types_valid |

### Notes

- **CMP-11/12:** Enforced at execution in `crates/runtime/src/runtime/execute.rs`. CMP-12 is structural — `compute()` returns `Result<Outputs, ComputeError>`, so errors have no outputs by construction.
- **Registration enforcement location:** `crates/runtime/src/compute/registry.rs`
- **Registration test location:** `crates/runtime/src/compute/registry.rs`
- **Execution test location:** `crates/runtime/src/runtime/tests.rs`
- **Composition rules (COMP-5/COMP-6):** enforced by Validation Phase invariant **V.4** (`ValidationError::TypeMismatch`) in `crates/runtime/src/runtime/validate.rs`.
- **CMP-19:** Enforced in `compute/registry.rs::validate_manifest` by rejecting manifests where a parameter default value type does not match the declared parameter type (`ValidationError::InvalidParameterType`).
- **CMP-20:** Structurally enforced in `compute/registry.rs::validate_manifest` by exhaustive `ValueType` matching for outputs (`Number | Series | Bool | String`).

---

## 13. Trigger Registration Phase

**Scope:** When a trigger manifest is registered with the system.

**Source:** EXTENSION_CONTRACTS_ROADMAP.md Phase 4, trigger.md (stable)

**Entry invariants:**
- Manifest is parseable
- Required fields are present

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| TRG-1 | ID format valid | trigger.md #TRG-1 | — | — | ✓ | trg_1_invalid_id_rejected |
| TRG-2 | Version valid semver | trigger.md #TRG-2 | — | — | ✓ | trg_2_invalid_version_rejected |
| TRG-3 | Kind is \"trigger\" | trigger.md #TRG-3 | ✓ | — | — | trg_3_kind_trigger_accepted |
| TRG-4 | At least one input | trigger.md #TRG-4 | — | — | ✓ | trg_4_no_inputs_rejected |
| TRG-5 | Input names unique | trigger.md #TRG-5 | — | — | ✓ | trg_5_duplicate_input_rejected |
| TRG-6 | Input types valid | trigger.md #TRG-6 | ✓ | — | — | trg_6_input_types_valid |
| TRG-7 | Exactly one output | trigger.md #TRG-7 | — | — | ✓ | trg_7_wrong_output_count_rejected |
| TRG-8 | Output is event type | trigger.md #TRG-8 | — | — | ✓ | trg_8_output_not_event_rejected |
| TRG-9 | State not allowed | trigger.md #TRG-9 | — | — | ✓ | trg_9_trigger_has_state_rejected |
| TRG-10 | Side effects not allowed | trigger.md #TRG-10 | — | — | ✓ | trg_10_trigger_has_side_effects_rejected |
| TRG-11 | Execution deterministic | trigger.md #TRG-11 | — | — | ✓ | trg_11_non_deterministic_execution_rejected |
| TRG-12 | Input cardinality single | trigger.md #TRG-12 | — | — | ✓ | trg_12_invalid_input_cardinality_rejected |
| TRG-13 | ID unique in registry | trigger.md #TRG-13 | — | — | ✓ | trg_13_duplicate_id_rejected |
| TRG-14 | Parameter default type matches declared type | trigger.md #TRG-14 | — | — | ✓ | trg_14_invalid_parameter_type_default_rejected |

### Notes

- **TRG-9 link:** TRG-STATE-1 (stateless triggers) remains enforced by registry validation.
- **TRG-14:** Enforced in `trigger/registry.rs::validate_manifest` by rejecting manifests where a parameter default value type does not match the declared parameter type (`TriggerValidationError::InvalidParameterType`).
- **Registration enforcement location:** `crates/runtime/src/trigger/registry.rs`
- **Registration test location:** `crates/runtime/src/trigger/registry.rs`
- **Composition rules (COMP-7/COMP-8):** enforced by Validation Phase invariant **V.2** (wiring matrix) in `crates/runtime/src/runtime/validate.rs`.

---

## 14. Action Registration Phase

**Scope:** When an action manifest is registered with the system.

**Source:** EXTENSION_CONTRACTS_ROADMAP.md Phase 5, action.md (stable)

**Entry invariants:**
- Manifest is parseable
- Required fields are present

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| ACT-1 | ID format valid | action.md #ACT-1 | — | — | ✓ | act_1_invalid_id_rejected |
| ACT-2 | Version valid semver | action.md #ACT-2 | — | — | ✓ | act_2_invalid_version_rejected |
| ACT-3 | Kind is "action" | action.md #ACT-3 | ✓ | — | — | act_3_kind_action_accepted |
| ACT-4 | At least one event input | action.md #ACT-4 | — | — | ✓ | act_4_no_event_input_rejected |
| ACT-5 | Input names unique | action.md #ACT-5 | — | — | ✓ | act_5_duplicate_input_rejected |
| ACT-6 | Input types valid | action.md #ACT-6 | ✓ | — | — | act_6_input_types_valid |
| ACT-7 | Exactly one output | action.md #ACT-7 | — | — | ✓ | act_7_wrong_output_count_rejected |
| ACT-8 | Output named "outcome" | action.md #ACT-8 | — | — | ✓ | act_8_output_not_outcome_rejected |
| ACT-9 | Output is event type | action.md #ACT-9 | — | — | ✓ | act_9_output_not_event_rejected |
| ACT-10 | State not allowed | action.md #ACT-10 | — | — | ✓ | act_10_action_has_state_rejected |
| ACT-11 | Side effects required | action.md #ACT-11 | — | — | ✓ | act_11_action_no_side_effects_rejected |
| ACT-12 | Gated by trigger | action.md #ACT-12 | — | — | ✓ | act_12_action_not_gated_rejected |
| ACT-13 | Effects block present | action.md #ACT-13 | ✓ | — | — | act_3_kind_action_accepted |
| ACT-14 | Write names unique | action.md #ACT-14 | — | — | ✓ | act_14_duplicate_write_name_rejected |
| ACT-15 | Write types valid | action.md #ACT-15 | — | — | ✓ | act_15_invalid_write_type_rejected |
| ACT-16 | Retryable false | action.md #ACT-16 | — | — | ✓ | act_16_retryable_not_allowed_rejected |
| ACT-17 | Execution deterministic | action.md #ACT-17 | — | — | ✓ | act_17_non_deterministic_execution_rejected |
| ACT-18 | ID unique in registry | action.md #ACT-18 | — | — | ✓ | act_18_duplicate_id_rejected |
| ACT-19 | Parameter default type matches declared type | action.md #ACT-19 | — | — | ✓ | act_19_invalid_parameter_type_default_rejected |

### Notes

- **ACT-12:** Enforced during graph validation in `crates/runtime/src/runtime/validate.rs` (ValidationError::ActionNotGated).
- **ACT-19:** Enforced in `action/registry.rs::validate_manifest` by rejecting manifests where a parameter default value type does not match the declared parameter type (`ActionValidationError::InvalidParameterType`).
- **Registration enforcement location:** `crates/runtime/src/action/registry.rs`
- **Registration test location:** `crates/runtime/src/action/registry.rs`
- **Validation test location:** `crates/runtime/src/runtime/tests.rs`

---

## 15. Action Composition Phase

**Scope:** When actions that declare writes are composed with an adapter.

**Source:** EXTENSION_CONTRACTS_ROADMAP.md Phase 5, action.md (stable)

**Entry invariants:**
- Adapter passes registration validation (ADP-* rules)
- Action manifests pass their registration validation

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| COMP-11 | Action writes target provided keys | action.md #COMP-11 | — | — | ✓ | comp_11_write_target_not_provided_rejected |
| COMP-12 | Action writes only writable keys | action.md #COMP-12 | — | — | ✓ | comp_12_write_target_not_writable_rejected |
| COMP-13 | Action write types match | action.md #COMP-13 | — | — | ✓ | comp_13_write_type_mismatch_rejected |
| COMP-14 | If action writes, adapter accepts set_context | action.md #COMP-14 | — | — | ✓ | comp_14_missing_set_context_rejected |

### Notes

- **COMP-9/COMP-10:** Enforced by Validation Phase invariant **V.2** (wiring matrix) in `crates/runtime/src/runtime/validate.rs`.
- **COMP-15:** Deferred until REP-SCOPE expansion (capture includes context/effect).
- **Enforcement location:** `crates/adapter/src/composition.rs` (invoked by `ergo_adapter::RuntimeHandle::run`).
- **Test location:** `crates/adapter/tests/composition_tests.rs`

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
- `execution_model.md`
- `V0_FREEZE.md`
- `adapter_contract.md`
- `SUPERVISOR.md`

And the stable specification set:
- `AUTHORING_LAYER.md`
- `CLUSTER_SPEC.md`

Changes to this document require the same review bar as changes to frozen specs.

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| v0.1 | 2025-01-XX | Claude (Structural Auditor) | Initial draft |
| v0.2 | 2025-01-XX | Claude Prime | F.1 closed — merged to cluster.rs |
| v0.3 | 2025-12-21 | Claude Code | E.3, E.7, D.11 closed; D.11 validation added; gap summary corrected |
| v0.4 | 2025-12-21 | Claude Code | X.9 closed — structurally enforced by type system |
| v0.5 | 2025-12-21 | Claude Code | F.6 closed — documented on infer_signature |
| v0.6 | 2025-12-21 | Claude Code | R.3 closed — compositionally enforced via F.2, X.5 |
| v0.7 | 2025-12-22 | Claude Code | X.7 closed — validation added to compute/registry.rs; R.4 closed by design |
| v0.8 | 2025-12-22 | Claude Prime | Core v0.1 freeze declared |
| v0.9 | 2025-12-27 | Claude Prime | Added Orchestration Phase (CXT-1, SUP-1–7) and Replay Phase (REP-1–5) |
| v0.10 | 2025-12-27 | Claude Prime | Supervisor + Replay freeze declaration (Stage C complete); Stage D verification declared |
| v0.11 | 2025-12-28 | Claude Prime | R.7 violation detected (Action gating); REP-6 gap added (stateful trigger capture); V.5 note updated |
| v0.12 | 2025-12-28 | Claude Code | R.7 closed — runtime gating implemented; ActionOutcome::Skipped added; test added |
| v0.13 | 2025-12-28 | Claude Code | TRG-STATE-1 added — triggers are stateless; R.5 updated; REP-6 closed by clarification |
| v0.14 | 2025-01-05 | Claude Code | V.7 added (single edge per input); R.7 strengthened (explicit TriggerEvent matching); I.3-I.5 strengthened (nested Exposed binding propagation); TRG-STATE-1 enforcement locus confirmed |
| v0.15 | 2025-01-05 | Claude Code | Audit #2 closures: E.8 added (deterministic runtime IDs); I.3 strengthened (default application); E.2 strengthened (mapping failures explicit) |
| v0.16 | 2025-01-05 | Claude Code | X.10 added: reject Series compute parameters at registration (Codex audit finding) |
| v0.17 | 2025-01-05 | Claude Code | X.11 added: guard Int→f64 conversion for exact representability (Codex audit finding) |
| v0.18 | 2025-01-05 | Claude Code | REP-1 strengthened: point-of-use hash verification in supervisor replay path (REP-1b) |
| v0.19 | 2025-01-05 | Claude Code | Added SUP-TICK-1, RTHANDLE-ID-1 (orchestration); REP-SCOPE, SOURCE-TRUST (replay scope/trust documentation) |
| v0.20 | 2026-01-05 | Claude Code | Added UI-REF-CLIENT-1: reference-client reframed as reference client |
| v0.21 | 2026-01-05 | Claude Code | Added X.12 / STRING-SOURCE-1: ValueType surface coverage complete with string_source primitive |
| v0.22 | 2026-01-21 | Claude Code | Added ADP-1..17 (Adapter Registration Phase) and COMP-1..3 (Adapter Composition Phase); +18 invariants |
| v0.23 | 2026-02-01 | Claude Code | Added SRC-1..11 (Source Registration Phase); +11 invariants |
| v0.24 | 2026-02-02 | Codex | Added CMP-1..17 (Compute Registration Phase); +17 invariants |
| v0.25 | 2026-02-02 | Codex | Added TRG-1..12 (Trigger Registration Phase); +12 invariants |
| v0.26 | 2026-02-02 | Codex | Added ACT-1..17 (Action Registration Phase) and COMP-11..14 (Action Composition Phase); +21 invariants |
| v0.27 | 2026-02-02 | Claude Code | Added I.7 (undeclared parameter binding rejection); +1 invariant; closes ESCALATION_PARAM_SILENT_DROP |
| v0.28 | 2026-02-06 | Codex | Added SRC-14, CMP-18, TRG-13, ACT-18 (duplicate ID rejection at registration); +4 invariants |
| v0.29 | 2026-02-06 | Claude (Structural Auditor) | Added RTHANDLE-ERRKIND-1: RuntimeHandle::run() pre-execution failures now map to ValidationFailed; fixes pathological retry of invalid graphs (was RuntimeError) and corrects composition failure category (was SemanticError); +1 invariant |
| v0.30 | 2026-02-12 | Codex | Added SRC-15, TRG-14, ACT-19 (parameter default type must match declared type at registration); +3 invariants |
| v0.31 | 2026-02-12 | Codex | Added CMP-20 (compute output type validity closure; maps `ValidationError::InvalidOutputType` to CMP-20 and records structural registration enforcement); +1 invariant |
| v0.32 | 2026-02-17 | Codex | Added canonical run/replay strictness section (RUN-CANON-1, RUN-CANON-2, REP-7) and documented strict v1 capture bundle requirements (required adapter_provenance, unknown-field rejection, legacy adapter_version deserialization failure). |
