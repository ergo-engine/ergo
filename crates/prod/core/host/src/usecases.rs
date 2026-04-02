//! usecases
//!
//! Purpose:
//! - Serve as the parent module, public type facade, and shared import authority for the host
//!   usecase subsystem.
//! - Define the public request/response/error/status types consumed through canonical host run,
//!   replay, validation, and manual-runner entrypoints.
//!
//! Owns:
//! - `HostRunError`, `HostReplayError`, `InterruptionReason`, `DriverConfig`, `RunControl`, and
//!   the public host request/response DTOs.
//! - Adapter dependency scanning and adapter composition checks over expanded graphs.
//! - The `pub(super)` prelude consumed by `live_prep`, `live_run`, and `process_driver`.
//!
//! Does not own:
//! - Live-run, replay, or process-driver mechanics, which live in the child modules.
//! - Hosted-runner execution semantics, which live in `runner.rs`.
//! - Manifest validation/composition surfaces owned by `manifest_usecases.rs`.
//!
//! Connects to:
//! - `live_prep.rs`, `live_run.rs`, and `process_driver.rs` as this subsystem's module root.
//! - `lib.rs`, CLI, and SDK through the re-exported public usecase and error surfaces.
//!
//! Safety notes:
//! - This file is a public compatibility seam: CLI and SDK pattern-match on `HostRunError`,
//!   `HostReplayError`, `InterruptionReason`, and request/response field names directly.
//! - `HostRunError` and `HostReplayError` now preserve typed setup/driver/step
//!   sources; only host-authored operational detail remains string-shaped.
//! - `interruption_from_egress_dispatch_failure(...)` intentionally drops protocol/I/O detail and
//!   maps into status-oriented interruption reasons.
//! - The broad `pub(super)` prelude and helper duplication in this facade, plus the shared
//!   `usecases/tests` infrastructure cleanup, are tracked in issue #74.

pub(super) use ergo_adapter::{
    adapter_fingerprint, compile_event_binder,
    fixture::{self, FixtureParseError},
    validate_action_adapter_composition, validate_capture_format,
    validate_source_adapter_composition, AdapterManifest, AdapterProvides, CompositionError,
    DemoSourceContextError, EventBindingError, EventTime, GraphId, InvalidAdapter, RuntimeHandle,
};
pub(super) use ergo_loader::LoaderError;
pub(super) use ergo_runtime::catalog::{
    build_core_catalog, core_registries, CorePrimitiveCatalog, CoreRegistrationError,
    CoreRegistries,
};
pub(super) use ergo_runtime::cluster::{
    expand, ClusterDefinition, ClusterLoader, ClusterVersionIndex, ExpandError, ExpandedGraph,
    PrimitiveCatalog, PrimitiveKind, Version, VersionTargetKind,
};
pub(super) use ergo_runtime::common::ErrorInfo;
pub(super) use ergo_runtime::provenance::{
    compute_runtime_provenance, RuntimeProvenanceError, RuntimeProvenanceScheme,
};
pub(super) use ergo_supervisor::replay::StrictReplayExpectations;
#[cfg(test)]
pub(super) use ergo_supervisor::Decision;
pub(super) use ergo_supervisor::{
    write_capture_bundle, CaptureBundle, CaptureJsonStyle, CaptureWriteError, Constraints,
    NO_ADAPTER_PROVENANCE,
};
pub(super) use serde::{Deserialize, Serialize};
pub(super) use std::collections::{BTreeSet, HashMap, HashSet};
pub(super) use std::fs;
pub(super) use std::io::{BufRead, BufReader, Read};
pub(super) use std::path::{Path, PathBuf};
pub(super) use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
pub(super) use std::sync::atomic::{AtomicBool, Ordering};
pub(super) use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
pub(super) use std::sync::Arc;
pub(super) use std::thread::{self, JoinHandle};
pub(super) use std::time::{Duration, Instant};

pub(super) use crate::egress::compute_egress_provenance;
pub(super) use crate::{
    decision_counts, replay_bundle_strict, runner::validate_hosted_runner_configuration,
    EgressConfig, EgressDispatchFailure, HostedAdapterConfig, HostedEvent, HostedReplayError,
    HostedRunner, HostedStepError,
};

#[derive(Debug, Clone, Default)]
pub struct AdapterDependencySummary {
    pub requires_adapter: bool,
    pub required_context_nodes: Vec<String>,
    pub write_nodes: Vec<String>,
}

