use super::*;

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
    };

    let PreparedLiveRunnerSetup {
        adapter_bound,
        dependency_summary,
        runner,
    } = prepare_live_runner_setup_from_assets(&assets, &options, runtime_surfaces)?;

    run_graph_with_policy(
        RunGraphRequest {
            graph_path,
            driver,
            capture_output,
            pretty_capture,
            adapter_bound,
            dependency_summary,
            runner,
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
#[allow(clippy::arc_with_non_send_sync)]
pub fn run_graph_from_assets(request: RunGraphFromAssetsRequest) -> RunGraphResponse {
    run_graph_from_assets_internal(
        request,
        None,
        DEFAULT_PROCESS_DRIVER_POLICY,
        RunControl::default(),
    )
}

/// Lower-level live run API over preloaded graph assets with host stop control and bounded-run limits.
#[allow(clippy::arc_with_non_send_sync)]
pub fn run_graph_from_assets_with_control(
    request: RunGraphFromAssetsRequest,
    control: RunControl,
) -> RunGraphResponse {
    run_graph_from_assets_internal(request, None, DEFAULT_PROCESS_DRIVER_POLICY, control)
}

/// Advanced lower-level live run API over preloaded graph assets with injected runtime surfaces.
#[allow(clippy::arc_with_non_send_sync)]
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
#[allow(clippy::arc_with_non_send_sync)]
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

pub(super) fn validate_driver_input(
    driver: &DriverConfig,
    adapter_bound: bool,
) -> Result<(), HostRunError> {
    match driver {
        DriverConfig::Fixture { path } => {
            let fixture_items = fixture::parse_fixture(path).map_err(|err| {
                HostRunError::InvalidInput(format!("failed to parse fixture: {err}"))
            })?;
            validate_fixture_items_input(&fixture_items, &path.display().to_string(), adapter_bound)
        }
        DriverConfig::FixtureItems {
            items,
            source_label,
        } => validate_fixture_items_input(items, source_label, adapter_bound),
        DriverConfig::Process { command } => validate_process_driver_command(command),
    }
}

#[allow(clippy::arc_with_non_send_sync)]
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
    let PreparedLiveRunnerSetup {
        adapter_bound,
        dependency_summary,
        runner,
    } = prepare_live_runner_setup_from_assets(&assets, &prep, runtime_surfaces)?;
    let DriverExecution {
        runner,
        event_count,
        episode_event_counts,
        terminal,
    } = execute_run_graph_with_policy(
        driver,
        adapter_bound,
        dependency_summary,
        runner,
        process_policy,
        control,
    )?;
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
    let DriverExecution {
        runner,
        event_count,
        episode_event_counts,
        terminal,
    } = execute_run_graph_with_policy(
        driver,
        adapter_bound,
        dependency_summary,
        runner,
        process_policy,
        control,
    )?;

    match terminal {
        DriverTerminal::Completed => {
            let summary = finalize_run_summary(
                capture_policy_for_paths(&graph_path, capture_output, pretty_capture),
                runner,
                event_count,
                episode_event_counts,
                false,
            )?;
            Ok(RunOutcome::Completed(summary))
        }
        DriverTerminal::Interrupted(reason) => {
            let host_stop_requested = matches!(reason, InterruptionReason::HostStopRequested);
            let summary = finalize_run_summary(
                capture_policy_for_paths(&graph_path, capture_output, pretty_capture),
                runner,
                event_count,
                episode_event_counts,
                host_stop_requested,
            )?;
            Ok(RunOutcome::Interrupted(InterruptedRun { summary, reason }))
        }
    }
}

fn validate_fixture_items_input(
    fixture_items: &[fixture::FixtureItem],
    source_label: &str,
    adapter_bound: bool,
) -> Result<(), HostRunError> {
    let mut episodes: Vec<(String, usize)> = Vec::new();
    let mut current_episode: Option<usize> = None;
    let mut event_counter = 0usize;
    let mut seen_fixture_event_ids = HashSet::new();

    for item in fixture_items {
        match item {
            fixture::FixtureItem::EpisodeStart { label } => {
                episodes.push((label.clone(), 0));
                current_episode = Some(episodes.len() - 1);
            }
            fixture::FixtureItem::Event {
                id, semantic_kind, ..
            } => {
                if current_episode.is_none() {
                    let label = format!("E{}", episodes.len() + 1);
                    episodes.push((label, 0));
                    current_episode = Some(episodes.len() - 1);
                }

                event_counter += 1;
                let event_id = id
                    .clone()
                    .unwrap_or_else(|| format!("fixture_evt_{event_counter}"));
                if !seen_fixture_event_ids.insert(event_id.clone()) {
                    return Err(HostRunError::InvalidInput(format!(
                        "fixture event id '{}' appears more than once in canonical run input",
                        event_id
                    )));
                }

                if adapter_bound {
                    if semantic_kind.is_none() {
                        return Err(HostRunError::InvalidInput(format!(
                            "fixture event '{}' is missing semantic_kind in adapter-bound canonical run",
                            event_id
                        )));
                    }
                } else if semantic_kind.is_some() {
                    return Err(HostRunError::InvalidInput(format!(
                        "fixture event '{}' set semantic_kind but canonical run is not adapter-bound",
                        event_id
                    )));
                }

                let index = current_episode.expect("episode index set");
                episodes[index].1 += 1;
            }
        }
    }

    if episodes.is_empty() {
        return Err(HostRunError::InvalidInput(format!(
            "fixture input '{source_label}' contained no episodes"
        )));
    }
    if event_counter == 0 {
        return Err(HostRunError::InvalidInput(format!(
            "fixture input '{source_label}' contained no events"
        )));
    }
    if let Some((label, _)) = episodes.iter().find(|(_, count)| *count == 0) {
        return Err(HostRunError::InvalidInput(format!(
            "episode '{}' has no events",
            label
        )));
    }

    Ok(())
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

    start_live_runner_egress(&mut runner)?;

    let lifecycle = RunLifecycleState::new(control);

    match driver {
        DriverConfig::Fixture { path } => {
            run_fixture_driver(path, adapter_bound, runner, &lifecycle)
        }
        DriverConfig::FixtureItems {
            items,
            source_label,
        } => run_fixture_items_driver(items, &source_label, adapter_bound, runner, &lifecycle),
        DriverConfig::Process { command } => {
            run_process_driver(command, runner, process_policy, &lifecycle)
        }
    }
}

fn run_fixture_driver(
    fixture_path: PathBuf,
    adapter_bound: bool,
    runner: HostedRunner,
    lifecycle: &RunLifecycleState,
) -> Result<DriverExecution, HostRunError> {
    let fixture_items = fixture::parse_fixture(&fixture_path)
        .map_err(|err| HostRunError::InvalidInput(format!("failed to parse fixture: {err}")))?;
    run_fixture_items_driver(
        fixture_items,
        &fixture_path.display().to_string(),
        adapter_bound,
        runner,
        lifecycle,
    )
}

fn run_fixture_items_driver(
    fixture_items: Vec<fixture::FixtureItem>,
    source_label: &str,
    adapter_bound: bool,
    mut runner: HostedRunner,
    lifecycle: &RunLifecycleState,
) -> Result<DriverExecution, HostRunError> {
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
        return Err(HostRunError::InvalidInput(format!(
            "fixture input '{source_label}' contained no episodes"
        )));
    }
    if event_counter == 0 {
        return Err(HostRunError::InvalidInput(format!(
            "fixture input '{source_label}' contained no events"
        )));
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

pub(super) fn host_stop_driver_execution(
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

    let bundle = finalize_hosted_runner_capture_with_stage(runner, host_stop_requested).map_err(
        |failure| match failure {
            HostedRunnerFinalizeFailure::PendingAcks(err) => {
                HostRunError::StepFailed(format!("egress pending-ack invariant: {err}"))
            }
            HostedRunnerFinalizeFailure::StopEgress(err) => {
                HostRunError::DriverIo(format!("stop egress channels: {err}"))
            }
        },
    )?;
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
    write_capture_bundle(capture_path, bundle, style)
        .map_err(|err| HostRunError::Io(format!("write capture bundle: {err}")))
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
            return Err(HostRunError::StepFailed(
                "fixture driver returned interrupted outcome unexpectedly".to_string(),
            ))
        }
    };
    let Some(capture_path) = summary.capture_path else {
        return Err(HostRunError::StepFailed(
            "fixture run did not produce a capture file path unexpectedly".to_string(),
        ));
    };
    Ok(RunFixtureResult {
        capture_path,
        episodes: summary.episodes,
        events: summary.events,
        episode_event_counts: summary.episode_event_counts,
    })
}
