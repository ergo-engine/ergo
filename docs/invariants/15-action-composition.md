---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-26
Owner: Documentation
Scope: Action composition invariants
Change Rule: Operational log
---

## 15. Action Composition Phase

**Scope:** When actions that declare writes or intents are composed with an adapter.

**Source:** `docs/primitives/action.md`

**Entry invariants:**

- Adapter passes registration validation (ADP-* rules)
- Action manifests pass their registration validation

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| COMP-9 | Action inputs follow gate/payload split | action.md #COMP-9 | — | — | ✓ | ✓ |
| COMP-10 | Action output not wireable | action.md #COMP-10 | — | — | ✓ | ✓ |
| COMP-11 | Action writes target provided keys | action.md #COMP-11 | — | — | ✓ | comp_11_write_target_not_provided_rejected |
| COMP-12 | Action writes only writable keys | action.md #COMP-12 | — | — | ✓ | comp_12_write_target_not_writable_rejected |
| COMP-13 | Action write types match | action.md #COMP-13 | — | — | ✓ | comp_13_write_type_mismatch_rejected |
| COMP-16 | Parameter-bound manifest names resolve | action.md #COMP-16 | — | — | ✓ | comp_action_dollar_key_missing_parameter_rejected |
| COMP-14 | If action writes or mirror writes, adapter accepts set_context | action.md #COMP-14 | — | — | ✓ | comp_14_missing_set_context_rejected |
| COMP-15 | Writes captured (planned) | action.md #COMP-15 | — | — | — | — |
| COMP-17 | If action declares intents, adapter accepts each intent effect kind | action.md #COMP-17 | — | — | ✓ | comp_17_missing_intent_effect_rejected |
| COMP-18 | Declared intent kinds must have payload schemas in adapter acceptance surface | action.md #COMP-18 | — | — | ✓ | — |
| COMP-19 | Intent fields are structurally compatible with adapter payload schema | action.md #COMP-19 | — | — | ✓ | — |

### Notes

- **COMP-9/COMP-10:** Action-composition aliases enforced by Validation Phase invariant **V.2** in `crates/kernel/runtime/src/runtime/validate.rs`. `COMP-9` refines Action input legality by destination input type; `COMP-10` captures Action terminality.
- **COMP-16:** Shared adapter-composition rule for parameter-bound manifest names. It applies both to source requirements and to `$`-prefixed action write names before context-key lookup.
- **COMP-14:** `mirror_writes` participate in the same host-internal `set_context` projection as top-level writes, so any Action that declares writes or mirror writes must compose against adapter acceptance of `set_context`.
- **COMP-18/COMP-19:** Adapter composition validates that each declared intent kind has a payload schema and that the manifest field set is structurally compatible with that schema before execution begins, but this currently matters only for the richer runtime/custom manifest surface because file-backed prod manifests do not deserialize intent declarations.
- **COMP-15:** Deferred as a composition-level counterpart to ADP-15/ADP-16 (adapter-manifest completeness). Same-ingestion Scope A replay already verifies host-owned effect integrity including `set_context` writes. See `08-replay.md` and `09-adapter-registration.md` notes. If this work is revived, open a dedicated gap-work file first to decide whether manifests canonically declare context/effect capture coverage and what guarantee that implies across ingestion modes.
- **Enforcement location:** `crates/kernel/adapter/src/composition.rs` (invoked by `ergo_adapter::RuntimeHandle::run`).
- **Test location:** `crates/kernel/adapter/tests/composition_tests.rs`

---
