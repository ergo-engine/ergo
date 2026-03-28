//! io
//!
//! Purpose:
//! - Expose the loader's public bundle/error types and top-level load entrypoints for filesystem
//!   and in-memory graph sources.
//! - Convert decoded discovery output into caller-facing source bundles or sealed
//!   `PreparedGraphAssets` handoff objects.
//! - Keep filesystem reads and best-effort path canonicalization on the loader transport surface.
//!
//! Owns:
//! - Open reporting/result DTOs (`FilesystemGraphBundle`, `InMemoryGraphBundle`) plus the sealed
//!   invariant-bearing `PreparedGraphAssets` handoff.
//! - `LoaderError` and its transport/decode/discovery variants.
//! - Public loading APIs that read graph sources and assemble source maps or prepared assets.
//! - Best-effort filesystem canonicalization for loader identity and bundle reporting.
//!
//! Does not own:
//! - Graph text decode rules, cluster discovery traversal, project/profile loading, or kernel
//!   semantic validation.
//! - Host prep options, orchestration, or any runtime execution behavior.
//!
//! Connects to:
//! - `discovery.rs` for filesystem and in-memory cluster discovery results.
//! - `resolver.rs` for in-memory source-id normalization and filesystem canonical-path identity.
//! - Host and SDK callers that consume loader bundles, prepared assets, and loader errors.
//!
//! Safety notes:
//! - Public loader errors must remain transport/decode/discovery errors and must not become
//!   kernel rule violations.
//! - `PreparedGraphAssets` is externally immutable by construction; callers read through accessors
//!   rather than constructing or mutating the sealed carrier directly.
//! - In-memory bundle ordering and labeling must stay truthful to normalized source IDs and caller
//!   supplied diagnostic labels.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use ergo_runtime::cluster::Version;

use crate::discovery::{discover_cluster_tree, discover_in_memory_cluster_tree_validated};
use crate::in_memory::InMemorySourceInput;
use crate::resolver::{normalize_in_memory_source_id, validate_in_memory_inputs};
use crate::DecodedAuthoringGraph;

// Bundle DTOs stay open because they report discovered sources back to callers rather than
// protecting loader-owned invariants.
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

// `PreparedGraphAssets` stays sealed because host prep depends on loader-owned invariants rather
// than caller-constructed reporting data.
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

    // Bundle reporting is keyed by canonical filesystem identity so aliasing does not duplicate
    // source entries once discovery has already resolved canonical source paths.
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
    let validated = validate_in_memory_inputs(sources, search_roots)?;
    let root_source = validated.root_source(&normalized_root_source_id)?;
    let discovery =
        discover_in_memory_cluster_tree_validated(&normalized_root_source_id, &validated)?;
    let root = discovery.root.clone();

    let mut source_map = BTreeMap::new();
    let mut source_labels = BTreeMap::new();
    source_map.insert(
        normalized_root_source_id.clone(),
        root_source.content.to_string(),
    );
    source_labels.insert(
        normalized_root_source_id.clone(),
        root_source.source_label.to_string(),
    );

    // Public in-memory bundle order follows normalized source ID ordering because the BTreeMap is
    // the canonical reporting surface, while caller order remains the resolver precedence rule.
    for source_id in discovery.cluster_source_ids.values() {
        if source_map.contains_key(source_id) {
            continue;
        }
        let source = validated.source(source_id).ok_or_else(|| {
            LoaderError::Discovery(LoaderDiscoveryError {
                message: format!("in-memory source_id '{}' was not provided", source_id),
            })
        })?;
        source_map.insert(
            source.normalized_source_id.clone(),
            source.content.to_string(),
        );
        source_labels.insert(
            source.normalized_source_id.clone(),
            source.source_label.to_string(),
        );
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
    let validated = validate_in_memory_inputs(sources, search_roots)?;
    let normalized_root_source_id = normalize_in_memory_source_id(root_source_id)?;
    let discovery =
        discover_in_memory_cluster_tree_validated(&normalized_root_source_id, &validated)?;
    Ok(PreparedGraphAssets {
        root: discovery.root,
        clusters: discovery.clusters,
        cluster_diagnostic_labels: discovery.cluster_diagnostic_labels,
        _sealed: (),
    })
}

pub(crate) fn canonicalize_or_self(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
