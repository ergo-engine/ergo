use super::*;
use ergo_adapter::{EventTime, ExternalEventKind};
use ergo_host::{HostedEvent, PROCESS_DRIVER_PROTOCOL_VERSION};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_temp_file(
    base: &std::path::Path,
    name: &str,
    contents: &str,
) -> Result<std::path::PathBuf, String> {
    let path = base.join(name);
    fs::write(&path, contents).map_err(|err| format!("write {}: {err}", path.display()))?;
    Ok(path)
}

fn write_process_driver_script(
    base: &Path,
    name: &str,
    lines: &[String],
) -> Result<PathBuf, String> {
    let script = format!(
        "#!/bin/sh\ncat <<'__ERGO_DRIVER__'\n{}\n__ERGO_DRIVER__\n",
        lines.join("\n")
    );
    write_temp_file(base, name, &script)
}

fn host_event(event_id: &str) -> HostedEvent {
    HostedEvent {
        event_id: event_id.to_string(),
        kind: ExternalEventKind::Command,
        at: EventTime::default(),
        semantic_kind: Some("tick".to_string()),
        payload: Some(json!({})),
    }
}

fn write_minimal_adapter(base: &std::path::Path) -> Result<std::path::PathBuf, String> {
    write_temp_file(
        base,
        "adapter.yaml",
        r#"kind: adapter
id: minimal_test_adapter
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys: []
event_kinds:
  - name: tick
    payload_schema:
      type: object
      additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.tick
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
"#,
    )
}

#[test]
fn parse_run_options_supports_short_flags_and_capture_alias() -> Result<(), String> {
    let opts = parse_run_options(&[
        "-f".to_string(),
        "fixture.jsonl".to_string(),
        "-a".to_string(),
        "adapter.yaml".to_string(),
        "--egress-config".to_string(),
        "egress.toml".to_string(),
        "-o".to_string(),
        "capture-short.json".to_string(),
        "-p".to_string(),
        "--cluster-path".to_string(),
        "clusters".to_string(),
    ])?;
    assert!(matches!(
        opts.driver,
        DriverConfig::Fixture { ref path } if path == Path::new("fixture.jsonl")
    ));
    assert_eq!(
        opts.adapter_path.as_deref(),
        Some(Path::new("adapter.yaml"))
    );
    assert_eq!(
        opts.egress_config_path.as_deref(),
        Some(Path::new("egress.toml"))
    );
    assert_eq!(
        opts.capture_output.as_deref(),
        Some(Path::new("capture-short.json"))
    );
    assert!(opts.pretty_capture);
    assert_eq!(opts.cluster_paths, vec![PathBuf::from("clusters")]);

    let alias_opts = parse_run_options(&[
        "-f".to_string(),
        "fixture.jsonl".to_string(),
        "--capture".to_string(),
        "capture-alias.json".to_string(),
    ])?;
    assert!(matches!(
        alias_opts.driver,
        DriverConfig::Fixture { ref path } if path == Path::new("fixture.jsonl")
    ));
    assert_eq!(
        alias_opts.capture_output.as_deref(),
        Some(Path::new("capture-alias.json"))
    );
    Ok(())
}

#[test]
fn parse_run_options_keeps_long_flag_compatibility() -> Result<(), String> {
    let opts = parse_run_options(&[
        "--fixture".to_string(),
        "fixture.jsonl".to_string(),
        "--adapter".to_string(),
        "adapter.yaml".to_string(),
        "--capture-output".to_string(),
        "capture-long.json".to_string(),
        "--pretty-capture".to_string(),
    ])?;
    assert!(matches!(
        opts.driver,
        DriverConfig::Fixture { ref path } if path == Path::new("fixture.jsonl")
    ));
    assert_eq!(
        opts.adapter_path.as_deref(),
        Some(Path::new("adapter.yaml"))
    );
    assert_eq!(
        opts.capture_output.as_deref(),
        Some(Path::new("capture-long.json"))
    );
    assert!(opts.pretty_capture);
    Ok(())
}

#[test]
fn parse_run_options_unknown_flag_is_actionable() {
    let err =
        parse_run_options(&["--bogus".to_string()]).expect_err("unknown run flag should fail");
    assert!(
        err.contains("code: cli.invalid_option")
            && err.contains("where: arg '--bogus'")
            && err.contains("fix: use -a|--adapter, --egress-config, -f|--fixture"),
        "unexpected error: {err}"
    );
}

#[test]
fn parse_run_options_supports_process_driver_argv() -> Result<(), String> {
    let opts = parse_run_options(&[
        "--driver-cmd".to_string(),
        "/bin/sh".to_string(),
        "--driver-arg".to_string(),
        "driver.sh".to_string(),
        "--driver-arg".to_string(),
        "--flag".to_string(),
    ])?;
    assert!(matches!(
        opts.driver,
        DriverConfig::Process { ref command }
            if command
                == &vec![
                    "/bin/sh".to_string(),
                    "driver.sh".to_string(),
                    "--flag".to_string()
                ]
    ));
    Ok(())
}

