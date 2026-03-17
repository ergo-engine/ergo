---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-16
Owner: Claude (Structural Auditor)
Scope: Loader responsibility boundary, decode contract
Change Rule: Tracks implementation
---

# Loader Contract

**Crate:** `crates/prod/core/loader`

The loader owns project discovery, graph file transport, format
decode, and cluster discovery. It operates entirely without catalog
access. The
catalog-access boundary (`yaml-format.md` §8.3) is the loader/kernel
divide.

---

## Responsibility

The loader does:

- Project root discovery via `ergo.toml`
- Project/profile manifest resolution into graph, adapter, ingress,
  egress, and cluster path references
- YAML decode of graph files into `ClusterDefinition` (JSON decode is
  stubbed but not yet wired)
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

- `load_graph_sources`
  File path plus search paths to `LoadedGraphBundle` (primary entry
  point).
- `discover_project_root`
  Nested path to project root containing `ergo.toml`.
- `load_project`
  Project root discovery plus `ergo.toml` parse into resolved
  profiles.
- `decode_graph_yaml`
  YAML string to `ClusterDefinition`.
- `parse_graph_file`
  File path to decoded cluster.
- `load_cluster_tree`
  Root path plus parsed root plus search paths to the full cluster
  tree keyed by `(id, version)`.
- `resolve_cluster_candidates`
  Base directory plus cluster ID plus search paths to deduplicated
  candidate file paths.

---

## Output Type

```rust
pub struct LoadedGraphBundle {
    pub root: ClusterDefinition,
    pub discovered_files: Vec<PathBuf>,
    pub source_map: BTreeMap<PathBuf, String>,
}
```

`ClusterDefinition` is a kernel-owned type. The loader produces it
directly. There is no parallel intermediate representation.
`DecodedAuthoringGraph` remains as a type alias, not a separate IR.

---

## Error Boundary

Loader errors are transport and decode failures, exposed via `LoaderError`:

- `LoaderIoError` — file not found, permission denied
- `LoaderDecodeError` — malformed YAML, missing required fields
- `LoaderDiscoveryError` — ambiguous candidates, circular references

These are NOT rule violations. The loader never produces
`RuleViolation` or references invariant IDs. Semantic errors begin at
the kernel boundary when expansion/validation consumes the
`ClusterDefinition`.

---

## Relationship to Other Documents

- **project-convention.md** — Defines how project resolution supplies
  graph paths, cluster search paths, and profiles to the loader
- **yaml-format.md** — Defines the YAML schema the loader decodes
- **cluster-spec.md** — Defines `ClusterDefinition` and the expansion
  algorithm the kernel applies after loading
- **kernel.md** — Defines the boundary rules (LAYER-1, LAYER-2) that
  constrain the loader
