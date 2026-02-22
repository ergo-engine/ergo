# Closure Register (v0)

Purpose: Track semantic gaps, hardening closures, and explicit v0 rejections.
Status: Living closure log (active maintenance; not archival-only).
Rule: Every closure must specify (a) disposition, (b) enforcement locus, (c) test evidence, (d) PR/commit.

Legend:
- CLOSE: enforced and tested in v0 without adding new semantics
- REJECT: explicitly invalid in v0 (typed error + test)
- V1 SEMANTICS: requires explicit semantics decision (parked)

---

## Closed / Rejected in v0

### TRG-STATE-1 — Trigger persistent state forbidden by construction
- **ID:** TRG-STATE-1
- **Rule:** Triggers are stateless across runs; runtime API does not support persisted trigger state.
- **Disposition:** CLOSE
- **Enforcement locus:** API removal + registry validation
  - `crates/runtime/src/trigger/mod.rs` (removed TriggerState + state param)
  - `crates/runtime/src/runtime/types.rs` (ExecutionContext is unit struct)
  - `crates/runtime/src/runtime/execute.rs` (removed state plumbing)
  - `crates/runtime/src/trigger/registry.rs` (kept `StatefulTriggerNotAllowed`)
- **Error:** `TriggerValidationError::StatefulTriggerNotAllowed` (registry)
- **Test:** existing suite (no dedicated test added; enforced by compilation + registry validation)
- **PR/Commit:** <fill>

---

### R.7 — TriggerEvent mapping is explicit; NotEmitted is unreachable post-gating
- **ID:** R.7-MAP
- **Rule:** Only `TriggerEvent::Emitted` maps to `ActionOutcome::Attempted`; `NotEmitted` must be caught by `should_skip_action()`.
- **Disposition:** CLOSE
- **Enforcement locus:** execute-time mapping in `crates/runtime/src/runtime/execute.rs`
- **Error:** `unreachable!("R.7 violation...")` (assertion of invariant)
- **Test:** existing `r7_action_skipped_when_trigger_not_emitted` (+ relies on should_skip_action)
- **PR/Commit:** <fill>

---

### V.MULTI-EDGE — Multi-edge fan-in is invalid in v0
- **ID:** V.MULTI-EDGE
- **Rule:** No input port may receive more than one inbound edge. Merges must be explicit via combiner nodes.
- **Disposition:** REJECT
- **Enforcement locus:** validation in `crates/runtime/src/runtime/validate.rs` (`enforce_single_edge_per_input`)
- **Error:** `ValidationError::MultipleInboundEdges { node, input }`
- **Test:** `validate_rejects_multiple_edges_to_same_input`
- **PR/Commit:** <fill>

---

### V.PANIC-HARDEN — Validator/executor return typed errors on malformed graphs
- **ID:** V.PANIC-HARDEN
- **Rule:** Malformed graphs must not panic; validation/execution return typed errors.
- **Disposition:** CLOSE
- **Enforcement locus:** `crates/runtime/src/runtime/validate.rs`, `crates/runtime/src/runtime/execute.rs`
- **Error:** `ValidationError::UnknownNode`, `ValidationError::MissingOutputMetadata`, `ExecError::MissingNode` (etc.)
- **Tests:** malformed-graph regression tests added (unknown edge node, invalid boundary output, missing node in topo)
- **PR/Commit:** <fill>

---

## Audit #2 — Findings (to be closed)

### A.1a — Primitive parameter defaults applied during expansion

- **ID:** A.1a
- **Rule:** Primitive parameters with `default: Some(value)` in `PrimitiveMetadata.parameters` must be present in `ExpandedNode.parameters` when no binding provided; missing required params without default fail at expand.
- **Disposition:** CLOSE
- **Enforcement locus:** expand (`crates/runtime/src/cluster.rs`)
  - `ParameterMetadata` struct added (lines 176-184)
  - `PrimitiveMetadata.parameters: Vec<ParameterMetadata>` added (line 166)
  - `resolve_impl_parameters()` implements default application logic (lines 988-1028)
  - Impl node handling in `expand_with_context` calls catalog + resolver (lines 695-709)
