#[cfg(test)]
use std::fs;
use std::path::{Path, PathBuf};

use crate::error_format::{render_cli_error, CliErrorInfo};
use crate::output;
use ergo_host::{
    run_graph_from_paths, DriverConfig, InterruptionReason, RunGraphFromPathsRequest, RunOutcome,
};

#[derive(Debug, Clone)]
pub struct GraphRunSummary {
    pub completion: GraphRunCompletion,
    pub episodes: usize,
    pub events: usize,
    pub invoked: usize,
    pub deferred: usize,
    pub capture_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphRunCompletion {
    Completed,
    Interrupted { reason: InterruptionReason },
}

pub fn run_graph_command(graph_path: &Path, args: &[String]) -> Result<GraphRunSummary, String> {
    let opts = parse_run_options(args)?;
    let capture_output = opts
        .capture_output
        .clone()
        .or_else(|| Some(default_capture_output_path(graph_path)));
    let outcome = run_graph_from_paths(RunGraphFromPathsRequest {
        graph_path: graph_path.to_path_buf(),
        cluster_paths: opts.cluster_paths,
        driver: opts.driver,
        adapter_path: opts.adapter_path,
        capture_output,
        pretty_capture: opts.pretty_capture,
    })
    .map_err(output::errors::render_host_run_error)?;

    let (completion, summary) = match outcome {
        RunOutcome::Completed(summary) => (GraphRunCompletion::Completed, summary),
        RunOutcome::Interrupted(interrupted) => (
            GraphRunCompletion::Interrupted {
                reason: interrupted.reason,
            },
            interrupted.summary,
        ),
    };

    Ok(GraphRunSummary {
        completion,
        episodes: summary.episodes,
        events: summary.events,
        invoked: summary.invoked,
        deferred: summary.deferred,
        capture_path: summary.capture_path,
    })
}

fn default_capture_output_path(graph_path: &Path) -> PathBuf {
    let stem = graph_path
        .file_stem()
        .map(|part| part.to_string_lossy().to_string())
        .filter(|part| !part.is_empty())
        .unwrap_or_else(|| "graph".to_string());
    let sanitized = sanitize_filename_component(&stem);
    PathBuf::from("target").join(format!("{sanitized}-capture.json"))
}

fn sanitize_filename_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }

    if out.is_empty() {
        "graph".to_string()
    } else {
        out
    }
}

#[derive(Debug)]
struct RunGraphOptions {
    adapter_path: Option<PathBuf>,
    driver: DriverConfig,
    capture_output: Option<PathBuf>,
    pretty_capture: bool,
    cluster_paths: Vec<PathBuf>,
}

