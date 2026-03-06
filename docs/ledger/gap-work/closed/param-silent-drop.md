---
Authority: ESCALATION
Severity: HIGH
Date: 2026-02-02
Author: Claude Opus 4.5 (Structural Auditor)
Status: RESOLVED — Option A implemented (2026-02-02)
---

# ESCALATION: Undeclared Parameter Bindings Are Silently Dropped

## 1. Finding

When a cluster node provides a parameter binding for a name that does not exist
in the target primitive's manifest, the binding is silently discarded during
expansion. No error, no warning, no log.

## 2. Reproduction

Given a primitive with manifest:

```yaml
parameters:
  - name: value
    type: number
    default: 0.0
```

And a cluster node binding:

```yaml
parameters:
  valeu: 42.0   # typo — "valeu" instead of "value"
```

**Expected:** Expansion fails with an error indicating `"valeu"` is not a declared parameter.

**Actual:** Expansion succeeds silently. `"valeu"` is discarded. Primitive receives
`value = 0.0` (the default). Graph executes with wrong data. No indication of error.

## 3. Root Cause

`resolve_impl_parameters()` in `crates/kernel/runtime/src/cluster.rs` (lines 1307-1347)
iterates only over declared specs:

```rust
for spec in specs {
    match bindings.get(&spec.name) { ... }
}
```

It never checks whether `bindings` contains keys absent from `specs`. The reverse
lookup (`bindings.keys() - specs.keys()`) does not exist anywhere in the codebase.

The same pattern exists in:

- `build_resolved_params()` (cluster.rs, lines 1353-1393) — cluster parameters
- `validate_parameter_bindings()` (cluster.rs, lines 889-944) — nested clusters

## 4. Scope of Impact

- **All four primitive types** (Source, Compute, Trigger, Action) are affected.
  All parameter resolution flows through `resolve_impl_parameters()`.
- **Nested clusters** are also affected via `build_resolved_params()`.
- **No phase catches this:** Not expansion, not validation, not execution.
- **Primitives with `.expect()` on parameters will panic** if the intended binding
  was the only source of that parameter (no default). This converts a user typo
  into a runtime panic instead of a typed validation error.
- **Primitives with defaults will silently use the wrong value.** The user believes
  they configured the primitive; the primitive runs with its default. This is
  truth-destroying per the system's own doctrine.

## 5. Evidence of Prior Intent

Dead enum variants exist in every per-primitive error enum since the initial commit
(`66f6576`, 2025-12-27):

- `SourceValidationError::UndeclaredParameter { node, parameter }`
- `TriggerValidationError::UndeclaredParameter { node, parameter }`
- `ActionValidationError::UndeclaredParameter { node, parameter }`
- `ValidationError::UndeclaredParameter { node, parameter }` (compute)

These variants are defined but **never constructed anywhere**. They have ErrorInfo
impls that fall through to `_ => "INTERNAL"`. No rule ID was ever assigned. No
invariant in invariants/INDEX.md tracks this condition. No entry in closure-register.md.

## 6. Proposed Fix

In `resolve_impl_parameters()`, after the spec iteration loop, add:

```rust
let spec_names: HashSet<&str> = specs.iter().map(|s| s.name.as_str()).collect();
for key in bindings.keys() {
    if !spec_names.contains(key.as_str()) {
        return Err(ExpandError::UndeclaredParameter {
            node_id: node_id.to_string(),
            parameter: key.clone(),
        });
    }
}
```

This requires:

1. Adding `UndeclaredParameter { node_id: String, parameter: String }` to `ExpandError`
2. Same check in `build_resolved_params()` for cluster parameters
3. Same check in `validate_parameter_bindings()` for nested clusters
4. A new invariant (e.g., `I.7`) in invariants/INDEX.md
5. Tests: `undeclared_parameter_binding_rejected`, `parameter_typo_rejected`

## 7. Disposition Options

- **A. Fix now.** Add the validation. Assign a rule ID. Document in invariants/INDEX.md.
- **B. Document as known gap.** Add to closure-register.md with V1 SEMANTICS disposition
  if there's a reason to accept undeclared bindings (e.g., forward compatibility).
- **C. Warn, don't reject.** Log undeclared bindings but don't fail expansion.
  Weakest option — silent data loss is still possible if logs are ignored.

## 8. Recommendation

Option A. This is a correctness issue, not a feature request. Silent data loss
violates the system's own principle that truth is not negotiable. The fix is
mechanical, low-risk, and the error variants already exist (just need wiring).

## 9. Related Dead Code

34 additional dead enum variants exist across the per-primitive error enums.
They duplicate graph-level checks that are already enforced by `runtime/validate.rs`
under different error types. These should be audited separately — either deleted
or assigned rule IDs. See conversation transcript for full inventory.

---

## 10. Resolution

**Disposition:** Option A — Fix now.

**Implementation (2026-02-02):**

1. Added `ExpandError::UndeclaredParameter { node_id, parameter }` variant to `cluster.rs`
2. Added I.7 check to `resolve_impl_parameters()` — primitive parameter bindings
3. Added I.7 check to `build_resolved_params()` — nested cluster parameter bindings
4. Added I.7 check to `validate_parameter_bindings()` — nested cluster pre-validation
5. Wired ErrorInfo: rule_id `"I.7"`, doc_anchor `authoring/cluster-spec.md#I.7`
6. Added I.7 to invariants/INDEX.md §2 (Instantiation Phase) — invariant #157
7. Added I.7 to cluster-spec.md instantiation rules table
8. Tests: `undeclared_primitive_parameter_binding_rejected`, `undeclared_cluster_parameter_binding_rejected`
9. Full test suite: 150/150 passing

**Verified:** `cargo test -p ergo-runtime` — all tests pass, no regressions.
