use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use ergo_runtime::cluster::Version;

use crate::discovery::{
    discover_cluster_tree, discover_in_memory_cluster_tree, InMemorySourceInput,
};
use crate::resolver::normalize_in_memory_source_id;
use crate::DecodedAuthoringGraph;

#[derive(Debug, Clone)]
pub struct FilesystemGraphBundle {
    pub root: DecodedAuthoringGraph,
    pub discovered_files: Vec<PathBuf>,
    pub source_map: BTreeMap<PathBuf, String>,
}

#[derive(Debug, Clone)]
pub struct InMemoryGraphBundle {
    pub root: DecodedAuthoringGraph,
    pub discovered_source_ids: Vec<String>,
    pub source_map: BTreeMap<String, String>,
    pub source_labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct PreparedGraphAssets {
    root: DecodedAuthoringGraph,
    clusters: HashMap<(String, Version), DecodedAuthoringGraph>,
    cluster_diagnostic_labels: HashMap<(String, Version), String>,
    pub(crate) _sealed: (),
}

impl PreparedGraphAssets {
    pub fn root(&self) -> &DecodedAuthoringGraph {
        &self.root
    }

    pub fn clusters(&self) -> &HashMap<(String, Version), DecodedAuthoringGraph> {
        &self.clusters
    }

    pub fn cluster_diagnostic_labels(&self) -> &HashMap<(String, Version), String> {
        &self.cluster_diagnostic_labels
    }
}

#[derive(Debug)]
pub enum LoaderError {
    Io(LoaderIoError),
    Decode(LoaderDecodeError),
    Discovery(LoaderDiscoveryError),
}

#[derive(Debug)]
pub struct LoaderIoError {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug)]
pub struct LoaderDecodeError {
    pub message: String,
}

#[derive(Debug)]
pub struct LoaderDiscoveryError {
    pub message: String,
}

impl std::fmt::Display for LoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error at '{}': {}", err.path.display(), err.message),
            Self::Decode(err) => write!(f, "decode error: {}", err.message),
            Self::Discovery(err) => write!(f, "discovery error: {}", err.message),
        }
    }
}

impl std::error::Error for LoaderError {}

pub fn load_graph_sources(
    path: &Path,
    search_paths: &[PathBuf],
) -> Result<FilesystemGraphBundle, LoaderError> {
    let canonical = canonicalize_or_self(path);
    let root_source_text = fs::read_to_string(path).map_err(|err| {
        LoaderError::Io(LoaderIoError {
            path: path.to_path_buf(),
            message: err.to_string(),
        })
    })?;
    let discovered = discover_cluster_tree(path, search_paths)?;
    let root = discovered.root.clone();

    let mut source_map = BTreeMap::new();
    source_map.insert(canonical.clone(), root_source_text);

    for source_path in discovered.cluster_sources.values() {
        if source_map.contains_key(source_path) {
            continue;
        }
        let text = fs::read_to_string(source_path).map_err(|err| {
            LoaderError::Io(LoaderIoError {
                path: source_path.clone(),
                message: err.to_string(),
            })
        })?;
        source_map.insert(source_path.clone(), text);
    }

    let discovered_files = source_map.keys().cloned().collect();

    Ok(FilesystemGraphBundle {
        root,
        discovered_files,
        source_map,
    })
}

pub fn load_graph_assets_from_paths(
    graph_path: &Path,
    cluster_paths: &[PathBuf],
) -> Result<PreparedGraphAssets, LoaderError> {
    let discovery = discover_cluster_tree(graph_path, cluster_paths)?;
    Ok(PreparedGraphAssets {
        root: discovery.root,
        clusters: discovery.clusters,
        cluster_diagnostic_labels: discovery.cluster_diagnostic_labels,
        _sealed: (),
    })
}

pub fn load_in_memory_graph_sources(
    root_source_id: &str,
    sources: &[InMemorySourceInput],
    search_roots: &[String],
) -> Result<InMemoryGraphBundle, LoaderError> {
    let normalized_root_source_id = normalize_in_memory_source_id(root_source_id)?;
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_labels = std::collections::HashSet::new();
    let mut normalized_sources = Vec::new();
    for source in sources {
        let normalized_source_id = normalize_in_memory_source_id(&source.source_id)?;
        if source.source_label.is_empty() {
            return Err(LoaderError::Discovery(LoaderDiscoveryError {
                message: "in-memory source_label must not be empty".to_string(),
            }));
        }
        if !seen_ids.insert(normalized_source_id.clone()) {
            return Err(LoaderError::Discovery(LoaderDiscoveryError {
                message: format!("duplicate in-memory source_id '{}'", normalized_source_id),
            }));
        }
        if !seen_labels.insert(source.source_label.clone()) {
            return Err(LoaderError::Discovery(LoaderDiscoveryError {
                message: format!(
                    "duplicate in-memory source_label '{}' (tranche 1 requires unique diagnostic labels per call)",
                    source.source_label
                ),
            }));
        }
        normalized_sources.push((normalized_source_id, source));
    }

    let root_source = normalized_sources
        .iter()
        .find(|(source_id, _)| source_id == &normalized_root_source_id)
        .ok_or_else(|| {
            LoaderError::Discovery(LoaderDiscoveryError {
                message: format!(
                    "root in-memory source_id '{}' was not provided",
                    normalized_root_source_id
                ),
            })
        })?;
    let discovery =
        discover_in_memory_cluster_tree(&normalized_root_source_id, sources, search_roots)?;
    let root = discovery.root.clone();

    let mut source_map = BTreeMap::new();
    let mut source_labels = BTreeMap::new();
    source_map.insert(
        normalized_root_source_id.clone(),
        root_source.1.content.clone(),
    );
    source_labels.insert(
        normalized_root_source_id.clone(),
        root_source.1.source_label.clone(),
    );

    for source_id in discovery.cluster_source_ids.values() {
        if source_map.contains_key(source_id) {
            continue;
        }
        let source = normalized_sources
            .iter()
            .find(|(normalized_source_id, _)| normalized_source_id == source_id)
            .ok_or_else(|| {
                LoaderError::Discovery(LoaderDiscoveryError {
                    message: format!("in-memory source_id '{}' was not provided", source_id),
                })
            })?;
        source_map.insert(source.0.clone(), source.1.content.clone());
        source_labels.insert(source.0.clone(), source.1.source_label.clone());
    }

    let discovered_source_ids = source_map.keys().cloned().collect();

    Ok(InMemoryGraphBundle {
        root,
        discovered_source_ids,
        source_map,
        source_labels,
    })
}

pub fn load_graph_assets_from_memory(
    root_source_id: &str,
    sources: &[InMemorySourceInput],
    search_roots: &[String],
) -> Result<PreparedGraphAssets, LoaderError> {
    let discovery = discover_in_memory_cluster_tree(root_source_id, sources, search_roots)?;
    Ok(PreparedGraphAssets {
        root: discovery.root,
        clusters: discovery.clusters,
        cluster_diagnostic_labels: discovery.cluster_diagnostic_labels,
        _sealed: (),
    })
}

pub fn canonicalize_or_self(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