fn parse_run_options(args: &[String]) -> Result<RunGraphOptions, String> {
    let mut adapter_path = None;
    let mut fixture_path: Option<PathBuf> = None;
    let mut driver_command: Vec<String> = Vec::new();
    let mut capture_output = None;
    let mut pretty_capture = false;
    let mut cluster_paths = Vec::new();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-a" | "--adapter" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.missing_option_value",
                            format!("{} requires a path", args[i]),
                        )
                        .with_where(format!("arg '{}'", args[i]))
                        .with_fix("provide -a <adapter.yaml>"),
                    )
                })?;
                adapter_path = Some(PathBuf::from(value));
                i += 2;
            }
            "-f" | "--fixture" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.missing_option_value",
                            format!("{} requires a path", args[i]),
                        )
                        .with_where(format!("arg '{}'", args[i]))
                        .with_fix("provide -f <events.jsonl>"),
                    )
                })?;
                fixture_path = Some(PathBuf::from(value));
                i += 2;
            }
            "--driver-cmd" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.missing_option_value",
                            format!("{} requires a program path", args[i]),
                        )
                        .with_where(format!("arg '{}'", args[i]))
                        .with_fix("provide --driver-cmd <program>"),
                    )
                })?;
                driver_command = vec![value.clone()];
                i += 2;
            }
            "--driver-arg" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.missing_option_value",
                            format!("{} requires an argv value", args[i]),
                        )
                        .with_where(format!("arg '{}'", args[i]))
                        .with_fix("provide --driver-arg <value>"),
                    )
                })?;
                if driver_command.is_empty() {
                    return Err(render_cli_error(
                        &CliErrorInfo::new(
                            "cli.invalid_option_order",
                            "--driver-arg requires --driver-cmd first",
                        )
                        .with_where(format!("arg '{}'", args[i]))
                        .with_fix("provide --driver-cmd <program> before any --driver-arg"),
                    ));
                }
                driver_command.push(value.clone());
                i += 2;
            }
            "-o" | "--capture" | "--capture-output" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.missing_option_value",
                            format!("{} requires a path", args[i]),
                        )
                        .with_where(format!("arg '{}'", args[i]))
                        .with_fix("provide -o <path>"),
                    )
                })?;
                capture_output = Some(PathBuf::from(value));
                i += 2;
            }
            "-p" | "--pretty-capture" => {
                pretty_capture = true;
                i += 1;
            }
            "--cluster-path" | "--search-path" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.missing_option_value",
                            format!("{} requires a path", args[i]),
                        )
                        .with_where(format!("arg '{}'", args[i]))
                        .with_fix("provide a directory path"),
                    )
                })?;
                cluster_paths.push(PathBuf::from(value));
                i += 2;
            }
            other => {
                return Err(render_cli_error(
                    &CliErrorInfo::new("cli.invalid_option", format!("unknown run option '{other}'"))
                        .with_where(format!("arg '{other}'"))
                        .with_fix("use -a|--adapter, -f|--fixture, --driver-cmd, --driver-arg, -o|--capture|--capture-output, -p|--pretty-capture, --cluster-path, or --search-path"),
                ))
            }
        }
    }

    let driver = match (fixture_path, driver_command.is_empty()) {
        (Some(path), true) => DriverConfig::Fixture { path },
        (None, false) => DriverConfig::Process {
            command: driver_command,
        },
        (Some(_), false) => {
            return Err(render_cli_error(
                &CliErrorInfo::new(
                    "cli.conflicting_options",
                    "run accepts either --fixture or --driver-cmd, not both",
                )
                .with_where("canonical run ingress")
                .with_fix("choose exactly one ingress source for this run"),
            ))
        }
        (None, true) => {
            return Err(render_cli_error(
                &CliErrorInfo::new(
                    "cli.missing_required_option",
                    "run requires either --fixture <events.jsonl> or --driver-cmd <program>",
                )
                .with_where("canonical run ingress")
                .with_fix("provide --fixture <events.jsonl> or --driver-cmd <program>"),
            ))
        }
    };

    Ok(RunGraphOptions {
        adapter_path,
        driver,
        capture_output,
        pretty_capture,
        cluster_paths,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ergo_adapter::{EventTime, ExternalEventKind};
    use ergo_host::HostedEvent;
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
            semantic_kind: None,
            payload: Some(json!({})),
        }
    }

    #[test]
    fn parse_run_options_supports_short_flags_and_capture_alias() -> Result<(), String> {
        let opts = parse_run_options(&[
            "-f".to_string(),
            "fixture.jsonl".to_string(),
            "-a".to_string(),
            "adapter.yaml".to_string(),
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
                && err.contains("fix: use -a|--adapter, -f|--fixture, --driver-cmd"),
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
            missing
                .contains("run requires either --fixture <events.jsonl> or --driver-cmd <program>"),
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
        let driver = write_process_driver_script(
            &temp_dir,
            "driver.sh",
            &[
                serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))
                    .map_err(|err| format!("serialize hello: {err}"))?,
                serde_json::to_string(&json!({"type":"event","event":host_event("evt1")}))
                    .map_err(|err| format!("serialize event: {err}"))?,
                serde_json::to_string(&json!({"type":"end"}))
                    .map_err(|err| format!("serialize end: {err}"))?,
            ],
        )?;
        let capture = temp_dir.join("capture.json");

        let args = vec![
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
        let driver = write_process_driver_script(
            &temp_dir,
            "driver.sh",
            &[
                serde_json::to_string(&json!({"type":"hello","protocol":"ergo-driver.v0"}))
                    .map_err(|err| format!("serialize hello: {err}"))?,
                serde_json::to_string(&json!({"type":"event","event":host_event("evt1")}))
                    .map_err(|err| format!("serialize event: {err}"))?,
            ],
        )?;
        let capture = temp_dir.join("capture.json");

        let args = vec![
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
}
