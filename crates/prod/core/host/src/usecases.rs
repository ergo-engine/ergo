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
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{
    decision_counts, replay_bundle_strict, HostedAdapterConfig, HostedEvent, HostedReplayError,
    HostedRunner,
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
                if !action.manifest().effects.writes.is_empty() {
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
    MissingIngressSource,
    AdapterRequired(AdapterDependencySummary),
    InvalidInput(String),
    StepFailed(String),
    Io(String),
}

impl std::fmt::Display for HostRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingIngressSource => write!(f, "canonical run requires an explicit ingress source"),
            Self::AdapterRequired(summary) => write!(
                f,
                "graph requires adapter capabilities but no adapter was provided (required context nodes: [{}], write nodes: [{}])",
                summary.required_context_nodes.join(", "),
                summary.write_nodes.join(", ")
            ),
            Self::InvalidInput(message) | Self::StepFailed(message) | Self::Io(message) => write!(f, "{message}"),
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

pub struct RunGraphRequest {
    pub graph_path: PathBuf,
    pub fixture_path: Option<PathBuf>,
    pub capture_output: Option<PathBuf>,
    pub pretty_capture: bool,
    pub adapter_bound: bool,
    pub dependency_summary: AdapterDependencySummary,
    pub runner: HostedRunner,
}

pub struct RunGraphFromPathsRequest {
    pub graph_path: PathBuf,
    pub cluster_paths: Vec<PathBuf>,
    pub fixture_path: PathBuf,
    pub adapter_path: Option<PathBuf>,
    pub capture_output: Option<PathBuf>,
    pub pretty_capture: bool,
}

#[derive(Debug)]
pub struct RunGraphResult {
    pub capture_path: PathBuf,
    pub episodes: usize,
    pub events: usize,
    pub invoked: usize,
    pub deferred: usize,
    pub episode_event_counts: Vec<(String, usize)>,
}

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

fn parse_adapter_manifest(path: &Path) -> Result<AdapterManifest, String> {
    let data = fs::read_to_string(path)
        .map_err(|err| format!("read adapter manifest '{}': {err}", path.display()))?;
    let value = serde_yaml::from_str::<serde_json::Value>(&data)
        .map_err(|err| format!("parse adapter manifest '{}': {err}", path.display()))?;
    serde_json::from_value::<AdapterManifest>(value)
        .map_err(|err| format!("decode adapter manifest '{}': {err}", path.display()))
}

fn prepare_graph_runtime(
    graph_path: &Path,
    cluster_paths: &[PathBuf],
) -> Result<PreparedGraphRuntime, String> {
    let root = ergo_loader::parse_graph_file(graph_path).map_err(|err| err.to_string())?;
    let clusters = ergo_loader::load_cluster_tree(graph_path, &root, cluster_paths)
        .map_err(|err| err.to_string())?;
    let loader = PreloadedClusterLoader::new(clusters);

    let catalog = build_core_catalog();
    let registries = core_registries().map_err(|err| format!("core registries: {err:?}"))?;
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
pub fn run_graph_from_paths(
    request: RunGraphFromPathsRequest,
) -> Result<RunGraphResult, HostRunError> {
    let RunGraphFromPathsRequest {
        graph_path,
        cluster_paths,
        fixture_path,
        adapter_path,
        capture_output,
        pretty_capture,
    } = request;

    let prepared =
        prepare_graph_runtime(&graph_path, &cluster_paths).map_err(HostRunError::InvalidInput)?;
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
    )
    .map_err(|err| {
        HostRunError::StepFailed(format!("failed to initialize canonical host runner: {err}"))
    })?;

    let fixture_path = if fixture_path.as_os_str().is_empty() {
        None
    } else {
        Some(fixture_path)
    };

    run_graph(RunGraphRequest {
        graph_path,
        fixture_path,
        capture_output,
        pretty_capture,
        adapter_bound: adapter_setup.adapter_bound,
        dependency_summary,
        runner,
    })
}

/// Canonical replay API for clients. Host owns capture load, graph loading, adapter composition, and runner setup.
pub fn replay_graph_from_paths(
    request: ReplayGraphFromPathsRequest,
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

    let prepared =
        prepare_graph_runtime(&graph_path, &cluster_paths).map_err(host_replay_setup_error)?;
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
pub fn run_graph(request: RunGraphRequest) -> Result<RunGraphResult, HostRunError> {
    let RunGraphRequest {
        graph_path,
        fixture_path,
        capture_output,
        pretty_capture,
        adapter_bound,
        dependency_summary,
        runner,
    } = request;

    let Some(fixture_path) = fixture_path else {
        return Err(HostRunError::MissingIngressSource);
    };

    if !adapter_bound && dependency_summary.requires_adapter {
        return Err(HostRunError::AdapterRequired(dependency_summary));
    }

    let fixture_items = fixture::parse_fixture(&fixture_path)
        .map_err(|err| HostRunError::InvalidInput(format!("failed to parse fixture: {err}")))?;

    let mut runner = runner;
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

                runner
                    .step(hosted_event)
                    .map_err(|err| HostRunError::StepFailed(format!("host step failed: {err}")))?;
                let index = current_episode.expect("episode index set");
                episodes[index].1 += 1;
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

    Ok(RunGraphResult {
        capture_path,
        episodes: episodes.len(),
        events: event_counter,
        invoked,
        deferred,
        episode_event_counts: episodes,
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
        fixture_path: Some(request.fixture_path),
        capture_output: Some(request.capture_output.clone()),
        pretty_capture: request.pretty_capture,
        adapter_bound: false,
        dependency_summary: AdapterDependencySummary::default(),
        runner: request.runner,
    })?;
    Ok(RunFixtureResult {
        capture_path: outcome.capture_path,
        episodes: outcome.episodes,
        events: outcome.events,
        episode_event_counts: outcome.episode_event_counts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn write_temp_file(
        base: &Path,
        name: &str,
        contents: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let path = base.join(name);
        fs::write(&path, contents)?;
        Ok(path)
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

        let result = run_graph_from_paths(RunGraphFromPathsRequest {
            graph_path: graph.clone(),
            cluster_paths: Vec::new(),
            fixture_path: fixture,
            adapter_path: None,
            capture_output: Some(capture.clone()),
            pretty_capture: false,
        })?;

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

        let _ = run_graph_from_paths(RunGraphFromPathsRequest {
            graph_path: graph.clone(),
            cluster_paths: Vec::new(),
            fixture_path: fixture,
            adapter_path: None,
            capture_output: Some(capture.clone()),
            pretty_capture: false,
        })?;

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
}