- **Catalog change:** `crates/runtime/src/catalog.rs` — all `register_*` functions now populate `parameters` from manifests
- **Error:** `ExpandError::MissingRequiredParameter`, `ExpandError::UnresolvedExposedBinding`
- **Tests:**
  - `defaulted_parameter_propagates_to_leaf`
  - `explicit_binding_overrides_default`
  - `missing_required_param_no_default_rejected`
- **PR/Commit:** <pending>

---

### A.1b — Cluster parameter defaults applied during nested expansion

- **ID:** A.1b
- **Rule:** Cluster parameters with `default: Some(value)` in `ClusterDefinition.parameters` must propagate to nested cluster instantiation when no binding provided; missing required params without default fail at expand.
- **Disposition:** CLOSE
- **Enforcement locus:** expand (`crates/runtime/src/cluster.rs`)
  - `build_resolved_params()` updated to accept `cluster_id` and `specs: &[ParameterSpec]` (lines 1034-1074)
  - Returns `Result<HashMap<String, ParameterValue>, ExpandError>`
  - Logic: binding present → use it; absent + default → apply default; absent + required + no default → error
  - Cluster node handling calls updated resolver (lines 745-750)
- **Error:** `ExpandError::MissingRequiredParameter`, `ExpandError::UnresolvedExposedBinding`
- **Test:** `cluster_parameter_default_propagates_to_nested`
- **PR/Commit:** <pending>

---

### B.1 — Primitive `.expect()` unreachable for validated graphs

- **ID:** B.1
- **Rule:** After expansion + validation, primitive `.expect()` on required inputs/params is unreachable by construction.
- **Disposition:** CLOSE (dependency: A.1a + A.1b)
- **Enforcement locus:** expand (A.1a, A.1b ensure params present) + validation (required inputs enforced)
- **Evidence:** A.1a tests + A.1b test + existing validation tests
- **PR/Commit:** <pending — closes with A.1a/A.1b PR>

---

### C.1 — Runtime ID assignment deterministic across runs

- **ID:** C.1
- **Rule:** Expansion must assign deterministic `runtime_id`s for identical cluster definitions (no HashMap iteration dependence).
- **Disposition:** CLOSE
- **Enforcement locus:** expand (`crates/runtime/src/cluster.rs:694-698`)
  - Node keys collected and sorted lexicographically before iteration
  - `ctx.next_runtime_id()` called in stable order
- **Test:** `expansion_runtime_ids_deterministic` (cluster.rs:2806-2883)
  - 5 nodes with varying names, expanded 3 times, IDs identical
  - Verifies alphabetical ordering (alpha→n0, bravo→n1, etc.)
- **PR/Commit:** <pending>

---

### A.2 — Boundary output mapping must not silently fallback

- **ID:** A.2
- **Rule:** If a boundary output references an unmapped node_id during expansion, expansion must fail with typed error.
- **Disposition:** CLOSE
- **Enforcement locus:** expand (`crates/runtime/src/cluster.rs:1112-1135`)
  - `map_boundary_outputs` returns `Result<Vec<OutputPortSpec>, ExpandError>`
  - Returns error when `mapping.get()` fails instead of fallback
- **Error:** `ExpandError::UnmappedBoundaryOutput { port_name, node_id }`
- **Test:** `unmapped_boundary_output_rejected` (cluster.rs:2905-2951)
- **PR/Commit:** <pending>

---

### A.3 — Nested output mapping must not silently skip

- **ID:** A.3
- **Rule:** If nested output mapping fails during expansion, expansion must fail with typed error.
- **Disposition:** CLOSE
- **Enforcement locus:** expand (`crates/runtime/src/cluster.rs:806-824`)
  - Nested cluster output mapping verifies all ports mapped
  - Returns error when target node has no mapping
- **Error:** `ExpandError::UnmappedNestedOutput { cluster_id, port_name }`
- **Test:** `nested_output_mapping_failure_rejected` (cluster.rs:2953-3024)
- **PR/Commit:** <pending>

