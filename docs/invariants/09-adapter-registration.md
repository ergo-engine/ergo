---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-26
Owner: Documentation
Scope: Adapter registration invariants
Change Rule: Operational log
---

## 9. Adapter Registration Phase

**Scope:** When an adapter manifest is registered with the system.

**Source:** `docs/primitives/adapter.md`

**Entry invariants:**

- Manifest is parseable YAML/JSON
- Required fields are present (serde validation)

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| ADP-1 | ID format valid | adapter.md #ADP-1 | — | — | ✓ | adp_1_invalid_id_rejected |
| ADP-2 | Version valid semver | adapter.md #ADP-2 | — | — | ✓ | adp_2_invalid_version_rejected |
| ADP-3 | Runtime compatibility satisfied | adapter.md #ADP-3 | — | — | ✓ | adp_3_incompatible_runtime_rejected |
| ADP-4 | Provides something | adapter.md #ADP-4 | — | — | ✓ | adp_4_empty_adapter_rejected |
| ADP-5 | Context key names unique | adapter.md #ADP-5 | — | — | ✓ | adp_5_duplicate_context_key_rejected |
| ADP-6 | Context key types valid | adapter.md #ADP-6 | — | — | ✓ | adp_6_invalid_context_type_rejected |
| ADP-7 | Event kind names unique | adapter.md #ADP-7 | — | — | ✓ | adp_7_duplicate_event_kind_rejected |
| ADP-8 | Event schemas valid JSON Schema | adapter.md #ADP-8 | — | — | ✓ | adp_8_invalid_schema_rejected |
| ADP-9 | Capture format version present | adapter.md #ADP-9 | — | — | ✓ | adp_9_no_capture_format_rejected |
| ADP-10 | Capture fields referentially valid | adapter.md #ADP-10 | — | — | ✓ | adp_10_invalid_capture_field_rejected |
| ADP-11 | Writable flag must be present | adapter.md #ADP-11 | — | — | ✓ | adp_11_missing_writable_flag_rejected |
| ADP-12 | Effect names unique | adapter.md #ADP-12 | — | — | ✓ | adp_12_duplicate_effect_name_rejected |
| ADP-13 | Effect schemas valid | adapter.md #ADP-13 | — | — | ✓ | adp_13_invalid_effect_schema_rejected |
| ADP-14 | Writable implies set_context accepted | adapter.md #ADP-14 | — | — | ✓ | adp_14_writable_without_set_context_rejected |
| ADP-15 | Writable keys must be capturable | adapter.md #ADP-15 | — | — | — | — |
| ADP-16 | Write effect must be capturable | adapter.md #ADP-16 | — | — | — | — |
| ADP-17 | Writable keys cannot be required | adapter.md #ADP-17 | — | — | ✓ | adp_17_writable_key_required_rejected |
| ADP-18 | Required event fields map to context keys with compatible types | adapter.md #ADP-18 | — | — | ✓ | adp_18_* |
| ADP-19 | Materialized event field types are supported | adapter.md #ADP-19 | — | — | ✓ | adp_19_* |

### Notes

- **ADP-15/ADP-16:** Deferred as adapter-manifest completeness items. Same-ingestion Scope A replay already verifies host-owned effect integrity including `set_context` writes (see `08-replay.md`). These rules would require the manifest to explicitly declare capturability of writable context keys and `set_context` effects in `capture.fields`. They are not blockers to current same-ingestion replay correctness. If this work is revived, open a dedicated gap-work file first to decide whether manifests canonically declare context/effect capture coverage and what guarantee that implies across ingestion modes.
- **ADP-18:** Only fields listed in `payload_schema.required` participate. If `required` is omitted, the invariant vacuously passes.
- **ADP-19:** Event payload schemas must materialize only supported runtime value types (`Number`, `Bool`, `String`, `Series`).
- **Enforcement location:** `crates/kernel/adapter/src/validate.rs`
- **Test location:** `crates/kernel/adapter/tests/validation.rs`

---
