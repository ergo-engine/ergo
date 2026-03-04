## 3. Expansion Phase

**Scope:** Recursive flattening of clusters to primitives.

**Entry invariants:**
- All referenced clusters are loadable
- All parameters are concretely bound (no unresolved `Exposed` bindings at root)

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| E.1 | Output contains only primitives (no `NodeKind::Cluster` survives) | cluster-spec.md §7 | — | ✓ | — | ✓ |
| E.2 | All placeholder edges are rewritten to node-to-node edges | cluster-spec.md §7 | — | ✓ | — | ✓ |
| E.3 | `ExternalInput` does not appear as edge target (sink) | (inferred) | — | ✓ | — | — |
| E.4 | Authoring path is preserved for each expanded node | cluster-spec.md §7.2 | — | — | — | ✓ |
| E.5 | Empty clusters are rejected | cluster-spec.md §6.1 | — | — | ✓ | ✓ |
| E.6 | Original cluster definitions are not mutated | (inferred) | — | — | — | — |
| E.7 | `ExpandedGraph` carries boundary ports for inference only | (inferred) | — | — | — | — |
| E.8 | Runtime ID assignment is deterministic for identical definitions | (inferred) | — | — | ✓ | ✓ |
| E.9 | Referenced nested clusters exist | cluster-spec.md §6.2 | — | — | ✓ | ✓ |

### Notes

- **E.3:** Requires assertion. Silent assumption is unacceptable.
- **E.6:** True by clone semantics but not explicitly enforced.
- **E.7:** Requires doc comment on `ExpandedGraph` to make contract explicit:

```rust
/// Expansion output. Contains only topology, primitive identity, and authoring trace.
/// `boundary_inputs` and `boundary_outputs` are retained for signature inference only
/// and must not influence runtime execution.
```

- **E.2:** ✅ Strengthened (2025-01-05). Boundary output mapping (`map_boundary_outputs`) and nested output mapping now return typed errors instead of silent fallback. Errors: `ExpandError::UnmappedBoundaryOutput { port_name, node_id }`, `ExpandError::UnmappedNestedOutput { cluster_id, port_name }`. Tests: `unmapped_boundary_output_rejected`, `nested_output_mapping_failure_rejected`.
- **E.8:** ✅ **CLOSED.** Enforced via sorted-key iteration in `expand_with_context` (cluster.rs:694-698). Test: `expansion_runtime_ids_deterministic`.
- **E.9:** Enforced in `cluster.rs::expand_with_context()` when resolving `NodeKind::Cluster` via `ClusterLoader::load`. Missing references return `ExpandError::MissingCluster { id, version }`. Test: `missing_nested_cluster_rejected`.
