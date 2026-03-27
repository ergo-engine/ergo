---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-26
Owner: Documentation
Scope: Validation-phase invariants for expanded graphs
Change Rule: Operational log
---

## 5. Validation Phase

**Scope:** Validating the unified DAG before execution.

**Entry invariants:**

- Graph is fully expanded (no clusters remain)
- Signature inference is complete

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| V.1 | No cycles exist in the graph | execution.md §2 | — | — | ✓ | ✓ |
| V.2 | All edges satisfy wiring matrix | ontology.md §3 | — | — | ✓ | ✓ |
| V.3 | All required inputs are connected | execution.md §2 | — | — | ✓ | ✓ |
| V.4 | All type constraints are satisfied at edges | cluster-spec.md §6.3 | — | — | ✓ | ✓ |
| V.5 | All action nodes are gated by trigger events | ontology.md §3 | — | — | ✓ | ✓ |
| V.6 | All nodes pass validation before any action executes | execution.md §7 | — | — | ✓ | ✓ |
| V.7 | Each input port receives at most one inbound edge | (inferred) | — | — | ✓ | ✓ |
| V.8 | Referenced primitive implementations exist in catalog | cluster-spec.md §6.3 | — | — | ✓ | ✓ |

### Notes

- Validation phase is well-covered by existing executor tests.
- **V.5:** Validation confirms structural wiring (Action has Trigger input). Runtime enforcement (R.7) additionally gates execution on `TriggerEvent::Emitted`. Both validation and runtime enforcement are now complete.
- **V.7:** ✅ **CLOSED.** Enforced in `runtime/validate.rs::enforce_single_edge_per_input()`. Returns `ValidationError::MultipleInboundEdges { node, input }` when multiple edges target same input port. Test: `validate_rejects_multiple_edges_to_same_input`.
  - **Prior behavior:** `execute.rs` used `HashMap::insert` for input collection; multiple edges to same input caused silent last-write-wins data loss.
  - **Rationale:** Silent data loss is truth-destroying. Aggregation semantics (Cardinality::Multiple) remain schema-placeholder only; if ever needed, require explicit v1 decision.
- **V.8:** Enforced at validation entry in `runtime/validate.rs`: each expanded node must resolve through `PrimitiveCatalog::get`, else `ValidationError::MissingPrimitive { id, version }`. Test: `validate_rejects_missing_primitive_metadata`.
