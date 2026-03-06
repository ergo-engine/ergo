use std::fs;
use std::path::{Path, PathBuf};

use crate::error_format::{render_cli_error, CliErrorInfo};
use ergo_host::{graph_to_dot_from_paths, GraphToDotFromPathsRequest};

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
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn write_temp_graph(contents: &str, name: &str) -> Result<PathBuf, String> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-cli-graph-to-dot-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;
        let path = temp_dir.join(name);
        fs::write(&path, contents).map_err(|err| format!("write graph: {err}"))?;
        Ok(path)
    }

    #[test]
    fn parse_graph_to_dot_options_requires_graph() {
        let err = parse_graph_to_dot_options(&[]).expect_err("missing graph path should fail");
        assert!(err.contains("graph-to-dot requires <graph.yaml>"));
        assert!(err.contains("code: cli.missing_required_option"));
    }

    #[test]
    fn parse_graph_to_dot_options_rejects_unknown_flag() {
        let args = vec!["graph.yaml".to_string(), "--bad".to_string()];
        let err = parse_graph_to_dot_options(&args).expect_err("unknown flag should fail");
        assert!(err.contains("unknown graph-to-dot option '--bad'"));
        assert!(err.contains("code: cli.invalid_option"));
    }

    #[test]
    fn graph_to_dot_outputs_styled_graph() -> Result<(), String> {
        let graph = r#"
kind: cluster
id: visual_test
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 3
  cmp:
    impl: gt@0.1.0
  trg:
    impl: emit_if_true@0.1.0
  act:
    impl: ack_action@0.1.0
edges:
  - src.value -> cmp.a
  - cmp.result -> trg.input
  - trg.event -> act.event
outputs:
  out: act.outcome
"#;

        let graph_path = write_temp_graph(graph, "visual_test.yaml")?;
        let args = vec![graph_path.to_string_lossy().to_string()];
        let dot = graph_to_dot_command(&args)?;

        assert!(dot.contains("digraph \"visual_test\""));
        assert!(dot.contains("shape=box"), "source style missing");
        assert!(dot.contains("shape=ellipse"), "compute style missing");
        assert!(dot.contains("shape=diamond"), "trigger style missing");
        assert!(dot.contains("shape=doubleoctagon"), "action style missing");
        assert!(
            dot.matches(" -> ").count() == 3,
            "expected three edges in dot output:\n{}",
            dot
        );
        assert!(
            dot.contains("label=\"src\""),
            "source authoring label missing"
        );
        assert!(
            dot.contains("label=\"cmp\""),
            "compute authoring label missing"
        );
        assert!(
            dot.contains("label=\"trg\""),
            "trigger authoring label missing"
        );
        assert!(
            dot.contains("label=\"act\""),
            "action authoring label missing"
        );

        Ok(())
    }

    #[test]
    fn graph_to_dot_show_flags_enrich_output() -> Result<(), String> {
        let graph = r#"
kind: cluster
id: visual_flags
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 3
  cmp:
    impl: gt@0.1.0
edges:
  - src.value -> cmp.a
outputs:
  out: cmp.result
"#;

        let graph_path = write_temp_graph(graph, "visual_flags.yaml")?;
        let args = vec![
            graph_path.to_string_lossy().to_string(),
            "--show-ports".to_string(),
            "--show-impl".to_string(),
        ];
        let dot = graph_to_dot_command(&args)?;

        assert!(dot.contains("impl: number_source@0.1.0"));
        assert!(dot.contains("impl: gt@0.1.0"));
        assert!(dot.contains("[label=\"value -> a\"]"));
        Ok(())
    }

    #[test]
    fn graph_to_dot_show_runtime_id_includes_runtime_line() -> Result<(), String> {
        let graph = r#"
kind: cluster
id: visual_runtime
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 3
edges: []
outputs:
  out: src.value
"#;
        let graph_path = write_temp_graph(graph, "visual_runtime.yaml")?;
        let args = vec![
            graph_path.to_string_lossy().to_string(),
            "--show-runtime-id".to_string(),
        ];
        let dot = graph_to_dot_command(&args)?;
        assert!(dot.contains("runtime: n"), "dot: {}", dot);
        Ok(())
    }

    #[test]
    fn graph_to_dot_writes_output_file() -> Result<(), String> {
        let graph = r#"
kind: cluster
id: visual_write
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

        let graph_path = write_temp_graph(graph, "visual_write.yaml")?;
        let out_path = graph_path.with_extension("dot");
        let args = vec![
            graph_path.to_string_lossy().to_string(),
            "-o".to_string(),
            out_path.to_string_lossy().to_string(),
        ];

        let message = graph_to_dot_command(&args)?;
        assert!(message.contains("wrote"));

        let dot = fs::read_to_string(&out_path).map_err(|err| format!("read dot: {err}"))?;
        assert!(dot.contains("digraph \"visual_write\""));
        Ok(())
    }

    #[test]
    fn graph_to_dot_renders_external_inputs() -> Result<(), String> {
        let graph = r#"
kind: cluster
id: visual_external
version: "0.1.0"
nodes:
  cmp:
    impl: gt@0.1.0
  trg:
    impl: emit_if_true@0.1.0
edges:
  - $threshold -> cmp.a
  - cmp.result -> trg.input
inputs:
  - name: threshold
    type: number
outputs:
  out: trg.event
"#;

        let graph_path = write_temp_graph(graph, "visual_external.yaml")?;
        let args = vec![graph_path.to_string_lossy().to_string()];
        let dot = graph_to_dot_command(&args)?;

        assert!(dot.contains("input: threshold"), "dot: {}", dot);
        assert!(dot.contains("external_input_visual_external/threshold"));
        Ok(())
    }
}
