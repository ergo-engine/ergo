## 15. Action Composition Phase

**Scope:** When actions that declare writes or intents are composed with an adapter.

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
| COMP-14 | If action writes or mirror writes, adapter accepts set_context | action.md #COMP-14 | — | — | ✓ | comp_14_missing_set_context_rejected |
| COMP-17 | If action declares intents, adapter accepts each intent effect kind | action.md #COMP-17 | — | — | ✓ | comp_17_missing_intent_effect_rejected |
| COMP-18 | Declared intent kinds must have payload schemas in adapter acceptance surface | action.md #COMP-18 | — | — | ✓ | — |
| COMP-19 | Intent fields are structurally compatible with adapter payload schema | action.md #COMP-19 | — | — | ✓ | — |

### Notes

- **COMP-10:** Enforced by Validation Phase invariant **V.2** (coarse boundary-kind wiring matrix) in `crates/kernel/runtime/src/runtime/validate.rs`.
- **COMP-9 (split Action inputs):** STABLE contract distinguishes Trigger-gated `event` inputs from scalar payload inputs (`Source`/`Compute`), and runtime validation now enforces this destination-input-type-aware split within **V.2**.
- **COMP-14:** `mirror_writes` participate in the same host-internal `set_context` projection as top-level writes, so any Action that declares writes or mirror writes must compose against adapter acceptance of `set_context`.
- **COMP-18/COMP-19:** Adapter composition now validates that each declared intent kind has a payload schema and that the manifest field set is structurally compatible with that schema before execution begins.
- **COMP-15:** Deferred until REP-SCOPE expansion beyond Scope A (cross-ingestion normalization).
- **Enforcement location:** `crates/kernel/adapter/src/composition.rs` (invoked by `ergo_adapter::RuntimeHandle::run`).
- **Test location:** `crates/kernel/adapter/tests/composition_tests.rs`

---