pub fn scan_adapter_dependencies(
    expanded: &ExpandedGraph,
    catalog: &CorePrimitiveCatalog,
    registries: &CoreRegistries,
) -> Result<AdapterDependencySummary, HostDependencyScanError> {
    let mut summary = AdapterDependencySummary::default();

    for (runtime_id, node) in &expanded.nodes {
        let meta = catalog
            .get(&node.implementation.impl_id, &node.implementation.version)
            .ok_or_else(|| HostDependencyScanError::MissingCatalogMetadata {
                primitive_id: node.implementation.impl_id.clone(),
                version: node.implementation.version.clone(),
            })?;

        match meta.kind {
            PrimitiveKind::Source => {
                let source = registries
                    .sources
                    .get(&node.implementation.impl_id)
                    .ok_or_else(|| HostDependencyScanError::MissingSourcePrimitive {
                        primitive_id: node.implementation.impl_id.clone(),
                    })?;
                if source
                    .manifest()
                    .requires
                    .context
                    .iter()
                    .any(|req| req.required)
                {
                    summary.required_context_nodes.push(runtime_id.clone());
                }
            }
            PrimitiveKind::Action => {
                let action = registries
                    .actions
                    .get(&node.implementation.impl_id)
                    .ok_or_else(|| HostDependencyScanError::MissingActionPrimitive {
                        primitive_id: node.implementation.impl_id.clone(),
                    })?;
                if !action.manifest().effects.writes.is_empty()
                    || !action.manifest().effects.intents.is_empty()
                {
                    summary.write_nodes.push(runtime_id.clone());
                }
            }
            _ => {}
        }
    }

    summary.requires_adapter =
        !summary.required_context_nodes.is_empty() || !summary.write_nodes.is_empty();
    Ok(summary)
}

