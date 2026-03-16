use ergo_adapter::{
    adapter_fingerprint, compile_event_binder, fixture, validate_action_adapter_composition,
    validate_capture_format, validate_source_adapter_composition, AdapterManifest, AdapterProvides,
    EventTime, GraphId, RuntimeHandle,
};
use ergo_runtime::catalog::{
    build_core_catalog, core_registries, CorePrimitiveCatalog, CoreRegistries,
};
use ergo_runtime::cluster::{
    expand, ClusterDefinition, ClusterLoader, ClusterVersionIndex, ExpandedGraph, PrimitiveCatalog,
    PrimitiveKind, Version,
};
use ergo_runtime::common::ErrorInfo;
use ergo_runtime::provenance::{compute_runtime_provenance, RuntimeProvenanceScheme};
use ergo_supervisor::replay::StrictReplayExpectations;
use ergo_supervisor::{
    write_capture_bundle, CaptureBundle, CaptureJsonStyle, Constraints, Decision,
    NO_ADAPTER_PROVENANCE,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::{
    decision_counts, replay_bundle_strict, EgressConfig, HostedAdapterConfig, HostedEvent,
    HostedReplayError, HostedRunner,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptionReason {
    DriverTerminated,
    ProtocolViolation,
    DriverIo,
}

impl InterruptionReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DriverTerminated => "driver_terminated",
            Self::ProtocolViolation => "protocol_violation",
            Self::DriverIo => "driver_io",
        }
    }
}

impl std::fmt::Display for InterruptionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
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
}

const DEFAULT_PROCESS_DRIVER_POLICY: ProcessDriverPolicy = ProcessDriverPolicy {
    startup_grace: Duration::from_secs(5),
    termination_grace: Duration::from_secs(5),
    poll_interval: Duration::from_millis(10),
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
    let clusters = ergo_loader::load_cluster_tree(graph_path, &root, cluster_paths)
        .map_err(|err| err.to_string())?;
    let loader = PreloadedClusterLoader::new(clusters);

    let (catalog, registries) = materialize_runtime_surfaces(runtime_surfaces)?;
    let expanded = expand(&root, &loader, &catalog)
        .map_err(|err| format!("graph expansion failed: {}", summarize_error_info(&err)))?;
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

/// Canonical run API for clients. Host owns graph loading, expansion, adapter composition, and runner setup.
// Allow non-Send/Sync in Arc: CoreRegistries and CorePrimitiveCatalog contain non-Send/Sync types.
#[allow(clippy::arc_with_non_send_sync)]
pub fn run_graph_from_paths(request: RunGraphFromPathsRequest) -> RunGraphResponse {
    run_graph_from_paths_internal(request, None, DEFAULT_PROCESS_DRIVER_POLICY)
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
    )
}

#[allow(clippy::arc_with_non_send_sync)]
fn run_graph_from_paths_internal(
    request: RunGraphFromPathsRequest,
    runtime_surfaces: Option<RuntimeSurfaces>,
    process_policy: ProcessDriverPolicy,
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
    let runner = HostedRunner::new(
        GraphId::new(graph_id),
        Constraints::default(),
        runtime,
        runtime_provenance,
        adapter_setup.adapter_config,
        egress_config,
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
        adapter_setup.adapter_provides,
    );
    let runner = HostedRunner::new(
        GraphId::new(bundle.graph_id.as_str().to_string()),
        bundle.config.clone(),
        runtime,
        runtime_provenance.clone(),
        adapter_setup.adapter_config,
        None,
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
    run_graph_with_policy(request, DEFAULT_PROCESS_DRIVER_POLICY)
}

fn run_graph_with_policy(
    request: RunGraphRequest,
    process_policy: ProcessDriverPolicy,
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

    let execution = match driver {
        DriverConfig::Fixture { path } => run_fixture_driver(path, adapter_bound, runner)?,
        DriverConfig::Process { command } => run_process_driver(command, runner, process_policy)?,
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
) -> Result<DriverExecution, HostRunError> {
    let fixture_items = fixture::parse_fixture(&fixture_path)
        .map_err(|err| HostRunError::InvalidInput(format!("failed to parse fixture: {err}")))?;

    let mut episodes: Vec<(String, usize)> = Vec::new();
    let mut current_episode: Option<usize> = None;
    let mut event_counter = 0usize;
    let mut seen_fixture_event_ids = HashSet::new();

    for item in fixture_items {
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
                        let index = current_episode.expect("episode index set");
                        episodes[index].1 += 1;
                    }
                    Err(crate::HostedStepError::EgressDispatchFailure { .. }) => {
                        let index = current_episode.expect("episode index set");
                        episodes[index].1 += 1;
                        return Ok(DriverExecution {
                            runner,
                            event_count: event_counter,
                            episode_event_counts: episodes,
                            terminal: DriverTerminal::Interrupted(InterruptionReason::DriverIo),
                        });
                    }
                    Err(err) => {
                        return Err(HostRunError::StepFailed(format!("host step failed: {err}")));
                    }
                }
            }
        }
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

    Ok(DriverExecution {
        runner,
        event_count: event_counter,
        episode_event_counts: episodes,
        terminal: DriverTerminal::Completed,
    })
}

