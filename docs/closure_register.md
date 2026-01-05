# Closure Register (v0)

Purpose: Track semantic gaps, hardening closures, and explicit v0 rejections.
Rule: Every closure must specify (a) disposition, (b) enforcement locus, (c) test evidence, (d) PR/commit.

Legend:
- CLOSE: enforced and tested in v0 without adding new semantics
- REJECT: explicitly invalid in v0 (typed error + test)
- V1 SEMANTICS: requires explicit semantics decision (parked)

---

## Closed / Rejected in v0

### TRG-STATE-1 — Trigger persistent state forbidden by construction
- **ID:** TRG-STATE-1
- **Rule:** Triggers are stateless across runs; runtime API does not support persisted trigger state.
- **Disposition:** CLOSE
- **Enforcement locus:** API removal + registry validation
  - `crates/runtime/src/trigger/mod.rs` (removed TriggerState + state param)
  - `crates/runtime/src/runtime/types.rs` (ExecutionContext is unit struct)
  - `crates/runtime/src/runtime/execute.rs` (removed state plumbing)
  - `crates/runtime/src/trigger/registry.rs` (kept `StatefulTriggerNotAllowed`)
- **Error:** `TriggerValidationError::StatefulTriggerNotAllowed` (registry)
- **Test:** existing suite (no dedicated test added; enforced by compilation + registry validation)
- **PR/Commit:** <fill>

---

### R.7 — TriggerEvent mapping is explicit; NotEmitted is unreachable post-gating
- **ID:** R.7-MAP
- **Rule:** Only `TriggerEvent::Emitted` maps to `ActionOutcome::Attempted`; `NotEmitted` must be caught by `should_skip_action()`.
- **Disposition:** CLOSE
- **Enforcement locus:** execute-time mapping in `crates/runtime/src/runtime/execute.rs`
- **Error:** `unreachable!("R.7 violation...")` (assertion of invariant)
- **Test:** existing `r7_action_skipped_when_trigger_not_emitted` (+ relies on should_skip_action)
- **PR/Commit:** <fill>

---

### V.MULTI-EDGE — Multi-edge fan-in is invalid in v0
- **ID:** V.MULTI-EDGE
- **Rule:** No input port may receive more than one inbound edge. Merges must be explicit via combiner nodes.
- **Disposition:** REJECT
- **Enforcement locus:** validation in `crates/runtime/src/runtime/validate.rs` (`enforce_single_edge_per_input`)
- **Error:** `ValidationError::MultipleInboundEdges { node, input }`
- **Test:** `validate_rejects_multiple_edges_to_same_input`
- **PR/Commit:** <fill>

---

### V.PANIC-HARDEN — Validator/executor return typed errors on malformed graphs
- **ID:** V.PANIC-HARDEN
- **Rule:** Malformed graphs must not panic; validation/execution return typed errors.
- **Disposition:** CLOSE
- **Enforcement locus:** `crates/runtime/src/runtime/validate.rs`, `crates/runtime/src/runtime/execute.rs`
- **Error:** `ValidationError::UnknownNode`, `ValidationError::MissingOutputMetadata`, `ExecError::MissingNode` (etc.)
- **Tests:** malformed-graph regression tests added (unknown edge node, invalid boundary output, missing node in topo)
- **PR/Commit:** <fill>

---

## Audit #2 — Findings (to be closed)

### A.1a — Primitive parameter defaults applied during expansion

- **ID:** A.1a
- **Rule:** Primitive parameters with `default: Some(value)` in `PrimitiveMetadata.parameters` must be present in `ExpandedNode.parameters` when no binding provided; missing required params without default fail at expand.
- **Disposition:** CLOSE
- **Enforcement locus:** expand (`crates/runtime/src/cluster.rs`)
  - `ParameterMetadata` struct added (lines 176-184)
  - `PrimitiveMetadata.parameters: Vec<ParameterMetadata>` added (line 166)
  - `resolve_impl_parameters()` implements default application logic (lines 988-1028)
  - Impl node handling in `expand_with_context` calls catalog + resolver (lines 695-709)
