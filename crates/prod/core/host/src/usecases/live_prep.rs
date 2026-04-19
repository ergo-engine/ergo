//! live_prep
//!
//! Purpose:
//! - Host-owned preparation seam for canonical validation, replay setup, and
//!   manual hosted-runner preparation.
//! - Bridges loader-prepared graph assets into runtime expansion, adapter
//!   setup, hosted-runner configuration validation, and replay/manual-runner
//!   setup for both path-backed and in-memory callers.
//!
//! Owns:
//! - Path-backed and asset-backed graph/runtime preparation through the shared
//!   `PreparedGraphRuntime` and `CanonicalAdapterSetup` phases.
//! - `RuntimeSurfaces` injection for advanced callers that prebuild runtime
//!   registries/catalogs.
//! - Host-owned finalization staging through
//!   `HostedRunnerFinalizeFailure::{PendingAcks, StopEgress}`.
//! - Canonical prep-only host APIs for validation, replay setup, and manual
//!   runner preparation.
//!
//! Does not own:
//! - Graph discovery/decode authority in `ergo_loader`.
//! - Replay doctrine or strict comparison semantics in `replay.rs` /
//!   `ergo_supervisor`.
//! - Hosted runner step/egress execution semantics in `runner.rs`.
//! - Driver execution orchestration in `live_run.rs`.
//!
//! Connects to:
//! - `ergo_loader` for graph asset loading and cluster discovery.
//! - runtime expansion/provenance, adapter validation/composition, and hosted
//!   runner configuration validation.
//! - `live_run.rs`, which consumes validated prep/finalization stages during
//!   canonical execution.
//! - CLI and SDK, which import the public validation/replay/manual-runner
//!   entrypoints re-exported through `usecases.rs` and `lib.rs`.
//!
//! Safety notes:
//! - `HostedRunnerFinalizeFailure` is load-bearing even though both branches
//!   currently preserve typed `HostedStepError` at the public run boundary:
//!   it keeps the finalization phases explicit so future host decisions do not
//!   have to rediscover where pending-ack versus stop-egress failures split.
//!   This module owns the 3-step staged orchestration in the capture finalization
//!   pipeline (check → stop egress → extract bundle). The runner-owned gate
//!   (`CaptureFinalizationState`) lives in `runner.rs`; the driver-level
//!   validation and summary DTO live in `live_run.rs`.
//! - Replay representability checks intentionally treat `set_context` as the
//!   host-internal handler effect path and exclude it from replay-owned
//!   external kinds.
//! - Loader, expansion, adapter-setup, hosted-runner validation, and replay
//!   capture read/parse failures now cross this seam as typed host setup
//!   errors rather than flattened strings.
//! - Replay representability failures still remain host-owned setup checks
//!   because they depend on host handler ownership doctrine, not just kernel
//!   replay semantics.
//! - Replay prep now routes both path-backed and asset-backed callers through
//!   one shared replay-request preparation pipeline and consumes the runner's
//!   host-internal handler-kind authority instead of repeating `set_context`
//!   knowledge locally.

#![allow(clippy::arc_with_non_send_sync)]

// Shared standard-library and external-crate prelude for usecase submodules.
use super::shared::*;
// Usecases-owned types used by this module.
use super::{
    scan_adapter_dependencies, summarize_expand_error, validate_adapter_composition,
    AdapterDependencySummary, AdapterInput, DriverConfig, HostAdapterSetupError,
    HostGraphPreparationError, HostReplayError, HostReplaySetupError, HostRunError, HostSetupError,
    LivePrepOptions, PrepareHostedRunnerFromPathsRequest, ReplayGraphFromAssetsRequest,
    ReplayGraphFromPathsRequest, ReplayGraphRequest, ReplayGraphResult, RunGraphFromAssetsRequest,
    RunGraphFromPathsRequest, RuntimeSurfaces, SessionIntent,
};
use ergo_adapter::ReportingRuntimeHandle;
// Sibling module types.
use super::live_run::{replay_graph, validate_driver_input};
// Crate-internal helpers.
use crate::diagnostics::emit_warnings_to_stderr;
use crate::host::BufferingRuntimeInvoker;
use crate::runner::host_internal_handler_kinds;

