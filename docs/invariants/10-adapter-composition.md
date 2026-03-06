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
| COMP-16 | Parameter-bound manifest names resolve | — | — | — | ✓ | comp_source_dollar_key_missing_parameter_rejected |

### Notes

- **COMP-1, COMP-2:** Only keys with `required: true` in source requirements must exist in adapter provides.
- **COMP-16:** `$`-prefixed manifest names must resolve to a String parameter value at composition time. Enforced for both source context requirements and action write specs.
- **Enforcement location:** `crates/kernel/adapter/src/composition.rs`
- **Test location:** `crates/kernel/adapter/tests/composition_tests.rs`

---
