---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-26
Owner: Documentation
Scope: Adapter composition invariants
Change Rule: Operational log
---

## 10. Adapter Composition Phase

**Scope:** When sources or actions are composed with an adapter.

**Source:** `docs/primitives/adapter.md`

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

- **COMP-1:** Only keys with `required: true` in source requirements must exist in adapter provides.
- **COMP-2:** Any source-required key that the adapter does provide must match type, including optional keys whose existence is allowed but not guaranteed.
- **COMP-16:** `$`-prefixed manifest names must resolve to a String parameter value at composition time. Enforced for both source context requirements and action write specs.
- **Enforcement location:** `crates/kernel/adapter/src/composition.rs`
- **Test location:** `crates/kernel/adapter/tests/composition_tests.rs`

---