---

### X.10 — Compute parameters must not be Series type

- **ID:** X.10
- **Rule:** Compute primitives must not declare parameters with `ValueType::Series`. Series is valid for inputs/outputs but not parameters.
- **Disposition:** REJECT
- **Enforcement locus:** catalog registration (`crates/runtime/src/catalog.rs:177-185`)
  - `map_compute_param_type` returns `Option` (None for Series)
  - `register_compute` returns `Result<(), ValidationError>`
  - Defensive unreachable in `map_compute_param_value` for Series
- **Error:** `ValidationError::UnsupportedParameterType { primitive, version, parameter, got }`
- **Test:** `series_parameter_type_rejected` (catalog.rs)
- **PR/Commit:** <pending>

---

### X.11 — Int→f64 conversion must be exactly representable

- **ID:** X.11
- **Rule:** Int parameter values converted to f64 must be within exact representation range (|i| ≤ 2^53). Values outside this range are rejected to prevent silent precision loss.
- **Disposition:** CLOSE
- **Enforcement locus:** execution (`crates/runtime/src/runtime/execute.rs:285-304`)
  - `MAX_SAFE_INT` constant defined as 9_007_199_254_740_992 (2^53)
  - `map_to_compute_parameter_value` returns `None` for Int where `i.abs() > MAX_SAFE_INT`
  - Caller in `execute_compute` produces `ExecError::ParameterOutOfRange` with full context
- **Error:** `ExecError::ParameterOutOfRange { node, parameter, value }`
- **Tests:**
  - `int_parameter_within_f64_exact_range_allowed` (tests.rs)
  - `int_parameter_out_of_range_rejected` (tests.rs)
- **PR/Commit:** <pending>

---

### REP-1b — Point-of-use hash verification in replay path

- **ID:** REP-1b
- **Invariant:** REP-1 (Capture records are self-validating)
- **Disposition:** CLOSE (defense-in-depth)
- **Change:** Point-of-use hash verification via `rehydrate_checked()` in supervisor replay path; prevents bypass of `validate_bundle()` / accidental unchecked `rehydrate()` usage.
- **Enforcement loci:**
  - adapter: `ExternalEventRecord::rehydrate_checked()`
  - supervisor: `replay_inner()` calls checked rehydrate, propagates error
  - `validate_bundle()` retained as belt-and-suspenders
- **Error:** `ReplayError::HashMismatch { event_id }`
- **Test:** `replay_rejects_mid_stream_corruption`
- **PR/Commit:** <pending>

---

### CAT-SYNC-1 — Catalog entries have registry counterparts

- **ID:** CAT-SYNC-1
- **Rule:** Every primitive registered in CorePrimitiveCatalog must have a corresponding entry in the runtime registries.
- **Disposition:** CLOSE (defense-in-depth)
- **Enforcement locus:** test (`crates/runtime/src/catalog.rs`)
- **Test:** `registry_catalog_key_parity`
- **PR/Commit:** <pending>

---

### REG-SYNC-1 — Core registries and catalog must not drift

- **ID:** REG-SYNC-1
- **Rule:** `CoreRegistries` and `CorePrimitiveCatalog` must be built from a shared source of primitive definitions and contain the identical primitive key set (id, version, kind).
- **Disposition:** CLOSE
- **Enforcement locus:** runtime build path (`crates/runtime/src/catalog.rs`)
  - Introduce unified build flow (`build_core()`) that feeds both registry registration and catalog registration from the same primitive definition lists.
  - Add `debug_assert!` parity checks for registry vs catalog key sets at build end.
- **Error:** `debug_assert!` failure in debug builds; test failure in CI.
- **Test:** `registry_catalog_key_parity` (bidirectional key equality across source/compute/trigger/action).
- **Relationship to CAT-SYNC-1:** supersedes test-only parity with construction-time parity guarantees.
- **PR/Commit:** working tree (uncommitted)

---

### CAT-LOCKDOWN-1 — Catalog registration APIs are crate-private

