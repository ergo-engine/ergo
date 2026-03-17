use std::path::Path;

use crate::cli::args;
use crate::output;

pub enum DispatchOutput {
    Text(String),
    Json(String),
}

fn wants_json_output(args: &[String]) -> bool {
    for i in 0..args.len() {
        if args[i] == "--format" && args.get(i + 1).is_some_and(|v| v == "json") {
            return true;
        }
        if args[i] == "--format=json" {
            return true;
        }
    }
    false
}

pub fn run() -> Result<DispatchOutput, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    dispatch_with_args(&args)
}

pub(crate) fn dispatch_with_args(args: &[String]) -> Result<DispatchOutput, String> {
    let mut args_it = args.iter().cloned();
    match args_it.next() {
        None => Ok(DispatchOutput::Text(output::text::usage())),
        Some(command) => match command.as_str() {
            "help" => {
                let topic_parts: Vec<String> = args_it.collect();
                if topic_parts.is_empty() {
                    return Ok(DispatchOutput::Text(output::text::usage()));
                }
                let topic = topic_parts.join(" ").to_ascii_lowercase();
                if let Some(help_text) =
                    output::text::help_topic(&topic, &crate::fixture_ops::fixture_usage())
                {
                    Ok(DispatchOutput::Text(help_text))
                } else {
                    Err(output::errors::unknown_help_topic(&topic_parts.join(" ")))
                }
            }
            "fixture" => {
                let target = args_it
                    .next()
                    .ok_or_else(crate::fixture_ops::fixture_usage)?;
                match target.as_str() {
                    "run" => {
                        let path = args_it
                            .next()
                            .ok_or_else(crate::fixture_ops::fixture_usage)?;
                        let rest: Vec<String> = args_it.collect();
                        let run_opts = args::parse_run_artifact_options(&rest, "fixture run")?;
                        let summary = crate::cli::handlers::run_fixture(
                            Path::new(&path),
                            run_opts.capture_output.as_deref(),
                            run_opts.pretty_capture,
                        )?;
                        Ok(DispatchOutput::Text(
                            output::text::render_fixture_run_summary(
                                &summary.episode_event_counts,
                                &summary.capture_path,
                            ),
                        ))
                    }
                    "inspect" => {
                        let rest: Vec<String> = args_it.collect();
                        let out = crate::fixture_ops::fixture_inspect_command(&rest)?;
                        if wants_json_output(&rest) {
                            Ok(DispatchOutput::Json(out))
                        } else {
                            Ok(DispatchOutput::Text(out))
                        }
                    }
                    "validate" => {
                        let rest: Vec<String> = args_it.collect();
                        let out = crate::fixture_ops::fixture_validate_command(&rest)?;
                        if wants_json_output(&rest) {
                            Ok(DispatchOutput::Json(out))
                        } else {
                            Ok(DispatchOutput::Text(out))
                        }
                    }
                    _ => Err(output::errors::invalid_fixture_subcommand(
                        &target,
                        &crate::fixture_ops::fixture_usage(),
                    )),
                }
            }
            "gen-docs" => {
                let rest: Vec<String> = args_it.collect();
                let out = crate::gen_docs::gen_docs_command(&rest)?;
                Ok(DispatchOutput::Text(out))
            }
            "init" => {
                let rest: Vec<String> = args_it.collect();
                let out = crate::init_project::init_command(&rest)?;
                Ok(DispatchOutput::Text(out))
            }
            "validate" => {
                let rest: Vec<String> = args_it.collect();
                let out = crate::validate::validate_command(&rest)?;
                if wants_json_output(&rest) {
                    Ok(DispatchOutput::Json(out))
                } else {
                    Ok(DispatchOutput::Text(out))
                }
            }
            "csv-to-fixture" => {
                let rest: Vec<String> = args_it.collect();
                let out = crate::csv_fixture::csv_to_fixture_command(&rest)?;
                Ok(DispatchOutput::Text(out))
            }
            "check-compose" => {
                let rest: Vec<String> = args_it.collect();
                let out = crate::validate::check_compose_command(&rest)?;
                if wants_json_output(&rest) {
                    Ok(DispatchOutput::Json(out))
                } else {
                    Ok(DispatchOutput::Text(out))
                }
            }
            "graph-to-dot" => {
                let rest: Vec<String> = args_it.collect();
                let out = crate::graph_to_dot::graph_to_dot_command(&rest)?;
                Ok(DispatchOutput::Text(out))
            }
            "render" => {
                let target = args_it.next().ok_or_else(crate::usage)?;
                match target.as_str() {
                    "graph" => {
                        let rest: Vec<String> = args_it.collect();
                        let out = crate::render::render_graph_command(&rest)?;
                        Ok(DispatchOutput::Text(out))
                    }
                    _ => Err(crate::usage()),
                }
            }
            "run" => {
                let target = args_it.next().ok_or_else(crate::usage)?;
                match target.as_str() {
                    "fixture" => Err(output::errors::removed_run_fixture()),
                    _ => {
                        let rest: Vec<String> = args_it.collect();
                        let summary =
                            crate::graph_yaml::run_graph_command(Path::new(&target), &rest)?;
                        Ok(DispatchOutput::Text(
                            output::text::render_graph_run_summary(
                                summary.completion,
                                summary.episodes,
                                summary.events,
                                summary.invoked,
                                summary.deferred,
                                &summary.capture_path,
                            ),
                        ))
                    }
                }
            }
            "replay" => {
                let path = args_it.next().ok_or_else(crate::usage)?;
                let rest: Vec<String> = args_it.collect();
                let replay_opts = args::parse_replay_options(&rest)?;
                let summary = crate::cli::handlers::replay_graph(
                    Path::new(&path),
                    replay_opts
                        .graph_path
                        .as_ref()
                        .expect("replay options enforce graph path"),
                    &replay_opts.cluster_paths,
                    replay_opts.adapter_path.as_deref(),
                )?;
                Ok(DispatchOutput::Text(output::text::render_replay_summary(
                    &summary.graph_id,
                    summary.events,
                    summary.invoked,
                    summary.deferred,
                    summary.skipped,
                )))
            }
            _ => Err(output::errors::unknown_command(&command)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
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

    fn cli_sdk_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(4)
            .expect("workspace root")
            .join("crates/prod/clients/sdk-rust")
    }

    #[test]
    fn run_dispatch_returns_text_summary() -> Result<(), String> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-cli-dispatch-run-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: dispatch_run
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
            "run".to_string(),
            graph.to_string_lossy().to_string(),
            "--fixture".to_string(),
            fixture.to_string_lossy().to_string(),
            "--capture-output".to_string(),
            capture.to_string_lossy().to_string(),
        ];
        let result = dispatch_with_args(&args)?;
        let text = match result {
            DispatchOutput::Text(text) => text,
            DispatchOutput::Json(_) => return Err("expected text output".to_string()),
        };
        assert!(
            text.contains("episodes=1 events=1"),
            "unexpected text: {text}"
        );
        assert!(
            text.contains(&format!("capture artifact: {}", capture.display())),
            "unexpected text: {text}"
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn replay_dispatch_returns_text_summary() -> Result<(), String> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-cli-dispatch-replay-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

        let graph = write_temp_file(
            &temp_dir,
            "graph.yaml",
            r#"
kind: cluster
id: dispatch_replay
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

        let run_args = vec![
            "run".to_string(),
            graph.to_string_lossy().to_string(),
            "--fixture".to_string(),
            fixture.to_string_lossy().to_string(),
            "--capture-output".to_string(),
            capture.to_string_lossy().to_string(),
        ];
        let _ = dispatch_with_args(&run_args)?;

        let replay_args = vec![
            "replay".to_string(),
            capture.to_string_lossy().to_string(),
            "--graph".to_string(),
            graph.to_string_lossy().to_string(),
        ];
        let result = dispatch_with_args(&replay_args)?;
        let text = match result {
            DispatchOutput::Text(text) => text,
            DispatchOutput::Json(_) => return Err("expected text output".to_string()),
        };
        assert!(
            text.contains("replay graph_id=dispatch_replay events=1"),
            "unexpected text: {text}"
        );
        assert!(
            text.contains("replay identity: match"),
            "unexpected text: {text}"
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn init_dispatch_routes_to_scaffold_command() -> Result<(), String> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-cli-dispatch-init-{}-{}",
            std::process::id(),
            index
        ));
        let project_root = temp_dir.join("sample-app");
        let sdk_path = cli_sdk_path();

        let args = vec![
            "init".to_string(),
            project_root.to_string_lossy().to_string(),
            "--sdk-path".to_string(),
            sdk_path.to_string_lossy().to_string(),
        ];
        let result = dispatch_with_args(&args)?;
        let text = match result {
            DispatchOutput::Text(text) => text,
            DispatchOutput::Json(_) => return Err("expected text output".to_string()),
        };

        assert!(text.contains("initialized Ergo SDK project"));
        assert!(project_root.join("Cargo.toml").exists());

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn help_init_dispatch_returns_init_notes() -> Result<(), String> {
        let result = dispatch_with_args(&["help".to_string(), "init".to_string()])?;
        let text = match result {
            DispatchOutput::Text(text) => text,
            DispatchOutput::Json(_) => return Err("expected text output".to_string()),
        };

        assert!(text.contains("ergo init <project-dir>"));
        assert!(text.contains("use --sdk-path outside the checkout"));
        assert!(text.contains("POSIX 'sh'"));
        Ok(())
    }
}