#[test]
fn parse_run_options_requires_exactly_one_ingress_source() {
    let missing =
        parse_run_options(&[]).expect_err("missing ingress should produce actionable error");
    assert!(
        missing.contains("run requires either --fixture <events.jsonl> or --driver-cmd <program>"),
        "unexpected missing ingress error: {missing}"
    );

    let conflicting = parse_run_options(&[
        "--fixture".to_string(),
        "fixture.jsonl".to_string(),
        "--driver-cmd".to_string(),
        "/bin/sh".to_string(),
    ])
    .expect_err("conflicting ingress should fail");
    assert!(
        conflicting.contains("run accepts either --fixture or --driver-cmd, not both"),
        "unexpected conflicting ingress error: {conflicting}"
    );
}

#[test]
fn run_graph_command_executes_fixture_driver_via_host() -> Result<(), String> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-graph-yaml-run-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

    let graph = write_temp_file(
        &temp_dir,
        "graph.yaml",
        r#"
kind: cluster
id: graph_yaml_run
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

    let args = vec![
        "--fixture".to_string(),
        fixture.to_string_lossy().to_string(),
        "--capture-output".to_string(),
        capture.to_string_lossy().to_string(),
    ];
    let summary = run_graph_command(&graph, &args)?;

    assert_eq!(summary.completion, GraphRunCompletion::Completed);
    assert_eq!(summary.episodes, 1);
    assert_eq!(summary.events, 1);
    assert_eq!(summary.capture_path, capture);
    assert!(summary.capture_path.exists());

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn run_graph_command_executes_process_driver_via_host() -> Result<(), String> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-graph-yaml-process-run-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

    let graph = write_temp_file(
        &temp_dir,
        "graph.yaml",
        r#"
kind: cluster
id: graph_yaml_process_run
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
    let adapter = write_minimal_adapter(&temp_dir)?;
    let driver = write_process_driver_script(
        &temp_dir,
        "driver.sh",
        &[
            serde_json::to_string(
                &json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}),
            )
            .map_err(|err| format!("serialize hello: {err}"))?,
            serde_json::to_string(&json!({"type":"event","event":host_event("evt1")}))
                .map_err(|err| format!("serialize event: {err}"))?,
            serde_json::to_string(&json!({"type":"end"}))
                .map_err(|err| format!("serialize end: {err}"))?,
        ],
    )?;
    let capture = temp_dir.join("capture.json");

    let args = vec![
        "--adapter".to_string(),
        adapter.to_string_lossy().to_string(),
        "--driver-cmd".to_string(),
        "/bin/sh".to_string(),
        "--driver-arg".to_string(),
        driver.to_string_lossy().to_string(),
        "--capture-output".to_string(),
        capture.to_string_lossy().to_string(),
    ];
    let summary = run_graph_command(&graph, &args)?;

    assert_eq!(summary.completion, GraphRunCompletion::Completed);
    assert_eq!(summary.episodes, 1);
    assert_eq!(summary.events, 1);
    assert_eq!(summary.capture_path, capture);
    assert!(summary.capture_path.exists());

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn run_graph_command_reports_interrupted_process_driver() -> Result<(), String> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-graph-yaml-process-interrupted-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

    let graph = write_temp_file(
        &temp_dir,
        "graph.yaml",
        r#"
kind: cluster
id: graph_yaml_process_interrupted
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
    let adapter = write_minimal_adapter(&temp_dir)?;
    let driver = write_process_driver_script(
        &temp_dir,
        "driver.sh",
        &[
            serde_json::to_string(
                &json!({"type":"hello","protocol":PROCESS_DRIVER_PROTOCOL_VERSION}),
            )
            .map_err(|err| format!("serialize hello: {err}"))?,
            serde_json::to_string(&json!({"type":"event","event":host_event("evt1")}))
                .map_err(|err| format!("serialize event: {err}"))?,
        ],
    )?;
    let capture = temp_dir.join("capture.json");

    let args = vec![
        "--adapter".to_string(),
        adapter.to_string_lossy().to_string(),
        "--driver-cmd".to_string(),
        "/bin/sh".to_string(),
        "--driver-arg".to_string(),
        driver.to_string_lossy().to_string(),
        "--capture-output".to_string(),
        capture.to_string_lossy().to_string(),
    ];
    let summary = run_graph_command(&graph, &args)?;

    assert_eq!(
        summary.completion,
        GraphRunCompletion::Interrupted {
            reason: InterruptionReason::DriverTerminated,
        }
    );
    assert_eq!(summary.episodes, 1);
    assert_eq!(summary.events, 1);
    assert_eq!(summary.capture_path, capture);
    assert!(summary.capture_path.exists());

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}
