//! graph_to_dot
//!
//! Purpose:
//! - Own the CLI command that expands a graph through the host DOT usecase and
//!   either prints or writes the resulting Graphviz DOT.
//!
//! Owns:
//! - CLI argument parsing and CLI-facing error rendering for `ergo graph-to-dot`.
//!
//! Does not own:
//! - DOT rendering semantics or graph expansion diagnostics; `ergo_host`
//!   provides the typed DOT surface this command wraps.
//!
//! Connects to:
//! - `ergo_host::graph_to_dot_from_paths(...)` for typed DOT generation.
//! - `render.rs`, which reuses the DOT-building helper before invoking Graphviz.
//!
//! Safety notes:
//! - This is a developer-facing debugging command, but its CLI error codes and
//!   detail text are still downstream-significant for tests and tooling.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error_format::{render_cli_error, CliErrorInfo};
use ergo_host::{graph_to_dot_from_paths, GraphToDotError, GraphToDotFromPathsRequest};

#[derive(Debug, Default, PartialEq)]
struct GraphToDotOptions {
    graph_path: Option<PathBuf>,
    output_path: Option<PathBuf>,
    cluster_paths: Vec<PathBuf>,
    show_ports: bool,
    show_impl: bool,
    show_runtime_id: bool,
}

pub fn graph_to_dot_command(args: &[String]) -> Result<String, String> {
    let options = parse_graph_to_dot_options(args)?;
    let graph_path = options
        .graph_path
        .as_ref()
        .expect("parse_graph_to_dot_options enforces graph_path");

    let dot = build_graph_dot(
        graph_path,
        &options.cluster_paths,
        options.show_ports,
        options.show_impl,
        options.show_runtime_id,
    )?;

    if let Some(output_path) = &options.output_path {
        fs::write(output_path, &dot).map_err(|err| {
            render_cli_error(
                &CliErrorInfo::new(
                    "cli.graph_to_dot_write_failed",
                    "failed to write DOT output file",
                )
                .with_where(format!("path '{}'", output_path.display()))
                .with_fix("verify output path and permissions")
                .with_detail(err.to_string()),
            )
        })?;
        return Ok(format!("wrote {}\n", output_path.display()));
    }

    Ok(dot)
}

pub(crate) fn build_graph_dot(
    graph_path: &Path,
    cluster_paths: &[PathBuf],
    show_ports: bool,
    show_impl: bool,
    show_runtime_id: bool,
) -> Result<String, String> {
    graph_to_dot_from_paths(GraphToDotFromPathsRequest {
        graph_path: graph_path.to_path_buf(),
        cluster_paths: cluster_paths.to_vec(),
        show_ports,
        show_impl,
        show_runtime_id,
    })
    .map_err(|err| render_graph_to_dot_error(&err))
}

fn render_graph_to_dot_error(err: &GraphToDotError) -> String {
    match err {
        GraphToDotError::Load(source) => render_cli_error(
            &CliErrorInfo::new(
                "cli.graph_to_dot_load_failed",
                "failed to load graph inputs for DOT rendering",
            )
            .with_where("graph-to-dot input loading")
            .with_fix("verify graph path, cluster search paths, and readable files")
            .with_detail(source.to_string()),
        ),
        GraphToDotError::Expansion(expansion) => render_cli_error(
            &CliErrorInfo::new(
                "cli.graph_to_dot_expand_failed",
                "failed to expand graph for DOT rendering",
            )
            .with_where("graph-to-dot graph expansion")
            .with_fix("repair graph authoring or provide the missing cluster versions")
            .with_detail(expansion.to_string()),
        ),
    }
}

fn parse_graph_to_dot_options(args: &[String]) -> Result<GraphToDotOptions, String> {
    let mut options = GraphToDotOptions::default();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.missing_option_value",
                            format!("{} requires a path", args[i]),
                        )
                        .with_where(format!("arg '{}'", args[i]))
                        .with_fix("provide -o <out.dot>"),
                    )
                })?;
                options.output_path = Some(PathBuf::from(value));
                i += 2;
            }
            "--show-ports" => {
                options.show_ports = true;
                i += 1;
            }
            "--show-impl" => {
                options.show_impl = true;
                i += 1;
            }
            "--show-runtime-id" => {
                options.show_runtime_id = true;
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
            other if !other.starts_with('-') && options.graph_path.is_none() => {
                options.graph_path = Some(PathBuf::from(other));
                i += 1;
            }
            other => {
                return Err(render_cli_error(
                    &CliErrorInfo::new(
                        "cli.invalid_option",
                        format!("unknown graph-to-dot option '{other}'"),
                    )
                    .with_where(format!("arg '{other}'"))
                    .with_fix(
                        "usage: ergo graph-to-dot <graph.yaml> [-o out.dot] [--show-ports] [--show-impl] [--show-runtime-id] [--cluster-path <path> ...]",
                    ),
                ))
            }
        }
    }

    if options.graph_path.is_none() {
        return Err(render_cli_error(
            &CliErrorInfo::new(
                "cli.missing_required_option",
                "graph-to-dot requires <graph.yaml>",
            )
            .with_where("graph-to-dot command arguments")
            .with_fix(
                "usage: ergo graph-to-dot <graph.yaml> [-o out.dot] [--show-ports] [--show-impl] [--show-runtime-id] [--cluster-path <path> ...]",
            ),
        ));
    }

    Ok(options)
}

#[cfg(test)]
mod tests;