pub fn validate_adapter_composition(
    expanded: &ExpandedGraph,
    catalog: &CorePrimitiveCatalog,
    registries: &CoreRegistries,
    provides: &AdapterProvides,
) -> Result<(), HostAdapterCompositionError> {
    validate_capture_format(&provides.capture_format_version)
        .map_err(HostAdapterCompositionError::CaptureFormat)?;

    for (runtime_id, node) in &expanded.nodes {
        let meta = catalog
            .get(&node.implementation.impl_id, &node.implementation.version)
            .ok_or_else(|| HostAdapterCompositionError::MissingCatalogMetadata {
                runtime_id: runtime_id.clone(),
                primitive_id: node.implementation.impl_id.clone(),
                version: node.implementation.version.clone(),
            })?;
        match meta.kind {
            PrimitiveKind::Source => {
                let source = registries
                    .sources
                    .get(&node.implementation.impl_id)
                    .ok_or_else(|| HostAdapterCompositionError::MissingSourcePrimitive {
                        runtime_id: runtime_id.clone(),
                        primitive_id: node.implementation.impl_id.clone(),
                    })?;
                validate_source_adapter_composition(
                    &source.manifest().requires,
                    provides,
                    &node.parameters,
                )
                .map_err(|source| HostAdapterCompositionError::Source {
                    runtime_id: runtime_id.clone(),
                    source,
                })?;
            }
            PrimitiveKind::Action => {
                let action = registries
                    .actions
                    .get(&node.implementation.impl_id)
                    .ok_or_else(|| HostAdapterCompositionError::MissingActionPrimitive {
                        runtime_id: runtime_id.clone(),
                        primitive_id: node.implementation.impl_id.clone(),
                    })?;
                validate_action_adapter_composition(
                    &action.manifest().effects,
                    provides,
                    &node.parameters,
                )
                .map_err(|source| HostAdapterCompositionError::Action {
                    runtime_id: runtime_id.clone(),
                    source,
                })?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn summarize_error_info(err: &impl ErrorInfo) -> String {
    format!("{} ({})", err.summary(), err.rule_id())
}

fn summarize_expand_error(
    err: &ExpandError,
    diagnostic_labels: &HashMap<(String, Version), String>,
) -> HostExpandError {
    let available_clusters = match err {
        ExpandError::UnsatisfiedVersionConstraint {
            target_kind: VersionTargetKind::Cluster,
            id,
            available_versions,
            ..
        } => available_versions
            .iter()
            .map(|version| HostAvailableCluster {
                id: id.clone(),
                version: version.to_string(),
                location: diagnostic_labels
                    .get(&(id.clone(), version.clone()))
                    .cloned()
                    .unwrap_or_else(|| format!("{id}@{version}")),
            })
            .collect(),
        _ => Vec::new(),
    };

    HostExpandError {
        source: err.clone(),
        context: HostExpandContext::Assets,
        available_clusters,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostAvailableCluster {
    pub id: String,
    pub version: String,
    pub location: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostExpandContext {
    Filesystem,
    Assets,
}

impl HostExpandContext {
    fn available_heading(self) -> &'static str {
        match self {
            Self::Filesystem => "available cluster files",
            Self::Assets => "available cluster sources",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HostExpandError {
    pub source: ExpandError,
    pub context: HostExpandContext,
    pub available_clusters: Vec<HostAvailableCluster>,
}

impl std::fmt::Display for HostExpandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.source.summary(), self.source.rule_id())?;
        if !self.available_clusters.is_empty() {
            let available = self
                .available_clusters
                .iter()
                .map(|cluster| {
                    format!(
                        "- {}@{} at {}",
                        cluster.id, cluster.version, cluster.location
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            write!(f, "\n{}:\n{}", self.context.available_heading(), available)?;
        }
        Ok(())
    }
}

impl std::error::Error for HostExpandError {}

#[derive(Debug)]
pub enum HostGraphPreparationError {
    CoreRegistries(CoreRegistrationError),
    Expansion(HostExpandError),
    RuntimeProvenance(RuntimeProvenanceError),
}

impl std::fmt::Display for HostGraphPreparationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CoreRegistries(err) => write!(f, "core registries: {err}"),
            Self::Expansion(err) => write!(f, "graph expansion failed: {err}"),
            Self::RuntimeProvenance(err) => {
                write!(f, "runtime provenance compute failed: {err}")
            }
        }
    }
}

impl std::error::Error for HostGraphPreparationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CoreRegistries(err) => Some(err),
            Self::RuntimeProvenance(err) => Some(err),
            Self::Expansion(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum HostDependencyScanError {
    MissingCatalogMetadata {
        primitive_id: String,
        version: Version,
    },
    MissingSourcePrimitive {
        primitive_id: String,
    },
    MissingActionPrimitive {
        primitive_id: String,
    },
}

impl std::fmt::Display for HostDependencyScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingCatalogMetadata {
                primitive_id,
                version,
            } => write!(
                f,
                "missing catalog metadata for primitive '{}@{}'",
                primitive_id, version
            ),
            Self::MissingSourcePrimitive { primitive_id } => {
                write!(f, "source '{}' missing in core registry", primitive_id)
            }
            Self::MissingActionPrimitive { primitive_id } => {
                write!(f, "action '{}' missing in core registry", primitive_id)
            }
        }
    }
}

impl std::error::Error for HostDependencyScanError {}

#[derive(Debug)]
pub enum HostAdapterCompositionError {
    CaptureFormat(CompositionError),
    Source {
        runtime_id: String,
        source: CompositionError,
    },
    Action {
        runtime_id: String,
        source: CompositionError,
    },
    MissingCatalogMetadata {
        runtime_id: String,
        primitive_id: String,
        version: Version,
    },
    MissingSourcePrimitive {
        runtime_id: String,
        primitive_id: String,
    },
    MissingActionPrimitive {
        runtime_id: String,
        primitive_id: String,
    },
}

impl std::fmt::Display for HostAdapterCompositionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CaptureFormat(source) => {
                write!(
                    f,
                    "adapter composition failed: {}",
                    summarize_error_info(source)
                )
            }
            Self::Source { runtime_id, source } => write!(
                f,
                "source composition failed for node '{}': {}",
                runtime_id,
                summarize_error_info(source)
            ),
            Self::Action { runtime_id, source } => write!(
                f,
                "action composition failed for node '{}': {}",
                runtime_id,
                summarize_error_info(source)
            ),
            Self::MissingCatalogMetadata {
                primitive_id,
                version,
                ..
            } => write!(
                f,
                "missing catalog metadata for primitive '{}@{}'",
                primitive_id, version
            ),
            Self::MissingSourcePrimitive { primitive_id, .. } => {
                write!(f, "source '{}' missing in core registry", primitive_id)
            }
            Self::MissingActionPrimitive { primitive_id, .. } => {
                write!(f, "action '{}' missing in core registry", primitive_id)
            }
        }
    }
}

impl std::error::Error for HostAdapterCompositionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CaptureFormat(source)
            | Self::Source { source, .. }
            | Self::Action { source, .. } => Some(source),
            Self::MissingCatalogMetadata { .. }
            | Self::MissingSourcePrimitive { .. }
            | Self::MissingActionPrimitive { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum HostAdapterSetupError {
    ManifestRead {
        path: PathBuf,
        source: std::io::Error,
    },
    ManifestSourceLabelEmpty,
    ManifestParse {
        source_label: String,
        source: serde_yaml::Error,
    },
    ManifestDecode {
        source_label: String,
        source: serde_json::Error,
    },
    Validation(InvalidAdapter),
    Composition(HostAdapterCompositionError),
    BinderCompile(EventBindingError),
    DemoSourceContext(DemoSourceContextError),
}

impl std::fmt::Display for HostAdapterSetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ManifestRead { path, source } => {
                write!(f, "read adapter manifest '{}': {source}", path.display())
            }
            Self::ManifestSourceLabelEmpty => {
                write!(f, "adapter manifest source_label must not be empty")
            }
            Self::ManifestParse {
                source_label,
                source,
            } => write!(f, "parse adapter manifest '{source_label}': {source}"),
            Self::ManifestDecode {
                source_label,
                source,
            } => write!(f, "decode adapter manifest '{source_label}': {source}"),
            Self::Validation(source) => write!(
                f,
                "adapter manifest validation failed: {}",
                summarize_error_info(source)
            ),
            Self::Composition(source) => write!(f, "{source}"),
            Self::BinderCompile(source) => {
                write!(f, "adapter event binder compilation failed: {source}")
            }
            Self::DemoSourceContext(source) => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for HostAdapterSetupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ManifestRead { source, .. } => Some(source),
            Self::ManifestParse { source, .. } => Some(source),
            Self::ManifestDecode { source, .. } => Some(source),
            Self::Validation(source) => Some(source),
            Self::Composition(source) => Some(source),
            Self::BinderCompile(source) => Some(source),
            Self::DemoSourceContext(source) => Some(source),
            Self::ManifestSourceLabelEmpty => None,
        }
    }
}