- **Catalog change:** `crates/runtime/src/catalog.rs` — all `register_*` functions now populate `parameters` from manifests
- **Error:** `ExpandError::MissingRequiredParameter`, `ExpandError::UnresolvedExposedBinding`
- **Tests:**
  - `defaulted_parameter_propagates_to_leaf`
  - `explicit_binding_overrides_default`
  - `missing_required_param_no_default_rejected`
- **PR/Commit:** <pending>

---

### A.1b — Cluster parameter defaults applied during nested expansion

- **ID:** A.1b
- **Rule:** Cluster parameters with `default: Some(value)` in `ClusterDefinition.parameters` must propagate to nested cluster instantiation when no binding provided; missing required params without default fail at expand.
- **Disposition:** CLOSE
- **Enforcement locus:** expand (`crates/runtime/src/cluster.rs`)
  - `build_resolved_params()` updated to accept `cluster_id` and `specs: &[ParameterSpec]` (lines 1034-1074)
  - Returns `Result<HashMap<String, ParameterValue>, ExpandError>`
  - Logic: binding present → use it; absent + default → apply default; absent + required + no default → error
  - Cluster node handling calls updated resolver (lines 745-750)
- **Error:** `ExpandError::MissingRequiredParameter`, `ExpandError::UnresolvedExposedBinding`
- **Test:** `cluster_parameter_default_propagates_to_nested`
- **PR/Commit:** <pending>

---

### B.1 — Primitive `.expect()` unreachable for validated graphs

- **ID:** B.1
- **Rule:** After expansion + validation, primitive `.expect()` on required inputs/params is unreachable by construction.
- **Disposition:** CLOSE (dependency: A.1a + A.1b)
- **Enforcement locus:** expand (A.1a, A.1b ensure params present) + validation (required inputs enforced)
- **Evidence:** A.1a tests + A.1b test + existing validation tests
- **PR/Commit:** <pending — closes with A.1a/A.1b PR>

---

### C.1 — Runtime ID assignment deterministic across runs

- **ID:** C.1
- **Rule:** Expansion must assign deterministic `runtime_id`s for identical cluster definitions (no HashMap iteration dependence).
- **Disposition:** CLOSE
- **Enforcement locus:** expand (`crates/runtime/src/cluster.rs:694-698`)
  - Node keys collected and sorted lexicographically before iteration
  - `ctx.next_runtime_id()` called in stable order
- **Test:** `expansion_runtime_ids_deterministic` (cluster.rs:2806-2883)
  - 5 nodes with varying names, expanded 3 times, IDs identical
  - Verifies alphabetical ordering (alpha→n0, bravo→n1, etc.)
- **PR/Commit:** <pending>

---

### A.2 — Boundary output mapping must not silently fallback

- **ID:** A.2
- **Rule:** If a boundary output references an unmapped node_id during expansion, expansion must fail with typed error.
- **Disposition:** CLOSE
- **Enforcement locus:** expand (`crates/runtime/src/cluster.rs:1112-1135`)
  - `map_boundary_outputs` returns `Result<Vec<OutputPortSpec>, ExpandError>`
  - Returns error when `mapping.get()` fails instead of fallback
- **Error:** `ExpandError::UnmappedBoundaryOutput { port_name, node_id }`
- **Test:** `unmapped_boundary_output_rejected` (cluster.rs:2905-2951)
- **PR/Commit:** <pending>

---

### A.3 — Nested output mapping must not silently skip

- **ID:** A.3
- **Rule:** If nested output mapping fails during expansion, expansion must fail with typed error.
- **Disposition:** CLOSE
- **Enforcement locus:** expand (`crates/runtime/src/cluster.rs:806-824`)
  - Nested cluster output mapping verifies all ports mapped
  - Returns error when target node has no mapping
- **Error:** `ExpandError::UnmappedNestedOutput { cluster_id, port_name }`
- **Test:** `nested_output_mapping_failure_rejected` (cluster.rs:2953-3024)
- **PR/Commit:** <pending>

---

## Semantics Decision Queue (v1)

### B.2 — Divide-by-zero behavior
- **ID:** B.2
- **Current behavior:** IEEE 754 propagation (`inf`, `-inf`, `NaN`)
- **Disposition:** V1 SEMANTICS
- **Decision needed:** Document IEEE 754 as acceptable in v0 vs. add explicit zero-check with typed error in v1 (semantics change)
- **PR/Commit:** <pending / doc note optional>
