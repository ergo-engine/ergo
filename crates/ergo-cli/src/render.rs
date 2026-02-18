use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error_format::{render_cli_error, CliErrorInfo};
use crate::graph_to_dot::build_graph_dot;

#[derive(Debug, Default, PartialEq)]
struct RenderGraphOptions {
    graph_path: Option<PathBuf>,
    output_path: Option<PathBuf>,
    cluster_paths: Vec<PathBuf>,
    show_ports: bool,
    show_impl: bool,
    show_runtime_id: bool,
}

pub fn render_graph_command(args: &[String]) -> Result<String, String> {
    render_graph_command_with_dot_bin(args, "dot")
}

fn render_graph_command_with_dot_bin(args: &[String], dot_bin: &str) -> Result<String, String> {
    let options = parse_render_graph_options(args)?;
    let graph_path = options
        .graph_path
        .as_ref()
        .expect("parse_render_graph_options enforces graph path");

    let output_path = options
        .output_path
        .clone()
        .unwrap_or_else(|| default_svg_output_path(graph_path));

    let dot_source = build_graph_dot(
        graph_path,
        &options.cluster_paths,
        options.show_ports,
        options.show_impl,
        options.show_runtime_id,
    )?;

    render_dot_to_svg(&dot_source, &output_path, dot_bin)?;
    Ok(format!("wrote {}\n", output_path.display()))
}

fn parse_render_graph_options(args: &[String]) -> Result<RenderGraphOptions, String> {
    let mut options = RenderGraphOptions::default();
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
                        .with_fix("provide -o <out.svg>"),
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
                        format!("unknown render graph option '{other}'"),
                    )
                    .with_where(format!("arg '{other}'"))
                    .with_fix(
                        "usage: ergo render graph <graph.yaml> [-o out.svg] [--show-ports] [--show-impl] [--show-runtime-id] [--cluster-path <path> ...]",
                    ),
                ))
            }
        }
    }

    if options.graph_path.is_none() {
        return Err(render_cli_error(
            &CliErrorInfo::new(
                "cli.missing_required_option",
                "render graph requires <graph.yaml>",
            )
            .with_where("render graph command arguments")
            .with_fix(
                "usage: ergo render graph <graph.yaml> [-o out.svg] [--show-ports] [--show-impl] [--show-runtime-id] [--cluster-path <path> ...]",
            ),
        ));
    }

    Ok(options)
}

fn default_svg_output_path(graph_path: &Path) -> PathBuf {
    graph_path.with_extension("svg")
}

fn render_dot_to_svg(dot_source: &str, output_path: &Path, dot_bin: &str) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| {
                render_cli_error(
                    &CliErrorInfo::new(
                        "cli.render_output_dir_create_failed",
                        "failed to create SVG output directory",
                    )
                    .with_where(format!("path '{}'", parent.display()))
                    .with_fix("verify output path permissions")
                    .with_detail(err.to_string()),
                )
            })?;
        }
    }

    let mut child = Command::new(dot_bin)
        .arg("-Tsvg")
        .arg("-o")
        .arg(output_path)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            let fix = if err.kind() == std::io::ErrorKind::NotFound {
                "install Graphviz (e.g. `brew install graphviz`) and rerun"
            } else {
                "verify Graphviz is installed and executable as `dot`"
            };
            render_cli_error(
                &CliErrorInfo::new(
                    "cli.graphviz_unavailable",
                    "failed to launch Graphviz `dot` for SVG rendering",
                )
                .with_where("render graph execution")
                .with_fix(fix)
                .with_detail(err.to_string()),
            )
        })?;

    {
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            render_cli_error(
                &CliErrorInfo::new(
                    "cli.graphviz_stdin_unavailable",
                    "failed to stream DOT input to Graphviz",
                )
                .with_where("render graph execution")
                .with_fix("retry command; if issue persists, reinstall Graphviz"),
            )
        })?;
        stdin.write_all(dot_source.as_bytes()).map_err(|err| {
            render_cli_error(
                &CliErrorInfo::new(
                    "cli.graphviz_write_failed",
                    "failed to stream DOT input to Graphviz",
                )
                .with_where("render graph execution")
                .with_fix("retry command; if issue persists, reinstall Graphviz")
                .with_detail(err.to_string()),
            )
        })?;
    }

    let output = child.wait_with_output().map_err(|err| {
        render_cli_error(
            &CliErrorInfo::new(
                "cli.graphviz_wait_failed",
                "failed while waiting for Graphviz render process",
            )
            .with_where("render graph execution")
            .with_fix("retry command and verify Graphviz health")
            .with_detail(err.to_string()),
        )
    })?;

    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(render_cli_error(
            &CliErrorInfo::new(
                "cli.graphviz_render_failed",
                "Graphviz failed to render SVG from DOT",
            )
            .with_where(format!("path '{}'", output_path.display()))
            .with_fix("verify graph input is valid and Graphviz is functional")
            .with_detail(detail),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn write_temp_graph(contents: &str, name: &str) -> Result<PathBuf, String> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-cli-render-graph-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;
        let path = temp_dir.join(name);
        fs::write(&path, contents).map_err(|err| format!("write graph: {err}"))?;
        Ok(path)
    }

    #[test]
    fn parse_render_graph_options_requires_graph() {
        let err = parse_render_graph_options(&[]).expect_err("missing graph path should fail");
        assert!(err.contains("render graph requires <graph.yaml>"));
        assert!(err.contains("code: cli.missing_required_option"));
    }

    #[test]
    fn parse_render_graph_options_rejects_unknown_flag() {
        let args = vec!["graph.yaml".to_string(), "--bad".to_string()];
        let err = parse_render_graph_options(&args).expect_err("unknown flag should fail");
        assert!(err.contains("unknown render graph option '--bad'"));
        assert!(err.contains("code: cli.invalid_option"));
    }

    #[test]
    fn render_graph_missing_dot_binary_is_actionable() -> Result<(), String> {
        let graph = r#"
kind: cluster
id: render_test
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 1
edges: []
outputs:
  out: src.value
"#;

        let graph_path = write_temp_graph(graph, "render_test.yaml")?;
        let args = vec![graph_path.to_string_lossy().to_string()];
        let err = render_graph_command_with_dot_bin(&args, "__missing_dot_binary__")
            .expect_err("missing dot binary should fail");
        assert!(err.contains("cli.graphviz_unavailable"), "err: {err}");
        assert!(err.contains("install Graphviz"), "err: {err}");
        Ok(())
    }
}