fn run_process_driver(
    command: Vec<String>,
    runner: HostedRunner,
    process_policy: ProcessDriverPolicy,
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
    let mut episodes = Vec::new();
    let mut runner = runner;

    loop {
        // Protocol truth decides what completion means; host policy only bounds how long we
        // wait for the driver to reveal that truth.
        let observation = recv_process_stream_observation(
            &stdout_rx,
            (!hello_received).then_some(process_policy.startup_grace),
        )
        .map_err(|failure| {
            let detail = abort_process_child(&mut child, stderr_handle.take());
            match failure {
                ProcessDriverReceiveFailure::Timeout => {
                    let suffix = if detail.is_empty() {
                        String::new()
                    } else {
                        format!(" ({detail})")
                    };
                    HostRunError::DriverStart(format!(
                        "process driver {command_display} did not emit a protocol frame within {}ms before startup completed{}",
                        process_policy.startup_grace.as_millis(),
                        suffix
                    ))
                }
                ProcessDriverReceiveFailure::Disconnected => {
                    if detail.is_empty() {
                        HostRunError::DriverIo(format!(
                            "receive process driver stdout for {command_display}: stdout reader disconnected unexpectedly"
                        ))
                    } else {
                        HostRunError::DriverIo(format!(
                            "receive process driver stdout for {command_display}: stdout reader disconnected unexpectedly ({detail})"
                        ))
                    }
                }
            }
        })?;

        let message = match observation {
            ProcessDriverStreamObservation::Line(line) => {
                let trimmed = line.trim_end_matches(['\r', '\n']);
                match serde_json::from_str::<ProcessDriverMessage>(trimmed) {
                    Ok(message) => message,
                    Err(err) => {
                        let _detail = abort_process_child(&mut child, stderr_handle.take());
                        if event_counter == 0 {
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

                if event_counter == 0 {
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
                        if event_counter == 0 {
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
                        if event_counter == 0 {
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
                if event_counter == 0 {
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
                    if episodes.is_empty() {
                        episodes.push(("E1".to_string(), 0));
                    }
                    episodes[0].1 += 1;
                }
                Err(crate::HostedStepError::EgressDispatchFailure { .. }) => {
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
                        terminal: DriverTerminal::Interrupted(InterruptionReason::DriverIo),
                    });
                }
                Err(err) => {
                    let _detail = abort_process_child(&mut child, stderr_handle.take());
                    return Err(HostRunError::StepFailed(format!("host step failed: {err}")));
                }
            },
            ProcessDriverMessage::End => {
                if event_counter == 0 {
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
    runner: HostedRunner,
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

    let (bundle, mut egress_runtime) = runner.into_capture_bundle_and_egress();
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

    if let Some(runtime) = egress_runtime.as_mut() {
        runtime
            .shutdown_channels()
            .map_err(|err| HostRunError::DriverIo(format!("stop egress channels: {err}")))?;
    }

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
    use ergo_adapter::ExternalEventKind;
    use ergo_runtime::catalog::CatalogBuilder;
    use ergo_runtime::common::{Value, ValueType};
    use ergo_runtime::runtime::ExecutionContext;
    use ergo_runtime::source::{
        Cadence as SourceCadence, ExecutionSpec as SourceExecutionSpec,
        OutputSpec as SourceOutputSpec, SourceKind, SourcePrimitive, SourcePrimitiveManifest,
        SourceRequires, StateSpec as SourceStateSpec,
    };
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct InjectedNumberSource {
        manifest: SourcePrimitiveManifest,
        output: f64,
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
            HashMap::from([("value".to_string(), Value::Number(self.output))])
        }
    }

    fn build_injected_runtime_surfaces(output: f64) -> RuntimeSurfaces {
        let mut builder = CatalogBuilder::new();
        builder.add_source(Box::new(InjectedNumberSource::new(output)));
        let (registries, catalog) = builder
            .build()
            .expect("injected runtime surfaces should build");
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
        }
    }

    fn run_graph_from_paths_with_process_policy(
        request: RunGraphFromPathsRequest,
        process_policy: ProcessDriverPolicy,
    ) -> RunGraphResponse {
        run_graph_from_paths_internal(request, None, process_policy)
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
