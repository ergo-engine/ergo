//! discovery
//!
//! Purpose:
//! - Discover referenced cluster definitions starting from a filesystem root path or an in-memory
//!   root source ID.
//! - Return loader-owned cluster trees plus source provenance needed for diagnostics and later
//!   loader I/O handoff.
//! - Expose public candidate-resolution helpers used by callers that need truthful discovery scope
//!   without entering kernel semantics.
//!
//! Owns:
//! - Filesystem and in-memory cluster-tree assembly over decoded `ClusterDefinition` values.
//! - Duplicate-definition, circular-reference, missing-cluster, and cluster-id/path agreement
//!   checks that belong to loader discovery rather than kernel validation.
//! - Public discovery result shapes, including diagnostic labels and source provenance.
//!
//! Does not own:
//! - Graph text decode details, raw source resolution mechanics, project/profile loading, or
//!   kernel semantic validation.
//! - Catalog access, rule IDs, or `RuleViolation` surfaces.
//! - The shared in-memory source input carrier; that lives in `in_memory.rs` and is re-exported
//!   here for compatibility.
//!
//! Connects to:
//! - `decode` for graph parsing and cluster-reference selector checks.
//! - `resolver` for filesystem and in-memory source lookup.
//! - `io` and downstream host callers for graph-asset loading and diagnostics.
//!
//! Safety notes:
//! - Discovery errors stay on the loader transport/discovery surface and must not become semantic
//!   rule violations.
//! - Source provenance is preserved separately for filesystem and in-memory discovery so later
//!   diagnostics can remain truthful.
//! - Duplicate and cycle detection rely on resolver-defined source identity semantics.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use ergo_runtime::cluster::Version;

use crate::decode::{
    parse_graph_content, parse_graph_file, selector_matches_version, validate_cluster_reference_id,
    DecodedAuthoringGraph,
};
pub use crate::in_memory::InMemorySourceInput;
use crate::io::{LoaderDiscoveryError, LoaderError};
use crate::resolver::{
    normalize_in_memory_source_id, validate_in_memory_inputs, ClusterResolver, FilesystemResolver,
    InMemoryResolver, ResolvedSourceCandidate, SourceRef, ValidatedInMemoryInputs,
    ValidatedInMemorySource,
};

#[derive(Debug, Clone)]
pub struct ClusterDiscovery {
    pub root: DecodedAuthoringGraph,
    pub clusters: HashMap<(String, Version), DecodedAuthoringGraph>,
    pub cluster_sources: HashMap<(String, Version), PathBuf>,
    pub cluster_diagnostic_labels: HashMap<(String, Version), String>,
}

#[derive(Debug, Clone)]
pub struct InMemoryClusterDiscovery {
    pub root: DecodedAuthoringGraph,
    pub clusters: HashMap<(String, Version), DecodedAuthoringGraph>,
    pub cluster_source_ids: HashMap<(String, Version), String>,
    pub cluster_source_labels: HashMap<(String, Version), String>,
    pub cluster_diagnostic_labels: HashMap<(String, Version), String>,
}

pub fn resolve_cluster_candidates(
    base_dir: &Path,
    cluster_id: &str,
    search_paths: &[PathBuf],
) -> Result<Vec<PathBuf>, LoaderError> {
    validate_cluster_id(cluster_id)?;
    let resolver = FilesystemResolver::new(search_paths);
    Ok(resolver.resolve_existing_candidate_paths(base_dir, cluster_id))
}

pub fn load_cluster_tree(
    root_path: &Path,
    search_paths: &[PathBuf],
) -> Result<HashMap<(String, Version), DecodedAuthoringGraph>, LoaderError> {
    let discovery = discover_cluster_tree(root_path, search_paths)?;
    Ok(discovery.clusters)
}

