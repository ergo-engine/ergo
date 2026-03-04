use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use ergo_runtime::cluster::Version;

use crate::decode::{parse_graph_file, selector_matches_version, DecodedAuthoringGraph};
use crate::io::{canonicalize_or_self, LoaderDiscoveryError, LoaderError};

pub struct ClusterDiscovery {
    pub clusters: HashMap<(String, Version), DecodedAuthoringGraph>,
    pub cluster_sources: HashMap<(String, Version), PathBuf>,
}

pub fn resolve_cluster_candidates(
    base_dir: &Path,
    cluster_id: &str,
    search_paths: &[PathBuf],
) -> Result<Vec<PathBuf>, LoaderError> {
    let filename = format!("{cluster_id}.yaml");

    let mut candidates = vec![
        base_dir.join(&filename),
        base_dir.join("clusters").join(&filename),
    ];

    for path in search_paths {
        candidates.push(path.join(&filename));
        candidates.push(path.join("clusters").join(&filename));
    }

    let mut seen = HashSet::new();
    let mut resolved = Vec::new();
    for candidate in candidates {
        if !candidate.exists() {
            continue;
        }
        let canonical = canonicalize_or_self(&candidate);
        if seen.insert(canonical) {
            resolved.push(candidate);
        }
    }

    Ok(resolved)
}

pub fn load_cluster_tree(
    root_path: &Path,
    root: &DecodedAuthoringGraph,
    search_paths: &[PathBuf],
) -> Result<HashMap<(String, Version), DecodedAuthoringGraph>, LoaderError> {
    let discovery = discover_cluster_tree(root_path, root, search_paths)?;
    Ok(discovery.clusters)
}

pub fn discover_cluster_tree(
    root_path: &Path,
    root: &DecodedAuthoringGraph,
    search_paths: &[PathBuf],
) -> Result<ClusterDiscovery, LoaderError> {
    let mut builder = ClusterTreeBuilder {
        clusters: HashMap::new(),
        cluster_sources: HashMap::new(),
        visiting_paths: HashSet::new(),
        visiting_keys: HashSet::new(),
        search_paths: search_paths.to_vec(),
    };
    builder.visit(root_path, root.clone())?;
    Ok(ClusterDiscovery {
        clusters: builder.clusters,
        cluster_sources: builder.cluster_sources,
    })
}

struct ClusterTreeBuilder {
    clusters: HashMap<(String, Version), DecodedAuthoringGraph>,
    cluster_sources: HashMap<(String, Version), PathBuf>,
    visiting_paths: HashSet<PathBuf>,
    visiting_keys: HashSet<(String, Version)>,
    search_paths: Vec<PathBuf>,
}

impl ClusterTreeBuilder {
    fn visit(&mut self, path: &Path, def: DecodedAuthoringGraph) -> Result<(), LoaderError> {
        let canonical = canonicalize_or_self(path);
        let cluster_key = (def.id.clone(), def.version.clone());

        if let Some(existing_path) = self.cluster_sources.get(&cluster_key) {
            if existing_path != &canonical {
                return Err(discovery_error(format!(
                    "cluster '{}@{}' is defined by multiple files: '{}' and '{}'",
                    def.id,
                    def.version,
                    existing_path.display(),
                    canonical.display()
                )));
            }
        } else {
            self.cluster_sources
                .insert(cluster_key.clone(), canonical.clone());
        }

        if !self.visiting_paths.insert(canonical.clone()) {
            return Err(discovery_error(format!(
                "circular cluster reference detected at '{}'",
                path.display()
            )));
        }
        if !self.visiting_keys.insert(cluster_key.clone()) {
            return Err(discovery_error(format!(
                "circular cluster reference detected for '{}@{}' at '{}'",
                def.id,
                def.version,
                path.display()
            )));
        }

        self.clusters
            .entry(cluster_key.clone())
            .or_insert_with(|| def.clone());

        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        for node in def.nodes.values() {
            let ergo_runtime::cluster::NodeKind::Cluster {
                cluster_id,
                version,
            } = &node.kind
            else {
                continue;
            };
            let cluster_paths =
                resolve_cluster_candidates(base_dir, cluster_id, &self.search_paths)?;
            if cluster_paths.is_empty() {
                return Err(discovery_error(format!(
                    "missing cluster file for '{}@{}' referenced by node '{}' in '{}'",
                    cluster_id,
                    version,
                    node.id,
                    path.display()
                )));
            }

            for cluster_path in cluster_paths {
                let nested = parse_graph_file(&cluster_path).map_err(|err| {
                    discovery_error(format!(
                        "failed parsing nested cluster '{}@{}' at '{}': {}",
                        cluster_id,
                        version,
                        cluster_path.display(),
                        err
                    ))
                })?;

                if nested.id != *cluster_id {
                    return Err(discovery_error(format!(
                        "cluster id mismatch in '{}': expected '{}', found '{}'",
                        cluster_path.display(),
                        cluster_id,
                        nested.id
                    )));
                }

                if !selector_matches_version(version, &nested.version).map_err(discovery_error)? {
                    continue;
                }

                self.visit(&cluster_path, nested)?;
            }
        }

        self.visiting_paths.remove(&canonical);
        self.visiting_keys.remove(&cluster_key);
        Ok(())
    }
}

fn discovery_error(message: String) -> LoaderError {
    LoaderError::Discovery(LoaderDiscoveryError { message })
}
