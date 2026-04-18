//! live_run
//!
//! Purpose:
//! - Own the canonical host live-run entrypoints over prepared assets, path-backed loading,
//!   explicit driver ingress, capture finalization, and lower-level replay summary reporting.
//!
//! Owns:
//! - Host run orchestration from loader/prep output into truthful `Completed` versus
//!   `Interrupted` outcomes.
//! - Fixture-driver execution over canonical `HostedRunner` stepping.
//! - Run-summary finalization, including capture writing and invoke/defer counting.
//!
//! Does not own:
//! - Graph loading, expansion, adapter binding, or hosted-runner construction, which live in
//!   `live_prep.rs`.
//! - `InterruptionReason` meaning or formatting, which lives in `usecases.rs`.
//! - Replay execution semantics, which live in `replay.rs` and `HostedRunner`.
//!
//! Connects to:
//! - `live_prep.rs` for prepared runner setup and finalization staging.
//! - `process_driver.rs` for process-ingress execution.
//! - CLI and SDK run entrypoints re-exported through `usecases.rs` and `lib.rs`.
//!
//! Safety notes:
//! - Host stop and bounded-run limits intentionally preserve partial truthful
//!   capture only after at least one committed event; fixture ingress now
//!   carries that boundary through `FixtureDriverCommitPhase`.
//! - Lower-level live-run entrypoints still defend direct callers with local
//!   fixture validation, but they do so through the same prepared fixture-input
//!   phase that canonical driver preflight uses indirectly.
//! - `PreparedFixtureInput` owns fixture normalization/validation, while
//!   `run_prepared_graph_with_policy(...)` owns the shared execute/finalize
//!   orchestration across path-backed and asset-backed run lanes.
//! - `RunSummary.events` and `episode_event_counts` come from driver bookkeeping, while
//!   `invoked`/`deferred` come from finalized capture truth.
//! - The current `InterruptionReason` Display/Debug contract is downstream-significant, but tests
//!   for that enum belong with its definition in `usecases.rs`, not here.
//! - `RunLifecycleState` is owned by this module and shared with
//!   `process_driver.rs` via `pub(super)` function parameter. This module
//!   owns the lifecycle policy (bounded-run limits, host stop); the process
//!   driver consumes it to decide when to stop reading events.
//! - The fixture step loop (`run_fixture_items_driver`) and the process driver
//!   step loop (`process_driver.rs`) share the same commit/interrupt outcome
//!   routing pattern but are structurally different ingress protocols (nested
//!   episode→event iteration vs. streaming message parsing). Unifying them
//!   would require a trait/callback abstraction that adds indirection without
//!   reducing meaningful risk. The duplication is structural, not accidental.

// Allow non-Send/Sync in Arc: CoreRegistries and CorePrimitiveCatalog contain non-Send/Sync types.
#![allow(clippy::arc_with_non_send_sync)]

use super::live_prep::{
    ensure_adapter_requirement_satisfied, ensure_production_adapter_bound,
    finalize_hosted_runner_capture_with_stage, load_graph_assets_from_paths,
    prepare_live_runner_setup_from_assets, session_intent_from_driver, start_live_runner_egress,
    HostedRunnerFinalizeFailure,
};
// Shared standard-library and external-crate prelude for usecase submodules.
use super::shared::*;
// Usecases-owned types used by this module.
use super::{
    interruption_from_egress_dispatch_failure, AdapterDependencySummary, AdapterInput,
    CapturePolicy, DriverConfig, HostDriverError, HostDriverInputError, HostDriverOutputError,
    HostReplayError, HostRunError, InterruptedRun, InterruptionReason, LivePrepOptions,
    ReplayGraphRequest, ReplayGraphResult, RunControl, RunFixtureRequest, RunFixtureResult,
    RunGraphFromAssetsRequest, RunGraphFromPathsRequest, RunGraphRequest, RunGraphResponse,
    RunOutcome, RunSummary, RuntimeSurfaces,
};
// Process driver types (sibling module, imported through parent).
use super::{
    run_process_driver, validate_process_driver_command, ProcessDriverPolicy,
    DEFAULT_PROCESS_DRIVER_POLICY,
};
use std::ops::ControlFlow;

pub(super) enum DriverTerminal {
    Completed,
    Interrupted(InterruptionReason),
}

