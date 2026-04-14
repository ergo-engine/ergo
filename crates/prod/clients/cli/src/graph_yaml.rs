//! graph_yaml.rs — Graph YAML parsing and version migration
//!
//! Purpose:
//! - Parses graph definition YAML files into the loader's internal
//!   representation, handling version detection and migration between
//!   graph schema versions.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error_format::{render_cli_error, CliErrorInfo};
use crate::output;
use ergo_host::{
    parse_egress_config_toml, run_graph_from_paths, DriverConfig, EgressConfig, InterruptionReason,
    RunGraphFromPathsRequest, RunOutcome,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphRunCompletion {
    Completed,
    Interrupted { reason: InterruptionReason },
}

pub fn run_graph_command(graph_path: &Path, args: &[String]) -> Result<GraphRunSummary, String> {
    let opts = parse_run_options(args)?;
    let egress_config = load_egress_config(opts.egress_config_path.as_deref())?;
    let capture_output = opts
        .capture_output
        .clone()
        .or_else(|| Some(default_capture_output_path(graph_path)));
    let outcome = run_graph_from_paths(RunGraphFromPathsRequest {
        graph_path: graph_path.to_path_buf(),
        cluster_paths: opts.cluster_paths,
        driver: opts.driver,
        adapter_path: opts.adapter_path,
        egress_config,
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
        capture_path: summary
            .capture_path
            .expect("filesystem graph run should always produce a capture path"),
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
    egress_config_path: Option<PathBuf>,
    driver: DriverConfig,
    capture_output: Option<PathBuf>,
    pretty_capture: bool,
    cluster_paths: Vec<PathBuf>,
}

fn parse_run_options(args: &[String]) -> Result<RunGraphOptions, String> {
    let mut adapter_path = None;
    let mut egress_config_path = None;
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
            "--egress-config" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.missing_option_value",
                            format!("{} requires a path", args[i]),
                        )
                        .with_where(format!("arg '{}'", args[i]))
                        .with_fix("provide --egress-config <egress.toml>"),
                    )
                })?;
                egress_config_path = Some(PathBuf::from(value));
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
                        .with_fix("use -a|--adapter, --egress-config, -f|--fixture, --driver-cmd, --driver-arg, -o|--capture|--capture-output, -p|--pretty-capture, --cluster-path, or --search-path"),
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
        egress_config_path,
        driver,
        capture_output,
        pretty_capture,
        cluster_paths,
    })
}

fn load_egress_config(path: Option<&Path>) -> Result<Option<EgressConfig>, String> {
    let Some(path) = path else {
        return Ok(None);
    };

    let raw = fs::read_to_string(path).map_err(|err| {
        render_cli_error(
            &CliErrorInfo::new(
                "cli.egress_config_read_failed",
                format!("failed to read egress config '{}': {err}", path.display()),
            )
            .with_where("run egress config")
            .with_fix("provide a readable --egress-config <path>"),
        )
    })?;
    let config = parse_egress_config_toml(&raw).map_err(|err| {
        render_cli_error(
            &CliErrorInfo::new("cli.egress_config_parse_failed", err)
                .with_where(format!("egress config '{}'", path.display()))
                .with_fix("fix the TOML schema for egress channels/routes"),
        )
    })?;
    Ok(Some(config))
}

#[cfg(test)]
mod tests;