- **ID:** CAT-LOCKDOWN-1
- **Rule:** External crates must not construct or mutate `CorePrimitiveCatalog` directly; catalog metadata admission must flow through core build paths.
- **Disposition:** CLOSE
- **Enforcement locus:**
  - `crates/runtime/src/catalog.rs`: `CorePrimitiveCatalog::new()` and `register_compute/register_source/register_trigger/register_action` are `pub(crate)`.
  - `crates/adapter/src/lib.rs`: migrated test callsite to `build_core_catalog()` (no direct catalog construction/registration).
- **Error:** compile-time visibility error on external direct catalog construction/registration attempts.
- **Test evidence:** `cargo test` (adapter tests compile through `build_core_catalog()` path), `registry_catalog_key_parity`.
- **Relationship to REG-SYNC-1:** complements shared-build drift prevention by blocking metadata-only admission from outside the runtime crate.
- **PR/Commit:** working tree (uncommitted)

---

### CMP-19 — Parameter default type matches declared type

- **ID:** CMP-19
- **Rule:** If a compute parameter declares `default: Some(value)`, the default value type must match the parameter's declared `value_type`.
- **Disposition:** CLOSE
- **Implemented action:** enforce CMP-19 in compute registration while keeping CMP-15 exclusive to `UnsupportedParameterType`.
- **Enforcement locus (mapping correction):** `crates/runtime/src/common/errors.rs` (`InvalidParameterType` maps to `CMP-19`, not `CMP-15`).
- **Enforcement locus (runtime):** `crates/runtime/src/compute/registry.rs` parameter validation loop (typed rejection via `ValidationError::InvalidParameterType` when default type mismatches).
- **Test (v0 correction):** mapping assertion coverage for `UnsupportedParameterType -> CMP-15` and `InvalidParameterType -> CMP-19`.
- **Test (runtime):** `cmp_19_invalid_parameter_type_default_rejected`.
- **Follow-up status (resolved):** CLI parse-time default coercion failures now map to concrete per-kind rule IDs via typed parse errors in `crates/ergo-cli/src/validate.rs` (`CMP-19`, `SRC-15`, `TRG-14`, `ACT-19`) instead of `rule_id = INTERNAL`.
- **PR/Commit:** working tree (uncommitted)

---

### SRC-15 — Source parameter default type matches declared type

- **ID:** SRC-15
- **Rule:** If a source parameter declares `default: Some(value)`, the default value type must match the parameter's declared `value_type`.
- **Disposition:** CLOSE
- **Enforcement locus:** `crates/runtime/src/source/registry.rs` — parameter validation loop in `validate_manifest()`. Error: `SourceValidationError::InvalidParameterType { parameter, expected, got }`.
- **Rule ID mapping:** `crates/runtime/src/source/mod.rs` — `Self::InvalidParameterType { .. } => "SRC-15"`.
- **Path:** `$.parameters[].default`
- **Fix:** Change parameter default value to match the declared parameter type.
- **Test:** `src_15_invalid_parameter_type_default_rejected`, `src_15_matching_parameter_default_accepted`
- **PR/Commit:** working tree (uncommitted)

---

### TRG-14 — Trigger parameter default type matches declared type

- **ID:** TRG-14
- **Rule:** If a trigger parameter declares `default: Some(value)`, the default value type must match the parameter's declared `value_type`.
- **Disposition:** CLOSE
- **Enforcement locus:** `crates/runtime/src/trigger/registry.rs` — parameter validation loop in `validate_manifest()`. Error: `TriggerValidationError::InvalidParameterType { parameter, expected, got }`.
- **Rule ID mapping:** `crates/runtime/src/trigger/mod.rs` — `Self::InvalidParameterType { .. } => "TRG-14"`.
- **Path:** `$.parameters[].default`
- **Fix:** Change parameter default value to match the declared parameter type.
- **Test:** `trg_14_invalid_parameter_type_default_rejected`, `trg_14_matching_parameter_default_accepted`
- **PR/Commit:** working tree (uncommitted)