#[derive(Debug)]
pub enum HostSetupError {
    LoadGraphAssets(LoaderError),
    DependencyScan(HostDependencyScanError),
    GraphPreparation(HostGraphPreparationError),
    AdapterSetup(HostAdapterSetupError),
    HostedRunnerValidation(HostedStepError),
    HostedRunnerInitialization(HostedStepError),
    StartEgress(HostedStepError),
}

impl std::fmt::Display for HostSetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadGraphAssets(err) => write!(f, "{err}"),
            Self::DependencyScan(err) => write!(f, "{err}"),
            Self::GraphPreparation(err) => write!(f, "{err}"),
            Self::AdapterSetup(err) => write!(f, "{err}"),
            Self::HostedRunnerValidation(err) => {
                write!(f, "host configuration validation failed: {err}")
            }
            Self::HostedRunnerInitialization(err) => {
                write!(f, "failed to initialize canonical host runner: {err}")
            }
            Self::StartEgress(err) => write!(f, "start egress channels: {err}"),
        }
    }
}

impl std::error::Error for HostSetupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LoadGraphAssets(err) => Some(err),
            Self::DependencyScan(err) => Some(err),
            Self::GraphPreparation(err) => Some(err),
            Self::AdapterSetup(err) => Some(err),
            Self::HostedRunnerValidation(err) | Self::HostedRunnerInitialization(err) => Some(err),
            Self::StartEgress(err) => Some(err),
        }
    }
}

#[derive(Debug)]
pub enum HostReplaySetupError {
    CaptureRead {
        path: PathBuf,
        source: std::io::Error,
    },
    CaptureParse {
        path: PathBuf,
        source: serde_json::Error,
    },
    LiveEgressConfigurationNotAllowed,
    Setup(HostSetupError),
}

impl std::fmt::Display for HostReplaySetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CaptureRead { path, source } => write!(
                f,
                "failed to read capture artifact '{}': {source}",
                path.display()
            ),
            Self::CaptureParse { path, source } => write!(
                f,
                "failed to parse capture artifact '{}': {source}",
                path.display()
            ),
            Self::LiveEgressConfigurationNotAllowed => {
                write!(f, "replay does not accept live egress configuration")
            }
            Self::Setup(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for HostReplaySetupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CaptureRead { source, .. } => Some(source),
            Self::CaptureParse { source, .. } => Some(source),
            Self::Setup(err) => Some(err),
            Self::LiveEgressConfigurationNotAllowed => None,
        }
    }
}

