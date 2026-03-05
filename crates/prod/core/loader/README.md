# ergo-loader

`ergo-loader` owns transport and format decode only.

## Scope

- Read source bytes from filesystem paths.
- Decode YAML/JSON graph content into `ClusterDefinition` (`DecodedAuthoringGraph` is a type alias).
- Discover candidate cluster files from base/search paths.

## Non-Goals

- No ontology/rule validation.
- No `RuleViolation` emission.
- No run-policy or canonical execution decisions.

## Error Categories

- `LoaderIoError`: filesystem and path read failures.
- `LoaderDecodeError`: syntax/format decode failures.
- `LoaderDiscoveryError`: file discovery/path resolution failures.

## Public Entry Points

- `load_graph_sources(...) -> Result<LoadedGraphBundle, LoaderError>`
- `decode_graph_yaml(...) -> Result<DecodedAuthoringGraph, LoaderError>`
- `decode_graph_json(...) -> Result<DecodedAuthoringGraph, LoaderError>`
- `resolve_cluster_candidates(...) -> Result<Vec<PathBuf>, LoaderError>`
- `load_cluster_tree(...) -> Result<HashMap<(String, Version), DecodedAuthoringGraph>, LoaderError>`

## LoadedGraphBundle

- `root: ClusterDefinition`
- `discovered_files: Vec<PathBuf>`
- `source_map: BTreeMap<PathBuf, String>`
