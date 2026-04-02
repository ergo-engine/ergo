//! graph_to_dot tests
//!
//! Purpose:
//! - Exercise the CLI DOT-export command surface and lock its output/error behavior.
//!
//! Owns:
//! - Command-level option parsing, DOT rendering output checks, and CLI error rendering expectations specific to `graph_to_dot.rs`.
//!
//! Does not own:
//! - DOT expansion semantics or typed host error shaping; `ergo_host` owns those.
//!
//! Connects to:
//! - `graph_to_dot.rs` and the typed host DOT surface it wraps for CLI callers.
//!
//! Safety notes:
//! - These tests intentionally lock CLI-visible DOT output and error text because downstream tooling treats them as developer-facing contract surfaces.

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

fn make_temp_dir(label: &str) -> Result<PathBuf, String> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-cli-graph-to-dot-{label}-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;
    Ok(temp_dir)
}

fn write_temp_file(dir: &Path, name: &str, contents: &str) -> Result<PathBuf, String> {
    let path = dir.join(name);
    fs::write(&path, contents).map_err(|err| format!("write file '{}': {err}", path.display()))?;
    Ok(path)
}

fn cluster_version_miss_graph_yaml(graph_id: &str) -> String {
    format!(
        r#"
kind: cluster
id: {graph_id}
version: "0.1.0"
nodes:
  nested:
    cluster: shared_value@^2.0
edges: []
outputs:
  result: nested.value
"#
    )
}

fn shared_value_graph_yaml(version: &str, value: f64) -> String {
    format!(
        r#"
kind: cluster
id: shared_value
version: "{version}"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: {value}
edges: []
outputs:
  value: src.value
"#
    )
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

#[test]
fn graph_to_dot_preserves_host_expand_error_text() -> Result<(), String> {
    let temp_dir = make_temp_dir("version-miss")?;
    let search_a = temp_dir.join("search-a");
    let search_b = temp_dir.join("search-b");
    fs::create_dir_all(&search_a).map_err(|err| format!("create search-a: {err}"))?;
    fs::create_dir_all(&search_b).map_err(|err| format!("create search-b: {err}"))?;

    let graph_path = write_temp_file(
        &temp_dir,
        "graph.yaml",
        &cluster_version_miss_graph_yaml("cli_visual_miss"),
    )?;
    let shared_v1 = write_temp_file(
        &search_a,
        "shared_value.yaml",
        &shared_value_graph_yaml("1.0.0", 3.0),
    )?;
    let shared_v1_5 = write_temp_file(
        &search_b,
        "shared_value.yaml",
        &shared_value_graph_yaml("1.5.0", 4.0),
    )?;
    let shared_v1_canonical = fs::canonicalize(&shared_v1)
        .map_err(|err| format!("canonicalize shared_v1: {err}"))?
        .display()
        .to_string();
    let shared_v1_5_canonical = fs::canonicalize(&shared_v1_5)
        .map_err(|err| format!("canonicalize shared_v1_5: {err}"))?
        .display()
        .to_string();

    let args = vec![
        graph_path.to_string_lossy().to_string(),
        "--cluster-path".to_string(),
        search_a.to_string_lossy().to_string(),
        "--cluster-path".to_string(),
        search_b.to_string_lossy().to_string(),
    ];
    let err = graph_to_dot_command(&args).expect_err("version miss should bubble host text");

    assert!(err.contains("graph expansion failed: [I.6]"), "err: {err}");
    assert!(err.contains("available cluster files"), "err: {err}");
    assert!(err.contains(&shared_v1_canonical), "err: {err}");
    assert!(err.contains(&shared_v1_5_canonical), "err: {err}");
    Ok(())
}