#[derive(Debug)]
pub enum HostDriverInputError {
    FixtureParse(FixtureParseError),
    DuplicateEventId {
        event_id: String,
    },
    MissingSemanticKind {
        event_id: String,
    },
    UnexpectedSemanticKind {
        event_id: String,
    },
    NoEpisodes {
        source_label: String,
    },
    NoEvents {
        source_label: String,
    },
    EpisodeWithoutEvents {
        label: String,
    },
    ProcessCommandEmpty,
    ProcessExecutableBlank,
    ProcessPathMetadata {
        path: PathBuf,
        source: std::io::Error,
    },
    ProcessPathNotFile {
        path: PathBuf,
    },
    ProcessPathNotExecutable {
        path: PathBuf,
    },
}

impl std::fmt::Display for HostDriverInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FixtureParse(source) => write!(f, "failed to parse fixture: {source}"),
            Self::DuplicateEventId { event_id } => write!(
                f,
                "fixture event id '{}' appears more than once in canonical run input",
                event_id
            ),
            Self::MissingSemanticKind { event_id } => write!(
                f,
                "fixture event '{}' is missing semantic_kind in adapter-bound canonical run",
                event_id
            ),
            Self::UnexpectedSemanticKind { event_id } => write!(
                f,
                "fixture event '{}' set semantic_kind but canonical run is not adapter-bound",
                event_id
            ),
            Self::NoEpisodes { source_label } => {
                write!(f, "fixture input '{source_label}' contained no episodes")
            }
            Self::NoEvents { source_label } => {
                write!(f, "fixture input '{source_label}' contained no events")
            }
            Self::EpisodeWithoutEvents { label } => {
                write!(f, "episode '{}' has no events", label)
            }
            Self::ProcessCommandEmpty => {
                write!(f, "process driver requires at least one argv element")
            }
            Self::ProcessExecutableBlank => {
                write!(f, "process driver executable must not be empty")
            }
            Self::ProcessPathMetadata { path, source } => {
                write!(f, "inspect process driver '{}': {source}", path.display())
            }
            Self::ProcessPathNotFile { path } => {
                write!(f, "process driver '{}' is not a file", path.display())
            }
            Self::ProcessPathNotExecutable { path } => {
                write!(f, "process driver '{}' is not executable", path.display())
            }
        }
    }
}

impl std::error::Error for HostDriverInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::FixtureParse(source) => Some(source),
            Self::DuplicateEventId { .. }
            | Self::MissingSemanticKind { .. }
            | Self::UnexpectedSemanticKind { .. }
            | Self::NoEpisodes { .. }
            | Self::NoEvents { .. }
            | Self::EpisodeWithoutEvents { .. }
            | Self::ProcessCommandEmpty
            | Self::ProcessExecutableBlank
            | Self::ProcessPathNotFile { .. }
            | Self::ProcessPathNotExecutable { .. } => None,
            Self::ProcessPathMetadata { source, .. } => Some(source),
        }
    }
}

#[derive(Debug)]
pub enum HostDriverOutputError {
    StopBeforeFirstCommittedEvent,
    ProducedNoEpisodes,
    ProducedNoEvents,
    EpisodeWithoutEvents { label: String },
    UnexpectedInterruptedOutcome,
    MissingCapturePath,
}

impl std::fmt::Display for HostDriverOutputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StopBeforeFirstCommittedEvent => {
                write!(f, "host stop requested before first committed event")
            }
            Self::ProducedNoEpisodes => write!(f, "driver produced no episodes"),
            Self::ProducedNoEvents => write!(f, "driver produced no events"),
            Self::EpisodeWithoutEvents { label } => {
                write!(f, "episode '{}' has no events", label)
            }
            Self::UnexpectedInterruptedOutcome => {
                write!(
                    f,
                    "fixture driver returned interrupted outcome unexpectedly"
                )
            }
            Self::MissingCapturePath => {
                write!(
                    f,
                    "fixture run did not produce a capture file path unexpectedly"
                )
            }
        }
    }
}

impl std::error::Error for HostDriverOutputError {}

#[derive(Debug)]
pub struct HostDriverStartError {
    detail: String,
    source: Option<std::io::Error>,
}

