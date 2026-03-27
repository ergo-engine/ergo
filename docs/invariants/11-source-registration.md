---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-26
Owner: Documentation
Scope: Source registration invariants
Change Rule: Operational log
---

## 11. Source Registration Phase

**Scope:** When a source manifest is registered with the system.

**Source:** `docs/primitives/source.md`

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
| SRC-11 | Provided context types match adapter | source.md #SRC-11 | — | — | ✓ | src_11_context_type_mismatch_rejected |
| SRC-12 | Execution deterministic | source.md #SRC-12 | — | — | ✓ | src_12_non_deterministic_execution_rejected |
| SRC-13 | Cadence is continuous | source.md #SRC-13 | — | — | ✓ | (structurally enforced) |
| SRC-14 | ID unique in registry | source.md #SRC-14 | — | — | ✓ | src_14_duplicate_id_rejected |
| SRC-15 | Parameter default type matches declared type | source.md #SRC-15 | — | — | ✓ | src_15_invalid_parameter_type_default_rejected |
| SRC-16 | $key context references bound to declared parameter | source.md #SRC-16 | — | — | ✓ | src_16_dollar_key_referencing_nonexistent_param_rejected |
| SRC-17 | $key context references must be String type | source.md #SRC-17 | — | — | ✓ | src_17_dollar_key_referencing_non_string_param_rejected |

### Notes

- **SRC-1 through SRC-9, SRC-12, SRC-14, SRC-15:** Registration-time manifest validation.
- **SRC-16/SRC-17:** Registration-time cross-check for parameter-bound manifest names (`$key` convention). Ensures `$`-prefixed context requirement names reference declared String-typed parameters.
- **SRC-13:** Structurally enforced — `Cadence` enum only has `Continuous` variant. Enforcement code at `registry.rs:77-78` will be exercised when cadence variants expand.
- **SRC-10:** Composition-time validation that required context keys exist.
- **SRC-11:** Composition-time validation that any source-required key the adapter does provide matches type, including optional keys whose existence is allowed but not guaranteed. Alias tests provide source-contract traceability through COMP-2 (§10).
- **Registration enforcement location:** `crates/kernel/runtime/src/source/registry.rs`
- **Registration test location:** `crates/kernel/runtime/src/source/tests.rs`
- **Composition enforcement location:** `crates/kernel/adapter/src/composition.rs` (invoked by `ergo_adapter::RuntimeHandle::run` after graph validation, before execution)
- **Composition test location:** `crates/kernel/adapter/tests/composition_tests.rs`

---
