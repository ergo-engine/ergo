---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-27
Owner: Documentation
Scope: Compute registration invariants
Change Rule: Operational log
---

## 12. Compute Registration Phase

**Scope:** When a compute manifest is registered with the system.

**Source:** `docs/primitives/compute.md`

**Entry invariants:**

- Manifest is parseable
- Required fields are present

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| CMP-1 | ID format valid | compute.md #CMP-1 | — | — | ✓ | cmp_1_invalid_id_rejected |
| CMP-2 | Version valid semver | compute.md #CMP-2 | — | — | ✓ | cmp_2_invalid_version_rejected |
| CMP-3 | Kind is "compute" | compute.md #CMP-3 | ✓ | — | — | cmp_3_kind_compute_accepted |
| CMP-4 | At least one input | compute.md #CMP-4 | — | — | ✓ | cmp_4_no_inputs_rejected |
| CMP-5 | Input names unique | compute.md #CMP-5 | — | — | ✓ | cmp_5_duplicate_inputs_rejected |
| CMP-6 | At least one output | compute.md #CMP-6 | — | — | ✓ | cmp_6_no_outputs_rejected |
| CMP-7 | Output names unique | compute.md #CMP-7 | — | — | ✓ | cmp_7_duplicate_outputs_rejected |
| CMP-8 | Side effects not allowed | compute.md #CMP-8 | — | — | ✓ | cmp_8_side_effects_rejected |
| CMP-9 | State resettable if allowed | compute.md #CMP-9 | — | — | ✓ | cmp_9_state_not_resettable_rejected |
| CMP-10 | Errors deterministic | compute.md #CMP-10 | — | — | ✓ | cmp_10_non_deterministic_errors_rejected |
| CMP-11 | All outputs produced on success | compute.md #CMP-11 | — | — | — | cmp_11_missing_output_fails |
| CMP-12 | No outputs produced on error | compute.md #CMP-12 | ✓ | — | — | cmp_12_compute_error_fails |
| CMP-13 | Input types valid | compute.md #CMP-13 | — | — | ✓ | cmp_13_invalid_input_type_rejected |
| CMP-14 | Input cardinality single | compute.md #CMP-14 | — | — | ✓ | cmp_14_invalid_input_cardinality_rejected |
| CMP-15 | Parameter types valid | compute.md #CMP-15 | — | — | ✓ | cmp_15_invalid_parameter_type_rejected |
| CMP-16 | Cadence is continuous | compute.md #CMP-16 | — | — | ✓ | cmp_16_invalid_cadence_rejected |
| CMP-17 | Execution deterministic | compute.md #CMP-17 | — | — | ✓ | cmp_17_non_deterministic_execution_rejected |
| CMP-18 | ID unique in registry | compute.md #CMP-18 | — | — | ✓ | cmp_18_duplicate_id_rejected |
| CMP-19 | Parameter default type matches declared type | compute.md #CMP-19 | — | — | ✓ | cmp_19_invalid_parameter_type_default_rejected |
| CMP-20 | Output types valid | compute.md #CMP-20 | ✓ | — | — | cmp_20_output_types_valid |

### Related Composition Alias Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| COMP-4 | Source output type equals Compute input type | compute.md #COMP-4 | — | — | ✓ | ✓ |
| COMP-5 | Input type equals upstream output type | compute.md #COMP-5 | — | — | ✓ | ✓ |
| COMP-6 | Output type equals downstream input type | compute.md #COMP-6 | — | — | ✓ | ✓ |

### Notes

- **CMP-11/12:** These are execution-owned invariants, not registration-time validation hooks. Enforcement lives in `crates/kernel/runtime/src/runtime/execute.rs`; CMP-12 is additionally structural because `compute()` returns `Result<Outputs, ComputeError>`, so errors have no outputs by construction.
- **Registration enforcement location:** `crates/prod/core/host/src/manifest_usecases.rs` (file-manifest parse) and `crates/kernel/runtime/src/compute/registry.rs` (typed runtime registry)
- **Registration test location:** `crates/kernel/runtime/src/compute/registry.rs` (typed runtime registry) and `crates/prod/clients/cli/tests/phase7_cli.rs` (file-manifest validation coverage)
- **Execution test location:** `crates/kernel/runtime/src/runtime/tests.rs`
- **COMP-4/COMP-5/COMP-6:** Compute-adjacent composition aliases enforced by Validation Phase invariant **V.4** (`ValidationError::TypeMismatch`) in `crates/kernel/runtime/src/runtime/validate.rs`.
- **CMP-15:** The semantic contract is `Number | Bool`. On the file-backed front door, `crates/prod/core/host/src/manifest_usecases.rs` still accepts `int` as an alias for `Number` and can reject malformed parameter spellings before typed registration; the runtime registry then enforces the canonical contract by rejecting `String` and `Series`.
- **CMP-19:** Current prod file-manifest validation rejects default mismatches in `crates/prod/core/host/src/manifest_usecases.rs` (`ComputeParseError::InvalidParameterDefault`). The typed runtime-registry path in `crates/kernel/runtime/src/compute/registry.rs` still rejects mismatched defaults as `ValidationError::InvalidParameterType`.
- **CMP-20:** Invalid output type strings are rejected on the prod file-manifest surface during parse; the typed runtime layer only admits `Number | Series | Bool | String`.

---
