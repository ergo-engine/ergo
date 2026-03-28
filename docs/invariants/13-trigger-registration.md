---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-27
Owner: Documentation
Scope: Trigger registration invariants
Change Rule: Operational log
---

## 13. Trigger Registration Phase

**Scope:** When a trigger manifest is registered with the system.

**Source:** `docs/primitives/trigger.md`

**Entry invariants:**

- Manifest is parseable
- Required fields are present

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| TRG-1 | ID format valid | trigger.md #TRG-1 | — | — | ✓ | trg_1_invalid_id_rejected |
| TRG-2 | Version valid semver | trigger.md #TRG-2 | — | — | ✓ | trg_2_invalid_version_rejected |
| TRG-3 | Kind is "trigger" | trigger.md #TRG-3 | ✓ | — | — | trg_3_kind_trigger_accepted |
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

### Related Composition Alias Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| COMP-7 | Trigger input from Compute or Trigger | trigger.md #COMP-7 | — | — | ✓ | ✓ |
| COMP-8 | Trigger output to Action or Trigger | trigger.md #COMP-8 | — | — | ✓ | ✓ |

### Notes

- **TRG-9 link:** TRG-STATE-1 (stateless triggers) remains enforced by registry validation.
- **TRG-14:** Current prod file-manifest validation rejects default mismatches in `crates/prod/core/host/src/manifest_usecases.rs` (`TriggerParseError::InvalidParameterDefault`). The typed runtime-registry path in `crates/kernel/runtime/src/trigger/registry.rs` still rejects mismatched defaults as `TriggerValidationError::InvalidParameterType`.
- **Registration enforcement location:** `crates/prod/core/host/src/manifest_usecases.rs` (file-manifest parse) and `crates/kernel/runtime/src/trigger/registry.rs` (typed runtime registry)
- **Registration test location:** `crates/kernel/runtime/src/trigger/registry.rs` (typed runtime registry) and `crates/prod/clients/cli/tests/phase7_cli.rs` (file-manifest validation coverage)
- **COMP-7/COMP-8:** Trigger-adjacent composition aliases enforced by Validation Phase invariant **V.2** (wiring matrix) in `crates/kernel/runtime/src/runtime/validate.rs`.

---