pub(super) struct DriverExecution {
    pub(super) runner: HostedRunner,
    pub(super) event_count: usize,
    pub(super) episode_event_counts: Vec<(String, usize)>,
    pub(super) terminal: DriverTerminal,
}

pub(super) struct RunLifecycleState {
    pub(super) control: RunControl,
    pub(super) started_at: Instant,
}

impl RunLifecycleState {
    pub(super) fn new(control: RunControl) -> Self {
        Self {
            control,
            started_at: Instant::now(),
        }
    }

    pub(super) fn should_stop(&self, committed_event_count: usize) -> bool {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixtureDriverCommitPhase {
    BeforeFirstCommittedStep,
    AfterFirstCommittedStep,
}

#[derive(Debug)]
struct PreparedFixtureEpisode {
    label: String,
    events: Vec<HostedEvent>,
}

#[derive(Debug)]
pub(super) struct PreparedFixtureInput {
    episodes: Vec<PreparedFixtureEpisode>,
}

impl PreparedFixtureInput {
    fn from_items(
        fixture_items: impl IntoIterator<Item = fixture::FixtureItem>,
        source_label: &str,
        adapter_bound: bool,
    ) -> Result<Self, HostDriverInputError> {
        let mut episodes: Vec<PreparedFixtureEpisode> = Vec::new();
        let mut current_episode: Option<usize> = None;
        let mut event_counter = 0usize;
        let mut seen_fixture_event_ids = HashSet::new();

        for item in fixture_items {
            match item {
                fixture::FixtureItem::EpisodeStart { label } => {
                    episodes.push(PreparedFixtureEpisode {
                        label,
                        events: Vec::new(),
                    });
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
                        episodes.push(PreparedFixtureEpisode {
                            label,
                            events: Vec::new(),
                        });
                        current_episode = Some(episodes.len() - 1);
                    }

                    event_counter += 1;
                    let event_id = id.unwrap_or_else(|| format!("fixture_evt_{event_counter}"));
                    if !seen_fixture_event_ids.insert(event_id.clone()) {
                        return Err(HostDriverInputError::DuplicateEventId { event_id });
                    }

                    let hosted_event = if adapter_bound {
                        let semantic_kind = semantic_kind.ok_or_else(|| {
                            HostDriverInputError::MissingSemanticKind {
                                event_id: event_id.clone(),
                            }
                        })?;
                        HostedEvent {
                            event_id,
                            kind,
                            at: EventTime::default(),
                            semantic_kind: Some(semantic_kind),
                            payload: Some(payload.unwrap_or_else(|| {
                                serde_json::Value::Object(serde_json::Map::new())
                            })),
                        }
                    } else {
                        if semantic_kind.is_some() {
                            return Err(HostDriverInputError::UnexpectedSemanticKind { event_id });
                        }
                        HostedEvent {
                            event_id,
                            kind,
                            at: EventTime::default(),
                            semantic_kind: None,
                            payload,
                        }
                    };

                    let index = current_episode.expect("episode index set");
                    episodes[index].events.push(hosted_event);
                }
            }
        }

        if episodes.is_empty() {
            return Err(HostDriverInputError::NoEpisodes {
                source_label: source_label.to_string(),
            });
        }

        let event_count: usize = episodes.iter().map(|episode| episode.events.len()).sum();
        if event_count == 0 {
            return Err(HostDriverInputError::NoEvents {
                source_label: source_label.to_string(),
            });
        }

        if let Some(episode) = episodes.iter().find(|episode| episode.events.is_empty()) {
            return Err(HostDriverInputError::EpisodeWithoutEvents {
                label: episode.label.clone(),
            });
        }

        Ok(Self { episodes })
    }
}

#[derive(Debug)]
struct FixtureDriverProgress {
    event_count: usize,
    committed_event_count: usize,
    commit_phase: FixtureDriverCommitPhase,
    episode_event_counts: Vec<(String, usize)>,
}

impl FixtureDriverProgress {
    fn new(input: &PreparedFixtureInput) -> Self {
        Self {
            event_count: 0,
            committed_event_count: 0,
            commit_phase: FixtureDriverCommitPhase::BeforeFirstCommittedStep,
            episode_event_counts: input
                .episodes
                .iter()
                .map(|episode| (episode.label.clone(), 0))
                .collect(),
        }
    }

    fn committed_event_count(&self) -> usize {
        self.committed_event_count
    }

