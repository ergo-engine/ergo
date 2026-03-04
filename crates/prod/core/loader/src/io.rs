use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::decode::parse_graph_file;
use crate::discovery::discover_cluster_tree;
use crate::DecodedAuthoringGraph;

#[derive(Debug, Clone)]
pub struct LoadedGraphBundle {
    pub root: DecodedAuthoringGraph,
    pub discovered_files: Vec<PathBuf>,
    pub source_map: BTreeMap<PathBuf, String>,
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
) -> Result<LoadedGraphBundle, LoaderError> {
    let canonical = canonicalize_or_self(path);
    let root_source_text = fs::read_to_string(path).map_err(|err| {
        LoaderError::Io(LoaderIoError {
            path: path.to_path_buf(),
            message: err.to_string(),
        })
    })?;
    let root = parse_graph_file(path)?;
    let discovered = discover_cluster_tree(path, &root, search_paths)?;

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

    Ok(LoadedGraphBundle {
        root,
        discovered_files,
        source_map,
    })
}

pub fn canonicalize_or_self(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
