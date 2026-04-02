//! graph_dot_usecase::tests
//!
//! Purpose:
//! - Smoke-test that the DOT rendering pipeline produces expected output from
//!   an in-memory graph, while locking the canonical typed DOT error surface
//!   and its Display compatibility for filesystem and asset-backed failures.

use super::*;

use ergo_loader::{load_graph_assets_from_memory, InMemorySourceInput};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

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

fn make_temp_dir(label: &str) -> Result<PathBuf, String> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "ergo-host-graph-dot-{label}-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
    Ok(dir)
}

fn write_temp_file(dir: &Path, name: &str, contents: &str) -> Result<PathBuf, String> {
    let path = dir.join(name);
    fs::write(&path, contents)
        .map_err(|err| format!("write temp file '{}': {err}", path.display()))?;
    Ok(path)
}

#[test]
fn graph_to_dot_from_assets_renders_loaded_in_memory_graph(
) -> Result<(), Box<dyn std::error::Error>> {
    let assets = load_graph_assets_from_memory(
        "mem/root.yaml",
        &[InMemorySourceInput {
            source_id: "mem/root.yaml".to_string(),
            source_label: "mem/root.yaml".to_string(),
            content: r#"
kind: cluster
id: memory_visual
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
"#
            .to_string(),
        }],
        &[],
    )?;

    let dot = graph_to_dot_from_assets(GraphToDotFromAssetsRequest {
        assets,
        show_ports: true,
        show_impl: false,
        show_runtime_id: false,
    })?;

    assert!(dot.contains("digraph \"memory_visual\""));
    assert!(dot.contains("src"));
    assert!(dot.contains("cmp"));
    assert!(dot.contains("value -> a"));
    Ok(())
}

#[test]
fn graph_to_dot_from_paths_distinguishes_loader_failures() {
    let missing = std::env::temp_dir().join(format!(
        "ergo-host-graph-dot-missing-{}-{}.yaml",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));

    let err = graph_to_dot_from_paths(GraphToDotFromPathsRequest {
        graph_path: missing.clone(),
        cluster_paths: Vec::new(),
        show_ports: false,
        show_impl: false,
        show_runtime_id: false,
    })
    .expect_err("missing graph path should surface a typed load error");

    match err {
        GraphToDotError::Load(LoaderError::Io(detail)) => {
            assert_eq!(detail.path, missing);
            assert!(
                detail.message.contains("No such file") || detail.message.contains("not found")
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn graph_to_dot_from_assets_preserves_expand_diagnostics() -> Result<(), Box<dyn std::error::Error>>
{
    let assets = load_graph_assets_from_memory(
        "mem/root.yaml",
        &[
            InMemorySourceInput {
                source_id: "mem/root.yaml".to_string(),
                source_label: "mem/root-row".to_string(),
                content: cluster_version_miss_graph_yaml("memory_visual_miss"),
            },
            InMemorySourceInput {
                source_id: "search-a/shared_value.yaml".to_string(),
                source_label: "shared-v1-row".to_string(),
                content: shared_value_graph_yaml("1.0.0", 3.0),
            },
            InMemorySourceInput {
                source_id: "search-b/shared_value.yaml".to_string(),
                source_label: "shared-v1_5-row".to_string(),
                content: shared_value_graph_yaml("1.5.0", 4.0),
            },
        ],
        &["search-a".to_string(), "search-b".to_string()],
    )?;

    let err = graph_to_dot_from_assets(GraphToDotFromAssetsRequest {
        assets,
        show_ports: false,
        show_impl: false,
        show_runtime_id: false,
    })
    .expect_err("in-memory version miss should surface a typed expansion error");

    match &err {
        GraphToDotError::Expansion(GraphToDotExpansionError {
            source,
            context,
            available_clusters,
        }) => {
            assert_eq!(source.rule_id(), "I.6");
            assert!(source.summary().contains("shared_value"));
            assert!(source.summary().contains("^2.0"));
            assert_eq!(*context, GraphToDotExpansionContext::Assets);
            assert_eq!(available_clusters.len(), 2);
            assert_eq!(available_clusters[0].location, "shared-v1-row");
            assert_eq!(available_clusters[1].location, "shared-v1_5-row");
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let display = err.to_string();
    assert!(display.contains("graph expansion failed: [I.6]"));
    assert!(display.contains("available cluster sources"));
    assert!(display.contains("shared_value@1.0.0 at shared-v1-row"));
    assert!(display.contains("shared_value@1.5.0 at shared-v1_5-row"));
    Ok(())
}

#[test]
fn graph_to_dot_from_paths_preserves_filesystem_expand_diagnostics(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = make_temp_dir("paths")?;
    let search_a = temp_dir.join("search-a");
    let search_b = temp_dir.join("search-b");
    fs::create_dir_all(&search_a)?;
    fs::create_dir_all(&search_b)?;

    let graph_path = write_temp_file(
        &temp_dir,
        "graph.yaml",
        &cluster_version_miss_graph_yaml("filesystem_visual_miss"),
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
    let shared_v1_canonical = fs::canonicalize(&shared_v1)?.display().to_string();
    let shared_v1_5_canonical = fs::canonicalize(&shared_v1_5)?.display().to_string();

    let err = graph_to_dot_from_paths(GraphToDotFromPathsRequest {
        graph_path,
        cluster_paths: vec![search_a, search_b],
        show_ports: false,
        show_impl: false,
        show_runtime_id: false,
    })
    .expect_err("filesystem version miss should surface a typed expansion error");

    match &err {
        GraphToDotError::Expansion(GraphToDotExpansionError {
            source,
            context,
            available_clusters,
            ..
        }) => {
            assert_eq!(source.rule_id(), "I.6");
            assert_eq!(*context, GraphToDotExpansionContext::Filesystem);
            assert_eq!(available_clusters.len(), 2);
            assert!(available_clusters
                .iter()
                .any(|cluster| cluster.location == shared_v1_canonical));
            assert!(available_clusters
                .iter()
                .any(|cluster| cluster.location == shared_v1_5_canonical));
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let display = err.to_string();
    assert!(display.contains("graph expansion failed: [I.6]"));
    assert!(display.contains("available cluster files"));
    assert!(display.contains(&shared_v1_canonical));
    assert!(display.contains(&shared_v1_5_canonical));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}
