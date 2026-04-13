---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-26
Owner: Documentation
Scope: Expansion-phase invariants for flattening clusters to implementations
Change Rule: Operational log
---

## 3. Expansion Phase

**Scope:** Recursive flattening of clusters to primitives.

**Entry invariants:**

- All referenced clusters are loadable
- All parameters are concretely bound (no unresolved `Exposed` bindings at root)

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| E.1 | Output contains only primitives (no `NodeKind::Cluster` survives) | cluster-spec.md §7 | — | ✓ | — | ✓ |
| E.2 | Placeholder edge rewrites are applied where resolvable during expansion | cluster-spec.md §7 | — | ✓ | — | ✓ |
| E.3 | No surviving `ExternalInput` endpoint reaches executable runtime | (inferred) | — | ✓ | — | — |
| E.4 | Authoring path is preserved for each expanded node | cluster-spec.md §7.2 | — | — | — | ✓ |
| E.5 | Empty clusters are rejected | cluster-spec.md §6.1 | — | — | ✓ | ✓ |
| E.6 | Original cluster definitions are not mutated | (inferred) | — | — | — | — |
| E.7 | `ExpandedGraph` retains boundary ports for signature inference, and boundary outputs also drive runtime result collection | (inferred) | — | — | — | — |
| E.8 | Runtime ID assignment is deterministic for identical definitions | (inferred) | — | — | ✓ | ✓ |
| E.9 | Referenced nested clusters exist | cluster-spec.md §6.2 | — | — | ✓ | ✓ |

### Notes

- **E.3:** Expansion may still materialize `ExpandedEndpoint::ExternalInput` for unresolved or top-level `$input` sources. Enforced during expansion in `cluster.rs`; returns `ExpandError::InvariantViolation` if an `ExternalInput` survives as an edge sink. Runtime validation also rejects any surviving `ExternalInput` endpoint before execution.
- **E.6:** True by clone semantics but not explicitly enforced.
- **E.7:** `ExpandedGraph` already documents retained boundary ports. `boundary_inputs` remain part of signature inference, while `boundary_outputs` also survive validation and are used during execution to assemble named runtime outputs.
- **E.2:** Boundary output mapping (`map_boundary_outputs`) and nested output mapping now return typed `D.4` mapping errors instead of silently falling through. `ExpandError::UnmappedBoundaryOutput { ... }` and `ExpandError::UnmappedNestedOutput { ... }` are output-port mapping failures, not placeholder-edge rewrite failures.
- **E.8:** ✅ **CLOSED.** Enforced via sorted-key iteration in `expand_with_context` (cluster.rs:694-698). Test: `expansion_runtime_ids_deterministic`.
- **E.9:** Enforced in `cluster.rs::expand_with_context()` when resolving `NodeKind::Cluster` via `ClusterLoader::load`. Missing references return `ExpandError::MissingCluster { id, version }`. Test: `missing_nested_cluster_rejected`.