    fn record_committed_event(&mut self, episode_index: usize) {
        self.event_count += 1;
        self.committed_event_count += 1;
        self.commit_phase = FixtureDriverCommitPhase::AfterFirstCommittedStep;
        self.episode_event_counts[episode_index].1 += 1;
    }

    fn record_interrupted_event(&mut self, episode_index: usize) {
        self.event_count += 1;
        self.episode_event_counts[episode_index].1 += 1;
    }

    fn into_parts(self) -> (usize, FixtureDriverCommitPhase, Vec<(String, usize)>) {
        (
            self.event_count,
            self.commit_phase,
            self.episode_event_counts,
        )
    }
}

struct FixtureDriverState {
    runner: HostedRunner,
    progress: FixtureDriverProgress,
}

impl FixtureDriverState {
    fn new(runner: HostedRunner, input: &PreparedFixtureInput) -> Self {
        Self {
            runner,
            progress: FixtureDriverProgress::new(input),
        }
    }

    fn committed_event_count(&self) -> usize {
        self.progress.committed_event_count()
    }

    fn record_committed_event(&mut self, episode_index: usize) {
        self.progress.record_committed_event(episode_index);
    }

    fn record_interrupted_event(&mut self, episode_index: usize) {
        self.progress.record_interrupted_event(episode_index);
    }

    fn into_execution(self, terminal: DriverTerminal) -> DriverExecution {
        let (event_count, _, episode_event_counts) = self.progress.into_parts();
        DriverExecution {
            runner: self.runner,
            event_count,
            episode_event_counts,
            terminal,
        }
    }

