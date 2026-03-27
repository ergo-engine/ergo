---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-26
Owner: Documentation
Scope: Definition-phase invariants for authored clusters
Change Rule: Operational log
---

## 1. Definition Phase

**Scope:** When a cluster is authored and saved.

**Entry invariants:** None (this is the origin point for authoring).

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| D.1 | Cluster contains ≥1 node | cluster-spec.md §6.1 | — | — | ✓ | ✓ |
| D.2 | All edges reference existing nodes and ports | cluster-spec.md §6.1 | — | — | ✓ | ✓ |
| D.3 | All edges satisfy wiring matrix | cluster-spec.md §6.1 | — | — | ✓ | ✓ |
| D.4 | Every output port references a valid internal node output | cluster-spec.md §6.1 | — | — | ✓ | ✓ |
| D.5 | Every input port has a unique name | cluster-spec.md §6.1 | — | — | ✓ | ✓ |
| D.6 | Every output port has a unique name | cluster-spec.md §6.1 | — | — | ✓ | ✓ |
| D.7 | All parameters have valid types | cluster-spec.md §6.1 | — | — | ✓ | ✓ |
| D.8 | Parameter defaults are type-compatible | cluster-spec.md §6.1 | — | — | ✓ | ✓ |
| D.9 | No duplicate parameter names | cluster-spec.md §6.1 | — | — | ✓ | ✓ |
| D.10 | Declared signature validation runs when a signature is present (currently wireability-only) | cluster-spec.md §4 | — | — | ✓ | ✓ |
| D.11 | Declared wireability cannot exceed inferred wireability | cluster-spec.md §4, §6 | — | — | ✓ | ✓ |

### Notes

- **D.5–D.9:** Enforced in `cluster.rs::validate_cluster_definition` (returns `ExpandError::DuplicateInputPort|DuplicateOutputPort|DuplicateParameter|ParameterDefaultTypeMismatch|InvalidDeriveKeySlot`). Tests: `duplicate_input_ports_rejected`, `duplicate_output_ports_rejected`, `duplicate_parameters_rejected`, `parameter_default_type_mismatch_rejected`. D.8 also covers `derive_key` defaults: `DeriveKey` on non-String parameter triggers `ParameterDefaultTypeMismatch`; empty `slot_name` triggers `InvalidDeriveKeySlot`. Tests: `cluster_derive_key_on_non_string_param_rejected`, `cluster_derive_key_empty_slot_name_rejected`.
- **D.10–D.11:** Enforced during `expand()` via `infer_signature` + `validate_declared_signature` (`ExpandError::DeclaredSignatureInvalid`). Current prod validation is narrower than full structural signature equivalence: the concrete enforced check is that declared wireability must not exceed inferred wireability. Test: `declared_wireability_cannot_exceed_inferred`.