impl HostDriverStartError {
    fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            source: None,
        }
    }

    fn with_source(detail: impl Into<String>, source: std::io::Error) -> Self {
        Self {
            detail: detail.into(),
            source: Some(source),
        }
    }
}

impl std::fmt::Display for HostDriverStartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.detail)
    }
}

impl std::error::Error for HostDriverStartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|source| source as _)
    }
}

#[derive(Debug)]
pub struct HostDriverProtocolError {
    detail: String,
    source: Option<serde_json::Error>,
}

impl HostDriverProtocolError {
    fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            source: None,
        }
    }

    fn with_json_source(detail: impl Into<String>, source: serde_json::Error) -> Self {
        Self {
            detail: detail.into(),
            source: Some(source),
        }
    }
}

impl std::fmt::Display for HostDriverProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.detail)
    }
}

impl std::error::Error for HostDriverProtocolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|source| source as _)
    }
}

#[derive(Debug)]
pub struct HostDriverIoError {
    detail: String,
    source: Option<std::io::Error>,
}

impl HostDriverIoError {
    fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            source: None,
        }
    }

    fn with_source(detail: impl Into<String>, source: std::io::Error) -> Self {
        Self {
            detail: detail.into(),
            source: Some(source),
        }
    }
}

impl std::fmt::Display for HostDriverIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.detail)
    }
}

impl std::error::Error for HostDriverIoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|source| source as _)
    }
}

#[derive(Debug)]
pub enum HostDriverError {
    Input(HostDriverInputError),
    Start(HostDriverStartError),
    Protocol(HostDriverProtocolError),
    Io(HostDriverIoError),
    Output(HostDriverOutputError),
}

impl std::fmt::Display for HostDriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input(err) => write!(f, "{err}"),
            Self::Start(err) => write!(f, "{err}"),
            Self::Protocol(err) => write!(f, "{err}"),
            Self::Io(err) => write!(f, "{err}"),
            Self::Output(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for HostDriverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Input(err) => Some(err),
            Self::Start(err) => Some(err),
            Self::Protocol(err) => Some(err),
            Self::Io(err) => Some(err),
            Self::Output(err) => Some(err),
        }
    }
}

#[derive(Debug)]
pub enum HostRunError {
    AdapterRequired(AdapterDependencySummary),
    Setup(HostSetupError),
    Driver(HostDriverError),
    Step(HostedStepError),
    CaptureWrite(CaptureWriteError),
}

impl std::fmt::Display for HostRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AdapterRequired(summary) => write!(
                f,
                "graph requires adapter capabilities but no adapter was provided (required context nodes: [{}], write nodes: [{}])",
                summary.required_context_nodes.join(", "),
                summary.write_nodes.join(", ")
            ),
            Self::Setup(err) => write!(f, "{err}"),
            Self::Driver(err) => write!(f, "{err}"),
            Self::Step(err) => write!(f, "{err}"),
            Self::CaptureWrite(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for HostRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AdapterRequired(_) => None,
            Self::Setup(err) => Some(err),
            Self::Driver(err) => Some(err),
            Self::Step(err) => Some(err),
            Self::CaptureWrite(err) => Some(err),
        }
    }
}

#[derive(Debug)]
pub enum HostReplayError {
    Hosted(HostedReplayError),
    GraphIdMismatch { expected: String, got: String },
    ExternalKindsNotRepresentable { missing: Vec<String> },
    Setup(HostReplaySetupError),
}

impl std::fmt::Display for HostReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hosted(err) => write!(f, "{err}"),
            Self::GraphIdMismatch { expected, got } => write!(
                f,
                "graph_id mismatch (expected '{}', got '{}')",
                expected, got
            ),
            Self::ExternalKindsNotRepresentable { missing } => write!(
                f,
                "capture includes external effect kinds not representable by replay graph ownership surface: [{}]",
                missing.join(", ")
            ),
            Self::Setup(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for HostReplayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hosted(err) => Some(err),
            Self::Setup(err) => Some(err),
            Self::GraphIdMismatch { .. } | Self::ExternalKindsNotRepresentable { .. } => None,
        }
    }
}

impl From<HostedReplayError> for HostReplayError {
    fn from(value: HostedReplayError) -> Self {
        Self::Hosted(value)
    }
}