---

### ACT-19 — Action parameter default type matches declared type

- **ID:** ACT-19
- **Rule:** If an action parameter declares `default: Some(value)`, the default value type must match the parameter's declared `value_type`.
- **Disposition:** CLOSE
- **Enforcement locus:** `crates/runtime/src/action/registry.rs` — parameter validation loop in `validate_manifest()`. Error: `ActionValidationError::InvalidParameterType { parameter, expected, got }`.
- **Rule ID mapping:** `crates/runtime/src/action/mod.rs` — `Self::InvalidParameterType { .. } => "ACT-19"`.
- **Path:** `$.parameters[].default`
- **Fix:** Change parameter default value to match the declared parameter type.
- **Test:** `act_19_invalid_parameter_type_default_rejected`, `act_19_matching_parameter_default_accepted`
- **PR/Commit:** working tree (uncommitted)

---

### CMP-20 — Compute output type validity maps to explicit rule ID

- **ID:** CMP-20
- **Rule:** Compute output types must be valid `ValueType` values (`Number | Series | Bool | String`).
- **Disposition:** CLOSE
- **Implemented action:** replace explicit INTERNAL mapping for `ValidationError::InvalidOutputType` with `CMP-20`.
- **Enforcement locus (mapping):** `crates/runtime/src/common/errors.rs` — `Self::InvalidOutputType { .. } => "CMP-20"`, with explicit `doc_anchor`, `path`, and `fix` entries.
- **Enforcement locus (registration):** `crates/runtime/src/compute/registry.rs` output validation loop includes exhaustive `ValueType` matching (`CMP-20` structural enforcement by type system).
- **Test (mapping):** `cmp_20_reserved_for_invalid_output_type` in `crates/runtime/src/common/errors.rs`.
- **Test (registration):** `cmp_20_output_types_valid` in `crates/runtime/src/compute/registry.rs`.
- **Notes:** Invalid output type strings in raw CLI manifests still fail in parse conversion before registry validation; this closure addresses the runtime rule-ID gap and registration contract mapping.
- **PR/Commit:** working tree (uncommitted)

---

### INTERNAL-CATCHALL-1 — Remove wildcard INTERNAL mappings in ErrorInfo::rule_id

- **ID:** INTERNAL-CATCHALL-1
- **Rule:** `ErrorInfo::rule_id()` implementations must be exhaustive; wildcard fallthrough (`_ => "INTERNAL"`) is forbidden.
- **Disposition:** CLOSE
- **Enforcement loci:** `rule_id()` matches in:
  - `crates/runtime/src/source/mod.rs`
  - `crates/runtime/src/trigger/mod.rs`
  - `crates/runtime/src/action/mod.rs`
  - `crates/runtime/src/common/errors.rs`
  - `crates/runtime/src/cluster.rs`
  - `crates/runtime/src/runtime/types.rs` (`ValidationError`, `ExecError`)
- **Required treatment per variant:** assign real rule ID when governed; keep explicit `INTERNAL` arm only for documented defense-in-depth variants; remove dead/phase-impure variants per approved disposition.
- **Test evidence:** compile-time exhaustiveness after wildcard removal + targeted regression tests for reassigned/live variants.
- **PR/Commit:** working tree (uncommitted)

---

### CAPTURE-FMT-1 — Capture format version is single-source-of-truth

- **ID:** CAPTURE-FMT-1
- **Rule:** The capture bundle format version must be defined in exactly one place and referenced everywhere else.
- **Disposition:** CLOSE
- **Enforcement locus:** constant (`crates/supervisor/src/lib.rs`)
  - `pub(crate) const CAPTURE_FORMAT_VERSION: &str = "v1";`
  - `capture.rs` uses `crate::CAPTURE_FORMAT_VERSION.to_string()` for bundle creation
  - `replay.rs` uses `crate::CAPTURE_FORMAT_VERSION` for version validation
- **Test:** N/A (compile-time consistency)
- **PR/Commit:** working tree (v1 hard-break cleanup)

