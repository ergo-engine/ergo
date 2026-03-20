use ergo_adapter::{
    adapter_fingerprint, compile_event_binder, fixture, validate_action_adapter_composition,
    validate_capture_format, validate_source_adapter_composition, AdapterManifest, AdapterProvides,
    EventTime, GraphId, RuntimeHandle,
};
use ergo_runtime::catalog::{
    build_core_catalog, core_registries, CorePrimitiveCatalog, CoreRegistries,
};
use ergo_runtime::cluster::{
    expand, ClusterDefinition, ClusterLoader, ClusterVersionIndex, ExpandError, ExpandedGraph,
    PrimitiveCatalog, PrimitiveKind, Version, VersionTargetKind,
};
use ergo_runtime::common::ErrorInfo;
use ergo_runtime::provenance::{compute_runtime_provenance, RuntimeProvenanceScheme};
use ergo_supervisor::replay::StrictReplayExpectations;
use ergo_supervisor::{
    write_capture_bundle, CaptureBundle, CaptureJsonStyle, Constraints, Decision,
    NO_ADAPTER_PROVENANCE,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::egress::compute_egress_provenance;
use crate::{
    decision_counts, replay_bundle_strict, EgressConfig, EgressDispatchFailure,
    HostedAdapterConfig, HostedEvent, HostedReplayError, HostedRunner,
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

fn summarize_expand_error(
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverConfig {
    Fixture { path: PathBuf },
    Process { command: Vec<String> },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSummary {
    pub capture_path: PathBuf,
    pub episodes: usize,
    pub events: usize,
    pub invoked: usize,
    pub deferred: usize,
    pub episode_event_counts: Vec<(String, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterruptedRun {
    pub summary: RunSummary,
    pub reason: InterruptionReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunOutcome {
    Completed(RunSummary),
    Interrupted(InterruptedRun),
}

pub type RunGraphResponse = Result<RunOutcome, HostRunError>;

pub struct ReplayGraphRequest {
    pub bundle: CaptureBundle,
    pub runner: HostedRunner,
    pub expected_adapter_provenance: String,
    pub expected_runtime_provenance: String,
}

pub struct ReplayGraphFromPathsRequest {
    pub capture_path: PathBuf,
    pub graph_path: PathBuf,
    pub cluster_paths: Vec<PathBuf>,
    pub adapter_path: Option<PathBuf>,
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

pub struct RuntimeSurfaces {
    registries: CoreRegistries,
    catalog: CorePrimitiveCatalog,
}

impl RuntimeSurfaces {
    pub fn new(registries: CoreRegistries, catalog: CorePrimitiveCatalog) -> Self {
        Self {
            registries,
            catalog,
        }
    }

    pub(crate) fn into_parts(self) -> (CoreRegistries, CorePrimitiveCatalog) {
        (self.registries, self.catalog)
    }
}

#[derive(Clone)]
struct PreloadedClusterLoader {
    clusters: HashMap<(String, Version), ClusterDefinition>,
}

impl PreloadedClusterLoader {
    fn new(clusters: HashMap<(String, Version), ClusterDefinition>) -> Self {
        Self { clusters }
    }
}

impl ClusterLoader for PreloadedClusterLoader {
    fn load(&self, id: &str, version: &Version) -> Option<ClusterDefinition> {
        self.clusters
            .get(&(id.to_string(), version.clone()))
            .cloned()
    }
}

impl ClusterVersionIndex for PreloadedClusterLoader {
    fn available_versions(&self, id: &str) -> Vec<Version> {
        let mut versions = self
            .clusters
            .keys()
            .filter_map(|(candidate_id, version)| {
                if candidate_id == id {
                    Some(version.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        versions.sort();
        versions
    }
}

struct PreparedGraphRuntime {
    graph_id: String,
    runtime_provenance: String,
    expanded: ExpandedGraph,
    catalog: CorePrimitiveCatalog,
    registries: CoreRegistries,
}

struct CanonicalAdapterSetup {
    adapter_bound: bool,
    adapter_provides: AdapterProvides,
    adapter_config: Option<HostedAdapterConfig>,
    expected_adapter_provenance: String,
}

#[derive(Debug)]
enum DriverTerminal {
    Completed,
    Interrupted(InterruptionReason),
}

struct DriverExecution {
    runner: HostedRunner,
    event_count: usize,
    episode_event_counts: Vec<(String, usize)>,
    terminal: DriverTerminal,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ProcessDriverMessage {
    Hello { protocol: String },
    Event { event: HostedEvent },
    End,
}

#[derive(Debug)]
enum ProcessDriverStreamObservation {
    Line(String),
    Eof,
    ReadError(ProcessDriverReadFailure),
}

#[derive(Debug)]
enum ProcessDriverReadFailure {
    InvalidEncoding(String),
    Io(String),
}

#[derive(Clone, Copy, Debug)]
struct ProcessDriverPolicy {
    startup_grace: Duration,
    termination_grace: Duration,
    poll_interval: Duration,
    event_recv_timeout: Duration,
}

const DEFAULT_PROCESS_DRIVER_POLICY: ProcessDriverPolicy = ProcessDriverPolicy {
    startup_grace: Duration::from_secs(5),
    termination_grace: Duration::from_secs(5),
    poll_interval: Duration::from_millis(10),
    event_recv_timeout: Duration::from_millis(100),
};

#[derive(Debug)]
enum ProcessDriverReceiveFailure {
    Timeout,
    Disconnected,
}

fn parse_adapter_manifest(path: &Path) -> Result<AdapterManifest, String> {
    let data = fs::read_to_string(path)
        .map_err(|err| format!("read adapter manifest '{}': {err}", path.display()))?;
    let value = serde_yaml::from_str::<serde_json::Value>(&data)
        .map_err(|err| format!("parse adapter manifest '{}': {err}", path.display()))?;
    serde_json::from_value::<AdapterManifest>(value)
        .map_err(|err| format!("decode adapter manifest '{}': {err}", path.display()))
}

fn materialize_runtime_surfaces(
    runtime_surfaces: Option<RuntimeSurfaces>,
) -> Result<(CorePrimitiveCatalog, CoreRegistries), String> {
    match runtime_surfaces {
        Some(runtime_surfaces) => {
            let (registries, catalog) = runtime_surfaces.into_parts();
            Ok((catalog, registries))
        }
        None => {
            let catalog = build_core_catalog();
            let registries =
                core_registries().map_err(|err| format!("core registries: {err:?}"))?;
            Ok((catalog, registries))
        }
    }
}

fn prepare_graph_runtime(
    graph_path: &Path,
    cluster_paths: &[PathBuf],
    runtime_surfaces: Option<RuntimeSurfaces>,
) -> Result<PreparedGraphRuntime, String> {
    let root = ergo_loader::parse_graph_file(graph_path).map_err(|err| err.to_string())?;
    let discovery = ergo_loader::discovery::discover_cluster_tree(graph_path, &root, cluster_paths)
        .map_err(|err| err.to_string())?;
    let cluster_sources = discovery.cluster_sources;
    let clusters = discovery.clusters;
    let loader = PreloadedClusterLoader::new(clusters);

    let (catalog, registries) = materialize_runtime_surfaces(runtime_surfaces)?;
    let expanded = expand(&root, &loader, &catalog).map_err(|err| {
        format!(
            "graph expansion failed: {}",
            summarize_expand_error(&err, &cluster_sources)
        )
    })?;
    let runtime_provenance =
        compute_runtime_provenance(RuntimeProvenanceScheme::Rpv1, &root.id, &expanded, &catalog)
            .map_err(|err| format!("runtime provenance compute failed: {err}"))?;

    Ok(PreparedGraphRuntime {
        graph_id: root.id,
        runtime_provenance,
        expanded,
        catalog,
        registries,
    })
}

fn prepare_adapter_setup(
    adapter_path: Option<&Path>,
    prepared: &PreparedGraphRuntime,
) -> Result<CanonicalAdapterSetup, String> {
    let Some(path) = adapter_path else {
        return Ok(CanonicalAdapterSetup {
            adapter_bound: false,
            adapter_provides: AdapterProvides::default(),
            adapter_config: None,
            expected_adapter_provenance: NO_ADAPTER_PROVENANCE.to_string(),
        });
    };

    let manifest = parse_adapter_manifest(path)?;
    ergo_adapter::validate_adapter(&manifest).map_err(|err| {
        format!(
            "adapter manifest validation failed: {}",
            summarize_error_info(&err)
        )
    })?;
    let provides = AdapterProvides::from_manifest(&manifest);
    validate_adapter_composition(
        &prepared.expanded,
        &prepared.catalog,
        &prepared.registries,
        &provides,
    )?;
    let adapter_provenance = adapter_fingerprint(&manifest);
    let binder = compile_event_binder(&provides)
        .map_err(|err| format!("adapter event binder compilation failed: {err}"))?;

    Ok(CanonicalAdapterSetup {
        adapter_bound: true,
        adapter_provides: provides.clone(),
        adapter_config: Some(HostedAdapterConfig {
            provides,
            binder,
            adapter_provenance: adapter_provenance.clone(),
        }),
        expected_adapter_provenance: adapter_provenance,
    })
}

fn host_replay_setup_error(message: impl Into<String>) -> HostReplayError {
    HostReplayError::Setup(message.into())
}

struct RunLifecycleState {
    control: RunControl,
    started_at: Instant,
}

impl RunLifecycleState {
    fn new(control: RunControl) -> Self {
        Self {
            control,
            started_at: Instant::now(),
        }
    }

    fn should_stop(&self, committed_event_count: usize) -> bool {
        if self.control.stop.is_requested() {
            return true;
        }

        let duration_reached = self
            .control
            .max_duration
            .is_some_and(|max_duration| self.started_at.elapsed() >= max_duration);
        let max_events_reached = self
            .control
            .max_events
            .is_some_and(|max_events| committed_event_count as u64 >= max_events);

        if duration_reached || max_events_reached {
            self.control.stop.request_stop();
            return true;
        }

        false
    }
}

fn captured_external_effect_kinds(bundle: &CaptureBundle) -> HashSet<String> {
    bundle
        .decisions
        .iter()
        .flat_map(|decision| decision.effects.iter())
        .filter_map(|effect| {
            let kind = effect.effect.kind.as_str();
            (kind != "set_context").then(|| kind.to_string())
        })
        .collect()
}

fn replay_owned_external_kinds(
    runtime: &RuntimeHandle,
    adapter_provides: &AdapterProvides,
    handler_kinds: &BTreeSet<String>,
) -> HashSet<String> {
    runtime
        .graph_emittable_effect_kinds()
        .into_iter()
        .filter(|kind| adapter_provides.effects.contains(kind))
        .filter(|kind| !handler_kinds.contains(kind))
        .collect()
}

/// Canonical run API for clients. Host owns graph loading, expansion, adapter composition, and runner setup.
// Allow non-Send/Sync in Arc: CoreRegistries and CorePrimitiveCatalog contain non-Send/Sync types.
#[allow(clippy::arc_with_non_send_sync)]
pub fn run_graph_from_paths(request: RunGraphFromPathsRequest) -> RunGraphResponse {
    run_graph_from_paths_internal(
        request,
        None,
        DEFAULT_PROCESS_DRIVER_POLICY,
        RunControl::default(),
    )
}

/// Canonical run API with host stop control and bounded-run limits.
#[allow(clippy::arc_with_non_send_sync)]
pub fn run_graph_from_paths_with_control(
    request: RunGraphFromPathsRequest,
    control: RunControl,
) -> RunGraphResponse {
    run_graph_from_paths_internal(request, None, DEFAULT_PROCESS_DRIVER_POLICY, control)
}

/// Advanced run API for callers that prebuild runtime surfaces before invoking the canonical host path.
#[allow(clippy::arc_with_non_send_sync)]
pub fn run_graph_from_paths_with_surfaces(
    request: RunGraphFromPathsRequest,
    runtime_surfaces: RuntimeSurfaces,
) -> RunGraphResponse {
    run_graph_from_paths_internal(
        request,
        Some(runtime_surfaces),
        DEFAULT_PROCESS_DRIVER_POLICY,
        RunControl::default(),
    )
}

/// Advanced controlled run API for callers that prebuild runtime surfaces before invoking the canonical host path.
#[allow(clippy::arc_with_non_send_sync)]
pub fn run_graph_from_paths_with_surfaces_and_control(
    request: RunGraphFromPathsRequest,
    runtime_surfaces: RuntimeSurfaces,
    control: RunControl,
) -> RunGraphResponse {
    run_graph_from_paths_internal(
        request,
        Some(runtime_surfaces),
        DEFAULT_PROCESS_DRIVER_POLICY,
        control,
    )
}

#[allow(clippy::arc_with_non_send_sync)]
fn run_graph_from_paths_internal(
    request: RunGraphFromPathsRequest,
    runtime_surfaces: Option<RuntimeSurfaces>,
    process_policy: ProcessDriverPolicy,
    control: RunControl,
) -> RunGraphResponse {
    let RunGraphFromPathsRequest {
        graph_path,
        cluster_paths,
        driver,
        adapter_path,
        egress_config,
        capture_output,
        pretty_capture,
    } = request;

    let prepared = prepare_graph_runtime(&graph_path, &cluster_paths, runtime_surfaces)
        .map_err(HostRunError::InvalidInput)?;
    let dependency_summary =
        scan_adapter_dependencies(&prepared.expanded, &prepared.catalog, &prepared.registries)
            .map_err(HostRunError::InvalidInput)?;
    let adapter_setup = prepare_adapter_setup(adapter_path.as_deref(), &prepared)
        .map_err(HostRunError::InvalidInput)?;

    let PreparedGraphRuntime {
        graph_id,
        runtime_provenance,
        expanded,
        catalog,
        registries,
    } = prepared;

    let runtime = RuntimeHandle::new(
        Arc::new(expanded),
        Arc::new(catalog),
        Arc::new(registries),
        adapter_setup.adapter_provides,
    );
    let egress_provenance = egress_config.as_ref().map(compute_egress_provenance);
    let runner = HostedRunner::new(
        GraphId::new(graph_id),
        Constraints::default(),
        runtime,
        runtime_provenance,
        adapter_setup.adapter_config,
        egress_config,
        egress_provenance,
        None,
    )
    .map_err(|err| {
        HostRunError::StepFailed(format!("failed to initialize canonical host runner: {err}"))
    })?;

    run_graph_with_policy(
        RunGraphRequest {
            graph_path,
            driver,
            capture_output,
            pretty_capture,
            adapter_bound: adapter_setup.adapter_bound,
            dependency_summary,
            runner,
        },
        process_policy,
        control,
    )
}

/// Canonical replay API for clients. Host owns capture load, graph loading, adapter composition, and runner setup.
// Allow non-Send/Sync in Arc: CoreRegistries and CorePrimitiveCatalog contain non-Send/Sync types.
#[allow(clippy::arc_with_non_send_sync)]
pub fn replay_graph_from_paths(
    request: ReplayGraphFromPathsRequest,
) -> Result<ReplayGraphResult, HostReplayError> {
    replay_graph_from_paths_internal(request, None)
}

/// Advanced replay API for callers that prebuild runtime surfaces before invoking the canonical host path.
#[allow(clippy::arc_with_non_send_sync)]
pub fn replay_graph_from_paths_with_surfaces(
    request: ReplayGraphFromPathsRequest,
    runtime_surfaces: RuntimeSurfaces,
) -> Result<ReplayGraphResult, HostReplayError> {
    replay_graph_from_paths_internal(request, Some(runtime_surfaces))
}

#[allow(clippy::arc_with_non_send_sync)]
fn replay_graph_from_paths_internal(
    request: ReplayGraphFromPathsRequest,
    runtime_surfaces: Option<RuntimeSurfaces>,
) -> Result<ReplayGraphResult, HostReplayError> {
    let ReplayGraphFromPathsRequest {
        capture_path,
        graph_path,
        cluster_paths,
        adapter_path,
    } = request;

    let data = fs::read_to_string(&capture_path).map_err(|err| {
        host_replay_setup_error(format!(
            "failed to read capture artifact '{}': {err}",
            capture_path.display()
        ))
    })?;
    let bundle = serde_json::from_str::<CaptureBundle>(&data).map_err(|err| {
        host_replay_setup_error(format!(
            "failed to parse capture artifact '{}': {err}",
            capture_path.display()
        ))
    })?;

    let prepared = prepare_graph_runtime(&graph_path, &cluster_paths, runtime_surfaces)
        .map_err(host_replay_setup_error)?;
    if bundle.graph_id.as_str() != prepared.graph_id {
        return Err(HostReplayError::GraphIdMismatch {
            expected: prepared.graph_id,
            got: bundle.graph_id.as_str().to_string(),
        });
    }

    let adapter_setup = prepare_adapter_setup(adapter_path.as_deref(), &prepared)
        .map_err(host_replay_setup_error)?;
    let PreparedGraphRuntime {
        runtime_provenance,
        expanded,
        catalog,
        registries,
        ..
    } = prepared;

    let runtime = RuntimeHandle::new(
        Arc::new(expanded),
        Arc::new(catalog),
        Arc::new(registries),
        adapter_setup.adapter_provides.clone(),
    );
    let handler_kinds = BTreeSet::from(["set_context".to_string()]);
    let replay_external_kinds =
        replay_owned_external_kinds(&runtime, &adapter_setup.adapter_provides, &handler_kinds);
    let captured_external_kinds = captured_external_effect_kinds(&bundle);
    let mut missing: Vec<String> = captured_external_kinds
        .difference(&replay_external_kinds)
        .cloned()
        .collect();
    if !missing.is_empty() {
        missing.sort();
        return Err(HostReplayError::ExternalKindsNotRepresentable { missing });
    }
    let runner = HostedRunner::new(
        GraphId::new(bundle.graph_id.as_str().to_string()),
        bundle.config.clone(),
        runtime,
        runtime_provenance.clone(),
        adapter_setup.adapter_config,
        None,
        None,
        Some(replay_external_kinds),
    )
    .map_err(|err| {
        host_replay_setup_error(format!(
            "failed to initialize canonical host replay runner: {err}"
        ))
    })?;

    replay_graph(ReplayGraphRequest {
        bundle,
        runner,
        expected_adapter_provenance: adapter_setup.expected_adapter_provenance,
        expected_runtime_provenance: runtime_provenance,
    })
}

/// Lower-level canonical run API used once a fully configured `HostedRunner` exists.
pub fn run_graph(request: RunGraphRequest) -> RunGraphResponse {
    run_graph_with_policy(request, DEFAULT_PROCESS_DRIVER_POLICY, RunControl::default())
}

/// Lower-level canonical run API with host stop control and bounded-run limits.
pub fn run_graph_with_control(request: RunGraphRequest, control: RunControl) -> RunGraphResponse {
    run_graph_with_policy(request, DEFAULT_PROCESS_DRIVER_POLICY, control)
}

fn run_graph_with_policy(
    request: RunGraphRequest,
    process_policy: ProcessDriverPolicy,
    control: RunControl,
) -> RunGraphResponse {
    let RunGraphRequest {
        graph_path,
        driver,
        capture_output,
        pretty_capture,
        adapter_bound,
        dependency_summary,
        mut runner,
    } = request;

    if !adapter_bound && dependency_summary.requires_adapter {
        return Err(HostRunError::AdapterRequired(dependency_summary));
    }

    runner
        .start_egress_channels()
        .map_err(|err| HostRunError::DriverIo(format!("start egress channels: {err}")))?;

    let lifecycle = RunLifecycleState::new(control);

    let execution = match driver {
        DriverConfig::Fixture { path } => {
            run_fixture_driver(path, adapter_bound, runner, &lifecycle)?
        }
        DriverConfig::Process { command } => {
            run_process_driver(command, runner, process_policy, &lifecycle)?
        }
    };

    match execution.terminal {
        DriverTerminal::Completed => {
            let summary = finalize_run_summary(
                &graph_path,
                capture_output,
                pretty_capture,
                execution.runner,
                execution.event_count,
                execution.episode_event_counts,
            )?;
            Ok(RunOutcome::Completed(summary))
        }
        DriverTerminal::Interrupted(reason) => {
            let summary = finalize_run_summary(
                &graph_path,
                capture_output,
                pretty_capture,
                execution.runner,
                execution.event_count,
                execution.episode_event_counts,
            )?;
            Ok(RunOutcome::Interrupted(InterruptedRun { summary, reason }))
        }
    }
}

fn run_fixture_driver(
    fixture_path: PathBuf,
    adapter_bound: bool,
    mut runner: HostedRunner,
    lifecycle: &RunLifecycleState,
) -> Result<DriverExecution, HostRunError> {
    let fixture_items = fixture::parse_fixture(&fixture_path)
        .map_err(|err| HostRunError::InvalidInput(format!("failed to parse fixture: {err}")))?;

    let mut episodes: Vec<(String, usize)> = Vec::new();
    let mut current_episode: Option<usize> = None;
    let mut event_counter = 0usize;
    let mut committed_event_count = 0usize;
    let mut seen_fixture_event_ids = HashSet::new();

    for item in fixture_items {
        if lifecycle.should_stop(committed_event_count) {
            return host_stop_driver_execution(
                runner,
                event_counter,
                committed_event_count,
                episodes,
            );
        }

        match item {
            fixture::FixtureItem::EpisodeStart { label } => {
                episodes.push((label, 0));
                current_episode = Some(episodes.len() - 1);
            }
            fixture::FixtureItem::Event {
                id,
                kind,
                payload,
                semantic_kind,
            } => {
                if lifecycle.should_stop(committed_event_count) {
                    return host_stop_driver_execution(
                        runner,
                        event_counter,
                        committed_event_count,
                        episodes,
                    );
                }

                if current_episode.is_none() {
                    let label = format!("E{}", episodes.len() + 1);
                    episodes.push((label, 0));
                    current_episode = Some(episodes.len() - 1);
                }

                event_counter += 1;
                let event_id = id.unwrap_or_else(|| format!("fixture_evt_{}", event_counter));
                if !seen_fixture_event_ids.insert(event_id.clone()) {
                    return Err(HostRunError::InvalidInput(format!(
                        "fixture event id '{}' appears more than once in canonical run input",
                        event_id
                    )));
                }

                let hosted_event = if adapter_bound {
                    let semantic = semantic_kind.ok_or_else(|| {
                        HostRunError::InvalidInput(format!(
                            "fixture event '{}' is missing semantic_kind in adapter-bound canonical run",
                            event_id
                        ))
                    })?;
                    HostedEvent {
                        event_id,
                        kind,
                        at: EventTime::default(),
                        semantic_kind: Some(semantic),
                        payload: Some(
                            payload.unwrap_or_else(|| {
                                serde_json::Value::Object(serde_json::Map::new())
                            }),
                        ),
                    }
                } else {
                    if semantic_kind.is_some() {
                        return Err(HostRunError::InvalidInput(format!(
                            "fixture event '{}' set semantic_kind but canonical run is not adapter-bound",
                            event_id
                        )));
                    }
                    HostedEvent {
                        event_id,
                        kind,
                        at: EventTime::default(),
                        semantic_kind: None,
                        payload,
                    }
                };

                match runner.step(hosted_event) {
                    Ok(_) => {
                        committed_event_count += 1;
                        let index = current_episode.expect("episode index set");
                        episodes[index].1 += 1;
                        if lifecycle.should_stop(committed_event_count) {
                            return host_stop_driver_execution(
                                runner,
                                event_counter,
                                committed_event_count,
                                episodes,
                            );
                        }
                    }
                    Err(crate::HostedStepError::EgressDispatchFailure(failure)) => {
                        let index = current_episode.expect("episode index set");
                        episodes[index].1 += 1;
                        return Ok(DriverExecution {
                            runner,
                            event_count: event_counter,
                            episode_event_counts: episodes,
                            terminal: DriverTerminal::Interrupted(
                                interruption_from_egress_dispatch_failure(failure),
                            ),
                        });
                    }
                    Err(err) => {
                        return Err(HostRunError::StepFailed(format!("host step failed: {err}")));
                    }
                }
            }
        }
    }

    if lifecycle.should_stop(committed_event_count) {
        return host_stop_driver_execution(runner, event_counter, committed_event_count, episodes);
    }

    if episodes.is_empty() {
        return Err(HostRunError::InvalidInput(
            "fixture contained no episodes".to_string(),
        ));
    }
    if event_counter == 0 {
        return Err(HostRunError::InvalidInput(
            "fixture contained no events".to_string(),
        ));
    }
    if let Some((label, _)) = episodes.iter().find(|(_, count)| *count == 0) {
        return Err(HostRunError::InvalidInput(format!(
            "episode '{}' has no events",
            label
        )));
    }

    if lifecycle.should_stop(committed_event_count) {
        return host_stop_driver_execution(runner, event_counter, committed_event_count, episodes);
    }

    Ok(DriverExecution {
        runner,
        event_count: event_counter,
        episode_event_counts: episodes,
        terminal: DriverTerminal::Completed,
    })
}

fn host_stop_driver_execution(
    runner: HostedRunner,
    event_count: usize,
    committed_event_count: usize,
    episode_event_counts: Vec<(String, usize)>,
) -> Result<DriverExecution, HostRunError> {
    if committed_event_count == 0 {
        return Err(HostRunError::StepFailed(
            "host stop requested before first committed event".to_string(),
        ));
    }

    Ok(DriverExecution {
        runner,
        event_count,
        episode_event_counts,
        terminal: DriverTerminal::Interrupted(InterruptionReason::HostStopRequested),
    })
}

fn process_driver_host_stop_execution(
    child: &mut Child,
    stderr_handle: &mut Option<JoinHandle<String>>,
    runner: HostedRunner,
    event_count: usize,
    committed_event_count: usize,
    episode_event_counts: Vec<(String, usize)>,
) -> Result<DriverExecution, HostRunError> {
    let _detail = abort_process_child(child, stderr_handle.take());
    host_stop_driver_execution(
        runner,
        event_count,
        committed_event_count,
        episode_event_counts,
    )
}

fn run_process_driver(
    command: Vec<String>,
    runner: HostedRunner,
    process_policy: ProcessDriverPolicy,
    lifecycle: &RunLifecycleState,
) -> Result<DriverExecution, HostRunError> {
    if command.is_empty() {
        return Err(HostRunError::InvalidInput(
            "process driver requires at least one argv element".to_string(),
        ));
    }

    let command_display = format!("{command:?}");
    let mut child = spawn_process_driver(&command)?;
    let mut stderr_handle = child.stderr.take().map(drain_process_stderr);
    let stdout = child.stdout.take().ok_or_else(|| {
        HostRunError::DriverStart(format!(
            "process driver {command_display} did not expose a stdout protocol stream"
        ))
    })?;
    let stdout_rx = spawn_process_stdout_reader(stdout);
    let mut hello_received = false;
    let mut event_counter = 0usize;
    let mut committed_event_count = 0usize;
    let mut episodes = Vec::new();
    let mut runner = runner;
    let startup_deadline = Instant::now() + process_policy.startup_grace;

    loop {
        if lifecycle.should_stop(committed_event_count) {
            return process_driver_host_stop_execution(
                &mut child,
                &mut stderr_handle,
                runner,
                event_counter,
                committed_event_count,
                episodes,
            );
        }

        if !hello_received && Instant::now() >= startup_deadline {
            let detail = abort_process_child(&mut child, stderr_handle.take());
            let suffix = if detail.is_empty() {
                String::new()
            } else {
                format!(" ({detail})")
            };
            return Err(HostRunError::DriverStart(format!(
                "process driver {command_display} did not emit a protocol frame within {}ms before startup completed{}",
                process_policy.startup_grace.as_millis(),
                suffix
            )));
        }

        let recv_timeout = if hello_received {
            process_policy.event_recv_timeout
        } else {
            startup_deadline
                .saturating_duration_since(Instant::now())
                .min(process_policy.event_recv_timeout)
        };

        let observation = match recv_process_stream_observation(&stdout_rx, Some(recv_timeout)) {
            Ok(observation) => observation,
            Err(ProcessDriverReceiveFailure::Timeout) => {
                if lifecycle.should_stop(committed_event_count) {
                    return process_driver_host_stop_execution(
                        &mut child,
                        &mut stderr_handle,
                        runner,
                        event_counter,
                        committed_event_count,
                        episodes,
                    );
                }
                continue;
            }
            Err(ProcessDriverReceiveFailure::Disconnected) => {
                let detail = abort_process_child(&mut child, stderr_handle.take());
                return Err(if detail.is_empty() {
                    HostRunError::DriverIo(format!(
                        "receive process driver stdout for {command_display}: stdout reader disconnected unexpectedly"
                    ))
                } else {
                    HostRunError::DriverIo(format!(
                        "receive process driver stdout for {command_display}: stdout reader disconnected unexpectedly ({detail})"
                    ))
                });
            }
        };

        let message = match observation {
            ProcessDriverStreamObservation::Line(line) => {
                let trimmed = line.trim_end_matches(['\r', '\n']);
                match serde_json::from_str::<ProcessDriverMessage>(trimmed) {
                    Ok(message) => message,
                    Err(err) => {
                        let _detail = abort_process_child(&mut child, stderr_handle.take());
                        if committed_event_count == 0 {
                            return Err(HostRunError::DriverProtocol(format!(
                                "process driver {command_display} emitted invalid JSONL protocol before first committed step: {err}"
                            )));
                        }
                        return Ok(DriverExecution {
                            runner,
                            event_count: event_counter,
                            episode_event_counts: episodes,
                            terminal: DriverTerminal::Interrupted(
                                InterruptionReason::ProtocolViolation,
                            ),
                        });
                    }
                }
            }
            ProcessDriverStreamObservation::Eof => {
                let exit_status =
                    wait_for_child_exit(&mut child, process_policy).map_err(|err| {
                        let detail = abort_process_child(&mut child, stderr_handle.take());
                        if detail.is_empty() {
                            HostRunError::DriverIo(format!(
                                "wait on process driver {command_display} after stdout EOF: {err}"
                            ))
                        } else {
                            HostRunError::DriverIo(format!(
                                "wait on process driver {command_display} after stdout EOF: {err} ({detail})"
                            ))
                        }
                    })?;

                let detail = match exit_status {
                    Some(_) => take_process_stderr(stderr_handle.take()),
                    None => abort_process_child(&mut child, stderr_handle.take()),
                };

                if committed_event_count == 0 {
                    let suffix = if detail.is_empty() {
                        String::new()
                    } else {
                        format!(" ({detail})")
                    };
                    let message = match exit_status {
                        Some(status) => format!(
                            "process driver {command_display} ended before first committed step ({}){}",
                            format_exit_status(status),
                            suffix
                        ),
                        None => format!(
                            "process driver {command_display} closed stdout but did not exit within {}ms before first committed step{}",
                            process_policy.termination_grace.as_millis(),
                            suffix
                        ),
                    };
                    return Err(HostRunError::DriverStart(message));
                }

                return Ok(DriverExecution {
                    runner,
                    event_count: event_counter,
                    episode_event_counts: episodes,
                    terminal: DriverTerminal::Interrupted(InterruptionReason::DriverTerminated),
                });
            }
            ProcessDriverStreamObservation::ReadError(failure) => {
                let extra = abort_process_child(&mut child, stderr_handle.take());
                match failure {
                    ProcessDriverReadFailure::InvalidEncoding(detail) => {
                        let message = if extra.is_empty() {
                            format!(
                                "process driver {command_display} emitted malformed protocol bytes: {detail}"
                            )
                        } else {
                            format!(
                                "process driver {command_display} emitted malformed protocol bytes: {detail} ({extra})"
                            )
                        };
                        if committed_event_count == 0 {
                            return Err(HostRunError::DriverProtocol(message));
                        }
                        return Ok(DriverExecution {
                            runner,
                            event_count: event_counter,
                            episode_event_counts: episodes,
                            terminal: DriverTerminal::Interrupted(
                                InterruptionReason::ProtocolViolation,
                            ),
                        });
                    }
                    ProcessDriverReadFailure::Io(detail) => {
                        let message = if extra.is_empty() {
                            format!("read process driver stdout for {command_display}: {detail}")
                        } else {
                            format!(
                                "read process driver stdout for {command_display}: {detail} ({extra})"
                            )
                        };
                        if committed_event_count == 0 {
                            return Err(HostRunError::DriverIo(message));
                        }
                        return Ok(DriverExecution {
                            runner,
                            event_count: event_counter,
                            episode_event_counts: episodes,
                            terminal: DriverTerminal::Interrupted(InterruptionReason::DriverIo),
                        });
                    }
                }
            }
        };

        if !hello_received {
            match message {
                ProcessDriverMessage::Hello { protocol } if protocol == "ergo-driver.v0" => {
                    hello_received = true;
                    continue;
                }
                ProcessDriverMessage::Hello { protocol } => {
                    let _detail = abort_process_child(&mut child, stderr_handle.take());
                    return Err(HostRunError::DriverProtocol(format!(
                        "process driver {command_display} declared unsupported protocol '{protocol}'"
                    )));
                }
                other => {
                    let _detail = abort_process_child(&mut child, stderr_handle.take());
                    return Err(HostRunError::DriverProtocol(format!(
                        "process driver {command_display} must send hello first, got {}",
                        process_message_name(&other)
                    )));
                }
            }
        }

        match message {
            ProcessDriverMessage::Hello { .. } => {
                let _detail = abort_process_child(&mut child, stderr_handle.take());
                if committed_event_count == 0 {
                    return Err(HostRunError::DriverProtocol(format!(
                        "process driver {command_display} sent duplicate hello before first committed step"
                    )));
                }
                return Ok(DriverExecution {
                    runner,
                    event_count: event_counter,
                    episode_event_counts: episodes,
                    terminal: DriverTerminal::Interrupted(InterruptionReason::ProtocolViolation),
                });
            }
            ProcessDriverMessage::Event { event } => match runner.step(event) {
                Ok(_) => {
                    event_counter += 1;
                    committed_event_count += 1;
                    if episodes.is_empty() {
                        episodes.push(("E1".to_string(), 0));
                    }
                    episodes[0].1 += 1;
                    if lifecycle.should_stop(committed_event_count) {
                        return process_driver_host_stop_execution(
                            &mut child,
                            &mut stderr_handle,
                            runner,
                            event_counter,
                            committed_event_count,
                            episodes,
                        );
                    }
                }
                Err(crate::HostedStepError::EgressDispatchFailure(failure)) => {
                    event_counter += 1;
                    if episodes.is_empty() {
                        episodes.push(("E1".to_string(), 0));
                    }
                    episodes[0].1 += 1;
                    let _detail = abort_process_child(&mut child, stderr_handle.take());
                    return Ok(DriverExecution {
                        runner,
                        event_count: event_counter,
                        episode_event_counts: episodes,
                        terminal: DriverTerminal::Interrupted(
                            interruption_from_egress_dispatch_failure(failure),
                        ),
                    });
                }
                Err(err) => {
                    let _detail = abort_process_child(&mut child, stderr_handle.take());
                    return Err(HostRunError::StepFailed(format!("host step failed: {err}")));
                }
            },
            ProcessDriverMessage::End => {
                if committed_event_count == 0 {
                    let _detail = abort_process_child(&mut child, stderr_handle.take());
                    return Err(HostRunError::DriverProtocol(format!(
                        "process driver {command_display} ended before first committed step"
                    )));
                }

                let terminal = drain_process_after_end(
                    &command_display,
                    &mut child,
                    &stdout_rx,
                    &mut stderr_handle,
                    process_policy,
                )
                .map_err(|message| HostRunError::DriverIo(message))?;

                return Ok(DriverExecution {
                    runner,
                    event_count: event_counter,
                    episode_event_counts: episodes,
                    terminal,
                });
            }
        }
    }
}

fn spawn_process_driver(command: &[String]) -> Result<Child, HostRunError> {
    let mut child = Command::new(&command[0]);
    child.args(&command[1..]);
    child.stdin(Stdio::null());
    child.stdout(Stdio::piped());
    child.stderr(Stdio::piped());
    child.spawn().map_err(|err| {
        HostRunError::DriverStart(format!("spawn process driver {:?}: {err}", command))
    })
}

fn spawn_process_stdout_reader(stdout: ChildStdout) -> Receiver<ProcessDriverStreamObservation> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = tx.send(ProcessDriverStreamObservation::Eof);
                    break;
                }
                Ok(_) => {
                    if tx.send(ProcessDriverStreamObservation::Line(line)).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    let failure = if err.kind() == std::io::ErrorKind::InvalidData {
                        ProcessDriverReadFailure::InvalidEncoding(err.to_string())
                    } else {
                        ProcessDriverReadFailure::Io(err.to_string())
                    };
                    let _ = tx.send(ProcessDriverStreamObservation::ReadError(failure));
                    break;
                }
            }
        }
    });
    rx
}

fn drain_process_stderr(stderr: impl Read + Send + 'static) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut output = String::new();
        let _ = reader.read_to_string(&mut output);
        output
    })
}

fn recv_process_stream_observation(
    stdout_rx: &Receiver<ProcessDriverStreamObservation>,
    timeout: Option<Duration>,
) -> Result<ProcessDriverStreamObservation, ProcessDriverReceiveFailure> {
    match timeout {
        Some(timeout) => stdout_rx.recv_timeout(timeout).map_err(|err| match err {
            RecvTimeoutError::Timeout => ProcessDriverReceiveFailure::Timeout,
            RecvTimeoutError::Disconnected => ProcessDriverReceiveFailure::Disconnected,
        }),
        None => stdout_rx
            .recv()
            .map_err(|_| ProcessDriverReceiveFailure::Disconnected),
    }
}

fn abort_process_child(child: &mut Child, stderr_handle: Option<JoinHandle<String>>) -> String {
    let _ = child.kill();
    let _ = child.wait();
    take_process_stderr(stderr_handle)
}

fn take_process_stderr(stderr_handle: Option<JoinHandle<String>>) -> String {
    stderr_handle
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn wait_for_child_exit(
    child: &mut Child,
    process_policy: ProcessDriverPolicy,
) -> std::io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + process_policy.termination_grace;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(process_policy.poll_interval);
    }
}

fn drain_process_after_end(
    command_display: &str,
    child: &mut Child,
    stdout_rx: &Receiver<ProcessDriverStreamObservation>,
    stderr_handle: &mut Option<JoinHandle<String>>,
    process_policy: ProcessDriverPolicy,
) -> Result<DriverTerminal, String> {
    let deadline = Instant::now() + process_policy.termination_grace;
    let mut stdout_eof = false;
    let mut exit_status: Option<ExitStatus> = None;

    loop {
        if exit_status.is_none() {
            match child.try_wait() {
                Ok(status) => exit_status = status,
                Err(err) => {
                    let detail = abort_process_child(child, stderr_handle.take());
                    return if detail.is_empty() {
                        Err(format!("wait on process driver {command_display}: {err}"))
                    } else {
                        Err(format!(
                            "wait on process driver {command_display}: {err} ({detail})"
                        ))
                    };
                }
            }
        }

        if stdout_eof {
            if let Some(status) = exit_status {
                let _detail = take_process_stderr(stderr_handle.take());
                return Ok(if status.success() {
                    DriverTerminal::Completed
                } else {
                    DriverTerminal::Interrupted(InterruptionReason::DriverTerminated)
                });
            }

            if Instant::now() >= deadline {
                let _detail = abort_process_child(child, stderr_handle.take());
                return Ok(DriverTerminal::Interrupted(
                    InterruptionReason::DriverTerminated,
                ));
            }

            thread::sleep(process_policy.poll_interval);
            continue;
        }

        let now = Instant::now();
        if now >= deadline {
            let _detail = abort_process_child(child, stderr_handle.take());
            return Ok(DriverTerminal::Interrupted(
                InterruptionReason::DriverTerminated,
            ));
        }

        let timeout = (deadline - now).min(process_policy.poll_interval);
        match stdout_rx.recv_timeout(timeout) {
            Ok(ProcessDriverStreamObservation::Line(_)) => {
                let _detail = abort_process_child(child, stderr_handle.take());
                return Ok(DriverTerminal::Interrupted(
                    InterruptionReason::ProtocolViolation,
                ));
            }
            Ok(ProcessDriverStreamObservation::Eof) => {
                stdout_eof = true;
            }
            Ok(ProcessDriverStreamObservation::ReadError(failure)) => match failure {
                ProcessDriverReadFailure::InvalidEncoding(_detail) => {
                    let _extra = abort_process_child(child, stderr_handle.take());
                    return Ok(DriverTerminal::Interrupted(
                        InterruptionReason::ProtocolViolation,
                    ));
                }
                ProcessDriverReadFailure::Io(detail) => {
                    let extra = abort_process_child(child, stderr_handle.take());
                    return if extra.is_empty() {
                        Err(format!(
                            "read process driver stdout for {command_display}: {detail}"
                        ))
                    } else {
                        Err(format!(
                            "read process driver stdout for {command_display}: {detail} ({extra})"
                        ))
                    };
                }
            },
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                let extra = abort_process_child(child, stderr_handle.take());
                return if extra.is_empty() {
                    Err(format!(
                        "stdout reader disconnected unexpectedly for process driver {command_display}"
                    ))
                } else {
                    Err(format!(
                        "stdout reader disconnected unexpectedly for process driver {command_display} ({extra})"
                    ))
                };
            }
        }
    }
}

fn format_exit_status(status: ExitStatus) -> String {
    status.to_string()
}

fn process_message_name(message: &ProcessDriverMessage) -> &'static str {
    match message {
        ProcessDriverMessage::Hello { .. } => "hello",
        ProcessDriverMessage::Event { .. } => "event",
        ProcessDriverMessage::End => "end",
    }
}

fn finalize_run_summary(
    graph_path: &Path,
    capture_output: Option<PathBuf>,
    pretty_capture: bool,
    mut runner: HostedRunner,
    event_count: usize,
    episode_event_counts: Vec<(String, usize)>,
) -> Result<RunSummary, HostRunError> {
    if episode_event_counts.is_empty() {
        return Err(HostRunError::InvalidInput(
            "driver produced no episodes".to_string(),
        ));
    }
    if event_count == 0 {
        return Err(HostRunError::InvalidInput(
            "driver produced no events".to_string(),
        ));
    }
    if let Some((label, _)) = episode_event_counts.iter().find(|(_, count)| *count == 0) {
        return Err(HostRunError::InvalidInput(format!(
            "episode '{}' has no events",
            label
        )));
    }

    runner
        .ensure_no_pending_egress_acks()
        .map_err(|err| HostRunError::StepFailed(format!("egress pending-ack invariant: {err}")))?;

    // Freeze egress lifecycle before capture finalization so late channel activity
    // cannot alter dispatch truth after artifact write.
    runner
        .stop_egress_channels()
        .map_err(|err| HostRunError::DriverIo(format!("stop egress channels: {err}")))?;

    let bundle = runner.into_capture_bundle();
    let capture_path = capture_output.unwrap_or_else(|| {
        let stem = graph_path
            .file_stem()
            .and_then(|part| part.to_str())
            .unwrap_or("graph");
        PathBuf::from("target").join(format!("{stem}-capture.json"))
    });
    let style = if pretty_capture {
        CaptureJsonStyle::Pretty
    } else {
        CaptureJsonStyle::Compact
    };
    write_capture_bundle(&capture_path, &bundle, style)
        .map_err(|err| HostRunError::Io(format!("write capture bundle: {err}")))?;

    let invoked = bundle
        .decisions
        .iter()
        .filter(|record| record.decision == Decision::Invoke)
        .count();
    let deferred = bundle
        .decisions
        .iter()
        .filter(|record| record.decision == Decision::Defer)
        .count();

    Ok(RunSummary {
        capture_path,
        episodes: episode_event_counts.len(),
        events: event_count,
        invoked,
        deferred,
        episode_event_counts,
    })
}

/// Lower-level canonical replay API used once a fully configured `HostedRunner` exists.
pub fn replay_graph(request: ReplayGraphRequest) -> Result<ReplayGraphResult, HostReplayError> {
    let replayed_bundle = replay_bundle_strict(
        &request.bundle,
        request.runner,
        StrictReplayExpectations {
            expected_adapter_provenance: &request.expected_adapter_provenance,
            expected_runtime_provenance: &request.expected_runtime_provenance,
        },
    )?;

    let (invoked, deferred, skipped) = decision_counts(&replayed_bundle);
    Ok(ReplayGraphResult {
        graph_id: replayed_bundle.graph_id,
        events: replayed_bundle.events.len(),
        invoked,
        deferred,
        skipped,
    })
}

pub fn run_fixture(request: RunFixtureRequest) -> Result<RunFixtureResult, HostRunError> {
    let outcome = run_graph(RunGraphRequest {
        graph_path: PathBuf::from("fixture"),
        driver: DriverConfig::Fixture {
            path: request.fixture_path,
        },
        capture_output: Some(request.capture_output.clone()),
        pretty_capture: request.pretty_capture,
        adapter_bound: false,
        dependency_summary: AdapterDependencySummary::default(),
        runner: request.runner,
    })?;
    let summary = match outcome {
        RunOutcome::Completed(summary) => summary,
        RunOutcome::Interrupted(_) => {
            return Err(HostRunError::StepFailed(
                "fixture driver returned interrupted outcome unexpectedly".to_string(),
            ))
        }
    };
    Ok(RunFixtureResult {
        capture_path: summary.capture_path,
        episodes: summary.episodes,
        events: summary.events,
        episode_event_counts: summary.episode_event_counts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::egress::{EgressChannelConfig, EgressRoute};
    use ergo_adapter::ExternalEventKind;
    use ergo_runtime::action::{
        ActionEffects, ActionKind, ActionOutcome, ActionPrimitive, ActionPrimitiveManifest,
        ActionValue, ActionValueType, Cardinality as ActionCardinality,
        ExecutionSpec as ActionExecutionSpec, InputSpec as ActionInputSpec, IntentFieldSpec,
        IntentSpec, OutputSpec as ActionOutputSpec, StateSpec as ActionStateSpec,
    };
    use ergo_runtime::catalog::CatalogBuilder;
    use ergo_runtime::common::{Value, ValueType};
    use ergo_runtime::runtime::ExecutionContext;
    use ergo_runtime::source::{
        Cadence as SourceCadence, ExecutionSpec as SourceExecutionSpec,
        OutputSpec as SourceOutputSpec, SourceKind, SourcePrimitive, SourcePrimitiveManifest,
        SourceRequires, StateSpec as SourceStateSpec,
    };
    use serde_json::json;
    use std::collections::{BTreeMap, HashMap};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct InjectedNumberSource {
        manifest: SourcePrimitiveManifest,
        output: f64,
        counter: Option<Arc<AtomicUsize>>,
    }

    struct InjectedIntentAction {
        manifest: ActionPrimitiveManifest,
    }

    impl InjectedNumberSource {
        fn new(output: f64) -> Self {
            Self {
                manifest: SourcePrimitiveManifest {
                    id: "injected_number_source".to_string(),
                    version: "0.1.0".to_string(),
                    kind: SourceKind::Source,
                    inputs: vec![],
                    outputs: vec![SourceOutputSpec {
                        name: "value".to_string(),
                        value_type: ValueType::Number,
                    }],
                    parameters: vec![],
                    requires: SourceRequires {
                        context: Vec::new(),
                    },
                    execution: SourceExecutionSpec {
                        deterministic: true,
                        cadence: SourceCadence::Continuous,
                    },
                    state: SourceStateSpec { allowed: false },
                    side_effects: false,
                },
                output,
                counter: None,
            }
        }

        fn new_observed(output: f64, counter: Arc<AtomicUsize>) -> Self {
            Self {
                counter: Some(counter),
                ..Self::new(output)
            }
        }
    }

    impl SourcePrimitive for InjectedNumberSource {
        fn manifest(&self) -> &SourcePrimitiveManifest {
            &self.manifest
        }

        fn produce(
            &self,
            _parameters: &HashMap<String, ergo_runtime::source::ParameterValue>,
            _ctx: &ExecutionContext,
        ) -> HashMap<String, Value> {
            if let Some(counter) = &self.counter {
                counter.fetch_add(1, Ordering::SeqCst);
            }
            HashMap::from([("value".to_string(), Value::Number(self.output))])
        }
    }

    impl InjectedIntentAction {
        fn new() -> Self {
            Self {
                manifest: ActionPrimitiveManifest {
                    id: "injected_intent_action".to_string(),
                    version: "0.1.0".to_string(),
                    kind: ActionKind::Action,
                    inputs: vec![
                        ActionInputSpec {
                            name: "event".to_string(),
                            value_type: ActionValueType::Event,
                            required: true,
                            cardinality: ActionCardinality::Single,
                        },
                        ActionInputSpec {
                            name: "qty".to_string(),
                            value_type: ActionValueType::Number,
                            required: true,
                            cardinality: ActionCardinality::Single,
                        },
                    ],
                    outputs: vec![ActionOutputSpec {
                        name: "outcome".to_string(),
                        value_type: ActionValueType::Event,
                    }],
                    parameters: vec![],
                    effects: ActionEffects {
                        writes: vec![],
                        intents: vec![IntentSpec {
                            name: "place_order".to_string(),
                            fields: vec![IntentFieldSpec {
                                name: "qty".to_string(),
                                value_type: ValueType::Number,
                                from_input: Some("qty".to_string()),
                                from_param: None,
                            }],
                            mirror_writes: vec![],
                        }],
                    },
                    execution: ActionExecutionSpec {
                        deterministic: true,
                        retryable: false,
                    },
                    state: ActionStateSpec { allowed: false },
                    side_effects: true,
                },
            }
        }
    }

    impl ActionPrimitive for InjectedIntentAction {
        fn manifest(&self) -> &ActionPrimitiveManifest {
            &self.manifest
        }

        fn execute(
            &self,
            _inputs: &HashMap<String, ActionValue>,
            _parameters: &HashMap<String, ergo_runtime::action::ParameterValue>,
        ) -> HashMap<String, ActionValue> {
            HashMap::from([(
                "outcome".to_string(),
                ActionValue::Event(ActionOutcome::Completed),
            )])
        }
    }

    fn build_injected_runtime_surfaces(output: f64) -> RuntimeSurfaces {
        let mut builder = CatalogBuilder::new();
        builder.add_source(Box::new(InjectedNumberSource::new(output)));
        builder.add_action(Box::new(InjectedIntentAction::new()));
        let (registries, catalog) = builder
            .build()
            .expect("injected runtime surfaces should build");
        RuntimeSurfaces::new(registries, catalog)
    }

    fn build_observed_runtime_surfaces(output: f64, counter: Arc<AtomicUsize>) -> RuntimeSurfaces {
        let mut builder = CatalogBuilder::new();
        builder.add_source(Box::new(InjectedNumberSource::new_observed(output, counter)));
        builder.add_action(Box::new(InjectedIntentAction::new()));
        let (registries, catalog) = builder
            .build()
            .expect("observed runtime surfaces should build");
        RuntimeSurfaces::new(registries, catalog)
    }

    fn write_temp_file(
        base: &Path,
        name: &str,
        contents: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let path = base.join(name);
        fs::write(&path, contents)?;
        Ok(path)
    }

    fn write_intent_graph(
        base: &Path,
        name: &str,
        graph_id: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        write_temp_file(
            base,
            name,
            &format!(
                r#"
kind: cluster
id: {graph_id}
version: "0.1.0"
nodes:
  gate:
    impl: const_bool@0.1.0
    params:
      value: true
  emit:
    impl: emit_if_true@0.1.0
  qty:
    impl: injected_number_source@0.1.0
  place:
    impl: injected_intent_action@0.1.0
edges:
  - "gate.value -> emit.input"
  - "emit.event -> place.event"
  - "qty.value -> place.qty"
outputs:
  outcome: place.outcome
"#
            ),
        )
    }

    fn write_intent_adapter_manifest(
        base: &Path,
        name: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        write_temp_file(
            base,
            name,
            r#"
kind: adapter
id: replay_intent_adapter
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: last_qty
    type: Number
    required: false
    writable: true
event_kinds:
  - name: price_bar
    payload_schema:
      type: object
      properties:
        price: { type: number }
      additionalProperties: false
accepts:
  effects:
    - name: set_context
      payload_schema:
        type: object
        additionalProperties: false
    - name: place_order
      payload_schema:
        type: object
        properties:
          qty: { type: number }
        required: [qty]
        additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.price_bar
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
"#,
        )
    }

    fn write_egress_ack_script(
        base: &Path,
        name: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        write_temp_file(
            base,
            name,
            r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  case "$line" in
    *'"type":"intent"'*)
      intent_id=$(printf '%s' "$line" | sed -n 's/.*"intent_id":"\([^"]*\)".*/\1/p')
      printf '{"type":"intent_ack","intent_id":"%s","status":"accepted","acceptance":"durable","egress_ref":"broker-ref-1"}\n' "$intent_id"
      ;;
    *'"type":"end"'*)
      exit 0
      ;;
  esac
done
"#,
        )
    }

    fn write_egress_protocol_script(
        base: &Path,
        name: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        write_temp_file(
            base,
            name,
            r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  case "$line" in
    *'"type":"intent"'*)
      printf '%s\n' '{"type":"intent_ack","intent_id":"wrong","status":"accepted","acceptance":"durable"}'
      ;;
    *'"type":"end"'*)
      exit 0
      ;;
  esac
done
"#,
        )
    }

    fn write_egress_io_script(
        base: &Path,
        name: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        write_temp_file(
            base,
            name,
            r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  case "$line" in
    *'"type":"intent"'*)
      exit 1
      ;;
    *'"type":"end"'*)
      exit 0
      ;;
  esac
done
"#,
        )
    }

    fn write_egress_hanging_shutdown_script(
        base: &Path,
        name: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        write_temp_file(
            base,
            name,
            r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  case "$line" in
    *'"type":"intent"'*)
      intent_id=$(printf '%s' "$line" | sed -n 's/.*"intent_id":"\([^"]*\)".*/\1/p')
      printf '{"type":"intent_ack","intent_id":"%s","status":"accepted","acceptance":"durable"}\n' "$intent_id"
      ;;
    *'"type":"end"'*)
      sleep 7
      exit 0
      ;;
  esac