#[derive(Debug, Clone)]
pub enum DriverConfig {
    Fixture {
        path: PathBuf,
    },
    FixtureItems {
        items: Vec<ergo_adapter::fixture::FixtureItem>,
        source_label: String,
    },
    Process {
        command: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterruptionReason {
    HostStopRequested,
    DriverTerminated,
    ProtocolViolation,
    DriverIo,
    EgressAckTimeout { channel: String, intent_id: String },
    EgressProtocolViolation { channel: String },
    EgressIo { channel: String },
}

impl InterruptionReason {
    pub fn code(&self) -> &'static str {
        match self {
            Self::HostStopRequested => "host_stop_requested",
            Self::DriverTerminated => "driver_terminated",
            Self::ProtocolViolation => "protocol_violation",
            Self::DriverIo => "driver_io",
            Self::EgressAckTimeout { .. } => "egress_ack_timeout",
            Self::EgressProtocolViolation { .. } => "egress_protocol_violation",
            Self::EgressIo { .. } => "egress_io",
        }
    }
}

impl std::fmt::Display for InterruptionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code())
    }
}

fn interruption_from_egress_dispatch_failure(failure: EgressDispatchFailure) -> InterruptionReason {
    match failure {
        EgressDispatchFailure::AckTimeout { channel, intent_id } => {
            InterruptionReason::EgressAckTimeout { channel, intent_id }
        }
        EgressDispatchFailure::ProtocolViolation { channel, .. } => {
            InterruptionReason::EgressProtocolViolation { channel }
        }
        EgressDispatchFailure::Io { channel, .. } => InterruptionReason::EgressIo { channel },
    }
}

#[derive(Debug, Clone)]
pub struct HostStopHandle {
    flag: Arc<AtomicBool>,
}

impl HostStopHandle {
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn request_stop(&self) {
        self.flag.store(true, Ordering::Release);
    }

    fn is_requested(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
}

impl Default for HostStopHandle {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default)]
pub struct RunControl {
    stop: HostStopHandle,
    max_duration: Option<Duration>,
    max_events: Option<u64>,
}

impl RunControl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_stop_handle(mut self, stop: HostStopHandle) -> Self {
        self.stop = stop;
        self
    }

    pub fn max_duration(mut self, max_duration: Duration) -> Self {
        self.max_duration = Some(max_duration);
        self
    }

    pub fn max_events(mut self, max_events: u64) -> Self {
        self.max_events = Some(max_events);
        self
    }
}

pub struct RunGraphRequest {
    pub graph_path: PathBuf,
    pub driver: DriverConfig,
    pub capture_output: Option<PathBuf>,
    pub pretty_capture: bool,
    pub adapter_bound: bool,
    pub dependency_summary: AdapterDependencySummary,
    pub runner: HostedRunner,
}

pub struct RunGraphFromPathsRequest {
    pub graph_path: PathBuf,
    pub cluster_paths: Vec<PathBuf>,
    pub driver: DriverConfig,
    pub adapter_path: Option<PathBuf>,
    pub egress_config: Option<EgressConfig>,
    pub capture_output: Option<PathBuf>,
    pub pretty_capture: bool,
}

#[derive(Debug, Clone)]
pub struct RunSummary {
    pub capture_bundle: CaptureBundle,
    pub capture_path: Option<PathBuf>,
    pub episodes: usize,
    pub events: usize,
    pub invoked: usize,
    pub deferred: usize,
    pub episode_event_counts: Vec<(String, usize)>,
}

#[derive(Debug, Clone)]
pub struct InterruptedRun {
    pub summary: RunSummary,
    pub reason: InterruptionReason,
}

#[derive(Debug, Clone)]
pub enum RunOutcome {
    Completed(RunSummary),
    Interrupted(InterruptedRun),
}

pub type RunGraphResponse = Result<RunOutcome, HostRunError>;

#[derive(Debug, Clone)]
pub enum AdapterInput {
    Path(PathBuf),
    Text {
        content: String,
        source_label: String,
    },
    Manifest(AdapterManifest),
}

pub struct ReplayGraphRequest {
    pub bundle: CaptureBundle,
    pub runner: HostedRunner,
    pub expected_adapter_provenance: String,
    pub expected_runtime_provenance: String,
}

pub struct ReplayGraphFromAssetsRequest {
    pub bundle: CaptureBundle,
    pub assets: ergo_loader::PreparedGraphAssets,
    pub prep: LivePrepOptions,
}

pub struct ReplayGraphFromPathsRequest {
    pub capture_path: PathBuf,
    pub graph_path: PathBuf,
    pub cluster_paths: Vec<PathBuf>,
    pub adapter_path: Option<PathBuf>,
}

