//! usecases live_prep tests
//!
//! Purpose:
//! - Exercise the host setup and replay-prep seams owned by `live_prep.rs`.
//!
//! Owns:
//! - Behavior checks for typed setup/replay failures, adapter requirements, and
//!   surface-specific prep wiring.
//!
//! Does not own:
//! - The public contract for facade-level error enums; `contract.rs` locks that.
//!
//! Connects to:
//! - `live_prep.rs`, `usecases.rs`, and shared host test helpers from the
//!   parent `usecases::tests` module.
//!
//! Safety notes:
//! - These tests intentionally lock host-owned preparation and replay-preflight
//!   behavior without redefining kernel semantics.

use super::super::process_driver::PROCESS_DRIVER_PROTOCOL_VERSION;
use super::*;

#[test]
fn summarize_expand_error_falls_back_to_cluster_key_when_label_missing() {
    let detail = summarize_expand_error(
        &ExpandError::UnsatisfiedVersionConstraint {
            target_kind: VersionTargetKind::Cluster,
            id: "shared_value".to_string(),
            selector: "^2.0".parse().expect("selector"),
            available_versions: vec![
                "1.0.0".parse().expect("version"),
                "1.5.0".parse().expect("version"),
            ],
        },
        &HashMap::new(),
    );
    let rendered = detail.to_string();

    assert!(rendered.contains("available cluster sources"));
    assert!(rendered.contains("shared_value@1.0.0 at shared_value@1.0.0"));
    assert!(rendered.contains("shared_value@1.5.0 at shared_value@1.5.0"));
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
        &number_source_graph_yaml("host_path_replay", 5.0),
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
        &injected_number_graph_yaml("host_injected_replay"),
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
fn prepare_hosted_runner_from_paths_with_surfaces_uses_injected_runtime_surfaces(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-prepare-runner-injected-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_temp_file(
        &temp_dir,
        "graph.yaml",
        &injected_number_graph_yaml("host_prepare_runner_injected"),
    )?;

    let mut runner = prepare_hosted_runner_from_paths_with_surfaces(
        PrepareHostedRunnerFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            adapter_path: None,
            egress_config: None,
        },
        build_injected_runtime_surfaces(7.5),
    )?;

    let outcome = runner.step(hosted_event("evt1"))?;
    assert_eq!(outcome.decision, Decision::Invoke);
    assert_eq!(
        outcome.termination,
        Some(ergo_adapter::RunTermination::Completed)
    );

    let bundle = finalize_hosted_runner_capture(runner, false)?;
    assert_eq!(bundle.events.len(), 1);
    assert_eq!(bundle.decisions.len(), 1);

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn prepare_hosted_runner_from_paths_smoke_test() -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-prepare-runner-smoke-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_temp_file(
        &temp_dir,
        "graph.yaml",
        &number_source_graph_yaml("host_prepare_runner_smoke", 5.0),
    )?;

    let mut runner = prepare_hosted_runner_from_paths(PrepareHostedRunnerFromPathsRequest {
        graph_path: graph,
        cluster_paths: Vec::new(),
        adapter_path: None,
        egress_config: None,
    })?;

    let outcome = runner.step(hosted_event("evt1"))?;
    assert_eq!(outcome.decision, Decision::Invoke);
    assert_eq!(
        outcome.termination,
        Some(ergo_adapter::RunTermination::Completed)
    );

    let bundle = finalize_hosted_runner_capture(runner, false)?;
    assert_eq!(bundle.graph_id.as_str(), "host_prepare_runner_smoke");
    assert_eq!(bundle.events.len(), 1);
    assert_eq!(bundle.decisions.len(), 1);
    assert_eq!(bundle.decisions[0].decision, Decision::Invoke);

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn finalize_hosted_runner_capture_rejects_zero_step_runners(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-finalize-zero-step-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_temp_file(
        &temp_dir,
        "graph.yaml",
        &injected_number_graph_yaml("host_finalize_zero_step"),
    )?;

    let runner = prepare_hosted_runner_from_paths_with_surfaces(
        PrepareHostedRunnerFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            adapter_path: None,
            egress_config: None,
        },
        build_injected_runtime_surfaces(7.5),
    )?;

    let err = finalize_hosted_runner_capture(runner, false)
        .expect_err("zero-step hosted runners must not finalize");
    assert!(
        matches!(err, HostedStepError::LifecycleViolation { .. }),
        "unexpected zero-step finalize error: {err:?}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn validate_graph_from_paths_accepts_simple_adapterless_graph(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-validate-simple-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_temp_file(
        &temp_dir,
        "graph.yaml",
        &const_number_graph_yaml("host_validate_simple", 7.5),
    )?;

    validate_graph_from_paths(PrepareHostedRunnerFromPathsRequest {
        graph_path: graph,
        cluster_paths: Vec::new(),
        adapter_path: None,
        egress_config: None,
    })?;

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn validate_graph_accepts_simple_adapterless_assets() -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-validate-assets-simple-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_temp_file(
        &temp_dir,
        "graph.yaml",
        &const_number_graph_yaml("host_validate_assets_simple", 7.5),
    )?;

    let assets = load_graph_assets_from_paths(&graph, &[])?;
    validate_graph_with_surfaces(
        &assets,
        &LivePrepOptions::default(),
        build_injected_runtime_surfaces(7.5),
    )?;

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn validate_graph_accepts_simple_in_memory_assets() -> Result<(), Box<dyn std::error::Error>> {
    let assets = load_graph_assets_from_memory(
        "graphs/root.yaml",
        &[ergo_loader::InMemorySourceInput {
            source_id: "graphs/root.yaml".to_string(),
            source_label: "root-memory".to_string(),
            content: const_number_graph_yaml("host_validate_in_memory_simple", 7.5),
        }],
        &[],
    )?;

    validate_graph_with_surfaces(
        &assets,
        &LivePrepOptions::default(),
        build_injected_runtime_surfaces(7.5),
    )?;

    Ok(())
}

#[test]
fn validate_graph_in_memory_assets_use_label_first_cluster_version_details(
) -> Result<(), Box<dyn std::error::Error>> {
    let assets = load_graph_assets_from_memory(
        "graphs/root.yaml",
        &[
            ergo_loader::InMemorySourceInput {
                source_id: "graphs/root.yaml".to_string(),
                source_label: "root-row".to_string(),
                content: cluster_version_miss_graph_yaml("host_validate_in_memory_version_miss"),
            },
            ergo_loader::InMemorySourceInput {
                source_id: "search-a/shared_value.yaml".to_string(),
                source_label: "shared-v1-row".to_string(),
                content: shared_value_graph_yaml("1.0.0", 3.0),
            },
            ergo_loader::InMemorySourceInput {
                source_id: "search-b/shared_value.yaml".to_string(),
                source_label: "shared-v1_5-row".to_string(),
                content: shared_value_graph_yaml("1.5.0", 4.0),
            },
        ],
        &["search-a".to_string(), "search-b".to_string()],
    )?;

    let err = validate_graph_with_surfaces(
        &assets,
        &LivePrepOptions::default(),
        build_injected_runtime_surfaces(42.0),
    )
    .expect_err("in-memory version-miss must fail before runner construction");

    match err {
        HostRunError::Setup(HostSetupError::GraphPreparation(
            detail @ HostGraphPreparationError::Expansion(_),
        )) => {
            let detail = detail.to_string();
            assert!(detail.contains("graph expansion failed"));
            assert!(detail.contains("shared_value"));
            assert!(detail.contains("^2.0"));
            assert!(detail.contains("available: 1.0.0, 1.5.0"));
            assert!(detail.contains("available cluster sources"));
            assert!(detail.contains("shared_value@1.0.0 at shared-v1-row"));
            assert!(detail.contains("shared_value@1.5.0 at shared-v1_5-row"));
            assert!(!detail.contains("search-a/shared_value.yaml"));
            assert!(!detail.contains("search-b/shared_value.yaml"));
        }
        other => panic!("unexpected error: {other:?}"),
    }

    Ok(())
}

#[test]
fn validate_graph_from_paths_reports_adapter_required_before_runner_validation(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-validate-adapter-required-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_validate_adapter_required")?;

    let err = validate_graph_from_paths_with_surfaces(
        PrepareHostedRunnerFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            adapter_path: None,
            egress_config: None,
        },
        build_injected_runtime_surfaces(42.0),
    )
    .expect_err("adapter-required validation should fail before runner construction");

    match err {
        HostRunError::AdapterRequired(summary) => assert!(summary.requires_adapter),
        other => panic!("unexpected adapter-required validation error: {other:?}"),
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn validate_graph_reports_adapter_required_before_runner_construction(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-validate-assets-adapter-required-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_validate_assets_adapter")?;
    let assets = load_graph_assets_from_paths(&graph, &[])?;

    let err = validate_graph_with_surfaces(
        &assets,
        &LivePrepOptions::default(),
        build_injected_runtime_surfaces(42.0),
    )
    .expect_err("adapter-required validation should fail before runner construction");

    match err {
        HostRunError::AdapterRequired(summary) => assert!(summary.requires_adapter),
        other => panic!("unexpected adapter-required validation error: {other:?}"),
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn validate_run_graph_from_paths_rejects_missing_fixture_driver(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-validate-run-missing-fixture-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_temp_file(
        &temp_dir,
        "graph.yaml",
        &number_source_graph_yaml("host_validate_missing_fixture", 2.5),
    )?;
    let missing_fixture = temp_dir.join("missing.jsonl");

    let err = validate_run_graph_from_paths_with_surfaces(
        RunGraphFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            driver: DriverConfig::Fixture {
                path: missing_fixture.clone(),
            },
            adapter_path: None,
            egress_config: None,
            capture_output: None,
            pretty_capture: false,
        },
        build_injected_runtime_surfaces(7.5),
    )
    .expect_err("missing fixture driver should fail full run validation");

    match err {
        HostRunError::Driver(HostDriverError::Input(
            detail @ HostDriverInputError::FixtureParse(_),
        )) => {
            let detail = detail.to_string();
            assert!(detail.contains("failed to parse fixture"));
            assert!(detail.contains("read fixture"));
        }
        other => panic!("unexpected missing-fixture validation error: {other:?}"),
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn validate_run_graph_from_assets_rejects_adapter_bound_fixture_items_missing_semantic_kind(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-validate-fixture-items-semantic-kind-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let assets = load_intent_in_memory_assets("host_validate_fixture_items_semantic_kind")?;
    let egress_script = write_egress_ack_script(&temp_dir, "egress.sh")?;
    let manifest =
        serde_json::from_value::<ergo_adapter::AdapterManifest>(serde_yaml::from_str::<
            serde_json::Value,
        >(
            intent_adapter_manifest_text()
        )?)?;

    let err = validate_run_graph_from_assets_with_surfaces(
        RunGraphFromAssetsRequest {
            assets,
            prep: LivePrepOptions {
                adapter: Some(AdapterInput::Manifest(manifest)),
                egress_config: Some(make_intent_egress_config(&egress_script)),
            },
            driver: DriverConfig::FixtureItems {
                items: vec![
                    ergo_adapter::fixture::FixtureItem::EpisodeStart {
                        label: "E1".to_string(),
                    },
                    ergo_adapter::fixture::FixtureItem::Event {
                        id: Some("evt1".to_string()),
                        kind: ExternalEventKind::Command,
                        payload: Some(serde_json::json!({})),
                        semantic_kind: None,
                    },
                ],
                source_label: "memory-fixture".to_string(),
            },
            capture: CapturePolicy::InMemory,
        },
        build_injected_runtime_surfaces(42.0),
    )
    .expect_err("adapter-bound fixture items missing semantic_kind must fail validation");

    match err {
        HostRunError::Driver(HostDriverError::Input(
            HostDriverInputError::MissingSemanticKind { .. },
        )) => {}
        other => panic!("unexpected fixture-items validation error: {other:?}"),
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn validate_graph_from_paths_enforces_handler_coverage_without_egress(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-validate-handler-coverage-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_validate_handler_coverage")?;
    let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;

    let err = validate_graph_from_paths_with_surfaces(
        PrepareHostedRunnerFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            adapter_path: Some(adapter),
            egress_config: None,
        },
        build_injected_runtime_surfaces(42.0),
    )
    .expect_err("handler coverage should fail without egress for external intent graphs");

    match err {
        HostRunError::Setup(HostSetupError::HostedRunnerValidation(detail)) => {
            let detail = detail.to_string();
            assert!(detail.contains("handler coverage failed"));
            assert!(!detail.contains("failed to initialize canonical host runner"));
        }
        other => panic!("unexpected handler-coverage validation error: {other:?}"),
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn validate_graph_from_paths_accepts_adapter_and_egress_without_starting_channels(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-validate-egress-success-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_validate_egress_success")?;
    let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;
    let egress_script = write_egress_ack_script(&temp_dir, "egress.sh")?;

    validate_graph_from_paths_with_surfaces(
        PrepareHostedRunnerFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            adapter_path: Some(adapter),
            egress_config: Some(make_intent_egress_config(&egress_script)),
        },
        build_injected_runtime_surfaces(42.0),
    )?;

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn validate_graph_from_paths_does_not_start_egress_processes(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-validate-egress-no-start-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_validate_egress_no_start")?;
    let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;
    let config = EgressConfig::builder(Duration::from_millis(50))
        .channel(
            "broker",
            EgressChannelConfig::process(vec!["/definitely/missing-egress-binary".to_string()])
                .expect("channel config should be valid"),
        )
        .expect("channel should insert")
        .route(
            "place_order",
            EgressRoute::new("broker", None).expect("route should be valid"),
        )
        .expect("route should insert")
        .build()
        .expect("egress config should build");

    validate_graph_from_paths_with_surfaces(
        PrepareHostedRunnerFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            adapter_path: Some(adapter),
            egress_config: Some(config),
        },
        build_injected_runtime_surfaces(42.0),
    )?;

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn prepare_hosted_runner_from_paths_surfaces_egress_startup_failure_before_first_step(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-prepare-runner-startup-fail-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_prepare_runner_startup_fail")?;
    let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;
    let config = EgressConfig::builder(Duration::from_millis(50))
        .channel(
            "broker",
            EgressChannelConfig::process(vec!["/definitely/missing-egress-binary".to_string()])
                .expect("channel config should be valid"),
        )
        .expect("channel should insert")
        .route(
            "place_order",
            EgressRoute::new("broker", None).expect("route should be valid"),
        )
        .expect("route should insert")
        .build()
        .expect("egress config should build");

    let err = match prepare_hosted_runner_from_paths_with_surfaces(
        PrepareHostedRunnerFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            adapter_path: Some(adapter),
            egress_config: Some(config),
        },
        build_injected_runtime_surfaces(42.0),
    ) {
        Ok(_) => panic!("startup failure should surface before manual stepping begins"),
        Err(err) => err,
    };

    assert!(matches!(
        err,
        HostRunError::Setup(HostSetupError::StartEgress(_))
    ));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn prepare_hosted_runner_surfaces_egress_startup_failure_after_runner_construction(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-prepare-assets-startup-fail-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_prepare_assets_startup_fail")?;
    let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;
    let config = EgressConfig::builder(Duration::from_millis(50))
        .channel(
            "broker",
            EgressChannelConfig::process(vec!["/definitely/missing-egress-binary".to_string()])
                .expect("channel config should be valid"),
        )
        .expect("channel should insert")
        .route(
            "place_order",
            EgressRoute::new("broker", None).expect("route should be valid"),
        )
        .expect("route should insert")
        .build()
        .expect("egress config should build");

    let assets = load_graph_assets_from_paths(&graph, &[])?;
    let options = LivePrepOptions {
        adapter: Some(AdapterInput::Path(adapter)),
        egress_config: Some(config),
    };

    let err = match prepare_hosted_runner_with_surfaces(
        assets,
        &options,
        build_injected_runtime_surfaces(42.0),
    ) {
        Ok(_) => panic!("startup failure should surface before manual stepping begins"),
        Err(err) => err,
    };

    assert!(matches!(
        err,
        HostRunError::Setup(HostSetupError::StartEgress(_))
    ));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn prepare_hosted_runner_accepts_simple_in_memory_assets() -> Result<(), Box<dyn std::error::Error>>
{
    let assets = load_graph_assets_from_memory(
        "graphs/root.yaml",
        &[ergo_loader::InMemorySourceInput {
            source_id: "graphs/root.yaml".to_string(),
            source_label: "root-memory".to_string(),
            content: const_number_graph_yaml("host_prepare_in_memory_simple", 7.5),
        }],
        &[],
    )?;

    let _runner = prepare_hosted_runner_with_surfaces(
        assets,
        &LivePrepOptions::default(),
        build_injected_runtime_surfaces(7.5),
    )?;

    Ok(())
}

#[test]
fn prepare_hosted_runner_from_paths_surfaces_dispatches_egress_and_finalizes_cleanly(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-prepare-runner-egress-success-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_prepare_runner_egress")?;
    let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;
    let sentinel = temp_dir.join("egress-ended.txt");
    let egress_script = write_egress_end_sentinel_script(&temp_dir, "egress.sh", &sentinel)?;

    let mut runner = prepare_hosted_runner_from_paths_with_surfaces(
        PrepareHostedRunnerFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            adapter_path: Some(adapter),
            egress_config: Some(make_intent_egress_config(&egress_script)),
        },
        build_injected_runtime_surfaces(42.0),
    )?;

    let outcome = runner.step(HostedEvent {
        event_id: "evt1".to_string(),
        kind: ExternalEventKind::Command,
        at: EventTime::default(),
        semantic_kind: Some("price_bar".to_string()),
        payload: Some(json!({"price": 101.5})),
    })?;
    assert_eq!(
        outcome.termination,
        Some(ergo_adapter::RunTermination::Completed)
    );

    let bundle = finalize_hosted_runner_capture(runner, false)?;
    assert_eq!(bundle.events.len(), 1);
    assert_eq!(bundle.decisions.len(), 1);
    assert_eq!(bundle.decisions[0].intent_acks.len(), 1);
    assert!(bundle.decisions[0].interruption.is_none());
    wait_for_path(&sentinel, Duration::from_secs(1))?;

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn prepare_hosted_runner_from_paths_can_finalize_after_egress_dispatch_failure(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-prepare-runner-dispatch-failure-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_intent_graph(
        &temp_dir,
        "graph.yaml",
        "host_prepare_runner_dispatch_failure",
    )?;
    let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;
    let egress_script = write_egress_io_script(&temp_dir, "egress.sh")?;

    let mut runner = prepare_hosted_runner_from_paths_with_surfaces(
        PrepareHostedRunnerFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            adapter_path: Some(adapter),
            egress_config: Some(make_intent_egress_config(&egress_script)),
        },
        build_injected_runtime_surfaces(42.0),
    )?;

    let step_err = runner
        .step(HostedEvent {
            event_id: "evt1".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: Some("price_bar".to_string()),
            payload: Some(json!({"price": 101.5})),
        })
        .expect_err("dispatch failure should interrupt manual stepping");
    assert!(
        matches!(step_err, HostedStepError::EgressDispatchFailure(_)),
        "unexpected dispatch failure: {step_err:?}"
    );

    let second_err = runner
        .step(HostedEvent {
            event_id: "evt2".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: Some("price_bar".to_string()),
            payload: Some(json!({"price": 102.0})),
        })
        .expect_err("dispatch failure must force finalization before another step");
    assert!(
        matches!(second_err, HostedStepError::LifecycleViolation { .. }),
        "unexpected post-dispatch lifecycle error: {second_err:?}"
    );

    let bundle = finalize_hosted_runner_capture(runner, false)?;
    assert_eq!(bundle.events.len(), 1);
    assert_eq!(bundle.decisions.len(), 1);
    assert!(bundle.decisions[0].interruption.is_some());

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn prepare_hosted_runner_from_paths_surfaces_reports_adapter_required_before_egress_validation(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-prepare-adapter-preflight-order-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_prepare_adapter_preflight")?;
    let err = match prepare_hosted_runner_from_paths_with_surfaces(
        PrepareHostedRunnerFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            adapter_path: None,
            egress_config: Some(make_intent_egress_config_with_timeout(
                &temp_dir.join("unused-egress.sh"),
                Duration::from_millis(50),
            )),
        },
        build_injected_runtime_surfaces(42.0),
    ) {
        Ok(_) => {
            panic!("adapter-required error should win before manual runner init validation")
        }
        Err(err) => err,
    };

    match err {
        HostRunError::AdapterRequired(summary) => assert!(summary.requires_adapter),
        other => panic!("unexpected manual adapter-preflight error: {other:?}"),
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn prepare_hosted_runner_surfaces_reports_adapter_required_before_runner_construction(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-prepare-assets-adapter-preflight-order-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_prepare_assets_adapter")?;
    let assets = load_graph_assets_from_paths(&graph, &[])?;
    let options = LivePrepOptions {
        adapter: None,
        egress_config: Some(make_intent_egress_config_with_timeout(
            &temp_dir.join("unused-egress.sh"),
            Duration::from_millis(50),
        )),
    };

    let err = match prepare_hosted_runner_with_surfaces(
        assets,
        &options,
        build_injected_runtime_surfaces(42.0),
    ) {
        Ok(_) => {
            panic!(
                "adapter-required error should win before runner construction and egress startup"
            )
        }
        Err(err) => err,
    };

    match err {
        HostRunError::AdapterRequired(summary) => assert!(summary.requires_adapter),
        other => panic!("unexpected manual adapter-preflight error: {other:?}"),
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
            serde_json::to_string(
                &json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}),
            )?,
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
    assert!(summary
        .capture_path
        .as_ref()
        .is_some_and(|path| path.exists()));

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
    let adapter = AdapterInput::Path(temp_dir.join("adapter.yaml"));
    let adapter_setup = prepare_adapter_setup(Some(&adapter), &prepared)
        .map_err(|err| format!("prepare replay adapter: {err}"))?;
    let runtime = RuntimeHandle::new(
        Arc::new(prepared.expanded),
        prepared.catalog,
        prepared.registries,
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
fn replay_from_paths_fails_when_capture_external_kind_is_not_representable(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-replay-intent-kind-mismatch-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_intent_replay_kind_mismatch")?;
    let adapter = write_intent_adapter_manifest(&temp_dir, "adapter.yaml")?;
    let egress_script = write_egress_ack_script(&temp_dir, "egress.sh")?;
    let driver = write_process_driver_script(
        &temp_dir,
        "driver.sh",
        &[
            serde_json::to_string(
                &json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}),
            )?,
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
fn validate_graph_accepts_in_memory_assets_with_adapter_text(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-validate-inline-adapter-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let assets = load_intent_in_memory_assets("host_validate_inline_adapter")?;
    let egress_script = write_egress_ack_script(&temp_dir, "egress.sh")?;
    let options = LivePrepOptions {
        adapter: Some(AdapterInput::Text {
            content: intent_adapter_manifest_text().to_string(),
            source_label: "inline-adapter".to_string(),
        }),
        egress_config: Some(make_intent_egress_config(&egress_script)),
    };

    validate_graph_with_surfaces(&assets, &options, build_injected_runtime_surfaces(42.0))?;

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn prepare_hosted_runner_accepts_in_memory_assets_with_adapter_manifest_object(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-prepare-manifest-object-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let assets = load_intent_in_memory_assets("host_prepare_manifest_object")?;
    let egress_script = write_egress_ack_script(&temp_dir, "egress.sh")?;
    let manifest =
        serde_json::from_value::<ergo_adapter::AdapterManifest>(serde_yaml::from_str::<
            serde_json::Value,
        >(
            intent_adapter_manifest_text()
        )?)?;
    let options = LivePrepOptions {
        adapter: Some(AdapterInput::Manifest(manifest)),
        egress_config: Some(make_intent_egress_config(&egress_script)),
    };

    let _runner = prepare_hosted_runner_with_surfaces(
        assets,
        &options,
        build_injected_runtime_surfaces(42.0),
    )?;

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn validate_graph_adapter_text_parse_error_mentions_source_label(
) -> Result<(), Box<dyn std::error::Error>> {
    let assets = load_intent_in_memory_assets("host_validate_bad_inline_adapter")?;
    let err = validate_graph_with_surfaces(
        &assets,
        &LivePrepOptions {
            adapter: Some(AdapterInput::Text {
                content: "not: [valid: yaml".to_string(),
                source_label: "inline-adapter-bad".to_string(),
            }),
            egress_config: None,
        },
        build_injected_runtime_surfaces(42.0),
    )
    .expect_err("invalid inline adapter YAML must fail");

    match err {
        HostRunError::Setup(HostSetupError::AdapterSetup(detail)) => {
            assert!(detail.to_string().contains("inline-adapter-bad"));
        }
        other => panic!("unexpected inline adapter parse error: {other:?}"),
    }

    Ok(())
}
