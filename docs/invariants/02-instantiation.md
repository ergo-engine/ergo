## 2. Instantiation Phase

**Scope:** When a cluster is placed in a parent context.

**Entry invariants:**
- Parent context exists and is valid
- Cluster definition passes Definition phase validation

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| I.1 | Wiring from parent edge source to cluster boundary kind is legal | cluster-spec.md §6.2 | — | — | ✓ | ✓ |
| I.2 | Port types match at connection points | cluster-spec.md §6.2 | — | — | ✓ | ✓ |
| I.3 | All required parameters are either bound or exposed | cluster-spec.md §6.2 | — | — | ✓ | ✓ |
| I.4 | Bound parameter values are type-compatible | cluster-spec.md §6.2 | — | — | ✓ | ✓ |
| I.5 | Exposed parameters reference parameters that exist in parent context | cluster-spec.md §6.2 | — | — | ✓ | ✓ |
| I.6 | Version constraints are satisfied | cluster-spec.md §6.2 | — | — | ✓ | ✓ |
| I.7 | Parameter bindings reference only declared parameters | cluster-spec.md §6.2 | — | — | ✓ | ✓ |

### Notes

- **I.3–I.5:** Enforced in `cluster.rs::expand_with_context` during nested cluster processing via `validate_parameter_bindings()`. Errors: `MissingRequiredParameter`, `ParameterBindingTypeMismatch`, `ExposedParameterNotFound`, `ExposedParameterTypeMismatch`. Tests: `required_parameter_missing_rejected`, `parameter_binding_type_mismatch_rejected`, `exposed_parameter_not_in_parent_rejected`, `exposed_parameter_type_mismatch_rejected`. Note: I.4 is enforced symmetrically for both Literal and Exposed bindings.
  - **Strengthened (2025-01-05):** Exposed bindings now propagate through nested cluster hierarchies via `resolve_bindings_with_context()` and `build_resolved_params()`. Prior behavior only validated at immediate cluster boundary; multi-level nesting (Parent → Middle → Inner → Leaf) now correctly receives propagated values. Added `ExpandError::UnresolvedExposedBinding { node_id, parameter, referenced }` for primitives with dangling Exposed bindings. Tests: `exposed_binding_propagates_to_leaf_primitive`, `unresolved_exposed_binding_rejected`. Location: `cluster.rs:expand_with_context()`.
  - **Default application (2025-01-05):** Parameters with `default: Some(value)` in either `ParameterMetadata` (primitives) or `ParameterSpec` (clusters) are automatically applied during expansion when no binding is provided. Enforced by `resolve_impl_parameters()` (primitives, cluster.rs:988-1028) and `build_resolved_params()` (clusters, cluster.rs:1034-1074). Tests: `defaulted_parameter_propagates_to_leaf`, `explicit_binding_overrides_default`, `missing_required_param_no_default_rejected`, `cluster_parameter_default_propagates_to_nested`.
- **I.7:** Enforced in `cluster.rs` across three functions: `resolve_impl_parameters()` (primitive nodes), `build_resolved_params()` (nested cluster instantiation), and `validate_parameter_bindings()` (nested cluster pre-validation). Each builds a `HashSet` of declared parameter names from the target's spec and rejects any binding key absent from that set. Error: `ExpandError::UndeclaredParameter { node_id, parameter }`. Tests: `undeclared_primitive_parameter_binding_rejected`, `undeclared_cluster_parameter_binding_rejected`. Prior to this fix, undeclared bindings were silently dropped — a typo in a parameter name would cause the primitive to receive its default value with no error. See `ESCALATION_PARAM_SILENT_DROP.md` for the full finding.
- **I.6:** Enforced in `crates/kernel/runtime/src/cluster.rs` by selector parsing + deterministic resolution (highest satisfying semver) using `ClusterVersionIndex` / `PrimitiveVersionIndex`. Errors: `InvalidVersionSelector`, `UnsatisfiedVersionConstraint`, `InvalidAvailableVersion`. CLI graph parsing in `crates/prod/clients/cli/src/graph_yaml.rs` also enforces strict semver for cluster definition versions and node selectors (exact semver or semver constraints).