pub fn discover_cluster_tree(
    root_path: &Path,
    search_paths: &[PathBuf],
) -> Result<ClusterDiscovery, LoaderError> {
    let root = parse_graph_file(root_path)?;
    let resolver = FilesystemResolver::new(search_paths);
    let mut builder = ClusterTreeBuilder::new(&resolver);
    builder.visit(SourceRef::from_opened_path(root_path), root.clone())?;
    let mut cluster_diagnostic_labels = HashMap::new();
    let cluster_sources = builder
        .cluster_sources
        .into_iter()
        .map(|(key, source_ref)| {
            // Diagnostics keep the opened label, while the public filesystem source map keeps the
            // canonical path identity chosen by resolver/source_ref semantics.
            cluster_diagnostic_labels.insert(key.clone(), source_ref.opened_label());
            (
                key,
                source_ref
                    .filesystem_canonical_path()
                    .expect("filesystem discovery must keep filesystem source refs")
                    .to_path_buf(),
            )
        })
        .collect();
    Ok(ClusterDiscovery {
        root,
        clusters: builder.clusters,
        cluster_sources,
        cluster_diagnostic_labels,
    })
}

pub fn discover_in_memory_cluster_tree(
    root_source_id: &str,
    sources: &[InMemorySourceInput],
    search_roots: &[String],
) -> Result<InMemoryClusterDiscovery, LoaderError> {
    let validated = validate_in_memory_inputs(sources, search_roots)?;
    let normalized_root_source_id = normalize_in_memory_source_id(root_source_id)?;
    let root_source = validated.root_source(&normalized_root_source_id)?;
    discover_in_memory_cluster_tree_validated(root_source, &validated)
}

pub(crate) fn discover_in_memory_cluster_tree_validated(
    root_source: &ValidatedInMemorySource<'_>,
    validated: &ValidatedInMemoryInputs<'_>,
) -> Result<InMemoryClusterDiscovery, LoaderError> {
    let resolver = InMemoryResolver::from_validated_inputs(validated);
    let root = parse_graph_content(root_source.content, root_source.source_label)?;
    let root_source_ref = root_source.source_ref();
    let mut builder = ClusterTreeBuilder::new(&resolver);
    builder.visit(root_source_ref, root.clone())?;

    let mut cluster_source_ids = HashMap::new();
    let mut cluster_source_labels = HashMap::new();
    let mut cluster_diagnostic_labels = HashMap::new();
    for (key, source_ref) in builder.cluster_sources {
        let source_id = source_ref
            .in_memory_source_id()
            .expect("in-memory discovery must keep in-memory source refs")
            .to_string();
        let diagnostic_label = source_ref.opened_label();
        cluster_source_labels.insert(key.clone(), diagnostic_label.clone());
        cluster_diagnostic_labels.insert(key.clone(), diagnostic_label);
        cluster_source_ids.insert(key, source_id);
    }

    Ok(InMemoryClusterDiscovery {
        root,
        clusters: builder.clusters,
        cluster_source_ids,
        cluster_source_labels,
        cluster_diagnostic_labels,
    })
}

struct ClusterTreeBuilder<'a, R: ClusterResolver> {
    clusters: HashMap<(String, Version), DecodedAuthoringGraph>,
    cluster_sources: HashMap<(String, Version), SourceRef>,
    decoded_sources: HashMap<SourceRef, DecodedAuthoringGraph>,
    completed_sources: HashSet<SourceRef>,
    visiting_sources: HashSet<SourceRef>,
    visiting_keys: HashSet<(String, Version)>,
    resolver: &'a R,
}

impl<'a, R: ClusterResolver> ClusterTreeBuilder<'a, R> {
    fn new(resolver: &'a R) -> Self {
        Self {
            clusters: HashMap::new(),
            cluster_sources: HashMap::new(),
            decoded_sources: HashMap::new(),
            completed_sources: HashSet::new(),
            visiting_sources: HashSet::new(),
            visiting_keys: HashSet::new(),
            resolver,
        }
    }