done
"#,
        )
    }

    fn write_egress_end_sentinel_script(
        base: &Path,
        name: &str,
        sentinel_path: &Path,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        write_temp_file(
            base,
            name,
            &format!(
                r#"#!/bin/sh
sentinel='{sentinel}'
printf '%s\n' '{{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}}'
while IFS= read -r line; do
  case "$line" in
    *'"type":"intent"'*)
      intent_id=$(printf '%s' "$line" | sed -n 's/.*"intent_id":"\([^"]*\)".*/\1/p')
      printf '{{"type":"intent_ack","intent_id":"%s","status":"accepted","acceptance":"durable"}}\n' "$intent_id"
      ;;
    *'"type":"end"'*)
      printf '%s\n' 'saw_end' > "$sentinel"
      exit 0
      ;;
  esac
done
"#,
                sentinel = sentinel_path.display()
            ),
        )
    }

    fn write_egress_ack_once_then_timeout_script(
        base: &Path,
        name: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        write_temp_file(
            base,
            name,
            r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
acked=0
while IFS= read -r line; do
  case "$line" in
    *'"type":"intent"'*)
      if [ "$acked" = "0" ]; then
        intent_id=$(printf '%s' "$line" | sed -n 's/.*"intent_id":"\([^"]*\)".*/\1/p')
        printf '{"type":"intent_ack","intent_id":"%s","status":"accepted","acceptance":"durable"}\n' "$intent_id"
        acked=1
      fi
      ;;
    *'"type":"end"'*)
      exit 0
      ;;
  esac