    fn into_host_stop_execution(self) -> Result<DriverExecution, HostRunError> {
        let (event_count, commit_phase, episode_event_counts) = self.progress.into_parts();
        if commit_phase == FixtureDriverCommitPhase::BeforeFirstCommittedStep {
            return Err(HostRunError::Driver(HostDriverError::Output(
                HostDriverOutputError::StopBeforeFirstCommittedEvent,
            )));
        }

        Ok(DriverExecution {
            runner: self.runner,
            event_count,
            episode_event_counts,
            terminal: DriverTerminal::Interrupted(InterruptionReason::HostStopRequested),
        })
    }
}

fn maybe_fixture_host_stop(
    state: FixtureDriverState,
    lifecycle: &RunLifecycleState,
) -> Result<ControlFlow<DriverExecution, FixtureDriverState>, HostRunError> {
    if lifecycle.should_stop(state.committed_event_count()) {
        state.into_host_stop_execution().map(ControlFlow::Break)
    } else {
        Ok(ControlFlow::Continue(state))
    }
}

/// Canonical run API for clients. Host owns graph loading, expansion, adapter composition, and runner setup.
pub fn run_graph_from_paths(request: RunGraphFromPathsRequest) -> RunGraphResponse {
    run_graph_from_paths_internal(
        request,
        None,
        DEFAULT_PROCESS_DRIVER_POLICY,
        RunControl::default(),
    )
}

/// Canonical run API with host stop control and bounded-run limits.
pub fn run_graph_from_paths_with_control(
    request: RunGraphFromPathsRequest,
    control: RunControl,
) -> RunGraphResponse {
    run_graph_from_paths_internal(request, None, DEFAULT_PROCESS_DRIVER_POLICY, control)
}

/// Advanced run API for callers that prebuild runtime surfaces before invoking the canonical host path.
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

pub(super) fn run_graph_from_paths_internal(
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

    let assets = load_graph_assets_from_paths(&graph_path, &cluster_paths)?;
    let options = LivePrepOptions {
        adapter: adapter_path.map(AdapterInput::Path),
        egress_config,
        session_intent: session_intent_from_driver(&driver),
    };

    let setup = prepare_live_runner_setup_from_assets(&assets, &options, runtime_surfaces)?;

    run_graph_with_policy(
        RunGraphRequest {
            graph_path,
            driver,
            capture_output,
            pretty_capture,
            adapter_bound: setup.adapter_bound(),
            dependency_summary: setup.dependency_summary().clone(),
            runner: setup.into_runner(),
        },
        process_policy,
        control,
    )
}

pub fn run_graph(request: RunGraphRequest) -> RunGraphResponse {
    run_graph_with_policy(
        request,
        DEFAULT_PROCESS_DRIVER_POLICY,
        RunControl::default(),
    )
}

/// Lower-level canonical run API with host stop control and bounded-run limits.
pub fn run_graph_with_control(request: RunGraphRequest, control: RunControl) -> RunGraphResponse {
    run_graph_with_policy(request, DEFAULT_PROCESS_DRIVER_POLICY, control)
}

/// Lower-level live run API over preloaded graph assets with explicit capture policy.
pub fn run_graph_from_assets(request: RunGraphFromAssetsRequest) -> RunGraphResponse {
    run_graph_from_assets_internal(
        request,
        None,
        DEFAULT_PROCESS_DRIVER_POLICY,
        RunControl::default(),
    )
}

/// Lower-level live run API over preloaded graph assets with host stop control and bounded-run limits.
pub fn run_graph_from_assets_with_control(
    request: RunGraphFromAssetsRequest,
    control: RunControl,
) -> RunGraphResponse {
    run_graph_from_assets_internal(request, None, DEFAULT_PROCESS_DRIVER_POLICY, control)
}

/// Advanced lower-level live run API over preloaded graph assets with injected runtime surfaces.
pub fn run_graph_from_assets_with_surfaces(
    request: RunGraphFromAssetsRequest,
    runtime_surfaces: RuntimeSurfaces,
) -> RunGraphResponse {
    run_graph_from_assets_internal(
        request,
        Some(runtime_surfaces),
        DEFAULT_PROCESS_DRIVER_POLICY,
        RunControl::default(),
    )
}

/// Advanced lower-level live run API over preloaded graph assets with injected runtime surfaces and control.
pub fn run_graph_from_assets_with_surfaces_and_control(
    request: RunGraphFromAssetsRequest,
    runtime_surfaces: RuntimeSurfaces,
    control: RunControl,
) -> RunGraphResponse {
    run_graph_from_assets_internal(
        request,
        Some(runtime_surfaces),
        DEFAULT_PROCESS_DRIVER_POLICY,
        control,
    )
}

/// Validated driver input. Produced by `validate_driver_input` and consumed by
/// the execution path, so fixture preparation happens exactly once.
pub(super) enum ValidatedDriverInput {
    Fixture(PreparedFixtureInput),
    Process { command: Vec<String> },
}

/// Validate driver input and produce a `ValidatedDriverInput` that the execution
/// path can consume directly. Fixture items are parsed and prepared once here;
/// the run path no longer needs to repeat that work.
pub(super) fn validate_driver_input(
    driver: &DriverConfig,
    adapter_bound: bool,
) -> Result<ValidatedDriverInput, HostRunError> {
    match driver {
        DriverConfig::Fixture { path } => {
            let fixture_items = fixture::parse_fixture(path).map_err(|err| {
                HostRunError::Driver(HostDriverError::Input(HostDriverInputError::FixtureParse(
                    err,
                )))
            })?;
            PreparedFixtureInput::from_items(
                fixture_items.into_iter(),
                &path.display().to_string(),
                adapter_bound,
            )
            .map(ValidatedDriverInput::Fixture)
            .map_err(|err| HostRunError::Driver(HostDriverError::Input(err)))
        }
        DriverConfig::FixtureItems {
            items,
            source_label,
        } => PreparedFixtureInput::from_items(items.iter().cloned(), source_label, adapter_bound)
            .map(ValidatedDriverInput::Fixture)
            .map_err(|err| HostRunError::Driver(HostDriverError::Input(err))),
        DriverConfig::Process { command } => {
            validate_process_driver_command(command)
                .map_err(|err| HostRunError::Driver(HostDriverError::Input(err)))?;
            Ok(ValidatedDriverInput::Process {
                command: command.clone(),
            })
        }
    }
}

pub(super) fn run_graph_from_assets_internal(
    request: RunGraphFromAssetsRequest,
    runtime_surfaces: Option<RuntimeSurfaces>,
    process_policy: ProcessDriverPolicy,
    control: RunControl,
) -> RunGraphResponse {
    let RunGraphFromAssetsRequest {
        assets,
        prep,
        driver,
        capture,
    } = request;
    let setup = prepare_live_runner_setup_from_assets(&assets, &prep, runtime_surfaces)?;
    run_prepared_graph_with_policy(
        driver,
        setup.adapter_bound(),
        setup.dependency_summary().clone(),
        setup.into_runner(),
        capture,
        process_policy,
        control,
    )
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
        runner,
    } = request;
    run_prepared_graph_with_policy(
        driver,
        adapter_bound,
        dependency_summary,
        runner,
        capture_policy_for_paths(&graph_path, capture_output, pretty_capture),
        process_policy,
        control,
    )
}

