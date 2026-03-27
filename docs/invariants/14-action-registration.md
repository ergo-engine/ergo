---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-26
Owner: Documentation
Scope: Action registration invariants
Change Rule: Operational log
---

## 14. Action Registration Phase

**Scope:** When an action manifest is registered with the system.

**Source:** `docs/primitives/action.md`

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
| ACT-13 | Effects surface normalized | action.md #ACT-13 | ✓ | — | — | — |
| ACT-14 | Write names unique | action.md #ACT-14 | — | — | ✓ | act_14_duplicate_write_name_rejected |
| ACT-15 | Write types valid | action.md #ACT-15 | — | — | ✓ | act_15_write_types_valid_accepts_all_scalar_variants |
| ACT-16 | Retryable false | action.md #ACT-16 | — | — | ✓ | act_16_retryable_not_allowed_rejected |
| ACT-17 | Execution deterministic | action.md #ACT-17 | — | — | ✓ | act_17_non_deterministic_execution_rejected |
| ACT-18 | ID unique in registry | action.md #ACT-18 | — | — | ✓ | act_18_duplicate_id_rejected |
| ACT-19 | Parameter default type matches declared type | action.md #ACT-19 | — | — | ✓ | act_19_invalid_parameter_type_default_rejected |
| ACT-20 | $key write references bound to declared parameter | action.md #ACT-20 | — | — | ✓ | act_20_dollar_key_write_referencing_nonexistent_param_rejected |
| ACT-21 | $key write references must be String type | action.md #ACT-21 | — | — | ✓ | act_21_dollar_key_write_referencing_non_string_param_rejected |
| ACT-22 | Write from_input references declared input | action.md #ACT-22 | — | — | ✓ | act_22_from_input_not_found_rejected |
| ACT-23 | Write from_input type compatible with write type | action.md #ACT-23 | — | — | ✓ | act_23_from_input_event_type_rejected |
| ACT-24 | Intent names unique | action.md #ACT-24 | — | — | ✓ | intent_validation_duplicate_intent_names_rejected |
| ACT-25 | Intent field names unique within each intent | action.md #ACT-25 | — | — | ✓ | intent_validation_duplicate_field_names_rejected |
| ACT-26 | Intent field declares a source | action.md #ACT-26 | — | — | ✓ | intent_validation_neither_source_set_rejected |
| ACT-27 | Intent field declares only one source | action.md #ACT-27 | — | — | ✓ | intent_validation_both_sources_set_rejected |
| ACT-28 | Intent field from_input references declared input | action.md #ACT-28 | — | — | ✓ | — |
| ACT-29 | Intent field from_input type compatible with field type | action.md #ACT-29 | — | — | ✓ | — |
| ACT-30 | Intent field from_param references declared parameter | action.md #ACT-30 | — | — | ✓ | — |
| ACT-31 | Intent field from_param type compatible with field type | action.md #ACT-31 | — | — | ✓ | — |
| ACT-32 | Mirror write from_field references declared intent field | action.md #ACT-32 | — | — | ✓ | intent_validation_from_field_missing_rejected |
| ACT-33 | Mirror write type matches referenced field type | action.md #ACT-33 | — | — | ✓ | intent_validation_from_field_type_mismatch_rejected |

### Notes

- **ACT-12:** Composition-time validation. Same predicate and enforcement as **V.5** (`ValidationError::ActionNotGated` in `crates/kernel/runtime/src/runtime/validate.rs`). It remains documented in the `ACT-*` family file for action-contract traceability.
- **ACT-19:** Enforced in `action/registry.rs::validate_manifest` by rejecting manifests where a parameter default value type does not match the declared parameter type (`ActionValidationError::InvalidParameterType`).
- **ACT-20/ACT-21:** Registration-time cross-check for parameter-bound write names (`$key` convention). Ensures `$`-prefixed write spec names reference declared String-typed parameters.
- **ACT-22/ACT-23:** Registration-time checks for write payload binding (`from_input`) and scalar type compatibility. These define the declarative "what" channel for action writes; they do not by themselves authorize upstream wiring.
- **ACT-13:** Current file-backed prod parsing defaults a missing `effects` block to empty writes instead of rejecting the manifest. The richer runtime manifest still always carries `ActionEffects` after parse/normalization.
- **ACT-24 through ACT-33:** Registration-time validation of first-class intent declarations. These checks enforce unique intent names, unique field names, exactly-one-source semantics for each field (`from_input` xor `from_param`), source existence/type compatibility, and `mirror_writes[].from_field` integrity when the richer runtime/custom manifest surface is used.
- **Registration enforcement location:** `crates/kernel/runtime/src/action/registry.rs`
- **Registration test location:** `crates/kernel/runtime/src/action/registry.rs`
- **Validation test location:** `crates/kernel/runtime/src/runtime/tests.rs`

---