done
"#,
        )
    }

    fn make_intent_egress_config(script_path: &Path) -> EgressConfig {
        make_intent_egress_config_with_timeout(script_path, Duration::from_millis(250))
    }

    fn make_intent_egress_config_with_timeout(
        script_path: &Path,
        timeout: Duration,
    ) -> EgressConfig {
        EgressConfig {
            default_ack_timeout: timeout,
            channels: BTreeMap::from([(
                "broker".to_string(),
                EgressChannelConfig::Process {
                    command: vec!["/bin/sh".to_string(), script_path.display().to_string()],
                },
            )]),
            routes: BTreeMap::from([(
                "place_order".to_string(),
                EgressRoute {
                    channel: "broker".to_string(),
                    ack_timeout: None,
                },
            )]),
        }
    }

    fn write_process_driver_script(
        base: &Path,
        name: &str,
        lines: &[String],
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let script = format!(
            "#!/bin/sh\ncat <<'__ERGO_DRIVER__'\n{}\n__ERGO_DRIVER__\n",
            lines.join("\n")
        );
        write_temp_file(base, name, &script)
    }

    fn write_process_driver_program(
        base: &Path,
        name: &str,
        body: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        write_temp_file(base, name, body)
    }

    fn hosted_event(event_id: &str) -> HostedEvent {
        HostedEvent {
            event_id: event_id.to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: None,
            payload: Some(json!({})),
        }
    }

    fn expect_completed(
        outcome: RunGraphResponse,
    ) -> Result<RunSummary, Box<dyn std::error::Error>> {
        match outcome? {
            RunOutcome::Completed(summary) => Ok(summary),
            RunOutcome::Interrupted(interrupted) => Err(format!(
                "expected completed run, got interrupted({})",
                interrupted.reason
            )
            .into()),
        }
    }

    fn short_test_process_driver_policy() -> ProcessDriverPolicy {
        ProcessDriverPolicy {
            startup_grace: Duration::from_millis(50),
            termination_grace: Duration::from_millis(50),
            poll_interval: Duration::from_millis(5),
            event_recv_timeout: Duration::from_millis(10),
        }
    }

    fn run_graph_from_paths_with_process_policy(
        request: RunGraphFromPathsRequest,
        process_policy: ProcessDriverPolicy,
    ) -> RunGraphResponse {
        run_graph_from_paths_internal(request, None, process_policy, RunControl::default())
    }

    fn run_graph_from_paths_with_process_policy_and_control(
        request: RunGraphFromPathsRequest,
        process_policy: ProcessDriverPolicy,
        control: RunControl,
    ) -> RunGraphResponse {
        run_graph_from_paths_internal(request, None, process_policy, control)
    }

    #[test]
    fn run_graph_from_paths_executes_simple_graph() -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-run-from-paths-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_path_graph
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let fixture = write_temp_file(
            &temp_dir,
            "fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        )?;
        let capture = temp_dir.join("capture.json");

        let result = expect_completed(run_graph_from_paths(RunGraphFromPathsRequest {
            graph_path: graph.clone(),
            cluster_paths: Vec::new(),
            driver: DriverConfig::Fixture { path: fixture },
            adapter_path: None,
            egress_config: None,
            capture_output: Some(capture.clone()),
            pretty_capture: false,
        }))?;

        assert_eq!(result.episodes, 1);
        assert_eq!(result.events, 1);
        assert_eq!(result.capture_path, capture);
        assert!(capture.exists());

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn run_graph_from_paths_surfaces_runtime_owned_cluster_version_details(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-cluster-version-miss-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(temp_dir.join("clusters"))?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_cluster_version_miss
version: "0.1.0"
nodes:
  nested:
    cluster: shared_value@^2.0
edges: []
outputs:
  result: nested.value
"#,
        )?;
        write_temp_file(
            &temp_dir,
            "shared_value.yaml",
            r#"
kind: cluster
id: shared_value
version: "1.5.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 4.0
edges: []
outputs:
  value: src.value
"#,
        )?;
        write_temp_file(
            &temp_dir.join("clusters"),
            "shared_value.yaml",
            r#"
kind: cluster
id: shared_value
version: "1.0.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 3.0
edges: []
outputs:
  value: src.value
"#,
        )?;
        let fixture = write_temp_file(
            &temp_dir,
            "fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        )?;

        let err = run_graph_from_paths(RunGraphFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            driver: DriverConfig::Fixture { path: fixture },
            adapter_path: None,
            egress_config: None,
            capture_output: None,
            pretty_capture: false,
        })
        .expect_err("version-miss cluster graph must fail before run");

        match err {
            HostRunError::InvalidInput(detail) => {
                assert!(detail.contains("graph expansion failed"));
                assert!(detail.contains("shared_value"));
                assert!(detail.contains("^2.0"));
                assert!(detail.contains("available: 1.0.0, 1.5.0"));
                assert!(detail.contains("available cluster files"));
                assert!(detail.contains("shared_value@1.0.0"));
                assert!(detail.contains("shared_value@1.5.0"));
                assert!(!detail.contains("discovery error"));
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn replay_graph_from_paths_replays_capture() -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-replay-from-paths-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_path_replay
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 5.0
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let fixture = write_temp_file(
            &temp_dir,
            "fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        )?;
        let capture = temp_dir.join("capture.json");

        let _ = run_graph_from_paths_with_surfaces(
            RunGraphFromPathsRequest {
                graph_path: graph.clone(),
                cluster_paths: Vec::new(),
                driver: DriverConfig::Fixture { path: fixture },
                adapter_path: None,
                egress_config: None,
                capture_output: Some(capture.clone()),
                pretty_capture: false,
            },
            build_injected_runtime_surfaces(9.0),
        )?;

        let replay = replay_graph_from_paths(ReplayGraphFromPathsRequest {
            capture_path: capture,
            graph_path: graph,
            cluster_paths: Vec::new(),
            adapter_path: None,
        })?;

        assert_eq!(replay.graph_id.as_str(), "host_path_replay");
        assert_eq!(replay.events, 1);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn run_graph_from_paths_with_surfaces_uses_injected_runtime_surfaces(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-run-injected-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_injected_run
version: "0.1.0"
nodes:
  src:
    impl: injected_number_source@0.1.0
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let fixture = write_temp_file(
            &temp_dir,
            "fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        )?;
        let capture = temp_dir.join("capture.json");

        let result = expect_completed(run_graph_from_paths_with_surfaces(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Fixture { path: fixture },
                adapter_path: None,
                egress_config: None,
                capture_output: Some(capture.clone()),
                pretty_capture: false,
            },
            build_injected_runtime_surfaces(7.5),
        ))?;

        assert_eq!(result.episodes, 1);
        assert_eq!(result.events, 1);
        assert_eq!(result.capture_path, capture);
        assert!(capture.exists());

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn replay_graph_from_paths_with_surfaces_uses_injected_runtime_surfaces(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-replay-injected-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_injected_replay
version: "0.1.0"
nodes:
  src:
    impl: injected_number_source@0.1.0
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let fixture = write_temp_file(
            &temp_dir,
            "fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        )?;
        let capture = temp_dir.join("capture.json");

        let _ = run_graph_from_paths_with_surfaces(
            RunGraphFromPathsRequest {
                graph_path: graph.clone(),
                cluster_paths: Vec::new(),
                driver: DriverConfig::Fixture { path: fixture },
                adapter_path: None,
                egress_config: None,
                capture_output: Some(capture.clone()),
                pretty_capture: false,
            },
            build_injected_runtime_surfaces(9.0),
        )?;

        let replay = replay_graph_from_paths_with_surfaces(
            ReplayGraphFromPathsRequest {
                capture_path: capture,
                graph_path: graph,
                cluster_paths: Vec::new(),
                adapter_path: None,
            },
            build_injected_runtime_surfaces(9.0),
        )?;

        assert_eq!(replay.graph_id.as_str(), "host_injected_replay");
        assert_eq!(replay.events, 1);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn live_run_with_external_intent_graph_requires_egress_config(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-live-intent-without-egress-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_intent_live_no_egress")?;
        let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;
        let fixture = write_temp_file(
            &temp_dir,
            "fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"semantic_kind\":\"price_bar\",\"payload\":{\"price\":100.0}}}\n",
        )?;
        let capture = temp_dir.join("capture.json");

        let outcome = run_graph_from_paths_with_surfaces(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Fixture { path: fixture },
                adapter_path: Some(adapter),
                egress_config: None,
                capture_output: Some(capture),
                pretty_capture: false,
            },
            build_injected_runtime_surfaces(42.0),
        );

        match outcome {
            Err(HostRunError::StepFailed(detail)) => {
                assert!(
                    detail.contains("handler coverage failed"),
                    "unexpected setup error: {detail}"
                );
            }
            other => panic!("expected step-failed setup error, got {other:?}"),
        }

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn replay_from_paths_handles_external_effect_capture_without_live_egress(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-replay-intent-no-egress-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_intent_replay")?;
        let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;
        let egress_script = write_egress_ack_script(&temp_dir, "egress.sh")?;
        let driver = write_process_driver_script(
            &temp_dir,
            "driver.sh",
            &[
                serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?,
                serde_json::to_string(&json!({
                    "type":"event",
                    "event": HostedEvent {
                        event_id: "evt1".to_string(),
                        kind: ExternalEventKind::Command,
                        at: EventTime::default(),
                        semantic_kind: Some("price_bar".to_string()),
                        payload: Some(json!({"price": 101.5}))
                    }
                }))?,
                serde_json::to_string(&json!({"type":"end"}))?,
            ],
        )?;
        let capture = temp_dir.join("capture.json");

        let outcome = run_graph_from_paths_with_surfaces(
            RunGraphFromPathsRequest {
                graph_path: graph.clone(),
                cluster_paths: Vec::new(),
                driver: DriverConfig::Process {
                    command: vec!["/bin/sh".to_string(), driver.display().to_string()],
                },
                adapter_path: Some(adapter.clone()),
                egress_config: Some(make_intent_egress_config(&egress_script)),
                capture_output: Some(capture.clone()),
                pretty_capture: false,
            },
            build_injected_runtime_surfaces(42.0),
        )?;
        let summary = match outcome {
            RunOutcome::Completed(summary) => summary,
            RunOutcome::Interrupted(interrupted) => {
                return Err(format!(
                    "expected completed run, got interrupted({})",
                    interrupted.reason
                )
                .into())
            }
        };
        assert!(summary.capture_path.exists());

        let bundle_data = fs::read_to_string(&capture)?;
        let bundle: CaptureBundle = serde_json::from_str(&bundle_data)?;
        let decision = bundle.decisions.first().expect("capture decision");
        let external_effect = decision
            .effects
            .iter()
            .find(|effect| effect.effect.kind != "set_context")
            .expect("capture should contain external effect");
        assert!(
            external_effect.effect.writes.is_empty(),
            "external effect writes must be empty"
        );
        assert!(
            !external_effect.effect.intents.is_empty(),
            "external effect must carry intents"
        );
        let durable_ack = decision
            .intent_acks
            .iter()
            .find(|ack| ack.status == "accepted" && ack.acceptance == "durable")
            .expect("capture should include durable-accept ack");
        assert_eq!(durable_ack.channel, "broker");

        let replay = replay_graph_from_paths_with_surfaces(
            ReplayGraphFromPathsRequest {
                capture_path: capture,
                graph_path: graph,
                cluster_paths: Vec::new(),
                adapter_path: Some(adapter),
            },
            build_injected_runtime_surfaces(42.0),
        )?;
        assert_eq!(replay.events, 1);

        // CHECK-15 closure: prove exact deterministic intent_id parity across capture/replay.
        let prepared = prepare_graph_runtime(
            &temp_dir.join("graph.yaml"),
            &Vec::new(),
            Some(build_injected_runtime_surfaces(42.0)),
        )
        .map_err(|err| format!("prepare replay runtime: {err}"))?;
        let adapter_setup = prepare_adapter_setup(Some(&temp_dir.join("adapter.yaml")), &prepared)
            .map_err(|err| format!("prepare replay adapter: {err}"))?;
        let runtime = RuntimeHandle::new(
            Arc::new(prepared.expanded),
            Arc::new(prepared.catalog),
            Arc::new(prepared.registries),
            adapter_setup.adapter_provides.clone(),
        );
        let handler_kinds = BTreeSet::from(["set_context".to_string()]);
        let replay_external_kinds =
            replay_owned_external_kinds(&runtime, &adapter_setup.adapter_provides, &handler_kinds);
        let replay_runner = HostedRunner::new(
            GraphId::new(bundle.graph_id.as_str().to_string()),
            bundle.config.clone(),
            runtime,
            prepared.runtime_provenance.clone(),
            adapter_setup.adapter_config,
            None,
            None,
            Some(replay_external_kinds),
        )
        .map_err(|err| format!("initialize replay runner: {err}"))?;
        let replayed_bundle = replay_bundle_strict(
            &bundle,
            replay_runner,
            StrictReplayExpectations {
                expected_adapter_provenance: &adapter_setup.expected_adapter_provenance,
                expected_runtime_provenance: &prepared.runtime_provenance,
            },
        )
        .map_err(|err| format!("strict replay failed: {err}"))?;
        let captured_intent_id = bundle
            .decisions
            .iter()
            .flat_map(|decision| decision.effects.iter())
            .find(|effect| effect.effect.kind != "set_context")
            .and_then(|effect| effect.effect.intents.first())
            .map(|intent| intent.intent_id.clone())
            .expect("captured external intent_id");
        let replayed_intent_id = replayed_bundle
            .decisions
            .iter()
            .flat_map(|decision| decision.effects.iter())
            .find(|effect| effect.effect.kind != "set_context")
            .and_then(|effect| effect.effect.intents.first())
            .map(|intent| intent.intent_id.clone())
            .expect("replayed external intent_id");
        assert_eq!(captured_intent_id, replayed_intent_id);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn egress_timeout_maps_to_typed_interruption_and_preserves_partial_acks(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-egress-timeout-interruption-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_intent_timeout")?;
        let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;
        let egress_script = write_egress_ack_once_then_timeout_script(&temp_dir, "egress.sh")?;
        let driver = write_process_driver_script(
            &temp_dir,
            "driver.sh",
            &[
                serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?,
                serde_json::to_string(&json!({
                    "type":"event",
                    "event": HostedEvent {
                        event_id: "evt1".to_string(),
                        kind: ExternalEventKind::Command,
                        at: EventTime::default(),
                        semantic_kind: Some("price_bar".to_string()),
                        payload: Some(json!({"price": 101.5}))
                    }
                }))?,
                serde_json::to_string(&json!({
                    "type":"event",
                    "event": HostedEvent {
                        event_id: "evt2".to_string(),
                        kind: ExternalEventKind::Command,
                        at: EventTime::default(),
                        semantic_kind: Some("price_bar".to_string()),
                        payload: Some(json!({"price": 101.6}))
                    }
                }))?,
                serde_json::to_string(&json!({"type":"end"}))?,
            ],
        )?;
        let capture = temp_dir.join("capture.json");

        let outcome = run_graph_from_paths_with_surfaces(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Process {
                    command: vec!["/bin/sh".to_string(), driver.display().to_string()],
                },
                adapter_path: Some(adapter),
                egress_config: Some(make_intent_egress_config_with_timeout(
                    &egress_script,
                    Duration::from_millis(80),
                )),
                capture_output: Some(capture),
                pretty_capture: false,
            },
            build_injected_runtime_surfaces(42.0),
        )?;

        let interrupted = match outcome {
            RunOutcome::Interrupted(interrupted) => interrupted,
            RunOutcome::Completed(_) => {
                return Err("expected interrupted run for timeout case".into());
            }
        };

        match interrupted.reason {
            InterruptionReason::EgressAckTimeout { channel, intent_id } => {
                assert_eq!(channel, "broker");
                assert!(intent_id.starts_with("eid1:sha256:"));
            }
            other => return Err(format!("expected EgressAckTimeout, got {other:?}").into()),
        }

        let bundle: CaptureBundle =
            serde_json::from_str(&fs::read_to_string(&interrupted.summary.capture_path)?)?;
        let ack_count: usize = bundle
            .decisions
            .iter()
            .map(|decision| decision.intent_acks.len())
            .sum();
        assert!(
            ack_count >= 1,
            "expected at least one preserved durable ack, got {ack_count}"
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn capture_egress_provenance_is_none_when_no_egress_config(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-no-egress-provenance-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_no_egress_prov
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 1.0
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let fixture = write_temp_file(
            &temp_dir,
            "fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        )?;
        let capture = temp_dir.join("capture.json");

        let outcome = run_graph_from_paths_with_surfaces(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Fixture { path: fixture },
                adapter_path: None,
                egress_config: None,
                capture_output: Some(capture.clone()),
                pretty_capture: false,
            },
            build_injected_runtime_surfaces(42.0),
        )?;
        match outcome {
            RunOutcome::Completed(_) => {}
            RunOutcome::Interrupted(interrupted) => {
                return Err(format!(
                    "expected completed run, got interrupted({})",
                    interrupted.reason
                )
                .into())
            }
        }

        let bundle_data = fs::read_to_string(&capture)?;
        let bundle: CaptureBundle = serde_json::from_str(&bundle_data)?;
        assert!(
            bundle.egress_provenance.is_none(),
            "capture without egress config must keep egress_provenance unset"
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn capture_egress_provenance_is_present_even_when_no_intents_emitted(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-egress-provenance-no-intents-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_egress_no_intents
version: "0.1.0"
nodes:
  ev:
    impl: emit_if_event_and_true@0.1.0
  disabled:
    impl: const_bool@0.1.0
    params:
      value: false
  qty:
    impl: const_number@0.1.0
    params:
      value: 1.0
  place:
    impl: context_set_number@0.1.0
    params:
      key: "last_qty"
edges:
  - "ev.event -> place.event"
  - "disabled.value -> ev.condition"
  - "qty.value -> place.value"
outputs:
  outcome: place.outcome
"#,
        )?;
        let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;
        let egress_script = write_egress_ack_script(&temp_dir, "egress.sh")?;
        let driver = write_process_driver_script(
            &temp_dir,
            "driver.sh",
            &[
                serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?,
                serde_json::to_string(&json!({
                    "type":"event",
                    "event": HostedEvent {
                        event_id: "evt1".to_string(),
                        kind: ExternalEventKind::Command,
                        at: EventTime::default(),
                        semantic_kind: Some("price_bar".to_string()),
                        payload: Some(json!({"price": 101.5}))
                    }
                }))?,
                serde_json::to_string(&json!({"type":"end"}))?,
            ],
        )?;
        let capture = temp_dir.join("capture.json");

        let outcome = run_graph_from_paths_with_surfaces(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Process {
                    command: vec!["/bin/sh".to_string(), driver.display().to_string()],
                },
                adapter_path: Some(adapter),
                egress_config: Some(make_intent_egress_config(&egress_script)),
                capture_output: Some(capture.clone()),
                pretty_capture: false,
            },
            build_injected_runtime_surfaces(42.0),
        )?;
        match outcome {
            RunOutcome::Completed(_) => {}
            RunOutcome::Interrupted(interrupted) => {
                return Err(format!(
                    "expected completed run, got interrupted({})",
                    interrupted.reason
                )
                .into())
            }
        }

        let bundle_data = fs::read_to_string(&capture)?;
        let bundle: CaptureBundle = serde_json::from_str(&bundle_data)?;
        assert!(
            bundle.egress_provenance.is_some(),
            "capture with egress config must persist egress_provenance even when no intents fire"
        );
        assert_eq!(
            bundle.decisions.len(),
            1,
            "sanity check: the run should process one event but emit no intents"
        );
        assert!(
            bundle.decisions[0]
                .effects
                .iter()
                .all(|effect| effect.effect.kind != "place_order"),
            "disabled trigger should prevent external intent emission"
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn egress_protocol_violation_maps_to_typed_interruption_reason(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-egress-protocol-interruption-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_intent_protocol")?;
        let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;
        let egress_script = write_egress_protocol_script(&temp_dir, "egress.sh")?;
        let driver = write_process_driver_script(
            &temp_dir,
            "driver.sh",
            &[
                serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?,
                serde_json::to_string(&json!({
                    "type":"event",
                    "event": HostedEvent {
                        event_id: "evt1".to_string(),
                        kind: ExternalEventKind::Command,
                        at: EventTime::default(),
                        semantic_kind: Some("price_bar".to_string()),
                        payload: Some(json!({"price": 101.5}))
                    }
                }))?,
                serde_json::to_string(&json!({"type":"end"}))?,
            ],
        )?;
        let capture = temp_dir.join("capture.json");

        let outcome = run_graph_from_paths_with_surfaces(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Process {
                    command: vec!["/bin/sh".to_string(), driver.display().to_string()],
                },
                adapter_path: Some(adapter),
                egress_config: Some(make_intent_egress_config_with_timeout(
                    &egress_script,
                    Duration::from_millis(100),
                )),
                capture_output: Some(capture),
                pretty_capture: false,
            },
            build_injected_runtime_surfaces(42.0),
        )?;

        let interrupted = match outcome {
            RunOutcome::Interrupted(interrupted) => interrupted,
            RunOutcome::Completed(_) => {
                return Err("expected interrupted run for protocol case".into());
            }
        };
        match interrupted.reason {
            InterruptionReason::EgressProtocolViolation { channel } => {
                assert_eq!(channel, "broker");
            }
            other => return Err(format!("expected EgressProtocolViolation, got {other:?}").into()),
        }

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn egress_io_maps_to_typed_interruption_reason() -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-egress-io-interruption-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_intent_io")?;
        let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;
        let egress_script = write_egress_io_script(&temp_dir, "egress.sh")?;
        let driver = write_process_driver_script(
            &temp_dir,
            "driver.sh",
            &[
                serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?,
                serde_json::to_string(&json!({
                    "type":"event",
                    "event": HostedEvent {
                        event_id: "evt1".to_string(),
                        kind: ExternalEventKind::Command,
                        at: EventTime::default(),
                        semantic_kind: Some("price_bar".to_string()),
                        payload: Some(json!({"price": 101.5}))
                    }
                }))?,
                serde_json::to_string(&json!({"type":"end"}))?,
            ],
        )?;
        let capture = temp_dir.join("capture.json");

        let outcome = run_graph_from_paths_with_surfaces(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Process {
                    command: vec!["/bin/sh".to_string(), driver.display().to_string()],
                },
                adapter_path: Some(adapter),
                egress_config: Some(make_intent_egress_config_with_timeout(
                    &egress_script,
                    Duration::from_millis(100),
                )),
                capture_output: Some(capture),
                pretty_capture: false,
            },
            build_injected_runtime_surfaces(42.0),
        )?;

        let interrupted = match outcome {
            RunOutcome::Interrupted(interrupted) => interrupted,
            RunOutcome::Completed(_) => {
                return Err("expected interrupted run for io case".into());
            }
        };
        match interrupted.reason {
            InterruptionReason::EgressIo { channel } => assert_eq!(channel, "broker"),
            other => return Err(format!("expected EgressIo, got {other:?}").into()),
        }

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn egress_startup_failure_surfaces_host_run_error() -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-egress-startup-hostrunerror-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_intent_startup_fail")?;
        let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;
        let driver = write_process_driver_script(
            &temp_dir,
            "driver.sh",
            &[
                serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?,
                serde_json::to_string(&json!({
                    "type":"event",
                    "event": HostedEvent {
                        event_id: "evt1".to_string(),
                        kind: ExternalEventKind::Command,
                        at: EventTime::default(),
                        semantic_kind: Some("price_bar".to_string()),
                        payload: Some(json!({"price": 101.5}))
                    }
                }))?,
                serde_json::to_string(&json!({"type":"end"}))?,
            ],
        )?;
        let capture = temp_dir.join("capture.json");

        let config = EgressConfig {
            default_ack_timeout: Duration::from_millis(50),
            channels: BTreeMap::from([(
                "broker".to_string(),
                EgressChannelConfig::Process {
                    command: vec!["/definitely/missing-egress-binary".to_string()],
                },
            )]),
            routes: BTreeMap::from([(
                "place_order".to_string(),
                EgressRoute {
                    channel: "broker".to_string(),
                    ack_timeout: None,
                },
            )]),
        };

        let err = run_graph_from_paths_with_surfaces(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Process {
                    command: vec!["/bin/sh".to_string(), driver.display().to_string()],
                },
                adapter_path: Some(adapter),
                egress_config: Some(config),
                capture_output: Some(capture),
                pretty_capture: false,
            },
            build_injected_runtime_surfaces(42.0),
        )
        .expect_err("startup failure should surface as host run error");

        assert!(
            matches!(err, HostRunError::DriverIo(_)),
            "expected HostRunError::DriverIo, got {err:?}"
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn egress_shutdown_failure_surfaces_host_run_error() -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-egress-shutdown-hostrunerror-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_intent_shutdown_fail")?;
        let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;
        let egress_script = write_egress_hanging_shutdown_script(&temp_dir, "egress.sh")?;
        let driver = write_process_driver_script(
            &temp_dir,
            "driver.sh",
            &[
                serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?,
                serde_json::to_string(&json!({
                    "type":"event",
                    "event": HostedEvent {
                        event_id: "evt1".to_string(),
                        kind: ExternalEventKind::Command,
                        at: EventTime::default(),
                        semantic_kind: Some("price_bar".to_string()),
                        payload: Some(json!({"price": 101.5}))
                    }
                }))?,
                serde_json::to_string(&json!({"type":"end"}))?,
            ],
        )?;
        let capture = temp_dir.join("capture.json");

        let err = run_graph_from_paths_with_surfaces(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Process {
                    command: vec!["/bin/sh".to_string(), driver.display().to_string()],
                },
                adapter_path: Some(adapter),
                egress_config: Some(make_intent_egress_config_with_timeout(
                    &egress_script,
                    Duration::from_millis(100),
                )),
                capture_output: Some(capture),
                pretty_capture: false,
            },
            build_injected_runtime_surfaces(42.0),
        )
        .expect_err("shutdown timeout should surface as host run error");

        assert!(
            matches!(err, HostRunError::DriverIo(_)),
            "expected HostRunError::DriverIo, got {err:?}"
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn replay_from_paths_fails_when_capture_external_kind_is_not_representable(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-replay-intent-kind-mismatch-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph =
            write_intent_graph(&temp_dir, "graph.yaml", "host_intent_replay_kind_mismatch")?;
        let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;
        let egress_script = write_egress_ack_script(&temp_dir, "egress.sh")?;
        let driver = write_process_driver_script(
            &temp_dir,
            "driver.sh",
            &[
                serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?,
                serde_json::to_string(&json!({
                    "type":"event",
                    "event": HostedEvent {
                        event_id: "evt1".to_string(),
                        kind: ExternalEventKind::Command,
                        at: EventTime::default(),
                        semantic_kind: Some("price_bar".to_string()),
                        payload: Some(json!({"price": 101.5}))
                    }
                }))?,
                serde_json::to_string(&json!({"type":"end"}))?,
            ],
        )?;
        let capture = temp_dir.join("capture.json");

        let _ = run_graph_from_paths_with_surfaces(
            RunGraphFromPathsRequest {
                graph_path: graph.clone(),
                cluster_paths: Vec::new(),
                driver: DriverConfig::Process {
                    command: vec!["/bin/sh".to_string(), driver.display().to_string()],
                },
                adapter_path: Some(adapter.clone()),
                egress_config: Some(make_intent_egress_config(&egress_script)),
                capture_output: Some(capture.clone()),
                pretty_capture: false,
            },
            build_injected_runtime_surfaces(42.0),
        )?;

        let mut bundle: CaptureBundle = serde_json::from_str(&fs::read_to_string(&capture)?)?;
        let effect = bundle
            .decisions
            .first_mut()
            .and_then(|decision| {
                decision
                    .effects
                    .iter_mut()
                    .find(|effect| effect.effect.kind == "place_order")
            })
            .expect("captured external place_order effect");
        effect.effect.kind = "cancel_order".to_string();
        if let Some(intent) = effect.effect.intents.first_mut() {
            intent.kind = "cancel_order".to_string();
        }
        fs::write(&capture, serde_json::to_string_pretty(&bundle)?)?;

        let err = replay_graph_from_paths_with_surfaces(
            ReplayGraphFromPathsRequest {
                capture_path: capture,
                graph_path: graph,
                cluster_paths: Vec::new(),
                adapter_path: Some(adapter),
            },
            build_injected_runtime_surfaces(42.0),
        )
        .expect_err("replay setup must fail for unrepresentable external effect kinds");

        match err {
            HostReplayError::ExternalKindsNotRepresentable { missing } => {
                assert!(
                    missing.iter().any(|kind| kind == "cancel_order"),
                    "expected cancel_order in missing kinds, got {missing:?}"
                );
            }
            other => panic!("expected ExternalKindsNotRepresentable, got {other:?}"),
        }

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn process_driver_executes_via_canonical_host_path() -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-process-run-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_process_graph
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let driver = write_process_driver_script(
            &temp_dir,
            "driver.sh",
            &[
                serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?,
                serde_json::to_string(&json!({"type":"event","event":hosted_event("evt1")}))?,
                serde_json::to_string(&json!({"type":"end"}))?,
            ],
        )?;
        let capture = temp_dir.join("capture.json");

        let result = run_graph_from_paths(RunGraphFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            driver: DriverConfig::Process {
                command: vec!["/bin/sh".to_string(), driver.display().to_string()],
            },
            adapter_path: None,
            egress_config: None,
            capture_output: Some(capture.clone()),
            pretty_capture: false,
        })?;

        match result {
            RunOutcome::Completed(summary) => {
                assert_eq!(summary.episodes, 1);
                assert_eq!(summary.events, 1);
                assert_eq!(summary.capture_path, capture);
                assert!(summary.capture_path.exists());
            }
            RunOutcome::Interrupted(interrupted) => {
                return Err(format!("expected completed run, got {:?}", interrupted.reason).into())
            }
        }

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn process_driver_host_stop_before_first_committed_event_returns_host_run_error_without_capture(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-process-stop-zero-event-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_process_stop_zero_event
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let hello = serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?;
        let driver = write_process_driver_program(
            &temp_dir,
            "driver.sh",
            &format!("#!/bin/sh\nprintf '%s\\n' '{hello}'\nexec sleep 5\n"),
        )?;
        let capture = temp_dir.join("capture.json");
        let stop = HostStopHandle::new();
        let stop_clone = stop.clone();
        let stopper = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            stop_clone.request_stop();
        });

        let err = run_graph_from_paths_with_process_policy_and_control(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Process {
                    command: vec!["/bin/sh".to_string(), driver.display().to_string()],
                },
                adapter_path: None,
                egress_config: None,
                capture_output: Some(capture.clone()),
                pretty_capture: false,
            },
            short_test_process_driver_policy(),
            RunControl::new().with_stop_handle(stop),
        )
        .expect_err("host stop before first committed event must surface HostRunError");

        stopper.join().expect("stopper thread must join");
        match err {
            HostRunError::StepFailed(message) => {
                assert!(
                    message.contains("host stop requested before first committed event"),
                    "unexpected error message: {message}"
                );
            }
            other => panic!("expected StepFailed host-stop error, got {other:?}"),
        }
        assert!(
            !capture.exists(),
            "zero-event host stop must not write a capture artifact"
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn process_driver_max_events_returns_host_stop_interruption_and_replayable_capture(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-process-max-events-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_process_max_events
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let hello = serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?;
        let mut body = format!("#!/bin/sh\ncat <<'__ERGO_DRIVER__'\n{hello}\n");
        for index in 1..=64 {
            body.push_str(&serde_json::to_string(&json!({
                "type":"event",
                "event": hosted_event(&format!("evt{index}"))
            }))?);
            body.push('\n');
        }
        body.push_str("__ERGO_DRIVER__\nexec sleep 5\n");
        let driver = write_process_driver_program(&temp_dir, "driver.sh", &body)?;
        let capture = temp_dir.join("capture.json");

        let outcome = run_graph_from_paths_with_process_policy_and_control(
            RunGraphFromPathsRequest {
                graph_path: graph.clone(),
                cluster_paths: Vec::new(),
                driver: DriverConfig::Process {
                    command: vec!["/bin/sh".to_string(), driver.display().to_string()],
                },
                adapter_path: None,
                egress_config: None,
                capture_output: Some(capture.clone()),
                pretty_capture: false,
            },
            short_test_process_driver_policy(),
            RunControl::new().max_events(3),
        )?;

        let interrupted = match outcome {
            RunOutcome::Interrupted(interrupted) => interrupted,
            RunOutcome::Completed(_) => {
                return Err("max_events stop must interrupt the process run".into())
            }
        };
        assert_eq!(interrupted.reason, InterruptionReason::HostStopRequested);
        assert_eq!(interrupted.summary.events, 3);
        assert!(capture.exists());

        let replay = replay_graph_from_paths(ReplayGraphFromPathsRequest {
            capture_path: capture,
            graph_path: graph,
            cluster_paths: Vec::new(),
            adapter_path: None,
        })?;
        assert_eq!(replay.graph_id.as_str(), "host_process_max_events");
        assert_eq!(replay.events, 3);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn process_driver_hot_stream_host_stop_does_not_wait_for_recv_timeout(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-process-hot-stop-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_process_hot_stop
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let marker = temp_dir.join("emitted-events.log");
        let hello = serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?;
        let mut body = format!(
            "#!/bin/sh\nmarker='{}'\nprintf '%s\\n' '{hello}'\n",
            marker.display()
        );
        for index in 1..=256 {
            let event_line = serde_json::to_string(&json!({
                "type":"event",
                "event": hosted_event(&format!("hot_evt{index}"))
            }))?;
            body.push_str(&format!(
                "printf '%s\\n' '{index}' >> \"$marker\"\nprintf '%s\\n' '{event_line}'\nsleep 0.001\n"
            ));
        }
        body.push_str("exec sleep 5\n");
        let driver = write_process_driver_program(&temp_dir, "driver.sh", &body)?;
        let capture = temp_dir.join("capture.json");
        let stop = HostStopHandle::new();
        let stop_clone = stop.clone();
        let marker_clone = marker.clone();
        let stopper = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let observed = fs::read_to_string(&marker_clone)
                    .map(|data| data.lines().count())
                    .unwrap_or(0);
                if observed >= 5 {
                    stop_clone.request_stop();
                    break;
                }
                if Instant::now() >= deadline {
                    panic!("timed out waiting for hot-stream marker events");
                }
                thread::sleep(Duration::from_millis(2));
            }
        });

        let started = Instant::now();
        let outcome = run_graph_from_paths_with_process_policy_and_control(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Process {
                    command: vec!["/bin/sh".to_string(), driver.display().to_string()],
                },
                adapter_path: None,
                egress_config: None,
                capture_output: Some(capture),
                pretty_capture: false,
            },
            ProcessDriverPolicy {
                event_recv_timeout: Duration::from_secs(2),
                ..short_test_process_driver_policy()
            },
            RunControl::new().with_stop_handle(stop),
        )?;
        stopper.join().expect("stopper thread must join");

        let interrupted = match outcome {
            RunOutcome::Interrupted(interrupted) => interrupted,
            RunOutcome::Completed(_) => return Err("hot-stream host stop must interrupt".into()),
        };
        assert_eq!(interrupted.reason, InterruptionReason::HostStopRequested);
        assert!(
            interrupted.summary.events >= 5,
            "expected at least five committed events before stop, got {}",
            interrupted.summary.events
        );
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "hot-stream stop should not wait for the 2s recv timeout"
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn fixture_max_events_returns_host_stop_interruption() -> Result<(), Box<dyn std::error::Error>>
    {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-fixture-max-events-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_fixture_max_events
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let fixture = write_temp_file(
            &temp_dir,
            "fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        )?;
        let capture = temp_dir.join("capture.json");

        let outcome = run_graph_from_paths_with_control(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Fixture { path: fixture },
                adapter_path: None,
                egress_config: None,
                capture_output: Some(capture.clone()),
                pretty_capture: false,
            },
            RunControl::new().max_events(2),
        )?;

        let interrupted = match outcome {
            RunOutcome::Interrupted(interrupted) => interrupted,
            RunOutcome::Completed(_) => return Err("fixture max_events must interrupt".into()),
        };
        assert_eq!(interrupted.reason, InterruptionReason::HostStopRequested);
        assert_eq!(interrupted.summary.events, 2);
        assert!(capture.exists());

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn fixture_external_stop_handle_returns_host_stop_interruption(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-fixture-external-stop-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_fixture_external_stop
version: "0.1.0"
nodes:
  src:
    impl: injected_number_source@0.1.0
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let mut fixture = String::from("{\"kind\":\"episode_start\",\"id\":\"E1\"}\n");
        for _ in 0..128 {
            fixture.push_str("{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n");
        }
        let fixture = write_temp_file(&temp_dir, "fixture.jsonl", &fixture)?;
        let capture = temp_dir.join("capture.json");
        let stop = HostStopHandle::new();
        let stop_clone = stop.clone();
        let source_counter = Arc::new(AtomicUsize::new(0));
        let observed_counter = source_counter.clone();
        let stopper = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while observed_counter.load(Ordering::SeqCst) < 5 {
                assert!(
                    Instant::now() < deadline,
                    "timed out waiting for observed fixture events"
                );
                thread::yield_now();
            }
            stop_clone.request_stop();
        });

        let outcome = run_graph_from_paths_with_surfaces_and_control(
            RunGraphFromPathsRequest {
                graph_path: graph.clone(),
                cluster_paths: Vec::new(),
                driver: DriverConfig::Fixture { path: fixture },
                adapter_path: None,
                egress_config: None,
                capture_output: Some(capture.clone()),
                pretty_capture: false,
            },
            build_observed_runtime_surfaces(2.5, source_counter),
            RunControl::new().with_stop_handle(stop),
        )?;
        stopper.join().expect("stopper thread must join");

        let interrupted = match outcome {
            RunOutcome::Interrupted(interrupted) => interrupted,
            RunOutcome::Completed(_) => return Err("fixture external stop must interrupt".into()),
        };
        assert_eq!(interrupted.reason, InterruptionReason::HostStopRequested);
        assert!(
            interrupted.summary.events >= 5,
            "expected at least five committed events before stop, got {}",
            interrupted.summary.events
        );
        assert!(
            interrupted.summary.events < 128,
            "external stop should interrupt before exhausting the fixture"
        );
        assert!(capture.exists());

        let replay = replay_graph_from_paths_with_surfaces(
            ReplayGraphFromPathsRequest {
                capture_path: capture,
                graph_path: graph,
                cluster_paths: Vec::new(),
                adapter_path: None,
            },
            build_injected_runtime_surfaces(2.5),
        )?;
        assert_eq!(replay.graph_id.as_str(), "host_fixture_external_stop");
        assert_eq!(replay.events, interrupted.summary.events);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn host_stop_still_shuts_down_egress_channels_cleanly() -> Result<(), Box<dyn std::error::Error>>
    {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-stop-egress-shutdown-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_stop_egress_shutdown")?;
        let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;
        let fixture = write_temp_file(
            &temp_dir,
            "fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"semantic_kind\":\"price_bar\",\"payload\":{\"price\":101.5}}}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"semantic_kind\":\"price_bar\",\"payload\":{\"price\":101.6}}}\n",
        )?;
        let sentinel = temp_dir.join("egress-ended.txt");
        let egress_script =
            write_egress_end_sentinel_script(&temp_dir, "egress.sh", &sentinel)?;
        let capture = temp_dir.join("capture.json");

        let outcome = run_graph_from_paths_with_surfaces_and_control(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Fixture { path: fixture },
                adapter_path: Some(adapter),
                egress_config: Some(make_intent_egress_config(&egress_script)),
                capture_output: Some(capture),
                pretty_capture: false,
            },
            build_injected_runtime_surfaces(42.0),
            RunControl::new().max_events(1),
        )?;

        let interrupted = match outcome {
            RunOutcome::Interrupted(interrupted) => interrupted,
            RunOutcome::Completed(_) => {
                return Err("host stop with egress must interrupt the run".into())
            }
        };
        assert_eq!(interrupted.reason, InterruptionReason::HostStopRequested);
        assert_eq!(interrupted.summary.events, 1);
        assert!(
            sentinel.exists(),
            "host stop finalization must send the egress end sentinel"
        );
        assert_eq!(fs::read_to_string(&sentinel)?.trim(), "saw_end");

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn process_driver_invalid_hello_fails_before_start() -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-process-invalid-hello-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_process_invalid_hello
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let driver = write_process_driver_script(
            &temp_dir,
            "driver.sh",
            &[serde_json::to_string(
                &json!({"type":"event","event":hosted_event("evt1")}),
            )?],
        )?;

        let err = run_graph_from_paths(RunGraphFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            driver: DriverConfig::Process {
                command: vec!["/bin/sh".to_string(), driver.display().to_string()],
            },
            adapter_path: None,
            egress_config: None,
            capture_output: Some(temp_dir.join("capture.json")),
            pretty_capture: false,
        })
        .expect_err("missing hello must fail before first committed step");

        assert!(matches!(err, HostRunError::DriverProtocol(_)));

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn process_driver_silent_before_hello_is_bounded_by_startup_grace(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-process-startup-timeout-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_process_startup_timeout
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let driver =
            write_process_driver_program(&temp_dir, "driver.sh", "#!/bin/sh\nexec sleep 5\n")?;

        let started = Instant::now();
        let err = run_graph_from_paths_with_process_policy(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Process {
                    command: vec!["/bin/sh".to_string(), driver.display().to_string()],
                },
                adapter_path: None,
                egress_config: None,
                capture_output: Some(temp_dir.join("capture.json")),
                pretty_capture: false,
            },
            short_test_process_driver_policy(),
        )
        .expect_err("silent startup must time out before canonical run begins");

        assert!(matches!(err, HostRunError::DriverStart(_)));
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "startup wait should be bounded"
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn process_driver_malformed_bytes_before_first_committed_step_return_driver_protocol(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-process-invalid-bytes-start-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_process_invalid_bytes_start
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let hello = serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?;
        let driver = write_process_driver_program(
            &temp_dir,
            "driver.sh",
            &format!("#!/bin/sh\nprintf '%s\\n' '{hello}'\nprintf '\\377\\n'\n"),
        )?;

        let err = run_graph_from_paths(RunGraphFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            driver: DriverConfig::Process {
                command: vec!["/bin/sh".to_string(), driver.display().to_string()],
            },
            adapter_path: None,
            egress_config: None,
            capture_output: Some(temp_dir.join("capture.json")),
            pretty_capture: false,
        })
        .expect_err("malformed bytes before first committed step must be protocol failure");

        assert!(matches!(err, HostRunError::DriverProtocol(_)));

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn process_driver_protocol_violation_after_start_returns_interrupted_and_replayable_capture(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-process-interrupted-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_process_interrupted
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let driver = write_process_driver_script(
            &temp_dir,
            "driver.sh",
            &[
                serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?,
                serde_json::to_string(&json!({"type":"event","event":hosted_event("evt1")}))?,
                "{not-json".to_string(),
            ],
        )?;
        let capture = temp_dir.join("capture.json");

        let outcome = run_graph_from_paths(RunGraphFromPathsRequest {
            graph_path: graph.clone(),
            cluster_paths: Vec::new(),
            driver: DriverConfig::Process {
                command: vec!["/bin/sh".to_string(), driver.display().to_string()],
            },
            adapter_path: None,
            egress_config: None,
            capture_output: Some(capture.clone()),
            pretty_capture: false,
        })?;

        let interrupted = match outcome {
            RunOutcome::Interrupted(interrupted) => interrupted,
            RunOutcome::Completed(_) => {
                return Err("expected interrupted run after protocol violation".into())
            }
        };
        assert_eq!(interrupted.reason, InterruptionReason::ProtocolViolation);
        assert_eq!(interrupted.summary.events, 1);
        assert_eq!(interrupted.summary.capture_path, capture);
        assert!(interrupted.summary.capture_path.exists());

        let replay = replay_graph_from_paths(ReplayGraphFromPathsRequest {
            capture_path: capture,
            graph_path: graph,
            cluster_paths: Vec::new(),
            adapter_path: None,
        })?;
        assert_eq!(replay.graph_id.as_str(), "host_process_interrupted");
        assert_eq!(replay.events, 1);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn process_driver_malformed_bytes_after_start_return_interrupted_and_replayable_capture(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-process-invalid-bytes-after-start-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_process_invalid_bytes_after_start
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let hello = serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?;
        let event = serde_json::to_string(&json!({"type":"event","event":hosted_event("evt1")}))?;
        let driver = write_process_driver_program(
            &temp_dir,
            "driver.sh",
            &format!(
                "#!/bin/sh\nprintf '%s\\n' '{hello}'\nprintf '%s\\n' '{event}'\nprintf '\\377\\n'\n"
            ),
        )?;
        let capture = temp_dir.join("capture.json");

        let outcome = run_graph_from_paths(RunGraphFromPathsRequest {
            graph_path: graph.clone(),
            cluster_paths: Vec::new(),
            driver: DriverConfig::Process {
                command: vec!["/bin/sh".to_string(), driver.display().to_string()],
            },
            adapter_path: None,
            egress_config: None,
            capture_output: Some(capture.clone()),
            pretty_capture: false,
        })?;

        let interrupted = match outcome {
            RunOutcome::Interrupted(interrupted) => interrupted,
            RunOutcome::Completed(_) => {
                return Err("expected interrupted run after malformed protocol bytes".into())
            }
        };
        assert_eq!(interrupted.reason, InterruptionReason::ProtocolViolation);
        assert_eq!(interrupted.summary.events, 1);
        assert!(capture.exists());

        let replay = replay_graph_from_paths(ReplayGraphFromPathsRequest {
            capture_path: capture,
            graph_path: graph,
            cluster_paths: Vec::new(),
            adapter_path: None,
        })?;
        assert_eq!(
            replay.graph_id.as_str(),
            "host_process_invalid_bytes_after_start"
        );
        assert_eq!(replay.events, 1);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn process_driver_non_zero_exit_after_end_returns_interrupted(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-process-nonzero-end-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_process_nonzero_end
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let hello = serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?;
        let event = serde_json::to_string(&json!({"type":"event","event":hosted_event("evt1")}))?;
        let end = serde_json::to_string(&json!({"type":"end"}))?;
        let driver = write_process_driver_program(
            &temp_dir,
            "driver.sh",
            &format!(
                "#!/bin/sh\nprintf '%s\\n' '{hello}'\nprintf '%s\\n' '{event}'\nprintf '%s\\n' '{end}'\nexit 1\n"
            ),
        )?;
        let capture = temp_dir.join("capture.json");

        let outcome = run_graph_from_paths_with_process_policy(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Process {
                    command: vec!["/bin/sh".to_string(), driver.display().to_string()],
                },
                adapter_path: None,
                egress_config: None,
                capture_output: Some(capture.clone()),
                pretty_capture: false,
            },
            short_test_process_driver_policy(),
        )?;

        let interrupted = match outcome {
            RunOutcome::Interrupted(interrupted) => interrupted,
            RunOutcome::Completed(_) => {
                return Err("non-zero exit after end must not be completed".into())
            }
        };
        assert_eq!(interrupted.reason, InterruptionReason::DriverTerminated);
        assert!(capture.exists());

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn process_driver_delayed_clean_exit_within_grace_returns_completed(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-process-delayed-clean-exit-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_process_delayed_clean_exit
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let hello = serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?;
        let event = serde_json::to_string(&json!({"type":"event","event":hosted_event("evt1")}))?;
        let end = serde_json::to_string(&json!({"type":"end"}))?;
        let driver = write_process_driver_program(
            &temp_dir,
            "driver.sh",
            &format!(
                "#!/bin/sh\nprintf '%s\\n' '{hello}'\nprintf '%s\\n' '{event}'\nprintf '%s\\n' '{end}'\nsleep 0.01\nexit 0\n"
            ),
        )?;
        let capture = temp_dir.join("capture.json");

        let summary = expect_completed(run_graph_from_paths_with_process_policy(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Process {
                    command: vec!["/bin/sh".to_string(), driver.display().to_string()],
                },
                adapter_path: None,
                egress_config: None,
                capture_output: Some(capture.clone()),
                pretty_capture: false,
            },
            short_test_process_driver_policy(),
        ))?;

        assert_eq!(summary.events, 1);
        assert_eq!(summary.capture_path, capture);
        assert!(summary.capture_path.exists());

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn process_driver_extra_output_after_end_returns_protocol_violation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-process-extra-after-end-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_process_extra_after_end
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let hello = serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?;
        let event = serde_json::to_string(&json!({"type":"event","event":hosted_event("evt1")}))?;
        let end = serde_json::to_string(&json!({"type":"end"}))?;
        let driver = write_process_driver_program(
            &temp_dir,
            "driver.sh",
            &format!(
                "#!/bin/sh\nprintf '%s\\n' '{hello}'\nprintf '%s\\n' '{event}'\nprintf '%s\\n' '{end}'\nprintf '%s\\n' 'trailing-garbage'\n"
            ),
        )?;
        let capture = temp_dir.join("capture.json");

        let outcome = run_graph_from_paths_with_process_policy(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Process {
                    command: vec!["/bin/sh".to_string(), driver.display().to_string()],
                },
                adapter_path: None,
                egress_config: None,
                capture_output: Some(capture.clone()),
                pretty_capture: false,
            },
            short_test_process_driver_policy(),
        )?;

        let interrupted = match outcome {
            RunOutcome::Interrupted(interrupted) => interrupted,
            RunOutcome::Completed(_) => {
                return Err("trailing output after end must not be completed".into())
            }
        };
        assert_eq!(interrupted.reason, InterruptionReason::ProtocolViolation);
        assert!(capture.exists());

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn process_driver_hang_after_end_is_bounded_and_interrupted(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-process-hang-after-end-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_process_hang_after_end
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let hello = serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?;
        let event = serde_json::to_string(&json!({"type":"event","event":hosted_event("evt1")}))?;
        let end = serde_json::to_string(&json!({"type":"end"}))?;
        let driver = write_process_driver_program(
            &temp_dir,
            "driver.sh",
            &format!(
                "#!/bin/sh\nprintf '%s\\n' '{hello}'\nprintf '%s\\n' '{event}'\nprintf '%s\\n' '{end}'\nexec sleep 5\n"
            ),
        )?;
        let capture = temp_dir.join("capture.json");

        let started = Instant::now();
        let outcome = run_graph_from_paths_with_process_policy(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Process {
                    command: vec!["/bin/sh".to_string(), driver.display().to_string()],
                },
                adapter_path: None,
                egress_config: None,
                capture_output: Some(capture.clone()),
                pretty_capture: false,
            },
            short_test_process_driver_policy(),
        )?;

        let interrupted = match outcome {
            RunOutcome::Interrupted(interrupted) => interrupted,
            RunOutcome::Completed(_) => return Err("hanging driver must not complete".into()),
        };
        assert_eq!(interrupted.reason, InterruptionReason::DriverTerminated);
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "hang-after-end path should be bounded"
        );
        assert!(capture.exists());

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn process_driver_stdout_eof_before_exit_is_bounded_and_interrupted(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-host-process-eof-hang-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir)?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: host_process_eof_hang
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#,
        )?;
        let hello = serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))?;
        let event = serde_json::to_string(&json!({"type":"event","event":hosted_event("evt1")}))?;
        let driver = write_process_driver_program(
            &temp_dir,
            "driver.sh",
            &format!(
                "#!/bin/sh\nprintf '%s\\n' '{hello}'\nprintf '%s\\n' '{event}'\nexec 1>&-\nexec sleep 5\n"
            ),
        )?;
        let capture = temp_dir.join("capture.json");

        let started = Instant::now();
        let outcome = run_graph_from_paths_with_process_policy(
            RunGraphFromPathsRequest {
                graph_path: graph,
                cluster_paths: Vec::new(),
                driver: DriverConfig::Process {
                    command: vec!["/bin/sh".to_string(), driver.display().to_string()],
                },
                adapter_path: None,
                egress_config: None,
                capture_output: Some(capture.clone()),
                pretty_capture: false,
            },
            short_test_process_driver_policy(),
        )?;

        let interrupted = match outcome {
            RunOutcome::Interrupted(interrupted) => interrupted,
            RunOutcome::Completed(_) => return Err("stdout-eof hang must not complete".into()),
        };
        assert_eq!(interrupted.reason, InterruptionReason::DriverTerminated);
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "stdout EOF wait should be bounded"
        );
        assert!(capture.exists());

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
