---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-02
Owner: Claude (Structural Auditor)
Scope: Loader responsibility boundary, decode contract
Change Rule: Tracks implementation
---

# Loader Contract

**Crate:** `crates/prod/core/loader`

The loader owns graph file transport, format decode, and cluster discovery. It operates entirely without catalog access — the catalog-access boundary (yaml-format.md §8.3) is the loader/kernel divide.

---

## Responsibility

The loader does:
- YAML/JSON decode of graph files into `ClusterDefinition`
- Shorthand expansion and format-level coercions tied to file format
- Cluster file discovery and candidate resolution
- Source map construction for diagnostics

The loader does NOT:
- Access the primitive catalog
- Perform semantic validation (wiring legality, type rules)
- Define or return `RuleViolation` types (LAYER-2)
- Perform expansion, inference, or execution

---

## API Surface

| Function | Responsibility |
|----------|---------------|
| `decode_graph_yaml` | YAML bytes → `ClusterDefinition` |
| `parse_graph_file` | File path → decoded cluster |
| `load_cluster_tree` | Root path → full cluster tree with discovery |
| `resolve_cluster_candidates` | Directory → deduplicated cluster file candidates |

---

## Output Type

```rust
pub struct LoadedGraphBundle {
    pub root: ClusterDefinition,
    pub discovered_files: Vec<PathBuf>,
    pub source_map: BTreeMap<PathBuf, String>,
}
```

`ClusterDefinition` is a kernel-owned type. The loader produces it directly — there is no parallel intermediate representation. `DecodedAuthoringGraph` remains as a type alias, not a separate IR.

---

## Error Boundary

Loader errors are transport and decode failures:
- IO errors (file not found, permission denied)
- Parse errors (malformed YAML, missing required fields)
- Discovery errors (ambiguous candidates, circular references)

These are NOT rule violations. The loader never produces `RuleViolation` or references invariant IDs. Semantic errors begin at the kernel boundary when expansion/validation consumes the `ClusterDefinition`.

---

## Relationship to Other Documents

- **yaml-format.md** — Defines the YAML schema the loader decodes
- **cluster-spec.md** — Defines `ClusterDefinition` and the expansion algorithm the kernel applies after loading
- **kernel.md** — Defines the boundary rules (LAYER-1, LAYER-2) that constrain the loader
