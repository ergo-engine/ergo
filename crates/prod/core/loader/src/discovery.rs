use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use ergo_runtime::cluster::Version;

use crate::decode::{parse_graph_file, selector_matches_version, DecodedAuthoringGraph};
use crate::io::{canonicalize_or_self, LoaderDiscoveryError, LoaderError};

pub struct ClusterDiscovery {
    pub clusters: HashMap<(String, Version), DecodedAuthoringGraph>,
    pub cluster_sources: HashMap<(String, Version), PathBuf>,
}

struct CandidateSearch {
    searched_paths: Vec<PathBuf>,
    existing_paths: Vec<PathBuf>,
}

pub fn resolve_cluster_candidates(
    base_dir: &Path,
    cluster_id: &str,
    search_paths: &[PathBuf],
) -> Result<Vec<PathBuf>, LoaderError> {
    Ok(collect_candidate_search(base_dir, cluster_id, search_paths).existing_paths)
}

fn collect_candidate_search(
    base_dir: &Path,
    cluster_id: &str,
    search_paths: &[PathBuf],
) -> CandidateSearch {
    let filename = format!("{cluster_id}.yaml");

    let mut candidates = vec![
        base_dir.join(&filename),
        base_dir.join("clusters").join(&filename),
    ];

    for path in search_paths {
        candidates.push(path.join(&filename));
        if path.file_name() != Some(OsStr::new("clusters")) {
            candidates.push(path.join("clusters").join(&filename));
        }
    }

    let mut searched_seen = HashSet::new();
    let mut searched_paths = Vec::new();
    let mut existing_seen = HashSet::new();
    let mut existing_paths = Vec::new();
    for candidate in candidates {
        if searched_seen.insert(candidate.clone()) {
            searched_paths.push(candidate.clone());
        }
        if !candidate.exists() {
            continue;
        }
        let canonical = canonicalize_or_self(&candidate);
        if existing_seen.insert(canonical) {
            existing_paths.push(candidate);
        }
    }

    CandidateSearch {
        searched_paths,
        existing_paths,
    }
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
        let (canonical, cluster_key) = self.record_cluster_definition(path, &def)?;

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

        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        for node in def.nodes.values() {
            let ergo_runtime::cluster::NodeKind::Cluster {
                cluster_id,
                version,
            } = &node.kind
            else {
                continue;
            };
            let candidate_search =
                collect_candidate_search(base_dir, cluster_id, &self.search_paths);
            if candidate_search.existing_paths.is_empty() {
                return Err(missing_cluster_error(
                    cluster_id,
                    version,
                    &node.id,
                    path,
                    &candidate_search,
                ));
            }

            for cluster_path in &candidate_search.existing_paths {
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
                    return Err(id_mismatch_error(
                        cluster_id,
                        version,
                        &nested.id,
                        cluster_path,
                        &node.id,
                        path,
                    ));
                }

                self.record_cluster_definition(cluster_path, &nested)?;

                if !selector_matches_version(version, &nested.version).map_err(discovery_error)? {
                    continue;
                }

                self.visit(cluster_path, nested)?;
            }
        }

        self.visiting_paths.remove(&canonical);
        self.visiting_keys.remove(&cluster_key);
        Ok(())
    }

    fn record_cluster_definition(
        &mut self,
        path: &Path,
        def: &DecodedAuthoringGraph,
    ) -> Result<(PathBuf, (String, Version)), LoaderError> {
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

        self.clusters
            .entry(cluster_key.clone())
            .or_insert_with(|| def.clone());

        Ok((canonical, cluster_key))
    }
}

fn missing_cluster_error(
    cluster_id: &str,
    version: &str,
    node_id: &str,
    file_path: &Path,
    search: &CandidateSearch,
) -> LoaderError {
    discovery_error(format!(
        "looked for '{}.yaml' for cluster '{}@{}' in:\n{}\nnot found.\ncluster resolution is filename-based: the file must be named '{}.yaml'.\nreferenced by node '{}' in '{}'",
        cluster_id,
        cluster_id,
        version,
        format_paths(&search.searched_paths),
        cluster_id,
        node_id,
        file_path.display()
    ))
}

fn id_mismatch_error(
    expected_id: &str,
    requested_version: &str,
    actual_id: &str,
    opened_path: &Path,
    node_id: &str,
    file_path: &Path,
) -> LoaderError {
    discovery_error(format!(
        "opened '{}' for cluster '{}@{}', but the YAML id is '{}'.\ncluster resolution is path-based, and the filename must match the YAML id field.\nfix: rename the file to '{}.yaml' or change the cluster id in the graph/YAML to match.\nreferenced by node '{}' in '{}'",
        opened_path.display(),
        expected_id,
        requested_version,
        actual_id,
        expected_id,
        node_id,
        file_path.display()
    ))
}

fn format_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| format!("- {}", path.display()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn discovery_error(message: String) -> LoaderError {
    LoaderError::Discovery(LoaderDiscoveryError { message })
}