---

### SUP-NOW-1 — Wall-clock ban covers entire supervisor crate

- **ID:** SUP-NOW-1
- **Rule:** No supervisor source file may use `SystemTime::now` or `Instant::now`. Deterministic replay requires all timing to flow through captured events.
- **Disposition:** CLOSE
- **Enforcement locus:** test (`crates/supervisor/tests/replay_harness.rs`)
  - `no_wall_clock_usage` test scans all `.rs` files in `src/` directory
  - Checks for forbidden patterns: `SystemTime::now`, `Instant::now`
  - Fails with descriptive error identifying file and pattern
- **Test:** `no_wall_clock_usage`
- **PR/Commit:** cd4dd86

---

### RENAME-FILLED-1 — ActionOutcome::Filled renamed to Completed

- **ID:** RENAME-FILLED-1
- **Change:** `ActionOutcome::Filled` → `ActionOutcome::Completed`
- **Rationale:** Domain neutrality (TERMINOLOGY.md §9). "Filled" implies order execution; "Completed" is generic.
- **Disposition:** CLOSE
- **Enforcement locus:** `crates/runtime/src/action/mod.rs`
- **Serialization:** Not currently serialized. Doc comment added for future alias requirement.
- **Test:** Existing `ack_action_respects_accept_parameter` and `hello_world_graph_produces_expected_outputs` verify Completed outcome.
- **PR/Commit:** 6bf4596

---

## STRING-SOURCE-1 — ValueType::String has a source producer

**Date:** 2026-01-05
**Status:** CLOSED
**Category:** stdlib Surface Coverage

### Finding

`ValueType::String` existed in the type system (`cluster::ValueType`, `RuntimeValue`) but had no source primitive to produce string values. This created a surface gap where string outputs could be declared but never originated.

### Resolution

Added `string_source` primitive:

- `crates/runtime/src/source/implementations/string/` (mod.rs, manifest.rs, impl.rs)
- `common::ValueType::String` and `common::Value::String` variants added
- Registered in `catalog.rs` (both `core_registries()` and `build_core_catalog()`)
- Four mapping functions updated to handle new variant (exhaustive match cascade)

### Tests

- `string_source_emits_configured_value`
- `string_source_defaults_to_empty_string`

### PR/Commit

PR #20 (feat/string-source)

---

## UI-REF-CLIENT-1

**Date:** 2026-01-05
**Status:** CLOSED
**Category:** Trust Boundary Clarification

### Finding

`crates/reference-client` was implicitly treated as a canonical contract implementation. Audit revealed:

- TypeScript types are incomplete (InputPortSpec, RuntimeEvent)
- No TypeScript type checking (missing tsconfig.json)
- Reference client conveniences should not be relied upon by production clients

### Resolution

Reframe as **Reference Client**:

- Runtime contract authority is Rust types + UI_RUNTIME_CONTRACT.md
- TypeScript mirror is best-effort, not enforcement
- UI-side validation is advisory only

### v1 Tracking

- UI-TSCHECK-1: Add tsconfig.json
- UI-CONTRACT-ALIGN-1: Fix TS contract drift
- UI-COERCION-1: Remove Inspector silent fallbacks
- UI-INT-GUARD-1: Add Int range guards

---

## CONTEXT-NUMBER-SOURCE-1 — Payload-derived context value

**Date:** 2026-01-10
**Status:** CLOSED
**Category:** Source Coverage Completion

### Finding

Source primitives lacked ability to read values from event payloads.
source.md §3 specifies "All dependencies must be parameters or orchestrator context"
but no context-reading source existed.

### Resolution

Added `context_number_source`:
- Reads key "x" from ExecutionContext via `ctx.value(key).and_then(|v| v.as_number())`
- Returns 0.0 on missing key or type mismatch (deterministic default)
- `SourcePrimitive::produce()` signature extended to receive `&ExecutionContext`

