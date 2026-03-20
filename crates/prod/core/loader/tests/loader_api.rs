use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use ergo_loader::{decode_graph_yaml, load_graph_sources, resolve_cluster_candidates, LoaderError};

fn make_temp_dir(prefix: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "ergo_loader_{prefix}_{}_{}",
        std::process::id(),
        ts
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn load_graph_sources_missing_file_returns_io_error() {
    let path = PathBuf::from("/tmp/ergo_loader_missing_graph_file.yaml");
    let err = load_graph_sources(&path, &[]).expect_err("missing graph must error");
    match err {
        LoaderError::Io(inner) => {
            assert_eq!(inner.path, path);
            assert!(!inner.message.is_empty());
        }
        _ => panic!("expected io error"),
    }
}

#[test]
fn decode_graph_yaml_invalid_yaml_returns_decode_error() {
    let err = decode_graph_yaml("not: [valid: yaml").expect_err("invalid YAML must error");
    match err {
        LoaderError::Decode(inner) => {
            let msg = inner.message.to_ascii_uppercase();
            assert!(!msg.contains("RULEVIOLATION"));
            assert!(!msg.contains("CMP-"));
            assert!(!msg.contains("TRG-"));
            assert!(!msg.contains("SRC-"));
            assert!(!msg.contains("ACT-"));
        }
        _ => panic!("expected decode error"),
    }
}

#[test]
fn load_graph_sources_returns_cluster_definition_root() {
    let temp_root = make_temp_dir("bundle_root");
    let graph_path = temp_root.join("graph.yaml");
    fs::write(
        &graph_path,
        r#"
kind: cluster
id: demo_bundle_root
version: "1.0.0"
nodes: {}
edges: []
"#,
    )
    .expect("write graph");

    let bundle = load_graph_sources(&graph_path, &[]).expect("bundle load");
    let canonical_graph_path = fs::canonicalize(&graph_path).expect("canonical graph path");
    assert_eq!(bundle.root.id, "demo_bundle_root");
    assert_eq!(bundle.root.version.to_string(), "1.0.0");
    assert!(bundle.discovered_files.contains(&canonical_graph_path));
    assert!(bundle.source_map.contains_key(&canonical_graph_path));

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn resolve_cluster_candidates_finds_existing_paths_and_dedupes() {
    let temp_root = make_temp_dir("resolve_candidates");
    let root_cluster = temp_root.join("foo.yaml");
    let nested_cluster_dir = temp_root.join("clusters");
    let nested_cluster = nested_cluster_dir.join("foo.yaml");
    let search_dir = temp_root.join("search");

    fs::create_dir_all(&nested_cluster_dir).expect("create nested cluster dir");
    fs::create_dir_all(&search_dir).expect("create search dir");
    fs::write(&root_cluster, "placeholder").expect("write root cluster");
    fs::write(&nested_cluster, "placeholder").expect("write nested cluster");

    let paths =
        resolve_cluster_candidates(&temp_root, "foo", &[search_dir.clone(), search_dir.clone()])
            .expect("candidate resolution");

    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&root_cluster));
    assert!(paths.contains(&nested_cluster));

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn load_graph_sources_missing_cluster_lists_searched_paths() {
    let temp_root = make_temp_dir("missing_cluster_paths");
    let search_dir = temp_root.join("search");
    fs::create_dir_all(&search_dir).expect("create search dir");

    let graph_path = temp_root.join("graph.yaml");
    fs::write(
        &graph_path,
        r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: missing_cluster@1.0.0
edges: []
"#,
    )
    .expect("write graph");

    let err = load_graph_sources(&graph_path, &[search_dir.clone()]).expect_err("missing cluster");
    let message = err.to_string();
    assert!(message.contains("looked for 'missing_cluster.yaml'"));
    assert!(message.contains(&temp_root.join("missing_cluster.yaml").display().to_string()));
    assert!(message.contains(
        &temp_root
            .join("clusters")
            .join("missing_cluster.yaml")
            .display()
            .to_string()
    ));
    assert!(message.contains(
        &search_dir
            .join("missing_cluster.yaml")
            .display()
            .to_string()
    ));
    assert!(message.contains(
        &search_dir
            .join("clusters")
            .join("missing_cluster.yaml")
            .display()
            .to_string()
    ));
    assert!(message.contains("cluster resolution is filename-based"));
    assert!(message.contains("referenced by node 'nested'"));

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn load_graph_sources_missing_cluster_avoids_redundant_nested_clusters_search_path() {
    let temp_root = make_temp_dir("missing_cluster_no_nested_clusters");
    let search_dir = temp_root.join("clusters");
    fs::create_dir_all(&search_dir).expect("create clusters search dir");

    let graph_path = temp_root.join("graph.yaml");
    fs::write(
        &graph_path,
        r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: missing_cluster@1.0.0
edges: []
"#,
    )
    .expect("write graph");

    let err = load_graph_sources(&graph_path, &[search_dir.clone()]).expect_err("missing cluster");
    let message = err.to_string();
    assert!(message.contains(
        &search_dir
            .join("missing_cluster.yaml")
            .display()
            .to_string()
    ));
    assert!(!message.contains(
        &search_dir
            .join("clusters")
            .join("missing_cluster.yaml")
            .display()
            .to_string()
    ));

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn load_graph_sources_cluster_id_mismatch_explains_filename_rule() {
    let temp_root = make_temp_dir("cluster_id_mismatch");
    let cluster_dir = temp_root.join("clusters");
    fs::create_dir_all(&cluster_dir).expect("create cluster dir");

    let graph_path = temp_root.join("graph.yaml");
    fs::write(
        &graph_path,
        r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: expected_cluster@1.0.0
edges: []
"#,
    )
    .expect("write graph");
    fs::write(
        cluster_dir.join("expected_cluster.yaml"),
        r#"
kind: cluster
id: actual_cluster
version: "1.0.0"
nodes: {}
edges: []
"#,
    )
    .expect("write cluster");

    let err = load_graph_sources(&graph_path, &[]).expect_err("id mismatch must error");
    let message = err.to_string();
    assert!(message.contains("opened"));
    assert!(message.contains("expected_cluster@1.0.0"));
    assert!(message.contains("YAML id is 'actual_cluster'"));
    assert!(message.contains("filename must match the YAML id field"));
    assert!(message.contains("rename the file to 'expected_cluster.yaml'"));
    assert!(message.contains("referenced by node 'nested'"));

    let _ = fs::remove_dir_all(&temp_root);
}
