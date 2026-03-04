#[cfg(test)]
use std::fs;
use std::path::{Path, PathBuf};

use crate::error_format::{render_cli_error, CliErrorInfo};
use crate::output;
use ergo_host::{run_graph_from_paths, RunGraphFromPathsRequest};

#[derive(Debug, Clone)]
pub struct GraphRunSummary {
    pub episodes: usize,
    pub events: usize,
    pub invoked: usize,
    pub deferred: usize,
    pub capture_path: PathBuf,
}

pub fn run_graph_command(graph_path: &Path, args: &[String]) -> Result<GraphRunSummary, String> {
    let opts = parse_run_options(args)?;
    let capture_output = opts
        .capture_output
        .clone()
        .or_else(|| Some(default_capture_output_path(graph_path)));
    let result = run_graph_from_paths(RunGraphFromPathsRequest {
        graph_path: graph_path.to_path_buf(),
        cluster_paths: opts.cluster_paths,
        fixture_path: opts.fixture_path.unwrap_or_default(),
        adapter_path: opts.adapter_path,
        capture_output,
        pretty_capture: opts.pretty_capture,
    })
    .map_err(output::errors::render_host_run_error)?;

    Ok(GraphRunSummary {
        episodes: result.episodes,
        events: result.events,
        invoked: result.invoked,
        deferred: result.deferred,
        capture_path: result.capture_path,
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

#[derive(Debug, Default)]
struct RunGraphOptions {
    adapter_path: Option<PathBuf>,
    fixture_path: Option<PathBuf>,
    capture_output: Option<PathBuf>,
    pretty_capture: bool,
    cluster_paths: Vec<PathBuf>,
}

fn parse_run_options(args: &[String]) -> Result<RunGraphOptions, String> {
    let mut options = RunGraphOptions::default();
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
                options.adapter_path = Some(PathBuf::from(value));
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
                options.fixture_path = Some(PathBuf::from(value));
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
                options.capture_output = Some(PathBuf::from(value));
                i += 2;
            }
            "-p" | "--pretty-capture" => {
                options.pretty_capture = true;
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
                options.cluster_paths.push(PathBuf::from(value));
                i += 2;
            }
            other => {
                return Err(render_cli_error(
                    &CliErrorInfo::new("cli.invalid_option", format!("unknown run option '{other}'"))
                        .with_where(format!("arg '{other}'"))
                        .with_fix("use -a|--adapter, -f|--fixture, -o|--capture|--capture-output, -p|--pretty-capture, --cluster-path, or --search-path"),
                ))
            }
        }
    }

    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(
            opts.fixture_path.as_deref(),
            Some(Path::new("fixture.jsonl"))
        );
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
        assert_eq!(
            opts.fixture_path.as_deref(),
            Some(Path::new("fixture.jsonl"))
        );
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
                && err.contains("fix: use -a|--adapter, -f|--fixture"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn run_graph_command_executes_via_host_from_paths() -> Result<(), String> {
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

        assert_eq!(summary.episodes, 1);
        assert_eq!(summary.events, 1);
        assert_eq!(summary.capture_path, capture);
        assert!(summary.capture_path.exists());

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
