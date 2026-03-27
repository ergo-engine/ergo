---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-26
Owner: Documentation
Scope: Inference-phase invariants for signature derivation
Change Rule: Operational log
---

## 4. Inference Phase

**Scope:** Deriving signature from expanded graph.

**Entry invariants:**

- Expanded graph is complete (E.1–E.5 hold)
- `PrimitiveCatalog` is canonical and version-consistent

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| F.1 | Input ports are never wireable | cluster-spec.md §3.2 | — | ✓ | — | ✓ |
| F.2 | Output wireability is determined by source node kind (Action → non-wireable) | cluster-spec.md §3.2 | — | — | — | ✓ |
| F.3 | `BoundaryKind` inference follows precedence: ActionLike → SourceLike → TriggerLike → ComputeLike | cluster-spec.md §3.4 | — | — | — | ✓ |
| F.4 | `has_side_effects` is true iff any expanded node is Action | cluster-spec.md §3.3 | — | — | — | ✓ |
| F.5 | `is_origin` is true iff no inputs AND all roots are Sources | cluster-spec.md §3.3 | — | — | — | ✓ |
| F.6 | Signature inference depends only on expanded graph + catalog (no other state) | (inferred) | — | — | — | — |

### Notes

- **F.1:** ✅ **CLOSED.** Fixed in cluster.rs. Enforcement:
  - Assertion: `cluster::infer_signature` hard-sets inferred input ports to `wireable: false` and asserts the invariant via `debug_assert!`
  - Test: `cluster::tests::input_ports_are_never_wireable`
  - Merged.

- **F.6:** True by construction. Document on `infer_signature`:

```rust
/// Signature inference assumes a canonical, version-consistent PrimitiveCatalog.
/// Providing a catalog with different or incomplete primitive metadata will produce
/// undefined or incorrect signatures.
```