pub struct PrepareHostedRunnerFromPathsRequest {
    pub graph_path: PathBuf,
    pub cluster_paths: Vec<PathBuf>,
    pub adapter_path: Option<PathBuf>,
    pub egress_config: Option<EgressConfig>,
}

#[derive(Debug, Clone, Default)]
pub struct LivePrepOptions {
    pub adapter: Option<AdapterInput>,
    pub egress_config: Option<EgressConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapturePolicy {
    InMemory,
    File { path: PathBuf, pretty: bool },
}

pub struct RunGraphFromAssetsRequest {
    pub assets: ergo_loader::PreparedGraphAssets,
    pub prep: LivePrepOptions,
    pub driver: DriverConfig,
    pub capture: CapturePolicy,
}

pub(super) struct PreparedLiveRunnerSetup {
    adapter_bound: bool,
    dependency_summary: AdapterDependencySummary,
    runner: HostedRunner,
}

struct ValidatedLiveRunnerSetup {
    graph_id: GraphId,
    runtime_provenance: String,
    runtime: RuntimeHandle,
    adapter_config: Option<HostedAdapterConfig>,
    egress_config: Option<EgressConfig>,
    egress_provenance: Option<String>,
    adapter_bound: bool,
    dependency_summary: AdapterDependencySummary,
}

#[derive(Debug)]
pub struct ReplayGraphResult {
    pub graph_id: GraphId,
    pub events: usize,
    pub invoked: usize,
    pub deferred: usize,
    pub skipped: usize,
}

pub struct RunFixtureRequest {
    pub fixture_path: PathBuf,
    pub capture_output: PathBuf,
    pub pretty_capture: bool,
    pub runner: HostedRunner,
}

#[derive(Debug)]
pub struct RunFixtureResult {
    pub capture_path: PathBuf,
    pub episodes: usize,
    pub events: usize,
    pub episode_event_counts: Vec<(String, usize)>,
}

#[derive(Clone)]
pub struct RuntimeSurfaces {
    registries: Arc<CoreRegistries>,
    catalog: Arc<CorePrimitiveCatalog>,
}

impl RuntimeSurfaces {
    pub fn new(registries: CoreRegistries, catalog: CorePrimitiveCatalog) -> Self {
        Self {
            registries: Arc::new(registries),
            catalog: Arc::new(catalog),
        }
    }

    pub(crate) fn into_shared_parts(self) -> (Arc<CorePrimitiveCatalog>, Arc<CoreRegistries>) {
        (self.catalog, self.registries)
    }
}

mod live_prep;
mod live_run;
mod process_driver;

pub use self::live_prep::{
    finalize_hosted_runner_capture, load_graph_assets_from_memory, load_graph_assets_from_paths,
    prepare_hosted_runner, prepare_hosted_runner_from_paths,
    prepare_hosted_runner_from_paths_with_surfaces, prepare_hosted_runner_with_surfaces,
    replay_graph_from_assets, replay_graph_from_assets_with_surfaces, replay_graph_from_paths,
    replay_graph_from_paths_with_surfaces, validate_graph, validate_graph_from_paths,
    validate_graph_from_paths_with_surfaces, validate_graph_with_surfaces,
    validate_run_graph_from_assets, validate_run_graph_from_assets_with_surfaces,
    validate_run_graph_from_paths, validate_run_graph_from_paths_with_surfaces,
};
pub use self::live_run::{
    replay_graph, run_fixture, run_graph, run_graph_from_assets,
    run_graph_from_assets_with_control, run_graph_from_assets_with_surfaces,
    run_graph_from_assets_with_surfaces_and_control, run_graph_from_paths,
    run_graph_from_paths_with_control, run_graph_from_paths_with_surfaces,
    run_graph_from_paths_with_surfaces_and_control, run_graph_with_control,
};

use self::live_prep::{
    ensure_adapter_requirement_satisfied, finalize_hosted_runner_capture_with_stage,
    prepare_live_runner_setup_from_assets, start_live_runner_egress, HostedRunnerFinalizeFailure,
};
use self::live_run::{
    host_stop_driver_execution, validate_driver_input, DriverExecution, DriverTerminal,
    RunLifecycleState,
};
use self::process_driver::{
    run_process_driver, validate_process_driver_command, ProcessDriverPolicy,
    DEFAULT_PROCESS_DRIVER_POLICY,
};

#[cfg(test)]
mod tests;