    fn visit(
        &mut self,
        source_ref: SourceRef,
        def: DecodedAuthoringGraph,
    ) -> Result<(), LoaderError> {
        self.decoded_sources
            .entry(source_ref.clone())
            .or_insert_with(|| def.clone());
        let cluster_key = self.record_cluster_definition(&source_ref, &def)?;
        if self.completed_sources.contains(&source_ref) {
            return Ok(());
        }

        // Detect both source-level cycles and semantic cluster-key cycles so discovery reports the
        // truthful failure even when the same cluster is reached through different paths.
        if !self.visiting_sources.insert(source_ref.clone()) {
            return Err(discovery_error(format!(
                "circular cluster reference detected at '{}'",
                source_ref.opened_label()
            )));
        }
        if !self.visiting_keys.insert(cluster_key.clone()) {
            return Err(discovery_error(format!(
                "circular cluster reference detected for '{}@{}' at '{}'",
                def.id,
                def.version,
                source_ref.opened_label()
            )));
        }

        let visit_result = self.visit_nested_clusters(&source_ref, &def);

        self.visiting_sources.remove(&source_ref);
        self.visiting_keys.remove(&cluster_key);
        if visit_result.is_ok() {
            self.completed_sources.insert(source_ref);
        }
        visit_result
    }

    fn visit_nested_clusters(
        &mut self,
        source_ref: &SourceRef,
        def: &DecodedAuthoringGraph,
    ) -> Result<(), LoaderError> {
        for node in def.nodes.values() {
            let ergo_runtime::cluster::NodeKind::Cluster {
                cluster_id,
                version,
            } = &node.kind
            else {
                continue;
            };
            let candidate_search = self.resolver.resolve(cluster_id, Some(source_ref))?;
            if candidate_search.found.is_empty() {
                return Err(missing_cluster_error(
                    cluster_id,
                    version,
                    &node.id,
                    source_ref,
                    &candidate_search.search_trace,
                ));
            }

            for candidate in candidate_search.found {
                let nested = self.decode_candidate_source(&candidate, cluster_id, version)?;

                if nested.id != *cluster_id {
                    return Err(id_mismatch_error(
                        cluster_id,
                        version,
                        &nested.id,
                        &candidate.source_ref,
                        &node.id,
                        source_ref,
                    ));
                }

                self.record_cluster_definition(&candidate.source_ref, &nested)?;

                if !selector_matches_version(version, &nested.version).map_err(discovery_error)? {
                    continue;
                }

                self.visit(candidate.source_ref, nested)?;
            }
        }

        Ok(())
    }

    fn decode_candidate_source(
        &mut self,
        candidate: &ResolvedSourceCandidate,
        cluster_id: &str,
        version: &str,
    ) -> Result<DecodedAuthoringGraph, LoaderError> {
        if let Some(decoded) = self.decoded_sources.get(&candidate.source_ref) {
            return Ok(decoded.clone());
        }

        let nested_content = self.resolver.read(&candidate.source_ref).map_err(|err| {
            discovery_error(format!(
                "failed parsing nested cluster '{}@{}' at '{}': {}",
                cluster_id, version, candidate.opened_label, err
            ))
        })?;
        let nested =
            parse_graph_content(&nested_content, &candidate.opened_label).map_err(|err| {
                discovery_error(format!(
                    "failed parsing nested cluster '{}@{}' at '{}': {}",
                    cluster_id, version, candidate.opened_label, err
                ))
            })?;
        self.decoded_sources
            .insert(candidate.source_ref.clone(), nested.clone());
        Ok(nested)
    }

    fn record_cluster_definition(
        &mut self,
        source_ref: &SourceRef,
        def: &DecodedAuthoringGraph,
    ) -> Result<(String, Version), LoaderError> {
        let cluster_key = (def.id.clone(), def.version.clone());

        if let Some(existing_source) = self.cluster_sources.get(&cluster_key) {
            if existing_source != source_ref {
                let left = conflict_source_display(existing_source);
                let right = conflict_source_display(source_ref);
                let source_kind = match (existing_source, source_ref) {
                    (SourceRef::Filesystem { .. }, SourceRef::Filesystem { .. }) => "files",
                    _ => "sources",
                };
                return Err(discovery_error(format!(
                    "cluster '{}@{}' is defined by multiple {}: {} and {}",
                    def.id, def.version, source_kind, left, right
                )));
            }
        } else {
            self.cluster_sources
                .insert(cluster_key.clone(), source_ref.clone());
        }

        self.clusters
            .entry(cluster_key.clone())
            .or_insert_with(|| def.clone());

        Ok(cluster_key)
    }
}