fn run_prepared_graph_with_policy(
    driver: DriverConfig,
    adapter_bound: bool,
    dependency_summary: AdapterDependencySummary,
    runner: HostedRunner,
    capture: CapturePolicy,
    process_policy: ProcessDriverPolicy,
    control: RunControl,
) -> RunGraphResponse {
    let execution = execute_run_graph_with_policy(
        driver,
        adapter_bound,
        dependency_summary,
        runner,
        process_policy,
        control,
    )?;
    finalize_driver_execution(capture, execution)
}

fn finalize_driver_execution(
    capture: CapturePolicy,
    execution: DriverExecution,
) -> RunGraphResponse {
    let DriverExecution {
        runner,
        event_count,
        episode_event_counts,
        terminal,
    } = execution;
    let host_stop_requested = matches!(
        terminal,
        DriverTerminal::Interrupted(InterruptionReason::HostStopRequested)
    );
    let summary = finalize_run_summary(
        capture,
        runner,
        event_count,
        episode_event_counts,
        host_stop_requested,
    )?;

    match terminal {
        DriverTerminal::Completed => Ok(RunOutcome::Completed(summary)),
        DriverTerminal::Interrupted(reason) => {
            Ok(RunOutcome::Interrupted(InterruptedRun { summary, reason }))
        }
    }
}

fn execute_run_graph_with_policy(
    driver: DriverConfig,
    adapter_bound: bool,
    dependency_summary: AdapterDependencySummary,
    mut runner: HostedRunner,
    process_policy: ProcessDriverPolicy,
    control: RunControl,
) -> Result<DriverExecution, HostRunError> {
    ensure_adapter_requirement_satisfied(adapter_bound, &dependency_summary)?;
    ensure_production_adapter_bound(adapter_bound, session_intent_from_driver(&driver))?;

    // Validate and prepare driver input once. The validated input carries any
    // prepared fixture state so the run path does not re-parse or re-prepare.
    let validated_input = validate_driver_input(&driver, adapter_bound)?;

    start_live_runner_egress(&mut runner)?;

    let lifecycle = RunLifecycleState::new(control);

    match validated_input {
        ValidatedDriverInput::Fixture(prepared) => {
            run_prepared_fixture_driver(prepared, runner, &lifecycle)
        }
        ValidatedDriverInput::Process { command } => {
            run_process_driver(command, runner, process_policy, &lifecycle)
        }
    }
}

/// Execute a fixture driver with already-prepared input. The preparation
/// (parsing, validation, episode grouping) was done in `validate_driver_input`,
/// so this function consumes the result directly — no re-parsing or re-preparation.
fn run_prepared_fixture_driver(
    prepared: PreparedFixtureInput,
    runner: HostedRunner,
    lifecycle: &RunLifecycleState,
) -> Result<DriverExecution, HostRunError> {
    let mut state = FixtureDriverState::new(runner, &prepared);

    for (episode_index, episode) in prepared.episodes.into_iter().enumerate() {
        state = match maybe_fixture_host_stop(state, lifecycle)? {
            ControlFlow::Break(execution) => return Ok(execution),
            ControlFlow::Continue(state) => state,
        };

        for event in episode.events {
            state = match maybe_fixture_host_stop(state, lifecycle)? {
                ControlFlow::Break(execution) => return Ok(execution),
                ControlFlow::Continue(state) => state,
            };

            match state.runner.step(event) {
                Ok(_) => {
                    state.record_committed_event(episode_index);
                    state = match maybe_fixture_host_stop(state, lifecycle)? {
                        ControlFlow::Break(execution) => return Ok(execution),
                        ControlFlow::Continue(state) => state,
                    };
                }
                Err(crate::HostedStepError::EgressDispatchFailure(failure)) => {
                    state.record_interrupted_event(episode_index);
                    return Ok(state.into_execution(DriverTerminal::Interrupted(
                        interruption_from_egress_dispatch_failure(failure),
                    )));
                }
                Err(err) => {
                    return Err(HostRunError::Step(err));
                }
            }
        }
    }

    state = match maybe_fixture_host_stop(state, lifecycle)? {
        ControlFlow::Break(execution) => return Ok(execution),
        ControlFlow::Continue(state) => state,
    };

    Ok(state.into_execution(DriverTerminal::Completed))
}