pub(super) struct PreparedLiveRunnerSetup {
    adapter_bound: bool,
    dependency_summary: AdapterDependencySummary,
    runner: HostedRunner,
}

impl PreparedLiveRunnerSetup {
    pub(super) fn adapter_bound(&self) -> bool {
        self.adapter_bound
    }

    pub(super) fn dependency_summary(&self) -> &AdapterDependencySummary {
        &self.dependency_summary
    }

    /// Consume the setup, yielding the hosted runner for the run phase.
    pub(super) fn into_runner(self) -> HostedRunner {
        self.runner
    }
}

struct ValidatedLiveRunnerSetup {
    graph_id: GraphId,
    runtime_provenance: String,
    runtime: BufferingRuntimeInvoker,
    adapter_config: Option<HostedAdapterConfig>,
    egress_config: Option<EgressConfig>,
    egress_provenance: Option<String>,
    adapter_bound: bool,
    dependency_summary: AdapterDependencySummary,
}

fn map_live_prep_loader_result(
    result: Result<ergo_loader::PreparedGraphAssets, ergo_loader::LoaderError>,
) -> Result<ergo_loader::PreparedGraphAssets, HostRunError> {
    result.map_err(|err| HostRunError::Setup(HostSetupError::LoadGraphAssets(err)))
}

pub fn load_graph_assets_from_paths(
    graph_path: &Path,
    cluster_paths: &[PathBuf],
) -> Result<ergo_loader::PreparedGraphAssets, HostRunError> {
    map_live_prep_loader_result(ergo_loader::load_graph_assets_from_paths(
        graph_path,
        cluster_paths,
    ))
}

