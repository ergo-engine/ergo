pub(super) use ergo_adapter::{
    adapter_fingerprint, compile_event_binder, fixture, validate_action_adapter_composition,
    validate_capture_format, validate_source_adapter_composition, AdapterManifest, AdapterProvides,
    EventTime, GraphId, RuntimeHandle,
};
pub(super) use ergo_runtime::catalog::{
    build_core_catalog, core_registries, CorePrimitiveCatalog, CoreRegistries,
};
pub(super) use ergo_runtime::cluster::{
    expand, ClusterDefinition, ClusterLoader, ClusterVersionIndex, ExpandError, ExpandedGraph,
    PrimitiveCatalog, PrimitiveKind, Version, VersionTargetKind,
};
pub(super) use ergo_runtime::common::ErrorInfo;
pub(super) use ergo_runtime::provenance::{compute_runtime_provenance, RuntimeProvenanceScheme};
pub(super) use ergo_supervisor::replay::StrictReplayExpectations;
pub(super) use ergo_supervisor::{
    write_capture_bundle, CaptureBundle, CaptureJsonStyle, Constraints, Decision,
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
) -> Result<AdapterDependencySummary, String> {
    let mut summary = AdapterDependencySummary::default();

    for (runtime_id, node) in &expanded.nodes {
        let meta = catalog
            .get(&node.implementation.impl_id, &node.implementation.version)
            .ok_or_else(|| {
                format!(
                    "missing catalog metadata for primitive '{}@{}'",
                    node.implementation.impl_id, node.implementation.version
                )
            })?;

        match meta.kind {
            PrimitiveKind::Source => {
                let source = registries
                    .sources
                    .get(&node.implementation.impl_id)
                    .ok_or_else(|| {
                        format!(
                            "source '{}' missing in core registry",
                            node.implementation.impl_id
                        )
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
                    .ok_or_else(|| {
                        format!(
                            "action '{}' missing in core registry",
                            node.implementation.impl_id
                        )
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
) -> Result<(), String> {
    validate_capture_format(&provides.capture_format_version)
        .map_err(|err| format!("adapter composition failed: {}", summarize_error_info(&err)))?;

    for (runtime_id, node) in &expanded.nodes {
        let meta = catalog
            .get(&node.implementation.impl_id, &node.implementation.version)
            .ok_or_else(|| {
                format!(
                    "missing catalog metadata for primitive '{}@{}'",
                    node.implementation.impl_id, node.implementation.version
                )
            })?;
        match meta.kind {
            PrimitiveKind::Source => {
                let source = registries
                    .sources
                    .get(&node.implementation.impl_id)
                    .ok_or_else(|| {
                        format!(
                            "source '{}' missing in core registry",
                            node.implementation.impl_id
                        )
                    })?;
                validate_source_adapter_composition(
                    &source.manifest().requires,
                    provides,
                    &node.parameters,
                )
                .map_err(|err| {
                    format!(
                        "source composition failed for node '{}': {}",
                        runtime_id,
                        summarize_error_info(&err)
                    )
                })?;
            }
            PrimitiveKind::Action => {
                let action = registries
                    .actions
                    .get(&node.implementation.impl_id)
                    .ok_or_else(|| {
                        format!(
                            "action '{}' missing in core registry",
                            node.implementation.impl_id
                        )
                    })?;
                validate_action_adapter_composition(
                    &action.manifest().effects,
                    provides,
                    &node.parameters,
                )
                .map_err(|err| {
                    format!(
                        "action composition failed for node '{}': {}",
                        runtime_id,
                        summarize_error_info(&err)
                    )
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

fn summarize_filesystem_expand_error(
    err: &ExpandError,
    cluster_sources: &HashMap<(String, Version), PathBuf>,
) -> String {
    let base = summarize_error_info(err);
    match err {
        ExpandError::UnsatisfiedVersionConstraint {
            target_kind: VersionTargetKind::Cluster,
            id,
            available_versions,
            ..
        } => {
            let available = available_versions
                .iter()
                .filter_map(|version| {
                    cluster_sources
                        .get(&(id.clone(), version.clone()))
                        .map(|path| format!("- {}@{} at {}", id, version, path.display()))
                })
                .collect::<Vec<_>>();
            if available.is_empty() {
                base
            } else {
                format!(
                    "{}\navailable cluster files:\n{}",
                    base,
                    available.join("\n")
                )
            }
        }
        _ => base,
    }
}

fn summarize_expand_error(
    err: &ExpandError,
    diagnostic_labels: &HashMap<(String, Version), String>,
) -> String {
    let base = summarize_error_info(err);
    match err {
        ExpandError::UnsatisfiedVersionConstraint {
            target_kind: VersionTargetKind::Cluster,
            id,
            available_versions,
            ..
        } => {
            let available = available_versions
                .iter()
                .map(|version| {
                    let label = diagnostic_labels
                        .get(&(id.clone(), version.clone()))
                        .cloned()
                        .unwrap_or_else(|| format!("{id}@{version}"));
                    format!("- {}@{} at {}", id, version, label)
                })
                .collect::<Vec<_>>();
            if available.is_empty() {
                base
            } else {
                format!(
                    "{}\navailable cluster sources:\n{}",
                    base,
                    available.join("\n")
                )
            }
        }
        _ => base,
    }
}

#[derive(Debug)]
pub enum HostRunError {
    AdapterRequired(AdapterDependencySummary),
    InvalidInput(String),
    DriverStart(String),
    DriverProtocol(String),
    DriverIo(String),
    StepFailed(String),
    Io(String),
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
            Self::InvalidInput(message)
            | Self::DriverStart(message)
            | Self::DriverProtocol(message)
            | Self::DriverIo(message)
            | Self::StepFailed(message)
            | Self::Io(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for HostRunError {}

#[derive(Debug)]
pub enum HostReplayError {
    Hosted(HostedReplayError),
    GraphIdMismatch { expected: String, got: String },
    ExternalKindsNotRepresentable { missing: Vec<String> },
    Setup(String),
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
            Self::Setup(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for HostReplayError {}

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
