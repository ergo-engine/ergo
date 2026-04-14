//! usecases process_driver tests
//!
//! Purpose:
//! - Exercise the host-owned process-ingress protocol and lifecycle seam from
//!   the outside through the canonical run surface.
//!
//! Owns:
//! - Behavior checks for hello ordering, startup bounds, protocol violations,
//!   and interruption/capture consequences specific to process ingress.
//!
//! Does not own:
//! - The public contract for facade-level error enums; `contract.rs` locks that.
//!
//! Connects to:
//! - `process_driver.rs`, `live_run.rs`, and shared host test helpers from the
//!   parent `usecases::tests` module.
//!
//! Safety notes:
//! - These tests intentionally pin the host-owned process-driver protocol token
//!   and lifecycle boundary because CLI and SDK rely on the resulting run/error
//!   behavior.

use super::*;
use crate::PROCESS_DRIVER_PROTOCOL_VERSION;

fn wait_for_nonempty_file(
    path: &Path,
    timeout: Duration,
) -> Result<String, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + timeout;
    loop {
        if path.exists() {
            let contents = fs::read_to_string(path)?;
            if !contents.trim().is_empty() {
                return Ok(contents);
            }
        }

        if Instant::now() >= deadline {
            return Err(
                format!("timed out waiting for non-empty file '{}'", path.display()).into(),
            );
        }

        thread::sleep(Duration::from_millis(10));
    }
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
        &number_source_graph_yaml("host_process_graph", 2.5),
    )?;
    let driver = write_process_driver_script(
        &temp_dir,
        "driver.sh",
        &[
            serde_json::to_string(
                &json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}),
            )?,
            serde_json::to_string(
                &json!({"type":"event","event":hosted_event_with_semantic_kind("evt1")}),
            )?,
            serde_json::to_string(&json!({"type":"end"}))?,
        ],
    )?;
    let adapter = write_minimal_adapter_manifest(&temp_dir, "adapter.yaml")?;
    let capture = temp_dir.join("capture.json");

    let result = run_graph_from_paths(RunGraphFromPathsRequest {
        graph_path: graph,
        cluster_paths: Vec::new(),
        driver: DriverConfig::Process {
            command: vec!["/bin/sh".to_string(), driver.display().to_string()],
        },
        adapter_path: Some(adapter),
        egress_config: None,
        capture_output: Some(capture.clone()),
        pretty_capture: false,
    })?;

    match result {
        RunOutcome::Completed(summary) => {
            assert_eq!(summary.episodes, 1);
            assert_eq!(summary.events, 1);
            assert_eq!(summary.capture_path.as_deref(), Some(capture.as_path()));
            assert!(summary
                .capture_path
                .as_ref()
                .is_some_and(|path| path.exists()));
        }
        RunOutcome::Interrupted(interrupted) => {
            return Err(format!("expected completed run, got {:?}", interrupted.reason).into())
        }
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn process_driver_assets_lane_matches_path_lane_summary_and_capture_shape(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-process-assets-parity-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_temp_file(
        &temp_dir,
        "graph.yaml",
        &core_graph_yaml("host_process_assets_parity"),
    )?;
    let assets = load_core_in_memory_assets("host_process_assets_parity")?;
    let driver = write_process_driver_script(
        &temp_dir,
        "driver.sh",
        &[
            serde_json::to_string(
                &json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}),
            )?,
            serde_json::to_string(
                &json!({"type":"event","event":hosted_event_with_semantic_kind("evt1")}),
            )?,
            serde_json::to_string(&json!({"type":"end"}))?,
        ],
    )?;
    let adapter = write_minimal_adapter_manifest(&temp_dir, "adapter.yaml")?;
    let path_capture = temp_dir.join("capture-path.json");
    let assets_capture = temp_dir.join("capture-assets.json");

    let path_summary = expect_completed(run_graph_from_paths_with_process_policy(
        RunGraphFromPathsRequest {
            graph_path: graph.clone(),
            cluster_paths: Vec::new(),
            driver: DriverConfig::Process {
                command: vec!["/bin/sh".to_string(), driver.display().to_string()],
            },
            adapter_path: Some(adapter.clone()),
            egress_config: None,
            capture_output: Some(path_capture.clone()),
            pretty_capture: false,
        },
        short_test_process_driver_policy(),
    ))?;
    let assets_summary = expect_assets_completed(run_graph_from_assets_with_process_policy(
        RunGraphFromAssetsRequest {
            assets,
            prep: LivePrepOptions {
                adapter: Some(AdapterInput::Text {
                    content: minimal_adapter_manifest_text().to_string(),
                    source_label: "inline-minimal-adapter".to_string(),
                }),
                egress_config: None,
                session_intent: SessionIntent::Production,
            },
            driver: DriverConfig::Process {
                command: vec!["/bin/sh".to_string(), driver.display().to_string()],
            },
            capture: CapturePolicy::File {
                path: assets_capture.clone(),
                pretty: false,
            },
        },
        short_test_process_driver_policy(),
    ))?;

    assert_eq!(assets_summary.episodes, path_summary.episodes);
    assert_eq!(assets_summary.events, path_summary.events);
    assert_eq!(assets_summary.invoked, path_summary.invoked);
    assert_eq!(assets_summary.deferred, path_summary.deferred);
    assert_eq!(
        assets_summary.episode_event_counts,
        path_summary.episode_event_counts
    );
    assert_eq!(
        assets_summary.capture_path.as_deref(),
        Some(assets_capture.as_path())
    );
    assert!(path_capture.exists());
    assert!(assets_capture.exists());

    let path_bundle: CaptureBundle = serde_json::from_str(&fs::read_to_string(&path_capture)?)?;
    let assets_bundle: CaptureBundle = serde_json::from_str(&fs::read_to_string(&assets_capture)?)?;
    assert_eq!(assets_bundle.graph_id, path_bundle.graph_id);
    assert_eq!(assets_bundle.events.len(), path_bundle.events.len());
    assert_eq!(assets_bundle.decisions.len(), path_bundle.decisions.len());
    assert_eq!(
        decision_counts(&assets_bundle),
        decision_counts(&path_bundle)
    );

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
        &number_source_graph_yaml("host_process_stop_zero_event", 2.5),
    )?;
    let hello =
        serde_json::to_string(&json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}))?;
    let driver = write_process_driver_program(
        &temp_dir,
        "driver.sh",
        &format!("#!/bin/sh\nprintf '%s\\n' '{hello}'\nexec sleep 5\n"),
    )?;
    let adapter = write_minimal_adapter_manifest(&temp_dir, "adapter.yaml")?;
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
            adapter_path: Some(adapter.clone()),
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
        HostRunError::Driver(HostDriverError::Output(
            HostDriverOutputError::StopBeforeFirstCommittedEvent,
        )) => {}
        other => panic!("expected driver output host-stop error, got {other:?}"),
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
        &number_source_graph_yaml("host_process_max_events", 2.5),
    )?;
    let hello =
        serde_json::to_string(&json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}))?;
    let mut body = format!("#!/bin/sh\ncat <<'__ERGO_DRIVER__'\n{hello}\n");
    for index in 1..=64 {
        body.push_str(&serde_json::to_string(&json!({
            "type":"event",
            "event": hosted_event_with_semantic_kind(&format!("evt{index}"))
        }))?);
        body.push('\n');
    }
    body.push_str("__ERGO_DRIVER__\nexec sleep 5\n");
    let driver = write_process_driver_program(&temp_dir, "driver.sh", &body)?;
    let adapter = write_minimal_adapter_manifest(&temp_dir, "adapter.yaml")?;
    let capture = temp_dir.join("capture.json");

    let outcome = run_graph_from_paths_with_process_policy_and_control(
        RunGraphFromPathsRequest {
            graph_path: graph.clone(),
            cluster_paths: Vec::new(),
            driver: DriverConfig::Process {
                command: vec!["/bin/sh".to_string(), driver.display().to_string()],
            },
            adapter_path: Some(adapter.clone()),
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
        adapter_path: Some(adapter.clone()),
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
        &number_source_graph_yaml("host_process_hot_stop", 2.5),
    )?;
    let marker = temp_dir.join("emitted-events.log");
    let hello =
        serde_json::to_string(&json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}))?;
    let mut body = format!(
        "#!/bin/sh\nmarker='{}'\nprintf '%s\\n' '{hello}'\n",
        marker.display()
    );
    for index in 1..=256 {
        let event_line = serde_json::to_string(&json!({
            "type":"event",
            "event": hosted_event_with_semantic_kind(&format!("hot_evt{index}"))
        }))?;
        body.push_str(&format!(
            "printf '%s\\n' '{index}' >> \"$marker\"\nprintf '%s\\n' '{event_line}'\nsleep 0.001\n"
        ));
    }
    body.push_str("exec sleep 5\n");
    let driver = write_process_driver_program(&temp_dir, "driver.sh", &body)?;
    let adapter = write_minimal_adapter_manifest(&temp_dir, "adapter.yaml")?;
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
            adapter_path: Some(adapter.clone()),
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
        interrupted.summary.events >= 3,
        "expected several committed events before stop, got {}",
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
fn process_driver_runs_in_separate_process_group() -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-process-pgid-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let pgid_path = temp_dir.join("driver.pgid");
    let driver = write_process_driver_program(
        &temp_dir,
        "driver.sh",
        &format!(
            "#!/bin/sh\nps -o pgid= -p $$ | tr -d ' ' > '{}'\nexec sleep 5\n",
            pgid_path.display()
        ),
    )?;
    let _adapter = write_minimal_adapter_manifest(&temp_dir, "adapter.yaml")?;

    let mut child = spawn_process_driver(&["/bin/sh".to_string(), driver.display().to_string()])?;
    let child_pgid_text = wait_for_nonempty_file(&pgid_path, Duration::from_secs(1))?;

    let parent_pgid = current_process_group_id()?;
    let child_pgid = child_pgid_text.trim().parse::<u32>()?;
    assert_ne!(
        child_pgid, parent_pgid,
        "process driver should not share the test process group"
    );

    kill_host_managed_child(&mut child);
    let _ = child.wait();

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn process_driver_signal_race_after_stop_still_returns_host_stop_requested(
) -> Result<(), Box<dyn std::error::Error>> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-host-process-stop-signal-race-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir)?;

    let graph = write_temp_file(
        &temp_dir,
        "graph.yaml",
        &number_source_graph_yaml("host_process_stop_signal_race", 2.5),
    )?;
    let capture = temp_dir.join("capture.json");
    let pid_path = temp_dir.join("driver.pid");
    let emitted_path = temp_dir.join("driver.emitted");
    let hello =
        serde_json::to_string(&json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}))?;
    let event = serde_json::to_string(
        &json!({"type":"event","event":hosted_event_with_semantic_kind("evt1")}),
    )?;
    let driver = write_process_driver_program(
        &temp_dir,
        "driver.sh",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' $$ > '{pid}'\nprintf '%s\\n' '{hello}'\nprintf '%s\\n' '{event}'\nprintf '%s\\n' 'emitted' > '{emitted}'\nexec sleep 5\n",
            pid = pid_path.display(),
            hello = hello,
            event = event,
            emitted = emitted_path.display()
        ),
    )?;
    let adapter = write_minimal_adapter_manifest(&temp_dir, "adapter.yaml")?;

    let stop = HostStopHandle::new();
    let stop_clone = stop.clone();
    let pid_path_clone = pid_path.clone();
    let emitted_path_clone = emitted_path.clone();
    let stopper = thread::spawn(move || -> Result<(), String> {
        let pid_text = wait_for_nonempty_file(&pid_path_clone, Duration::from_secs(1))
            .map_err(|err| err.to_string())?;
        wait_for_path(&emitted_path_clone, Duration::from_secs(1))
            .map_err(|err| err.to_string())?;
        thread::sleep(Duration::from_millis(50));
        stop_clone.request_stop();
        let pgid = pid_text
            .trim()
            .parse::<u32>()
            .map_err(|err| format!("parse driver pid '{}': {err}", pid_path_clone.display()))?;
        send_sigint_to_process_group(pgid)
            .map_err(|err| format!("send SIGINT to process driver group: {err}"))?;
        Ok(())
    });

    let outcome = run_graph_from_paths_with_process_policy_and_control(
        RunGraphFromPathsRequest {
            graph_path: graph.clone(),
            cluster_paths: Vec::new(),
            driver: DriverConfig::Process {
                command: vec!["/bin/sh".to_string(), driver.display().to_string()],
            },
            adapter_path: Some(adapter.clone()),
            egress_config: None,
            capture_output: Some(capture.clone()),
            pretty_capture: false,
        },
        ProcessDriverPolicy {
            event_recv_timeout: Duration::from_millis(200),
            ..short_test_process_driver_policy()
        },
        RunControl::new().with_stop_handle(stop),
    )?;
    stopper
        .join()
        .expect("stopper thread must join")
        .map_err(|err| err.to_string())?;

    let interrupted = match outcome {
        RunOutcome::Interrupted(interrupted) => interrupted,
        RunOutcome::Completed(_) => return Err("signal race after host stop must interrupt".into()),
    };
    assert_eq!(interrupted.reason, InterruptionReason::HostStopRequested);
    assert_eq!(interrupted.summary.events, 1);
    assert!(capture.exists());

    let replay = replay_graph_from_paths(ReplayGraphFromPathsRequest {
        capture_path: capture,
        graph_path: graph,
        cluster_paths: Vec::new(),
        adapter_path: Some(adapter.clone()),
    })?;
    assert_eq!(replay.graph_id.as_str(), "host_process_stop_signal_race");
    assert_eq!(replay.events, 1);

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
        &number_source_graph_yaml("host_process_invalid_hello", 2.5),
    )?;
    let driver = write_process_driver_script(
        &temp_dir,
        "driver.sh",
        &[serde_json::to_string(
            &json!({"type":"event","event":hosted_event_with_semantic_kind("evt1")}),
        )?],
    )?;
    let adapter = write_minimal_adapter_manifest(&temp_dir, "adapter.yaml")?;

    let err = run_graph_from_paths(RunGraphFromPathsRequest {
        graph_path: graph,
        cluster_paths: Vec::new(),
        driver: DriverConfig::Process {
            command: vec!["/bin/sh".to_string(), driver.display().to_string()],
        },
        adapter_path: Some(adapter.clone()),
        egress_config: None,
        capture_output: Some(temp_dir.join("capture.json")),
        pretty_capture: false,
    })
    .expect_err("missing hello must fail before first committed step");

    assert!(matches!(
        err,
        HostRunError::Driver(HostDriverError::Protocol(_))
    ));

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
        &number_source_graph_yaml("host_process_startup_timeout", 2.5),
    )?;
    let driver = write_process_driver_program(&temp_dir, "driver.sh", "#!/bin/sh\nexec sleep 5\n")?;
    let adapter = write_minimal_adapter_manifest(&temp_dir, "adapter.yaml")?;

    let started = Instant::now();
    let err = run_graph_from_paths_with_process_policy(
        RunGraphFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            driver: DriverConfig::Process {
                command: vec!["/bin/sh".to_string(), driver.display().to_string()],
            },
            adapter_path: Some(adapter.clone()),
            egress_config: None,
            capture_output: Some(temp_dir.join("capture.json")),
            pretty_capture: false,
        },
        short_test_process_driver_policy(),
    )
    .expect_err("silent startup must time out before canonical run begins");

    assert!(matches!(
        err,
        HostRunError::Driver(HostDriverError::Start(_))
    ));
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
        &number_source_graph_yaml("host_process_invalid_bytes_start", 2.5),
    )?;
    let hello =
        serde_json::to_string(&json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}))?;
    let driver = write_process_driver_program(
        &temp_dir,
        "driver.sh",
        &format!("#!/bin/sh\nprintf '%s\\n' '{hello}'\nprintf '\\377\\n'\n"),
    )?;
    let adapter = write_minimal_adapter_manifest(&temp_dir, "adapter.yaml")?;

    let err = run_graph_from_paths(RunGraphFromPathsRequest {
        graph_path: graph,
        cluster_paths: Vec::new(),
        driver: DriverConfig::Process {
            command: vec!["/bin/sh".to_string(), driver.display().to_string()],
        },
        adapter_path: Some(adapter.clone()),
        egress_config: None,
        capture_output: Some(temp_dir.join("capture.json")),
        pretty_capture: false,
    })
    .expect_err("malformed bytes before first committed step must be protocol failure");

    assert!(matches!(
        err,
        HostRunError::Driver(HostDriverError::Protocol(_))
    ));

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
        &number_source_graph_yaml("host_process_interrupted", 2.5),
    )?;
    let driver = write_process_driver_script(
        &temp_dir,
        "driver.sh",
        &[
            serde_json::to_string(
                &json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}),
            )?,
            serde_json::to_string(
                &json!({"type":"event","event":hosted_event_with_semantic_kind("evt1")}),
            )?,
            "{not-json".to_string(),
        ],
    )?;
    let adapter = write_minimal_adapter_manifest(&temp_dir, "adapter.yaml")?;
    let capture = temp_dir.join("capture.json");

    let outcome = run_graph_from_paths(RunGraphFromPathsRequest {
        graph_path: graph.clone(),
        cluster_paths: Vec::new(),
        driver: DriverConfig::Process {
            command: vec!["/bin/sh".to_string(), driver.display().to_string()],
        },
        adapter_path: Some(adapter.clone()),
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
    assert_eq!(
        interrupted.summary.capture_path.as_deref(),
        Some(capture.as_path())
    );
    assert!(interrupted
        .summary
        .capture_path
        .as_ref()
        .is_some_and(|path| path.exists()));

    let replay = replay_graph_from_paths(ReplayGraphFromPathsRequest {
        capture_path: capture,
        graph_path: graph,
        cluster_paths: Vec::new(),
        adapter_path: Some(adapter.clone()),
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
        &number_source_graph_yaml("host_process_invalid_bytes_after_start", 2.5),
    )?;
    let hello =
        serde_json::to_string(&json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}))?;
    let event = serde_json::to_string(
        &json!({"type":"event","event":hosted_event_with_semantic_kind("evt1")}),
    )?;
    let driver = write_process_driver_program(
        &temp_dir,
        "driver.sh",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' '{hello}'\nprintf '%s\\n' '{event}'\nprintf '\\377\\n'\n"
        ),
    )?;
    let adapter = write_minimal_adapter_manifest(&temp_dir, "adapter.yaml")?;
    let capture = temp_dir.join("capture.json");

    let outcome = run_graph_from_paths(RunGraphFromPathsRequest {
        graph_path: graph.clone(),
        cluster_paths: Vec::new(),
        driver: DriverConfig::Process {
            command: vec!["/bin/sh".to_string(), driver.display().to_string()],
        },
        adapter_path: Some(adapter.clone()),
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
        adapter_path: Some(adapter.clone()),
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
        &number_source_graph_yaml("host_process_nonzero_end", 2.5),
    )?;
    let hello =
        serde_json::to_string(&json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}))?;
    let event = serde_json::to_string(
        &json!({"type":"event","event":hosted_event_with_semantic_kind("evt1")}),
    )?;
    let end = serde_json::to_string(&json!({"type":"end"}))?;
    let driver = write_process_driver_program(
        &temp_dir,
        "driver.sh",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' '{hello}'\nprintf '%s\\n' '{event}'\nprintf '%s\\n' '{end}'\nexit 1\n"
        ),
    )?;
    let adapter = write_minimal_adapter_manifest(&temp_dir, "adapter.yaml")?;
    let capture = temp_dir.join("capture.json");

    let outcome = run_graph_from_paths_with_process_policy(
        RunGraphFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            driver: DriverConfig::Process {
                command: vec!["/bin/sh".to_string(), driver.display().to_string()],
            },
            adapter_path: Some(adapter.clone()),
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
        &number_source_graph_yaml("host_process_delayed_clean_exit", 2.5),
    )?;
    let hello =
        serde_json::to_string(&json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}))?;
    let event = serde_json::to_string(
        &json!({"type":"event","event":hosted_event_with_semantic_kind("evt1")}),
    )?;
    let end = serde_json::to_string(&json!({"type":"end"}))?;
    let driver = write_process_driver_program(
        &temp_dir,
        "driver.sh",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' '{hello}'\nprintf '%s\\n' '{event}'\nprintf '%s\\n' '{end}'\nsleep 0.01\nexit 0\n"
        ),
    )?;
    let adapter = write_minimal_adapter_manifest(&temp_dir, "adapter.yaml")?;
    let capture = temp_dir.join("capture.json");

    let summary = expect_completed(run_graph_from_paths_with_process_policy(
        RunGraphFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            driver: DriverConfig::Process {
                command: vec!["/bin/sh".to_string(), driver.display().to_string()],
            },
            adapter_path: Some(adapter.clone()),
            egress_config: None,
            capture_output: Some(capture.clone()),
            pretty_capture: false,
        },
        short_test_process_driver_policy(),
    ))?;

    assert_eq!(summary.events, 1);
    assert_eq!(summary.capture_path.as_deref(), Some(capture.as_path()));
    assert!(summary
        .capture_path
        .as_ref()
        .is_some_and(|path| path.exists()));

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
        &number_source_graph_yaml("host_process_extra_after_end", 2.5),
    )?;
    let hello =
        serde_json::to_string(&json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}))?;
    let event = serde_json::to_string(
        &json!({"type":"event","event":hosted_event_with_semantic_kind("evt1")}),
    )?;
    let end = serde_json::to_string(&json!({"type":"end"}))?;
    let driver = write_process_driver_program(
        &temp_dir,
        "driver.sh",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' '{hello}'\nprintf '%s\\n' '{event}'\nprintf '%s\\n' '{end}'\nprintf '%s\\n' 'trailing-garbage'\n"
        ),
    )?;
    let adapter = write_minimal_adapter_manifest(&temp_dir, "adapter.yaml")?;
    let capture = temp_dir.join("capture.json");

    let outcome = run_graph_from_paths_with_process_policy(
        RunGraphFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            driver: DriverConfig::Process {
                command: vec!["/bin/sh".to_string(), driver.display().to_string()],
            },
            adapter_path: Some(adapter.clone()),
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
        &number_source_graph_yaml("host_process_hang_after_end", 2.5),
    )?;
    let hello =
        serde_json::to_string(&json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}))?;
    let event = serde_json::to_string(
        &json!({"type":"event","event":hosted_event_with_semantic_kind("evt1")}),
    )?;
    let end = serde_json::to_string(&json!({"type":"end"}))?;
    let driver = write_process_driver_program(
        &temp_dir,
        "driver.sh",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' '{hello}'\nprintf '%s\\n' '{event}'\nprintf '%s\\n' '{end}'\nexec sleep 5\n"
        ),
    )?;
    let adapter = write_minimal_adapter_manifest(&temp_dir, "adapter.yaml")?;
    let capture = temp_dir.join("capture.json");

    let started = Instant::now();
    let outcome = run_graph_from_paths_with_process_policy(
        RunGraphFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            driver: DriverConfig::Process {
                command: vec!["/bin/sh".to_string(), driver.display().to_string()],
            },
            adapter_path: Some(adapter.clone()),
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
        &number_source_graph_yaml("host_process_eof_hang", 2.5),
    )?;
    let hello =
        serde_json::to_string(&json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}))?;
    let event = serde_json::to_string(
        &json!({"type":"event","event":hosted_event_with_semantic_kind("evt1")}),
    )?;
    let driver = write_process_driver_program(
        &temp_dir,
        "driver.sh",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' '{hello}'\nprintf '%s\\n' '{event}'\nexec 1>&-\nexec sleep 5\n"
        ),
    )?;
    let adapter = write_minimal_adapter_manifest(&temp_dir, "adapter.yaml")?;
    let capture = temp_dir.join("capture.json");

    let started = Instant::now();
    let outcome = run_graph_from_paths_with_process_policy(
        RunGraphFromPathsRequest {
            graph_path: graph,
            cluster_paths: Vec::new(),
            driver: DriverConfig::Process {
                command: vec!["/bin/sh".to_string(), driver.display().to_string()],
            },
            adapter_path: Some(adapter.clone()),
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