pub fn load_graph_assets_from_memory(
    root_source_id: &str,
    sources: &[ergo_loader::InMemorySourceInput],
    search_roots: &[String],
) -> Result<ergo_loader::PreparedGraphAssets, HostRunError> {
    map_live_prep_loader_result(ergo_loader::load_graph_assets_from_memory(
        root_source_id,
        sources,
        search_roots,
    ))
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

pub(super) struct PreparedGraphRuntime {
    pub(super) graph_id: String,
    pub(super) runtime_provenance: String,
    pub(super) expanded: ExpandedGraph,
    pub(super) catalog: Arc<CorePrimitiveCatalog>,
    pub(super) registries: Arc<CoreRegistries>,
}

pub(super) struct CanonicalAdapterSetup {
    pub(super) adapter_bound: bool,
    pub(super) adapter_provides: AdapterProvides,
    pub(super) adapter_config: Option<HostedAdapterConfig>,
    pub(super) expected_adapter_provenance: String,
}

fn parse_adapter_manifest(path: &Path) -> Result<AdapterManifest, HostAdapterSetupError> {
    let data = fs::read_to_string(path).map_err(|source| HostAdapterSetupError::ManifestRead {
        path: path.to_path_buf(),
        source,
    })?;
    let source_label = path.display().to_string();
    let value = serde_yaml::from_str::<serde_json::Value>(&data).map_err(|source| {
        HostAdapterSetupError::ManifestParse {
            source_label: source_label.clone(),
            source,
        }
    })?;
    serde_json::from_value::<AdapterManifest>(value).map_err(|source| {
        HostAdapterSetupError::ManifestDecode {
            source_label,
            source,
        }
    })
}

fn parse_adapter_manifest_text(
    source_label: &str,
    data: &str,
) -> Result<AdapterManifest, HostAdapterSetupError> {
    if source_label.is_empty() {
        return Err(HostAdapterSetupError::ManifestSourceLabelEmpty);
    }
    let value = serde_yaml::from_str::<serde_json::Value>(data).map_err(|source| {
        HostAdapterSetupError::ManifestParse {
            source_label: source_label.to_string(),
            source,
        }
    })?;
    serde_json::from_value::<AdapterManifest>(value).map_err(|source| {
        HostAdapterSetupError::ManifestDecode {
            source_label: source_label.to_string(),
            source,
        }
    })
}

fn materialize_runtime_surfaces(
    runtime_surfaces: Option<RuntimeSurfaces>,
) -> Result<(Arc<CorePrimitiveCatalog>, Arc<CoreRegistries>), HostGraphPreparationError> {
    match runtime_surfaces {
        Some(runtime_surfaces) => Ok(runtime_surfaces.into_shared_parts()),
        None => {
            let catalog = build_core_catalog();
            let registries =
                core_registries().map_err(HostGraphPreparationError::CoreRegistries)?;
            Ok((Arc::new(catalog), Arc::new(registries)))
        }
    }
}

#[cfg(test)]
pub(super) fn prepare_graph_runtime(
    graph_path: &Path,
    cluster_paths: &[PathBuf],
    runtime_surfaces: Option<RuntimeSurfaces>,
) -> Result<PreparedGraphRuntime, HostSetupError> {
    let assets = ergo_loader::load_graph_assets_from_paths(graph_path, cluster_paths)
        .map_err(HostSetupError::LoadGraphAssets)?;
    prepare_graph_runtime_from_assets(&assets, runtime_surfaces)
        .map_err(HostSetupError::GraphPreparation)
}

fn prepare_graph_runtime_from_assets(
    assets: &ergo_loader::PreparedGraphAssets,
    runtime_surfaces: Option<RuntimeSurfaces>,
) -> Result<PreparedGraphRuntime, HostGraphPreparationError> {
    let loader = PreloadedClusterLoader::new(assets.clusters().clone());
    let (catalog, registries) = materialize_runtime_surfaces(runtime_surfaces)?;
    let expanded = expand(assets.root(), &loader, catalog.as_ref()).map_err(|err| {
        HostGraphPreparationError::Expansion(summarize_expand_error(
            &err,
            assets.cluster_diagnostic_labels(),
        ))
    })?;
    let runtime_provenance = compute_runtime_provenance(
        RuntimeProvenanceScheme::Rpv1,
        &assets.root().id,
        &expanded,
        catalog.as_ref(),
    )
    .map_err(HostGraphPreparationError::RuntimeProvenance)?;

    Ok(PreparedGraphRuntime {
        graph_id: assets.root().id.clone(),
        runtime_provenance,
        expanded,
        catalog,
        registries,
    })
}

pub(super) fn prepare_adapter_setup(
    adapter: Option<&AdapterInput>,
    prepared: &PreparedGraphRuntime,
) -> Result<CanonicalAdapterSetup, HostAdapterSetupError> {
    let manifest = match adapter {
        None => {
            return Ok(CanonicalAdapterSetup {
                adapter_bound: false,
                adapter_provides: AdapterProvides::default(),
                adapter_config: None,
                expected_adapter_provenance: NO_ADAPTER_PROVENANCE.to_string(),
            });
        }
        Some(AdapterInput::Path(path)) => parse_adapter_manifest(path)?,
        Some(AdapterInput::Text {
            content,
            source_label,
        }) => parse_adapter_manifest_text(source_label, content)?,
        Some(AdapterInput::Manifest(manifest)) => manifest.clone(),
    };

    ergo_adapter::validate_adapter(&manifest).map_err(HostAdapterSetupError::Validation)?;
    let provides = AdapterProvides::from_manifest(&manifest);
    validate_adapter_composition(
        &prepared.expanded,
        &prepared.catalog,
        &prepared.registries,
        &provides,
    )
    .map_err(HostAdapterSetupError::Composition)?;
    let adapter_provenance = adapter_fingerprint(&manifest);
    let adapter_config = HostedAdapterConfig::new(provides.clone(), adapter_provenance.clone())
        .map_err(HostAdapterSetupError::BinderCompile)?;

    Ok(CanonicalAdapterSetup {
        adapter_bound: true,
        adapter_provides: provides.clone(),
        adapter_config: Some(adapter_config),
        expected_adapter_provenance: adapter_provenance,
    })
}

pub(super) fn ensure_adapter_requirement_satisfied(
    adapter_bound: bool,
    dependency_summary: &AdapterDependencySummary,
) -> Result<(), HostRunError> {
    if !adapter_bound && dependency_summary.requires_adapter {
        return Err(HostRunError::AdapterRequired(dependency_summary.clone()));
    }

    Ok(())
}

/// Derives `SessionIntent` from a `DriverConfig` variant.
pub(super) fn session_intent_from_driver(driver: &DriverConfig) -> SessionIntent {
    match driver {
        DriverConfig::Process { .. } => SessionIntent::Production,
        DriverConfig::Fixture { .. } | DriverConfig::FixtureItems { .. } => SessionIntent::Fixture,
    }
}

/// Production closure gate: rejects sessions with `SessionIntent::Production`
/// when no adapter contract is bound.
///
/// This gate is independent of the graph-dependency gate (`ensure_adapter_requirement_satisfied`).
/// The graph-dependency gate checks whether the graph structurally needs an adapter
/// (e.g. `required: true` context keys, action writes/intents).  This gate checks
/// whether the *execution path* demands a contract regardless of graph structure.
pub(super) fn ensure_production_adapter_bound(
    adapter_bound: bool,
    session_intent: SessionIntent,
) -> Result<(), HostRunError> {
    if !adapter_bound && session_intent == SessionIntent::Production {
        return Err(HostRunError::ProductionRequiresAdapter);
    }
    Ok(())
}

pub(super) fn start_live_runner_egress(runner: &mut HostedRunner) -> Result<(), HostRunError> {
    runner
        .start_egress_channels()
        .map_err(|err| HostRunError::Setup(HostSetupError::StartEgress(err)))
}

fn hosted_runner_setup_error(err: HostedStepError) -> HostRunError {
    HostRunError::Setup(HostSetupError::HostedRunnerValidation(err))
}

fn validate_live_runner_setup_from_assets(
    assets: &ergo_loader::PreparedGraphAssets,
    options: &LivePrepOptions,
    runtime_surfaces: Option<RuntimeSurfaces>,
) -> Result<ValidatedLiveRunnerSetup, HostRunError> {
    let prepared = prepare_graph_runtime_from_assets(assets, runtime_surfaces)
        .map_err(|err| HostRunError::Setup(HostSetupError::GraphPreparation(err)))?;
    let dependency_summary =
        scan_adapter_dependencies(&prepared.expanded, &prepared.catalog, &prepared.registries)
            .map_err(|err| HostRunError::Setup(HostSetupError::DependencyScan(err)))?;
    let adapter_setup = prepare_adapter_setup(options.adapter.as_ref(), &prepared)
        .map_err(|err| HostRunError::Setup(HostSetupError::AdapterSetup(err)))?;

    ensure_adapter_requirement_satisfied(adapter_setup.adapter_bound, &dependency_summary)?;

    let PreparedGraphRuntime {
        graph_id,
        runtime_provenance,
        expanded,
        catalog,
        registries,
    } = prepared;

    let runtime = ReportingRuntimeHandle::new(
        Arc::new(expanded),
        catalog,
        registries,
        adapter_setup.adapter_provides,
    );
    let egress_provenance = options
        .egress_config
        .as_ref()
        .map(compute_egress_provenance);
    let graph_emittable_effect_kinds = runtime.graph_emittable_effect_kinds();
    let warnings = validate_hosted_runner_configuration(
        adapter_setup.adapter_config.as_ref(),
        options.egress_config.as_ref(),
        egress_provenance.as_deref(),
        &HashSet::new(),
        &graph_emittable_effect_kinds,
    )
    .map_err(hosted_runner_setup_error)?;
    emit_warnings_to_stderr(&warnings);

    Ok(ValidatedLiveRunnerSetup {
        graph_id: GraphId::new(graph_id),
        runtime_provenance,
        runtime: BufferingRuntimeInvoker::new(runtime),
        adapter_config: adapter_setup.adapter_config,
        egress_config: options.egress_config.clone(),
        egress_provenance,
        adapter_bound: adapter_setup.adapter_bound,
        dependency_summary,
    })
}

fn build_live_runner_from_validated(
    validated: ValidatedLiveRunnerSetup,
) -> PreparedLiveRunnerSetup {
    let ValidatedLiveRunnerSetup {
        graph_id,
        runtime_provenance,
        runtime,
        adapter_config,
        egress_config,
        egress_provenance,
        adapter_bound,
        dependency_summary,
    } = validated;
    let runner = HostedRunner::new_validated(
        graph_id,
        Constraints::default(),
        runtime,
        runtime_provenance,
        adapter_config,
        egress_config,
        egress_provenance,
        HashSet::new(),
    );

    PreparedLiveRunnerSetup {
        adapter_bound,
        dependency_summary,
        runner,
    }
}

pub(super) fn prepare_live_runner_setup_from_assets(
    assets: &ergo_loader::PreparedGraphAssets,
    options: &LivePrepOptions,
    runtime_surfaces: Option<RuntimeSurfaces>,
) -> Result<PreparedLiveRunnerSetup, HostRunError> {
    let validated = validate_live_runner_setup_from_assets(assets, options, runtime_surfaces)?;
    Ok(build_live_runner_from_validated(validated))
}

fn host_replay_setup_error(err: HostReplaySetupError) -> HostReplayError {
    HostReplayError::Setup(err)
}

#[derive(Debug)]
pub(super) enum HostedRunnerFinalizeFailure {
    PendingAcks(HostedStepError),
    StopEgress(HostedStepError),
}

pub(super) fn finalize_hosted_runner_capture_with_stage(
    mut runner: HostedRunner,
    host_stop_requested: bool,
) -> Result<CaptureBundle, HostedRunnerFinalizeFailure> {
    runner
        .ensure_capture_finalizable()
        .map_err(HostedRunnerFinalizeFailure::PendingAcks)?;

    runner
        .ensure_no_pending_egress_acks(host_stop_requested)
        .map_err(HostedRunnerFinalizeFailure::PendingAcks)?;

    // Freeze egress lifecycle before capture finalization so late channel activity
    // cannot alter dispatch truth after artifact write.
    runner
        .stop_egress_channels()
        .map_err(HostedRunnerFinalizeFailure::StopEgress)?;

    Ok(runner.into_capture_bundle())
}

pub fn finalize_hosted_runner_capture(
    runner: HostedRunner,
    host_stop_requested: bool,
) -> Result<CaptureBundle, HostedStepError> {
    finalize_hosted_runner_capture_with_stage(runner, host_stop_requested).map_err(|failure| {
        match failure {
            HostedRunnerFinalizeFailure::PendingAcks(err)
            | HostedRunnerFinalizeFailure::StopEgress(err) => err,
        }
    })
}

fn captured_external_effect_kinds(bundle: &CaptureBundle) -> HashSet<String> {
    let handler_kinds = host_internal_handler_kinds();
    bundle
        .decisions
        .iter()
        .flat_map(|decision| decision.effects.iter())
        .filter_map(|effect| {
            let kind = effect.effect.kind.as_str();
            (!handler_kinds.contains(kind)).then(|| kind.to_string())
        })
        .collect()
}

pub(super) fn replay_owned_external_kinds(
    runtime: &ReportingRuntimeHandle,
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

pub fn validate_graph_from_paths(
    request: PrepareHostedRunnerFromPathsRequest,
) -> Result<(), HostRunError> {
    validate_graph_from_paths_internal(request, None)
}

/// Advanced validation API for callers that prebuild runtime surfaces before
/// invoking the canonical host validation path.
pub fn validate_graph_from_paths_with_surfaces(
    request: PrepareHostedRunnerFromPathsRequest,
    runtime_surfaces: RuntimeSurfaces,
) -> Result<(), HostRunError> {
    validate_graph_from_paths_internal(request, Some(runtime_surfaces))
}

/// Lower-level validation API over preloaded graph assets. Validation stops
/// after runtime/configuration validation and does not construct or start a
/// hosted session.
pub fn validate_graph(
    assets: &ergo_loader::PreparedGraphAssets,
    options: &LivePrepOptions,
) -> Result<(), HostRunError> {
    validate_graph_internal(assets, options, None)
}

/// Advanced lower-level validation API over preloaded graph assets with
/// injected runtime surfaces.
pub fn validate_graph_with_surfaces(
    assets: &ergo_loader::PreparedGraphAssets,
    options: &LivePrepOptions,
    runtime_surfaces: RuntimeSurfaces,
) -> Result<(), HostRunError> {
    validate_graph_internal(assets, options, Some(runtime_surfaces))
}

/// Canonical validation API over the full path-backed run request, including
/// driver preflight.
pub fn validate_run_graph_from_paths(
    request: RunGraphFromPathsRequest,
) -> Result<(), HostRunError> {
    validate_run_graph_from_paths_internal(request, None)
}

/// Advanced validation API over the full path-backed run request with injected
/// runtime surfaces.
pub fn validate_run_graph_from_paths_with_surfaces(
    request: RunGraphFromPathsRequest,
    runtime_surfaces: RuntimeSurfaces,
) -> Result<(), HostRunError> {
    validate_run_graph_from_paths_internal(request, Some(runtime_surfaces))
}

/// Lower-level validation API over the full in-memory run request, including
/// driver preflight.
pub fn validate_run_graph_from_assets(
    request: RunGraphFromAssetsRequest,
) -> Result<(), HostRunError> {
    validate_run_graph_from_assets_internal(request, None)
}

/// Advanced lower-level validation API over the full in-memory run request
/// with injected runtime surfaces.
pub fn validate_run_graph_from_assets_with_surfaces(
    request: RunGraphFromAssetsRequest,
    runtime_surfaces: RuntimeSurfaces,
) -> Result<(), HostRunError> {
    validate_run_graph_from_assets_internal(request, Some(runtime_surfaces))
}

fn validate_graph_internal(
    assets: &ergo_loader::PreparedGraphAssets,
    options: &LivePrepOptions,
    runtime_surfaces: Option<RuntimeSurfaces>,
) -> Result<(), HostRunError> {
    let validated = validate_live_runner_setup_from_assets(assets, options, runtime_surfaces)?;
    ensure_production_adapter_bound(validated.adapter_bound, options.session_intent)?;
    Ok(())
}

fn validate_graph_from_paths_internal(
    request: PrepareHostedRunnerFromPathsRequest,
    runtime_surfaces: Option<RuntimeSurfaces>,
) -> Result<(), HostRunError> {
    let PrepareHostedRunnerFromPathsRequest {
        graph_path,
        cluster_paths,
        adapter_path,
        egress_config,
        session_intent,
    } = request;
    let assets = load_graph_assets_from_paths(&graph_path, &cluster_paths)?;
    let options = LivePrepOptions {
        adapter: adapter_path.map(AdapterInput::Path),
        egress_config,
        session_intent,
    };
    validate_graph_internal(&assets, &options, runtime_surfaces)
}

fn validate_run_graph_from_paths_internal(
    request: RunGraphFromPathsRequest,
    runtime_surfaces: Option<RuntimeSurfaces>,
) -> Result<(), HostRunError> {
    let RunGraphFromPathsRequest {
        graph_path,
        cluster_paths,
        driver,
        adapter_path,
        egress_config,
        ..
    } = request;
    let assets = load_graph_assets_from_paths(&graph_path, &cluster_paths)?;
    let options = LivePrepOptions {
        adapter: adapter_path.map(AdapterInput::Path),
        egress_config,
        session_intent: session_intent_from_driver(&driver),
    };
    validate_run_graph_internal(&assets, &options, &driver, runtime_surfaces)
}

fn validate_run_graph_from_assets_internal(
    request: RunGraphFromAssetsRequest,
    runtime_surfaces: Option<RuntimeSurfaces>,
) -> Result<(), HostRunError> {
    let RunGraphFromAssetsRequest {
        assets,
        prep,
        driver,
        ..
    } = request;
    validate_run_graph_internal(&assets, &prep, &driver, runtime_surfaces)
}

fn validate_run_graph_internal(
    assets: &ergo_loader::PreparedGraphAssets,
    options: &LivePrepOptions,
    driver: &DriverConfig,
    runtime_surfaces: Option<RuntimeSurfaces>,
) -> Result<(), HostRunError> {
    let validated = validate_live_runner_setup_from_assets(assets, options, runtime_surfaces)?;
    ensure_production_adapter_bound(validated.adapter_bound, session_intent_from_driver(driver))?;
    validate_driver_input(driver, validated.adapter_bound).map(|_| ())
}

fn load_capture_bundle_from_path(capture_path: &Path) -> Result<CaptureBundle, HostReplayError> {
    let data = fs::read_to_string(capture_path).map_err(|source| {
        host_replay_setup_error(HostReplaySetupError::CaptureRead {
            path: capture_path.to_path_buf(),
            source,
        })
    })?;
    serde_json::from_str::<CaptureBundle>(&data).map_err(|source| {
        host_replay_setup_error(HostReplaySetupError::CaptureParse {
            path: capture_path.to_path_buf(),
            source,
        })
    })
}

fn prepare_replay_request_from_assets(
    bundle: CaptureBundle,
    assets: &ergo_loader::PreparedGraphAssets,
    prep: &LivePrepOptions,
    runtime_surfaces: Option<RuntimeSurfaces>,
) -> Result<ReplayGraphRequest, HostReplayError> {
    if prep.egress_config.is_some() {
        return Err(host_replay_setup_error(
            HostReplaySetupError::LiveEgressConfigurationNotAllowed,
        ));
    }

    let prepared = prepare_graph_runtime_from_assets(assets, runtime_surfaces).map_err(|err| {
        host_replay_setup_error(HostReplaySetupError::Setup(
            HostSetupError::GraphPreparation(err),
        ))
    })?;
    if bundle.graph_id.as_str() != prepared.graph_id {
        return Err(HostReplayError::GraphIdMismatch {
            expected: prepared.graph_id,
            got: bundle.graph_id.as_str().to_string(),
        });
    }

    let adapter_setup = prepare_adapter_setup(prep.adapter.as_ref(), &prepared).map_err(|err| {
        host_replay_setup_error(HostReplaySetupError::Setup(HostSetupError::AdapterSetup(
            err,
        )))
    })?;
    let PreparedGraphRuntime {
        runtime_provenance,
        expanded,
        catalog,
        registries,
        ..
    } = prepared;

    let runtime = ReportingRuntimeHandle::new(
        Arc::new(expanded),
        catalog,
        registries,
        adapter_setup.adapter_provides.clone(),
    );
    let handler_kinds = host_internal_handler_kinds();
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
        BufferingRuntimeInvoker::new(runtime),
        runtime_provenance.clone(),
        adapter_setup.adapter_config,
        None,
        None,
        Some(replay_external_kinds),
    )
    .map_err(|err| {
        host_replay_setup_error(HostReplaySetupError::Setup(
            HostSetupError::HostedRunnerInitialization(err),
        ))
    })?;

    Ok(ReplayGraphRequest {
        bundle,
        runner,
        expected_adapter_provenance: adapter_setup.expected_adapter_provenance,
        expected_runtime_provenance: runtime_provenance,
    })
}

/// Canonical replay API for clients. Host owns capture load, graph loading, adapter composition, and runner setup.
// Allow non-Send/Sync in Arc: CoreRegistries and CorePrimitiveCatalog contain non-Send/Sync types.
pub fn replay_graph_from_paths(
    request: ReplayGraphFromPathsRequest,
) -> Result<ReplayGraphResult, HostReplayError> {
    replay_graph_from_paths_internal(request, None)
}

/// Advanced replay API for callers that prebuild runtime surfaces before invoking the canonical host path.
pub fn replay_graph_from_paths_with_surfaces(
    request: ReplayGraphFromPathsRequest,
    runtime_surfaces: RuntimeSurfaces,
) -> Result<ReplayGraphResult, HostReplayError> {
    replay_graph_from_paths_internal(request, Some(runtime_surfaces))
}

/// Lower-level canonical replay API over preloaded graph assets and an in-memory capture bundle.
pub fn replay_graph_from_assets(
    request: ReplayGraphFromAssetsRequest,
) -> Result<ReplayGraphResult, HostReplayError> {
    replay_graph_from_assets_internal(request, None)
}

/// Advanced lower-level replay API over preloaded graph assets with injected runtime surfaces.
pub fn replay_graph_from_assets_with_surfaces(
    request: ReplayGraphFromAssetsRequest,
    runtime_surfaces: RuntimeSurfaces,
) -> Result<ReplayGraphResult, HostReplayError> {
    replay_graph_from_assets_internal(request, Some(runtime_surfaces))
}

/// Canonical manual-step preparation API for clients. Host owns graph loading,
/// expansion, adapter preflight, runner setup, and eager egress startup.
pub fn prepare_hosted_runner_from_paths(
    request: PrepareHostedRunnerFromPathsRequest,
) -> Result<HostedRunner, HostRunError> {
    prepare_hosted_runner_from_paths_internal(request, None)
}

/// Advanced manual-step preparation API for callers that prebuild runtime
/// surfaces before invoking the canonical host setup path.
pub fn prepare_hosted_runner_from_paths_with_surfaces(
    request: PrepareHostedRunnerFromPathsRequest,
    runtime_surfaces: RuntimeSurfaces,
) -> Result<HostedRunner, HostRunError> {
    prepare_hosted_runner_from_paths_internal(request, Some(runtime_surfaces))
}

/// Lower-level manual-step preparation API over preloaded graph assets.
pub fn prepare_hosted_runner(
    assets: ergo_loader::PreparedGraphAssets,
    options: &LivePrepOptions,
) -> Result<HostedRunner, HostRunError> {
    prepare_hosted_runner_internal(assets, options, None)
}

/// Advanced lower-level manual-step preparation API over preloaded graph
/// assets with injected runtime surfaces.
pub fn prepare_hosted_runner_with_surfaces(
    assets: ergo_loader::PreparedGraphAssets,
    options: &LivePrepOptions,
    runtime_surfaces: RuntimeSurfaces,
) -> Result<HostedRunner, HostRunError> {
    prepare_hosted_runner_internal(assets, options, Some(runtime_surfaces))
}

fn prepare_hosted_runner_internal(
    assets: ergo_loader::PreparedGraphAssets,
    options: &LivePrepOptions,
    runtime_surfaces: Option<RuntimeSurfaces>,
) -> Result<HostedRunner, HostRunError> {
    let PreparedLiveRunnerSetup {
        adapter_bound,
        mut runner,
        ..
    } = prepare_live_runner_setup_from_assets(&assets, options, runtime_surfaces)?;
    ensure_production_adapter_bound(adapter_bound, options.session_intent)?;
    start_live_runner_egress(&mut runner)?;
    Ok(runner)
}

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
    let bundle = load_capture_bundle_from_path(&capture_path)?;
    let assets =
        ergo_loader::load_graph_assets_from_paths(&graph_path, &cluster_paths).map_err(|err| {
            host_replay_setup_error(HostReplaySetupError::Setup(
                HostSetupError::LoadGraphAssets(err),
            ))
        })?;
    let replay_request = prepare_replay_request_from_assets(
        bundle,
        &assets,
        &LivePrepOptions {
            adapter: adapter_path.map(AdapterInput::Path),
            egress_config: None,
            session_intent: SessionIntent::Fixture,
        },
        runtime_surfaces,
    )?;
    replay_graph(replay_request)
}

fn replay_graph_from_assets_internal(
    request: ReplayGraphFromAssetsRequest,
    runtime_surfaces: Option<RuntimeSurfaces>,
) -> Result<ReplayGraphResult, HostReplayError> {
    let ReplayGraphFromAssetsRequest {
        bundle,
        assets,
        prep,
    } = request;
    let replay_request =
        prepare_replay_request_from_assets(bundle, &assets, &prep, runtime_surfaces)?;
    replay_graph(replay_request)
}

fn prepare_hosted_runner_from_paths_internal(
    request: PrepareHostedRunnerFromPathsRequest,
    runtime_surfaces: Option<RuntimeSurfaces>,
) -> Result<HostedRunner, HostRunError> {
    let PrepareHostedRunnerFromPathsRequest {
        graph_path,
        cluster_paths,
        adapter_path,
        egress_config,
        session_intent,
    } = request;
    let assets = load_graph_assets_from_paths(&graph_path, &cluster_paths)?;
    let options = LivePrepOptions {
        adapter: adapter_path.map(AdapterInput::Path),
        egress_config,
        session_intent,
    };
    prepare_hosted_runner_internal(assets, &options, runtime_surfaces)
}
