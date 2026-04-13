//! expand_diagnostics
//!
//! Purpose:
//! - Provide host-private helpers for turning `ExpandError` cluster-version
//!   failures into concrete available-cluster diagnostics.
//!
//! Owns:
//! - Extraction of available cluster ids, versions, and display locations from
//!   filesystem-backed discovery maps and in-memory asset labels.
//!
//! Does not own:
//! - Public host error surfaces such as `HostExpandError` or
//!   `GraphToDotExpansionError`.
//! - Cluster expansion semantics, which remain in `ergo_runtime::cluster`.
//!
//! Connects to:
//! - `graph_dot_usecase.rs` and `usecases.rs`, which each map the shared
//!   diagnostics into their own public error types.
//!
//! Safety notes:
//! - This helper only produces diagnostics for unsatisfied cluster-version
//!   constraints; all other expand failures intentionally surface no available
//!   cluster list.

use std::collections::HashMap;
use std::path::PathBuf;

use ergo_runtime::cluster::{ExpandError, Version, VersionTargetKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AvailableClusterDiagnostic {
    pub(crate) id: String,
    pub(crate) version: String,
    pub(crate) location: String,
}

pub(crate) fn available_clusters_from_files(
    err: &ExpandError,
    cluster_sources: &HashMap<(String, Version), PathBuf>,
) -> Vec<AvailableClusterDiagnostic> {
    match err {
        ExpandError::UnsatisfiedVersionConstraint {
            target_kind: VersionTargetKind::Cluster,
            id,
            available_versions,
            ..
        } => available_versions
            .iter()
            .filter_map(|version| {
                cluster_sources
                    .get(&(id.clone(), version.clone()))
                    .map(|path| AvailableClusterDiagnostic {
                        id: id.clone(),
                        version: version.to_string(),
                        location: path.display().to_string(),
                    })
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn available_clusters_from_labels(
    err: &ExpandError,
    cluster_diagnostic_labels: &HashMap<(String, Version), String>,
) -> Vec<AvailableClusterDiagnostic> {
    match err {
        ExpandError::UnsatisfiedVersionConstraint {
            target_kind: VersionTargetKind::Cluster,
            id,
            available_versions,
            ..
        } => available_versions
            .iter()
            .map(|version| AvailableClusterDiagnostic {
                id: id.clone(),
                version: version.to_string(),
                location: cluster_diagnostic_labels
                    .get(&(id.clone(), version.clone()))
                    .cloned()
                    .unwrap_or_else(|| format!("{id}@{version}")),
            })
            .collect(),
        _ => Vec::new(),
    }
}
