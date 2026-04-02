//! usecases live_run tests
//!
//! Purpose:
//! - Exercise the canonical host live-run and run-summary seams owned by
//!   `live_run.rs`.
//!
//! Owns:
//! - Behavior checks for completed versus interrupted runs, capture handling,
//!   and live-run error propagation through the public host run surface.
//!
//! Does not own:
//! - The public contract for facade-level error enums; `contract.rs` locks that.
//!
//! Connects to:
//! - `live_run.rs`, `process_driver.rs`, and shared host test helpers from the
//!   parent `usecases::tests` module.
//!
//! Safety notes:
//! - These tests intentionally lock host orchestration behavior where partial
//!   capture and interruption semantics are downstream-significant.

use super::super::process_driver::PROCESS_DRIVER_PROTOCOL_VERSION;
use super::*;

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
        &number_source_graph_yaml("host_path_graph", 2.5),
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
    assert_eq!(result.capture_path.as_deref(), Some(capture.as_path()));
    assert_eq!(result.capture_bundle.graph_id.as_str(), "host_path_graph");
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
        &cluster_version_miss_graph_yaml("host_cluster_version_miss"),
    )?;
    write_temp_file(
        &temp_dir,
        "shared_value.yaml",
        &shared_value_graph_yaml("1.5.0", 4.0),
    )?;
    write_temp_file(
        &temp_dir.join("clusters"),
        "shared_value.yaml",
        &shared_value_graph_yaml("1.0.0", 3.0),
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
        HostRunError::Setup(HostSetupError::GraphPreparation(
            detail @ HostGraphPreparationError::Expansion(_),
        )) => {
            let detail = detail.to_string();
            assert!(detail.contains("graph expansion failed"));
            assert!(detail.contains("shared_value"));
            assert!(detail.contains("^2.0"));
            assert!(detail.contains("available: 1.0.0, 1.5.0"));
            assert!(detail.contains("available cluster sources"));
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
        &injected_number_graph_yaml("host_injected_run"),
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
    assert_eq!(result.capture_path.as_deref(), Some(capture.as_path()));
    assert!(capture.exists());

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn run_graph_from_paths_surfaces_reports_adapter_required_before_egress_validation(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-run-adapter-preflight-order-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_intent_graph(&temp_dir, "graph.yaml", "host_run_adapter_preflight")?;
    let fixture = write_temp_file(
        &temp_dir,
        "fixture.jsonl",
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
    )?;
    let err = run_graph_from_paths_with_surfaces(
        RunGraphFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            driver: DriverConfig::Fixture { path: fixture },
            adapter_path: None,
            egress_config: Some(make_intent_egress_config_with_timeout(
                &temp_dir.join("unused-egress.sh"),
                Duration::from_millis(50),
            )),
            capture_output: Some(temp_dir.join("capture.json")),
            pretty_capture: false,
        },
        build_injected_runtime_surfaces(42.0),
    )
    .expect_err("adapter-required error should win before runner init egress validation");

    match err {
        HostRunError::AdapterRequired(summary) => assert!(summary.requires_adapter),
        other => panic!("unexpected canonical adapter-preflight error: {other:?}"),
    }

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
        Err(HostRunError::Setup(HostSetupError::HostedRunnerValidation(detail))) => {
            let detail = detail.to_string();
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

    let bundle: CaptureBundle = serde_json::from_str(&fs::read_to_string(
        interrupted
            .summary
            .capture_path
            .as_ref()
            .expect("path-backed run should produce capture path"),
    )?)?;
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
        &number_source_graph_yaml("host_no_egress_prov", 1.0),
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
        &context_set_no_intents_graph_yaml("host_egress_no_intents"),
    )?;
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

    assert!(matches!(
        err,
        HostRunError::Setup(HostSetupError::StartEgress(_))
    ));

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

    assert!(matches!(
        err,
        HostRunError::Step(HostedStepError::EgressProcess(_))
    ));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn fixture_max_events_returns_host_stop_interruption() -> Result<(), Box<dyn std::error::Error>> {
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
        &number_source_graph_yaml("host_fixture_max_events", 2.5),
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
        &injected_number_graph_yaml("host_fixture_external_stop"),
    )?;
    let total_events = 4096;
    let mut fixture = String::from("{\"kind\":\"episode_start\",\"id\":\"E1\"}\n");
    for _ in 0..total_events {
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
        interrupted.summary.events < total_events,
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
fn host_stop_still_shuts_down_egress_channels_cleanly() -> Result<(), Box<dyn std::error::Error>> {
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
    let egress_script = write_egress_end_sentinel_script(&temp_dir, "egress.sh", &sentinel)?;
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
fn run_graph_from_assets_executes_simple_graph_with_in_memory_capture(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-run-assets-memory-capture-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let assets = load_core_in_memory_assets("host_assets_memory_capture")?;
    let fixture = write_temp_file(
        &temp_dir,
        "fixture.jsonl",
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
    )?;

    let summary = expect_assets_completed(run_graph_from_assets(RunGraphFromAssetsRequest {
        assets,
        prep: LivePrepOptions::default(),
        driver: DriverConfig::Fixture { path: fixture },
        capture: CapturePolicy::InMemory,
    }))?;

    assert_eq!(summary.capture_path, None);
    assert_eq!(summary.events, 1);
    assert_eq!(
        summary.capture_bundle.graph_id.as_str(),
        "host_assets_memory_capture"
    );

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn run_graph_from_assets_executes_fixture_items_with_in_memory_capture(
) -> Result<(), Box<dyn std::error::Error>> {
    let assets = load_core_in_memory_assets("host_assets_fixture_items_memory_capture")?;
    let items = vec![
        ergo_adapter::fixture::FixtureItem::EpisodeStart {
            label: "E1".to_string(),
        },
        ergo_adapter::fixture::FixtureItem::Event {
            id: Some("evt1".to_string()),
            kind: ExternalEventKind::Command,
            payload: Some(serde_json::json!({})),
            semantic_kind: None,
        },
    ];

    let summary = expect_assets_completed(run_graph_from_assets(RunGraphFromAssetsRequest {
        assets,
        prep: LivePrepOptions::default(),
        driver: DriverConfig::FixtureItems {
            items,
            source_label: "memory-fixture".to_string(),
        },
        capture: CapturePolicy::InMemory,
    }))?;

    assert_eq!(summary.capture_path, None);
    assert_eq!(summary.events, 1);
    assert_eq!(
        summary.capture_bundle.graph_id.as_str(),
        "host_assets_fixture_items_memory_capture"
    );
    Ok(())
}

#[test]
fn run_graph_from_assets_executes_simple_graph_with_explicit_file_capture(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-run-assets-file-capture-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let assets = load_core_in_memory_assets("host_assets_file_capture")?;
    let fixture = write_temp_file(
        &temp_dir,
        "fixture.jsonl",
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
    )?;
    let capture = temp_dir.join("capture.json");

    let summary = expect_assets_completed(run_graph_from_assets(RunGraphFromAssetsRequest {
        assets,
        prep: LivePrepOptions::default(),
        driver: DriverConfig::Fixture { path: fixture },
        capture: CapturePolicy::File {
            path: capture.clone(),
            pretty: false,
        },
    }))?;

    assert_eq!(summary.capture_path.as_deref(), Some(capture.as_path()));
    assert!(capture.exists());
    let bundle: CaptureBundle = serde_json::from_str(&fs::read_to_string(&capture)?)?;
    assert_eq!(bundle.graph_id.as_str(), "host_assets_file_capture");

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn run_graph_from_assets_executes_fixture_items_with_explicit_file_capture(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-run-assets-fixture-items-file-capture-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let assets = load_core_in_memory_assets("host_assets_fixture_items_file_capture")?;
    let items = vec![
        ergo_adapter::fixture::FixtureItem::EpisodeStart {
            label: "E1".to_string(),
        },
        ergo_adapter::fixture::FixtureItem::Event {
            id: Some("evt1".to_string()),
            kind: ExternalEventKind::Command,
            payload: Some(serde_json::json!({})),
            semantic_kind: None,
        },
    ];
    let capture = temp_dir.join("capture.json");

    let summary = expect_assets_completed(run_graph_from_assets(RunGraphFromAssetsRequest {
        assets,
        prep: LivePrepOptions::default(),
        driver: DriverConfig::FixtureItems {
            items,
            source_label: "memory-fixture".to_string(),
        },
        capture: CapturePolicy::File {
            path: capture.clone(),
            pretty: false,
        },
    }))?;

    assert_eq!(summary.capture_path.as_deref(), Some(capture.as_path()));
    assert_eq!(summary.events, 1);
    assert_eq!(
        summary.capture_bundle.graph_id.as_str(),
        "host_assets_fixture_items_file_capture"
    );
    assert!(capture.exists());
    let bundle: CaptureBundle = serde_json::from_str(&fs::read_to_string(&capture)?)?;
    assert_eq!(
        bundle.graph_id.as_str(),
        "host_assets_fixture_items_file_capture"
    );

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn run_graph_from_assets_with_surfaces_uses_injected_runtime_surfaces(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-run-assets-injected-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let assets = load_injected_in_memory_assets("host_assets_injected_capture")?;
    let fixture = write_temp_file(
        &temp_dir,
        "fixture.jsonl",
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
    )?;

    let summary = expect_assets_completed(run_graph_from_assets_with_surfaces(
        RunGraphFromAssetsRequest {
            assets,
            prep: LivePrepOptions::default(),
            driver: DriverConfig::Fixture { path: fixture },
            capture: CapturePolicy::InMemory,
        },
        build_injected_runtime_surfaces(7.5),
    ))?;

    assert_eq!(
        summary.capture_bundle.graph_id.as_str(),
        "host_assets_injected_capture"
    );
    assert_eq!(summary.events, 1);

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn run_graph_from_assets_reports_adapter_required_before_egress_validation(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-run-assets-adapter-required-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let assets = load_intent_in_memory_assets("host_assets_adapter_required")?;
    let fixture = write_temp_file(
        &temp_dir,
        "fixture.jsonl",
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
    )?;
    let err = run_graph_from_assets_with_surfaces(
        RunGraphFromAssetsRequest {
            assets,
            prep: LivePrepOptions {
                adapter: None,
                egress_config: Some(make_intent_egress_config_with_timeout(
                    &temp_dir.join("unused-egress.sh"),
                    Duration::from_millis(50),
                )),
            },
            driver: DriverConfig::Fixture { path: fixture },
            capture: CapturePolicy::InMemory,
        },
        build_injected_runtime_surfaces(42.0),
    )
    .expect_err("adapter-required error should win before driver execution");

    match err {
        HostRunError::AdapterRequired(summary) => assert!(summary.requires_adapter),
        other => panic!("unexpected assets adapter-preflight error: {other:?}"),
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn run_graph_from_assets_fixture_items_preserve_adapter_required_preflight(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-run-assets-fixture-items-adapter-required-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let assets = load_intent_in_memory_assets("host_assets_fixture_items_adapter_required")?;
    let err = run_graph_from_assets_with_surfaces(
        RunGraphFromAssetsRequest {
            assets,
            prep: LivePrepOptions {
                adapter: None,
                egress_config: Some(make_intent_egress_config_with_timeout(
                    &temp_dir.join("unused-egress.sh"),
                    Duration::from_millis(50),
                )),
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
    .expect_err("adapter-required error should win before fixture-items execution");

    match err {
        HostRunError::AdapterRequired(summary) => assert!(summary.requires_adapter),
        other => panic!("unexpected fixture-items adapter-preflight error: {other:?}"),
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn run_graph_from_assets_process_driver_preserves_adapter_required_preflight(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-run-assets-process-adapter-required-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let assets = load_intent_in_memory_assets("host_assets_process_adapter_required")?;
    let err = run_graph_from_assets_with_surfaces(
        RunGraphFromAssetsRequest {
            assets,
            prep: LivePrepOptions {
                adapter: None,
                egress_config: Some(make_intent_egress_config_with_timeout(
                    &temp_dir.join("unused-egress.sh"),
                    Duration::from_millis(50),
                )),
            },
            driver: DriverConfig::Process {
                command: vec![
                    "/bin/sh".to_string(),
                    temp_dir.join("never-run.sh").display().to_string(),
                ],
            },
            capture: CapturePolicy::InMemory,
        },
        build_injected_runtime_surfaces(42.0),
    )
    .expect_err("adapter-required error should win before process driver launch");

    match err {
        HostRunError::AdapterRequired(summary) => assert!(summary.requires_adapter),
        other => panic!("unexpected process-driver adapter-preflight error: {other:?}"),
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}
