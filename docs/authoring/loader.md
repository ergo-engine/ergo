---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-26
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
- Project manifest loading plus per-profile resolution into graph,
  adapter, ingress, egress, capture, and cluster path references
- YAML/JSON decode of graph content into `ClusterDefinition`
- Shorthand expansion and format-level coercions tied to file format
- Cluster discovery and candidate resolution across filesystem paths or
  logical in-memory source ids
- Source map construction for diagnostics

The loader does NOT:

- Access the primitive catalog
- Perform semantic validation (wiring legality, type rules)
- Define or return `RuleViolation` types (LAYER-2)
- Perform semantic graph expansion, signature inference, or execution

---

## API Surface

- `load_graph_sources`
  File path plus search paths to `FilesystemGraphBundle` (primary filesystem entry
  point).
- `load_in_memory_graph_sources`
  Root source ID plus ordered in-memory sources plus logical search roots to
  `InMemoryGraphBundle`.
- `load_graph_assets_from_paths`
  Lower-level path loader to sealed `PreparedGraphAssets` for host prep and
  validation.
- `load_graph_assets_from_memory`
  Lower-level in-memory loader to sealed `PreparedGraphAssets` for host prep
  and validation.
- `discover_project_root`
  Nested path to project root containing `ergo.toml`.
- `load_project`
  Project root discovery plus `ergo.toml` parse into `ResolvedProject { root, manifest }`.
  Individual profiles are resolved later via `ResolvedProject::resolve_run_profile(...)`.
- `decode_graph_yaml`
  YAML string to `ClusterDefinition`.
- `decode_graph_yaml_labeled`
  YAML string plus caller-supplied human-facing source label to
  `ClusterDefinition`.
- `decode_graph_json`
  JSON string to `ClusterDefinition`.
- `parse_graph_file`
  File path to decoded cluster.
- `discovery::discover_cluster_tree`
  Module-public advanced helper under `ergo_loader::discovery`; root path plus
  search paths to discovery output including the internally parsed root,
  decoded clusters, and their source paths.
- `discovery::discover_in_memory_cluster_tree`
  Module-public advanced helper under `ergo_loader::discovery`; root source ID
  plus ordered in-memory sources plus logical search roots to in-memory
  discovery output, including the internally parsed root.
- `load_cluster_tree`
  Root path plus search paths to the full cluster tree keyed by `(id, version)`.
- `resolve_cluster_candidates`
  Base directory plus cluster ID plus search paths to deduplicated
  candidate file paths.
- `io::canonicalize_or_self`
  Module-public helper returning a canonical filesystem path when possible and
  the original path otherwise.

---

## Output Type

```rust
pub struct FilesystemGraphBundle {
    pub root: ClusterDefinition,
    pub discovered_files: Vec<PathBuf>,
    pub source_map: BTreeMap<PathBuf, String>,
}
```

```rust
pub struct PreparedGraphAssets {
    root: ClusterDefinition,
    clusters: HashMap<(String, Version), ClusterDefinition>,
    cluster_diagnostic_labels: HashMap<(String, Version), String>,
    pub(crate) _sealed: (),
}
```

```rust
pub struct ClusterDiscovery {
    pub root: ClusterDefinition,
    pub clusters: HashMap<(String, Version), ClusterDefinition>,
    pub cluster_sources: HashMap<(String, Version), PathBuf>,
    pub cluster_diagnostic_labels: HashMap<(String, Version), String>,
}
```

```rust
pub struct InMemorySourceInput {
    pub source_id: String,
    pub source_label: String,
    pub content: String,
}
```

```rust
pub struct InMemoryClusterDiscovery {
    pub root: ClusterDefinition,
    pub clusters: HashMap<(String, Version), ClusterDefinition>,
    pub cluster_source_ids: HashMap<(String, Version), String>,
    pub cluster_source_labels: HashMap<(String, Version), String>,
    pub cluster_diagnostic_labels: HashMap<(String, Version), String>,
}
```

```rust
pub struct InMemoryGraphBundle {
    pub root: ClusterDefinition,
    pub discovered_source_ids: Vec<String>,
    pub source_map: BTreeMap<String, String>,
    pub source_labels: BTreeMap<String, String>,
}
```

For the in-memory surface:

- `source_id` is the public logical source identity and lookup path
- callers should use path-like `source_id` values such as `graphs/root.yaml`
  because referrer-sensitive cluster resolution and search traces are derived
  from logical path structure
- logical paths are loader-defined, platform-independent, and use `/` separators
- `source_id` and `search_roots` must be relative logical paths; rooted paths,
  backslashes, `:`, empty path segments, and `.` / `..` segments are rejected
- `source_label` is the human-facing diagnostic label
- the current in-memory API requires unique `source_label` values per call so diagnostics remain
  unambiguous
- `source_label` is not semantic identity or lookup path
- `content` is graph authoring text parsed through the loader's string decode
  path; YAML and JSON authoring text are both accepted
- cluster lookup still follows the existing filename-style contract on
  `source_id`; a resolvable cluster source id must end in `<cluster_id>.yaml`
- `source_id` values must be unique and non-empty
- `source_label` values must be non-empty
- `root_source_id` must be present in the provided source list
- `discovery::discover_in_memory_cluster_tree` is a module-public advanced
  helper under `ergo_loader::discovery`; it parses the root internally from
  `root_source_id` and returns it in `InMemoryClusterDiscovery.root`
- `discovery::discover_cluster_tree` is the filesystem twin; it parses the root
  internally from `root_path` and returns it in `ClusterDiscovery.root`
- both discovery outputs expose `cluster_diagnostic_labels`; host/error
  consumers should read labels from that field rather than deriving them from
  `PathBuf` or `source_label` directly
- when multiple in-memory sources could satisfy the same lookup, caller `sources` order
  decides which matching source is chosen; `search_roots` define candidate scope and search
  trace order rather than source precedence
- `InMemoryGraphBundle.discovered_source_ids` follows lexicographic `source_id`
  order, matching `source_map` key order
- logical `search_roots` preserve referrer-sensitive discovery scope without
  inventing fake filesystem paths

`ClusterDefinition` is a kernel-owned type. The loader produces it
directly. There is no parallel intermediate representation.
`DecodedAuthoringGraph` remains as a type alias, not a separate IR.

`PreparedGraphAssets` is the lower-level sealed handoff used by the host prep
lane. It is constructed by the loader and re-exported by the host for advanced
callers, but it remains loader-owned and does not carry host runtime options.
The payload is externally immutable: callers read it through `root()`,
`clusters()`, and `cluster_diagnostic_labels()` accessors rather than mutating
the asset bundle directly.

---

## Error Boundary

Loader errors are transport and decode failures, exposed via `LoaderError`:

- `LoaderIoError` — file not found, permission denied
- `LoaderDecodeError` — malformed YAML, missing required fields
  (and malformed JSON / labeled string decode failures)
- `LoaderDiscoveryError` — discovery-time lookup and cluster-tree failures such
  as missing clusters, nested parse failures, ID mismatches, duplicate
  definitions, and circular references

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