### Justification
- source.md §3: "All dependencies must be parameters or orchestrator context"
- source.md §6: Lists `context_string` as canonical v0 example
- SUPERVISOR.md §2.2: ExecutionContext contains "event payloads"
- DEMO-2: Vertical proof showing trigger flip based on payload x=0.0 vs x=5.0

### Tests
- `context_number_source_reads_context_value`
- `context_number_source_missing_key_returns_default`
- `context_number_source_wrong_type_returns_default`

### PR/Commit
PR #31, #32

---

## B.2 — Divide-by-Zero Semantics

**Date:** 2026-01-10  
**Status:** CLOSED  
**Category:** Compute Semantics

### Finding

Divide implementation used IEEE 754 semantics, producing inf/-inf/NaN on divide-by-zero. These values propagated silently through the graph, causing counterintuitive trigger behavior.

### Resolution

Adopted Option C after stress-testing with ChatGPT:

1. **`divide`** (v0.2.0) — Math-true semantics
   - Zero check: `impl.rs` line 55
   - Returns `Err(ComputeError::DivisionByZero)`: line 56
   - Finite check: line 60
   - Returns `Err(ComputeError::NonFiniteResult)`: line 61
   - Version bump: `manifest.rs` line 9

2. **`safe_divide`** (v0.1.0) — Explicit fallback semantics
   - Zero check: `impl.rs` line 60
   - Returns fallback: line 61
   - Non-finite check: lines 63-64
   - Returns fallback: line 67
   - Fallback parameter required: `manifest.rs` lines 28, 31
   - Registered: `catalog.rs` lines 106, 425

3. **NUM-FINITE-1** — Runtime guard
   - Definition: `execute.rs` line 296
   - Number check: line 302
   - Series check: line 308
   - Called after Source: line 143
   - Called after Compute: line 201
   - Error type: `types.rs` line 127

4. **ErrKind::SemanticError** — Non-retryable classification
   - Definition: `lib.rs` lines 50-56
   - Retry logic: `lib.rs` lines 381, 384-386

### Justification

- Ontology §1.4 (Non-Normative): "Division by zero is undefined" is math, not domain policy
- Separating `divide` (strict) from `safe_divide` (explicit fallback) keeps primitives math-true
- NUM-FINITE-1 provides defense-in-depth against non-finite values from any source

### Implementation Notes

- `ComputeError::DivisionByZero` and `ComputeError::NonFiniteResult` are specific to the `divide` implementation
- Other computes producing inf/NaN (e.g., multiply overflow) are caught by the NUM-FINITE-1 runtime guard, which raises `ExecError::NonFiniteOutput`
- `safe_divide` does not validate that `fallback` is finite — if a non-finite fallback is provided, NUM-FINITE-1 will catch it after the compute returns
- Both error paths result in `ErrKind::SemanticError` (non-retryable)

### Tests
- `divide_by_zero_errors`
- `divide_by_negative_zero_errors`
- `divide_zero_by_zero_errors`
- `divide_overflow_errors`
- `safe_divide_by_zero_uses_fallback`
- `safe_divide_overflow_uses_fallback`
- `num_finite_guard_rejects_nan`
- `num_finite_guard_rejects_infinity`
- `semantic_error_not_retryable`

### PR/Commit
PR #35, commit 7baa74c

---

## I.6 Resolver + Replay Runtime Provenance (v2-only capture policy)

### Resolution

- I.6 is enforced in `crates/runtime/src/cluster.rs` via strict semver selector parsing and deterministic highest-satisfying resolution for primitive and nested cluster references.
- Canonical captures now require `runtime_provenance` (runtime surface fingerprint) in v2 bundles; strict replay checks both adapter provenance and runtime provenance.
- Repo policy is explicit: capture bundles and checked-in capture fixtures are ephemeral artifacts and may be migrated in lockstep with code (no schema backward-compat guarantee inside this repo).

### Notes

- `replay_checked` remains non-strict (bundle validation + hash validation + replay only).
- `replay_checked_strict` requires explicit adapter provenance (`NO_ADAPTER_PROVENANCE` for no-adapter captures) and runtime provenance.
- v1 capture support is intentionally removed in the final state.
