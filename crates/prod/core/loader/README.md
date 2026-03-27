# ergo-loader

`ergo-loader` owns project discovery/resolution, source transport, format
decode, and cluster discovery.

## Scope

- Read UTF-8 graph text from filesystem paths or caller-provided in-memory sources.
- Decode YAML/JSON graph content into `ClusterDefinition` (`DecodedAuthoringGraph` is a type alias).
- Discover candidate clusters from filesystem base/search paths or from
  loader-defined logical in-memory source paths and logical search roots.

## Non-Goals

- No ontology/rule validation.
- No `RuleViolation` emission.
- No run-policy or canonical execution decisions.

## Error Categories

- `LoaderIoError`: direct top-level filesystem read/open failures.
- `LoaderDecodeError`: syntax/format decode failures.
- `LoaderDiscoveryError`: filesystem path, logical source-path, and
  cluster-tree discovery failures, including nested cluster lookup/read
  problems surfaced during discovery.

## Public Entry Points

- `load_graph_sources(...) -> Result<FilesystemGraphBundle, LoaderError>`
- `load_in_memory_graph_sources(...) -> Result<InMemoryGraphBundle, LoaderError>`
- `load_graph_assets_from_paths(...) -> Result<PreparedGraphAssets, LoaderError>`
- `load_graph_assets_from_memory(...) -> Result<PreparedGraphAssets, LoaderError>`
- `decode_graph_yaml(...) -> Result<DecodedAuthoringGraph, LoaderError>`
- `decode_graph_yaml_labeled(...) -> Result<DecodedAuthoringGraph, LoaderError>`
- `decode_graph_json(...) -> Result<DecodedAuthoringGraph, LoaderError>`
- `parse_graph_file(...) -> Result<DecodedAuthoringGraph, LoaderError>`
- `resolve_cluster_candidates(...) -> Result<Vec<PathBuf>, LoaderError>`
- `load_cluster_tree(...) -> Result<HashMap<(String, Version), DecodedAuthoringGraph>, LoaderError>`
- `discovery::discover_cluster_tree(...) -> Result<ClusterDiscovery, LoaderError>`
- `discovery::discover_in_memory_cluster_tree(...) -> Result<InMemoryClusterDiscovery, LoaderError>`
- `discover_project_root(...) -> Result<PathBuf, ProjectError>`
- `load_project(...) -> Result<ResolvedProject, ProjectError>`

Module-public helper surfaces also include:

- `discovery::ClusterDiscovery`
- `discovery::InMemoryClusterDiscovery`
- `InMemorySourceInput`
- `io::canonicalize_or_self(...)`

The two `discovery::...` helpers are advanced module-public surfaces under
`ergo_loader::discovery`, not top-level re-exports.

`decode_graph_yaml(...)` keeps the historical `<memory>` label for simple string
use. `decode_graph_yaml_labeled(...)` should be used when the caller has a
truthful human-facing source label for diagnostics.

## FilesystemGraphBundle

- `root: ClusterDefinition`
- `discovered_files: Vec<PathBuf>`
- `source_map: BTreeMap<PathBuf, String>`

## PreparedGraphAssets

```rust
pub struct PreparedGraphAssets {
    root: ClusterDefinition,
    clusters: HashMap<(String, Version), ClusterDefinition>,
    cluster_diagnostic_labels: HashMap<(String, Version), String>,
    pub(crate) _sealed: (),
}
```

- lower-level loader handoff for host prep/validation
- constructed only by loader asset-loading functions
- externally immutable: callers read through `root()`, `clusters()`, and
  `cluster_diagnostic_labels()` accessors rather than mutating the payload
- carries semantic graph assets and diagnostic labels only
- does not carry source maps, discovered-file lists, or runtime configuration

## In-memory inputs and outputs

```rust
pub struct InMemorySourceInput {
    pub source_id: String,
    pub source_label: String,
    pub content: String,
}
```

- `source_id` is the stable public logical source identity and lookup path
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
- `discovery::discover_in_memory_cluster_tree(...)` parses the root internally
  from `root_source_id` and returns it in `InMemoryClusterDiscovery.root`
- `discovery::discover_cluster_tree(...)` parses the root internally from
  `root_path` and returns it in `ClusterDiscovery.root`
- both discovery outputs expose `cluster_diagnostic_labels`; host/error
  consumers should read labels from that field rather than deriving them from
  `PathBuf` or `source_label` directly

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
pub struct ClusterDiscovery {
    pub root: ClusterDefinition,
    pub clusters: HashMap<(String, Version), ClusterDefinition>,
    pub cluster_sources: HashMap<(String, Version), PathBuf>,
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

The in-memory loader APIs take ordered `&[InMemorySourceInput]` plus explicit
logical `search_roots`. Resolution precedence follows caller input order; the
bundle output order follows lexicographic `source_id` order.