fn missing_cluster_error(
    cluster_id: &str,
    version: &str,
    node_id: &str,
    referring_source: &SourceRef,
    search_trace: &[String],
) -> LoaderError {
    match referring_source {
        SourceRef::Filesystem { .. } => discovery_error(format!(
            "looked for '{}.yaml' for cluster '{}@{}' in:\n{}\nnot found.\ncluster resolution is filename-based: the file must be named '{}.yaml'.\nreferenced by node '{}' in '{}'",
            cluster_id,
            cluster_id,
            version,
            format_search_trace(search_trace),
            cluster_id,
            node_id,
            referring_source.opened_label()
        )),
        SourceRef::InMemory { .. } => discovery_error(format!(
            "looked for logical source paths ending in '{}.yaml' for cluster '{}@{}' in:\n{}\nnot found.\nin-memory cluster resolution is logical-path-based: a matching source_id must end in '{}.yaml'.\nreferenced by node '{}' in '{}'",
            cluster_id,
            cluster_id,
            version,
            format_search_trace(search_trace),
            cluster_id,
            node_id,
            referring_source.opened_label()
        )),
    }
}

fn id_mismatch_error(
    expected_id: &str,
    requested_version: &str,
    actual_id: &str,
    candidate_source: &SourceRef,
    node_id: &str,
    referring_source: &SourceRef,
) -> LoaderError {
    match candidate_source {
        SourceRef::Filesystem { .. } => discovery_error(format!(
            "opened '{}' for cluster '{}@{}', but the graph id is '{}'.\ncluster resolution is path-based, and the filename must match the decoded graph id field.\nfix: rename the file to '{}.yaml' or change the cluster id in the graph content to match.\nreferenced by node '{}' in '{}'",
            candidate_source.opened_label(),
            expected_id,
            requested_version,
            actual_id,
            expected_id,
            node_id,
            referring_source.opened_label()
        )),
        SourceRef::InMemory {
            source_id,
            source_label,
            ..
        } => discovery_error(format!(
            "opened in-memory source '{}' (source_id '{}') for cluster '{}@{}', but the graph id is '{}'.\nin-memory cluster resolution is logical-path-based, so the graph reference, source_id, and graph id must agree on '{}'.\nfix: change the graph id to '{}' or change the graph reference and source_id to match.\nreferenced by node '{}' in '{}'",
            source_label,
            source_id,
            expected_id,
            requested_version,
            actual_id,
            expected_id,
            expected_id,
            node_id,
            referring_source.opened_label()
        )),
    }
}

fn format_search_trace(paths: &[String]) -> String {
    paths
        .iter()
        .map(|path| format!("- {path}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn discovery_error(message: String) -> LoaderError {
    LoaderError::Discovery(LoaderDiscoveryError { message })
}

fn validate_cluster_id(cluster_id: &str) -> Result<(), LoaderError> {
    validate_cluster_reference_id(cluster_id)
        .map_err(|err| discovery_error(format!("invalid cluster_id '{}': {}", cluster_id, err)))
}

fn conflict_source_display(source_ref: &SourceRef) -> String {
    match source_ref {
        SourceRef::Filesystem { .. } => format!(
            "'{}'",
            source_ref
                .filesystem_canonical_path()
                .expect("filesystem source ref")
                .display()
        ),
        SourceRef::InMemory {
            source_id,
            source_label,
            ..
        } => format!("'{}' (source_id '{}')", source_label, source_id),
    }
}

#[cfg(test)]
mod tests;