struct FinalizedRunCapture {
    bundle: CaptureBundle,
    episodes: usize,
    events: usize,
    invoked: usize,
    deferred: usize,
    episode_event_counts: Vec<(String, usize)>,
}

fn finalize_run_capture(
    runner: HostedRunner,
    event_count: usize,
    episode_event_counts: Vec<(String, usize)>,
    host_stop_requested: bool,
) -> Result<FinalizedRunCapture, HostRunError> {
    if episode_event_counts.is_empty() {
        return Err(HostRunError::Driver(HostDriverError::Output(
            HostDriverOutputError::ProducedNoEpisodes,
        )));
    }
    if event_count == 0 {
        return Err(HostRunError::Driver(HostDriverError::Output(
            HostDriverOutputError::ProducedNoEvents,
        )));
    }
    if let Some((label, _)) = episode_event_counts.iter().find(|(_, count)| *count == 0) {
        return Err(HostRunError::Driver(HostDriverError::Output(
            HostDriverOutputError::EpisodeWithoutEvents {
                label: label.clone(),
            },
        )));
    }

    let bundle = finalize_hosted_runner_capture_with_stage(runner, host_stop_requested).map_err(
        |failure| match failure {
            HostedRunnerFinalizeFailure::PendingAcks(err) => HostRunError::Step(err),
            HostedRunnerFinalizeFailure::StopEgress(err) => HostRunError::Step(err),
        },
    )?;
    let (invoked, deferred, _) = decision_counts(&bundle);

    Ok(FinalizedRunCapture {
        bundle,
        episodes: episode_event_counts.len(),
        events: event_count,
        invoked,
        deferred,
        episode_event_counts,
    })
}

fn default_capture_output_path(graph_path: &Path) -> PathBuf {
    let stem = graph_path
        .file_stem()
        .and_then(|part| part.to_str())
        .unwrap_or("graph");
    PathBuf::from("target").join(format!("{stem}-capture.json"))
}

fn write_run_capture_bundle(
    capture_path: &Path,
    pretty_capture: bool,
    bundle: &CaptureBundle,
) -> Result<(), HostRunError> {
    let style = if pretty_capture {
        CaptureJsonStyle::Pretty
    } else {
        CaptureJsonStyle::Compact
    };
    write_capture_bundle(capture_path, bundle, style).map_err(HostRunError::CaptureWrite)
}

fn capture_policy_for_paths(
    graph_path: &Path,
    capture_output: Option<PathBuf>,
    pretty_capture: bool,
) -> CapturePolicy {
    CapturePolicy::File {
        path: capture_output.unwrap_or_else(|| default_capture_output_path(graph_path)),
        pretty: pretty_capture,
    }
}

fn finalize_run_summary(
    capture: CapturePolicy,
    runner: HostedRunner,
    event_count: usize,
    episode_event_counts: Vec<(String, usize)>,
    host_stop_requested: bool,
) -> Result<RunSummary, HostRunError> {
    let finalized = finalize_run_capture(
        runner,
        event_count,
        episode_event_counts,
        host_stop_requested,
    )?;
    let capture_path = match capture {
        CapturePolicy::InMemory => None,
        CapturePolicy::File { path, pretty } => {
            write_run_capture_bundle(&path, pretty, &finalized.bundle)?;
            Some(path)
        }
    };

    Ok(RunSummary {
        capture_bundle: finalized.bundle,
        capture_path,
        episodes: finalized.episodes,
        events: finalized.events,
        invoked: finalized.invoked,
        deferred: finalized.deferred,
        episode_event_counts: finalized.episode_event_counts,
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
            return Err(HostRunError::Driver(HostDriverError::Output(
                HostDriverOutputError::UnexpectedInterruptedOutcome,
            )))
        }
    };
    let Some(capture_path) = summary.capture_path else {
        return Err(HostRunError::Driver(HostDriverError::Output(
            HostDriverOutputError::MissingCapturePath,
        )));
    };
    Ok(RunFixtureResult {
        capture_path,
        episodes: summary.episodes,
        events: summary.events,
        episode_event_counts: summary.episode_event_counts,
    })
}
