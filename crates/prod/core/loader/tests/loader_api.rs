use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use ergo_loader::discovery::{discover_cluster_tree, discover_in_memory_cluster_tree};
use ergo_loader::{
    decode_graph_json, decode_graph_yaml, decode_graph_yaml_labeled, load_cluster_tree,
    load_graph_assets_from_memory, load_graph_assets_from_paths, load_graph_sources,
    load_in_memory_graph_sources, resolve_cluster_candidates, FilesystemGraphBundle,
    InMemoryGraphBundle, InMemorySourceInput, LoaderError, PreparedGraphAssets,
};

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
            assert!(inner.message.contains("<memory>"));
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
fn decode_graph_yaml_labeled_invalid_yaml_mentions_label() {
    let err = decode_graph_yaml_labeled("not: [valid: yaml", "db-row-17")
        .expect_err("invalid YAML must error");
    match err {
        LoaderError::Decode(inner) => {
            assert!(inner.message.contains("db-row-17"));
        }
        _ => panic!("expected decode error"),
    }
}

#[test]
fn decode_graph_json_returns_cluster_definition_root() {
    let graph = decode_graph_json(
        r#"{
  "kind": "cluster",
  "id": "json_root",
  "version": "1.0.0",
  "nodes": {},
  "edges": []
}"#,
    )
    .expect("json graph decode");

    assert_eq!(graph.id, "json_root");
    assert_eq!(graph.version.to_string(), "1.0.0");
}

#[test]
fn decode_graph_json_invalid_json_returns_decode_error() {
    let err = decode_graph_json("{ not valid json").expect_err("invalid JSON must error");
    match err {
        LoaderError::Decode(inner) => {
            assert!(inner.message.contains("parse JSON"));
        }
        _ => panic!("expected decode error"),
    }
}

#[test]
fn decode_graph_yaml_rejects_slash_in_cluster_reference_id() {
    let err = decode_graph_yaml(
        r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: foo/bar@1.0.0
edges: []
"#,
    )
    .expect_err("slash in cluster reference id must error");

    match err {
        LoaderError::Decode(inner) => {
            assert!(inner.message.contains("cluster id must not contain '/'"));
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

    let bundle: FilesystemGraphBundle = load_graph_sources(&graph_path, &[]).expect("bundle load");
    let canonical_graph_path = fs::canonicalize(&graph_path).expect("canonical graph path");
    assert_eq!(bundle.root.id, "demo_bundle_root");
    assert_eq!(bundle.root.version.to_string(), "1.0.0");
    assert!(bundle.discovered_files.contains(&canonical_graph_path));
    assert!(bundle.source_map.contains_key(&canonical_graph_path));

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn load_graph_assets_from_paths_returns_root_clusters_and_labels() {
    let temp_root = make_temp_dir("graph_assets_paths");
    let cluster_dir = temp_root.join("clusters");
    let graph_path = temp_root.join("graph.yaml");
    let nested_path = cluster_dir.join("shared.yaml");
    fs::create_dir_all(&cluster_dir).expect("create cluster dir");

    fs::write(
        &graph_path,
        r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: shared@1.0.0
edges: []
"#,
    )
    .expect("write root graph");
    fs::write(
        &nested_path,
        r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#,
    )
    .expect("write nested graph");

    let assets: PreparedGraphAssets =
        load_graph_assets_from_paths(&graph_path, &[]).expect("load graph assets");

    assert_eq!(assets.root().id, "root_graph");
    assert!(assets
        .clusters()
        .contains_key(&("shared".to_string(), "1.0.0".parse().expect("version"))));
    assert_eq!(
        assets
            .cluster_diagnostic_labels()
            .get(&("shared".to_string(), "1.0.0".parse().expect("version"))),
        Some(&nested_path.display().to_string())
    );

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
fn resolve_cluster_candidates_preserves_candidate_order() {
    let temp_root = make_temp_dir("resolve_candidates_order");
    let root_cluster = temp_root.join("foo.yaml");
    let nested_cluster_dir = temp_root.join("clusters");
    let nested_cluster = nested_cluster_dir.join("foo.yaml");
    let search_dir = temp_root.join("search");
    let search_cluster = search_dir.join("foo.yaml");
    let search_nested_cluster_dir = search_dir.join("clusters");
    let search_nested_cluster = search_nested_cluster_dir.join("foo.yaml");

    fs::create_dir_all(&nested_cluster_dir).expect("create nested cluster dir");
    fs::create_dir_all(&search_nested_cluster_dir).expect("create nested search cluster dir");
    fs::write(&root_cluster, "placeholder").expect("write root cluster");
    fs::write(&nested_cluster, "placeholder").expect("write nested cluster");
    fs::write(&search_cluster, "placeholder").expect("write search cluster");
    fs::write(&search_nested_cluster, "placeholder").expect("write nested search cluster");

    let paths = resolve_cluster_candidates(&temp_root, "foo", &[search_dir.clone()])
        .expect("candidate resolution");

    assert_eq!(
        paths,
        vec![
            root_cluster.clone(),
            nested_cluster.clone(),
            search_cluster.clone(),
            search_nested_cluster.clone()
        ]
    );

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn resolve_cluster_candidates_skips_directory_candidates() {
    let temp_root = make_temp_dir("resolve_candidates_skip_directory");
    let directory_candidate = temp_root.join("foo.yaml");
    let nested_cluster_dir = temp_root.join("clusters");
    let nested_cluster = nested_cluster_dir.join("foo.yaml");

    fs::create_dir_all(&directory_candidate).expect("create directory-shaped candidate");
    fs::create_dir_all(&nested_cluster_dir).expect("create nested cluster dir");
    fs::write(&nested_cluster, "placeholder").expect("write nested cluster");

    let paths = resolve_cluster_candidates(&temp_root, "foo", &[]).expect("candidate resolution");

    assert_eq!(paths, vec![nested_cluster.clone()]);

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn resolve_cluster_candidates_rejects_invalid_cluster_ids() {
    let temp_root = make_temp_dir("resolve_candidates_invalid_id");

    let err = resolve_cluster_candidates(&temp_root, "../outside", &[])
        .expect_err("path-like cluster ids must error");
    assert!(err.to_string().contains("invalid cluster_id '../outside'"));

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn load_graph_sources_search_order_prefers_base_dir_before_clusters_and_search_paths() {
    let temp_root = make_temp_dir("filesystem_search_order_base_first");
    let cluster_dir = temp_root.join("clusters");
    let search_dir = temp_root.join("search");
    fs::create_dir_all(&cluster_dir).expect("create cluster dir");
    fs::create_dir_all(&search_dir).expect("create search dir");

    let graph_path = temp_root.join("graph.yaml");
    let base_candidate = temp_root.join("shared.yaml");
    let cluster_candidate = cluster_dir.join("shared.yaml");
    let search_candidate = search_dir.join("shared.yaml");

    fs::write(
        &graph_path,
        r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: shared@1.0.0
edges: []
"#,
    )
    .expect("write root graph");
    fs::write(&base_candidate, "not: [valid: yaml").expect("write invalid base candidate");
    fs::write(
        &cluster_candidate,
        r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#,
    )
    .expect("write cluster candidate");
    fs::write(
        &search_candidate,
        r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#,
    )
    .expect("write search candidate");

    let err = load_graph_sources(&graph_path, &[search_dir.clone()])
        .expect_err("first matching base-dir candidate should surface first");
    let message = err.to_string();
    assert!(message.contains(&base_candidate.display().to_string()));
    assert!(!message.contains(&cluster_candidate.display().to_string()));
    assert!(!message.contains(&search_candidate.display().to_string()));

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn load_graph_sources_search_order_prefers_clusters_dir_before_search_paths() {
    let temp_root = make_temp_dir("filesystem_search_order_clusters_before_search");
    let cluster_dir = temp_root.join("clusters");
    let search_dir = temp_root.join("search");
    fs::create_dir_all(&cluster_dir).expect("create cluster dir");
    fs::create_dir_all(&search_dir).expect("create search dir");

    let graph_path = temp_root.join("graph.yaml");
    let cluster_candidate = cluster_dir.join("shared.yaml");
    let search_candidate = search_dir.join("shared.yaml");

    fs::write(
        &graph_path,
        r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: shared@1.0.0
edges: []
"#,
    )
    .expect("write root graph");
    fs::write(&cluster_candidate, "not: [valid: yaml")
        .expect("write invalid clusters-dir candidate");
    fs::write(
        &search_candidate,
        r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#,
    )
    .expect("write search candidate");

    let err = load_graph_sources(&graph_path, &[search_dir.clone()])
        .expect_err("clusters/ candidate should surface before search path candidate");
    let message = err.to_string();
    assert!(message.contains(&cluster_candidate.display().to_string()));
    assert!(!message.contains(&search_candidate.display().to_string()));

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn discovery_and_bundle_preserve_non_matching_versions_before_filter() {
    let temp_root = make_temp_dir("pre_filter_versions");
    let cluster_dir = temp_root.join("clusters");
    let search_dir = temp_root.join("search");
    fs::create_dir_all(&cluster_dir).expect("create cluster dir");
    fs::create_dir_all(&search_dir).expect("create search dir");

    let graph_path = temp_root.join("graph.yaml");
    let unmatched_path = cluster_dir.join("shared.yaml");
    let matched_path = search_dir.join("shared.yaml");

    fs::write(
        &graph_path,
        r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: shared@2.0.0
edges: []
"#,
    )
    .expect("write graph");
    fs::write(
        &unmatched_path,
        r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#,
    )
    .expect("write unmatched cluster");
    fs::write(
        &matched_path,
        r#"
kind: cluster
id: shared
version: "2.0.0"
nodes: {}
edges: []
"#,
    )
    .expect("write matched cluster");

    let discovery = discover_cluster_tree(&graph_path, &[search_dir.clone()]).expect("discovery");
    assert_eq!(discovery.root.id, "root_graph");
    assert!(discovery
        .cluster_sources
        .keys()
        .any(|(id, version)| id == "shared" && version.to_string() == "1.0.0"));
    assert!(discovery
        .cluster_sources
        .keys()
        .any(|(id, version)| id == "shared" && version.to_string() == "2.0.0"));

    let loaded_tree =
        load_cluster_tree(&graph_path, &[search_dir.clone()]).expect("load cluster tree");
    assert!(loaded_tree
        .keys()
        .any(|(id, version)| id == "shared" && version.to_string() == "1.0.0"));
    assert!(loaded_tree
        .keys()
        .any(|(id, version)| id == "shared" && version.to_string() == "2.0.0"));

    let bundle = load_graph_sources(&graph_path, &[search_dir.clone()]).expect("bundle load");
    let canonical_unmatched = fs::canonicalize(&unmatched_path).expect("canonical unmatched");
    let canonical_matched = fs::canonicalize(&matched_path).expect("canonical matched");
    assert!(bundle.source_map.contains_key(&canonical_unmatched));
    assert!(bundle.source_map.contains_key(&canonical_matched));

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn load_in_memory_graph_sources_returns_in_memory_graph_bundle_and_sorted_source_ids() {
    let search_roots = vec!["a".to_string()];
    let sources = vec![
        InMemorySourceInput {
            source_id: "z/root.yaml".to_string(),
            source_label: "db-row-root".to_string(),
            content: r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: shared@1.0.0
edges: []
"#
            .to_string(),
        },
        InMemorySourceInput {
            source_id: "a/shared.yaml".to_string(),
            source_label: "db-row-shared".to_string(),
            content: r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#
            .to_string(),
        },
    ];

    let bundle: InMemoryGraphBundle =
        load_in_memory_graph_sources("z/root.yaml", &sources, &search_roots)
            .expect("in-memory bundle load");

    assert_eq!(bundle.root.id, "root_graph");
    assert_eq!(
        bundle.discovered_source_ids,
        vec!["a/shared.yaml".to_string(), "z/root.yaml".to_string()]
    );
    assert_eq!(
        bundle
            .source_map
            .get("a/shared.yaml")
            .expect("shared source text")
            .contains("id: shared"),
        true
    );
    assert_eq!(
        bundle.source_labels.get("a/shared.yaml"),
        Some(&"db-row-shared".to_string())
    );
}

#[test]
fn load_graph_assets_from_memory_returns_root_clusters_and_labels() {
    let search_roots = vec!["a".to_string()];
    let sources = vec![
        InMemorySourceInput {
            source_id: "z/root.yaml".to_string(),
            source_label: "db-row-root".to_string(),
            content: r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: shared@1.0.0
edges: []
"#
            .to_string(),
        },
        InMemorySourceInput {
            source_id: "a/shared.yaml".to_string(),
            source_label: "db-row-shared".to_string(),
            content: r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#
            .to_string(),
        },
    ];

    let assets = load_graph_assets_from_memory("z/root.yaml", &sources, &search_roots)
        .expect("load in-memory graph assets");

    assert_eq!(assets.root().id, "root_graph");
    assert!(assets
        .clusters()
        .contains_key(&("shared".to_string(), "1.0.0".parse().expect("version"))));
    assert_eq!(
        assets
            .cluster_diagnostic_labels()
            .get(&("shared".to_string(), "1.0.0".parse().expect("version"))),
        Some(&"db-row-shared".to_string())
    );
}

#[test]
fn load_in_memory_graph_sources_accepts_json_authoring_text() {
    let search_roots = vec!["search".to_string()];
    let sources = vec![
        InMemorySourceInput {
            source_id: "graph.yaml".to_string(),
            source_label: "root-json-row".to_string(),
            content: r#"{
  "kind": "cluster",
  "id": "root_graph",
  "version": "1.0.0",
  "nodes": {
    "nested": {
      "cluster": "shared@1.0.0"
    }
  },
  "edges": []
}"#
            .to_string(),
        },
        InMemorySourceInput {
            source_id: "search/shared.yaml".to_string(),
            source_label: "shared-json-row".to_string(),
            content: r#"{
  "kind": "cluster",
  "id": "shared",
  "version": "1.0.0",
  "nodes": {},
  "edges": []
}"#
            .to_string(),
        },
    ];

    let bundle = load_in_memory_graph_sources("graph.yaml", &sources, &search_roots)
        .expect("in-memory JSON bundle load");

    assert_eq!(bundle.root.id, "root_graph");
    assert_eq!(
        bundle.discovered_source_ids,
        vec!["graph.yaml".to_string(), "search/shared.yaml".to_string()]
    );
    assert_eq!(
        bundle.source_labels.get("search/shared.yaml"),
        Some(&"shared-json-row".to_string())
    );
}

#[test]
fn load_in_memory_graph_sources_malformed_json_reports_json_not_yaml() {
    let sources = vec![InMemorySourceInput {
        source_id: "graph.yaml".to_string(),
        source_label: "root-json-row".to_string(),
        content: "{ not valid json".to_string(),
    }];

    let err = load_in_memory_graph_sources("graph.yaml", &sources, &[])
        .expect_err("malformed JSON must error");
    let message = err.to_string();
    assert!(message.contains("parse graph 'root-json-row'"));
    assert!(!message.contains("parse YAML"));
    assert!(!message.contains("parse JSON"));
}

#[test]
fn load_in_memory_graph_sources_preserves_graph_shape_errors() {
    let sources = vec![InMemorySourceInput {
        source_id: "graph.yaml".to_string(),
        source_label: "root-json-row".to_string(),
        content: r#"{
  "kind": "not_cluster",
  "id": "root_graph",
  "version": "1.0.0",
  "nodes": {},
  "edges": []
}"#
        .to_string(),
    }];

    let err = load_in_memory_graph_sources("graph.yaml", &sources, &[])
        .expect_err("graph-shape errors must stay specific");
    let message = err.to_string();
    assert!(message.contains("graph 'root-json-row'"));
    assert!(message.contains("kind must be 'cluster'"));
    assert!(!message.contains("expected valid YAML or JSON authoring content"));
}

#[test]
fn load_in_memory_graph_sources_rejects_duplicate_source_ids() {
    let sources = vec![
        InMemorySourceInput {
            source_id: "graph.yaml".to_string(),
            source_label: "row-a".to_string(),
            content: "kind: cluster\nid: one\nversion: \"1.0.0\"\nnodes: {}\nedges: []\n"
                .to_string(),
        },
        InMemorySourceInput {
            source_id: "graph.yaml".to_string(),
            source_label: "row-b".to_string(),
            content: "kind: cluster\nid: two\nversion: \"1.0.0\"\nnodes: {}\nedges: []\n"
                .to_string(),
        },
    ];

    let err = load_in_memory_graph_sources("graph.yaml", &sources, &[])
        .expect_err("duplicate source ids must error");
    assert!(err.to_string().contains("duplicate in-memory source_id"));
}

#[test]
fn load_in_memory_graph_sources_rejects_duplicate_source_labels() {
    let sources = vec![
        InMemorySourceInput {
            source_id: "graph-a.yaml".to_string(),
            source_label: "row-a".to_string(),
            content: "kind: cluster\nid: one\nversion: \"1.0.0\"\nnodes: {}\nedges: []\n"
                .to_string(),
        },
        InMemorySourceInput {
            source_id: "graph-b.yaml".to_string(),
            source_label: "row-a".to_string(),
            content: "kind: cluster\nid: two\nversion: \"1.0.0\"\nnodes: {}\nedges: []\n"
                .to_string(),
        },
    ];

    let err = load_in_memory_graph_sources("graph-a.yaml", &sources, &[])
        .expect_err("duplicate source labels must error");
    assert!(err.to_string().contains("duplicate in-memory source_label"));
}

#[test]
fn load_in_memory_graph_sources_rejects_rooted_source_ids() {
    let sources = vec![InMemorySourceInput {
        source_id: "/graph.yaml".to_string(),
        source_label: "row-a".to_string(),
        content: "kind: cluster\nid: one\nversion: \"1.0.0\"\nnodes: {}\nedges: []\n".to_string(),
    }];

    let err = load_in_memory_graph_sources("/graph.yaml", &sources, &[])
        .expect_err("rooted logical source ids must error");
    assert!(err
        .to_string()
        .contains("in-memory source_id must be a relative logical path"));
}

#[test]
fn load_in_memory_graph_sources_rejects_backslash_source_ids() {
    let sources = vec![InMemorySourceInput {
        source_id: "graphs\\root.yaml".to_string(),
        source_label: "row-a".to_string(),
        content: "kind: cluster\nid: one\nversion: \"1.0.0\"\nnodes: {}\nedges: []\n".to_string(),
    }];

    let err = load_in_memory_graph_sources("graphs\\root.yaml", &sources, &[])
        .expect_err("backslash logical source ids must error");
    assert!(err
        .to_string()
        .contains("in-memory source_id must use '/' separators"));
}

#[test]
fn load_in_memory_graph_sources_rejects_colon_source_ids() {
    let sources = vec![InMemorySourceInput {
        source_id: "C:/graph.yaml".to_string(),
        source_label: "row-a".to_string(),
        content: "kind: cluster\nid: one\nversion: \"1.0.0\"\nnodes: {}\nedges: []\n".to_string(),
    }];

    let err = load_in_memory_graph_sources("C:/graph.yaml", &sources, &[])
        .expect_err("colon logical source ids must error");
    assert!(err
        .to_string()
        .contains("in-memory source_id must not contain ':'"));
}

#[test]
fn load_in_memory_graph_sources_rejects_dot_segments_in_source_ids() {
    let sources = vec![InMemorySourceInput {
        source_id: "graphs/../root.yaml".to_string(),
        source_label: "row-a".to_string(),
        content: "kind: cluster\nid: one\nversion: \"1.0.0\"\nnodes: {}\nedges: []\n".to_string(),
    }];

    let err = load_in_memory_graph_sources("graphs/../root.yaml", &sources, &[])
        .expect_err("dot-segment logical source ids must error");
    assert!(err
        .to_string()
        .contains("in-memory source_id must not contain '.' or '..' segments"));
}

#[test]
fn load_in_memory_graph_sources_rejects_invalid_search_roots() {
    let sources = vec![InMemorySourceInput {
        source_id: "graphs/root.yaml".to_string(),
        source_label: "row-a".to_string(),
        content: "kind: cluster\nid: one\nversion: \"1.0.0\"\nnodes: {}\nedges: []\n".to_string(),
    }];

    let err =
        load_in_memory_graph_sources("graphs/root.yaml", &sources, &["search\\root".to_string()])
            .expect_err("invalid logical search roots must error");
    assert!(err
        .to_string()
        .contains("in-memory search_root must use '/' separators"));
}

#[test]
fn load_in_memory_graph_sources_rejects_colon_search_roots() {
    let sources = vec![InMemorySourceInput {
        source_id: "graphs/root.yaml".to_string(),
        source_label: "row-a".to_string(),
        content: "kind: cluster\nid: one\nversion: \"1.0.0\"\nnodes: {}\nedges: []\n".to_string(),
    }];

    let err =
        load_in_memory_graph_sources("graphs/root.yaml", &sources, &["C:/search".to_string()])
            .expect_err("colon logical search roots must error");
    assert!(err
        .to_string()
        .contains("in-memory search_root must not contain ':'"));
}

#[test]
fn discover_in_memory_cluster_tree_precedence_follows_caller_order() {
    let search_roots = vec!["a".to_string(), "b".to_string()];
    let sources = vec![
        InMemorySourceInput {
            source_id: "b/shared.yaml".to_string(),
            source_label: "beta-row".to_string(),
            content: "not: [valid: yaml".to_string(),
        },
        InMemorySourceInput {
            source_id: "a/shared.yaml".to_string(),
            source_label: "alpha-row".to_string(),
            content: r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#
            .to_string(),
        },
        InMemorySourceInput {
            source_id: "graph.yaml".to_string(),
            source_label: "root-row".to_string(),
            content: r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: shared@1.0.0
edges: []
"#
            .to_string(),
        },
    ];

    let err = load_in_memory_graph_sources("graph.yaml", &sources, &search_roots)
        .expect_err("caller-order precedence should surface the first matching bad source");
    assert!(err.to_string().contains("beta-row"));
}

#[test]
fn discover_in_memory_cluster_tree_preserves_non_matching_versions_before_filter() {
    let search_roots = vec!["search".to_string()];
    let sources = vec![
        InMemorySourceInput {
            source_id: "graph.yaml".to_string(),
            source_label: "root-row".to_string(),
            content: r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: shared@2.0.0
edges: []
"#
            .to_string(),
        },
        InMemorySourceInput {
            source_id: "clusters/shared.yaml".to_string(),
            source_label: "shared-v1".to_string(),
            content: r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#
            .to_string(),
        },
        InMemorySourceInput {
            source_id: "search/shared.yaml".to_string(),
            source_label: "shared-v2".to_string(),
            content: r#"
kind: cluster
id: shared
version: "2.0.0"
nodes: {}
edges: []
"#
            .to_string(),
        },
    ];

    let discovery = discover_in_memory_cluster_tree("graph.yaml", &sources, &search_roots)
        .expect("in-memory discovery");
    assert_eq!(discovery.root.id, "root_graph");

    assert!(discovery
        .cluster_source_ids
        .keys()
        .any(|(id, version)| id == "shared" && version.to_string() == "1.0.0"));
    assert!(discovery
        .cluster_source_ids
        .keys()
        .any(|(id, version)| id == "shared" && version.to_string() == "2.0.0"));
    assert_eq!(
        discovery
            .cluster_source_labels
            .get(&("shared".to_string(), "2.0.0".parse().expect("version"))),
        Some(&"shared-v2".to_string())
    );
    assert_eq!(
        discovery
            .cluster_diagnostic_labels
            .get(&("shared".to_string(), "2.0.0".parse().expect("version"))),
        Some(&"shared-v2".to_string())
    );
}

#[test]
fn discover_in_memory_cluster_tree_uses_referring_logical_dir_for_nested_lookup() {
    let sources = vec![
        InMemorySourceInput {
            source_id: "a/root.yaml".to_string(),
            source_label: "root-a-row".to_string(),
            content: r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: shared@1.0.0
edges: []
"#
            .to_string(),
        },
        InMemorySourceInput {
            source_id: "b/root.yaml".to_string(),
            source_label: "root-b-row".to_string(),
            content: r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: shared@1.0.0
edges: []
"#
            .to_string(),
        },
        InMemorySourceInput {
            source_id: "a/shared.yaml".to_string(),
            source_label: "alpha-row".to_string(),
            content: r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#
            .to_string(),
        },
        InMemorySourceInput {
            source_id: "b/shared.yaml".to_string(),
            source_label: "beta-row".to_string(),
            content: "not: [valid: yaml".to_string(),
        },
    ];

    let discovery_a = discover_in_memory_cluster_tree("a/root.yaml", &sources, &[])
        .expect("discovery for root a");
    assert_eq!(discovery_a.root.id, "root_graph");
    assert_eq!(
        discovery_a
            .cluster_source_labels
            .get(&("shared".to_string(), "1.0.0".parse().expect("version"))),
        Some(&"alpha-row".to_string())
    );

    let err = discover_in_memory_cluster_tree("b/root.yaml", &sources, &[])
        .expect_err("root b should resolve through b/shared.yaml and fail there");
    assert!(err.to_string().contains("beta-row"));
}

#[test]
fn discover_in_memory_cluster_tree_parses_and_returns_root_from_root_source_id() {
    let sources = vec![
        InMemorySourceInput {
            source_id: "a/root.yaml".to_string(),
            source_label: "root-a-row".to_string(),
            content: r#"
kind: cluster
id: root_a
version: "1.0.0"
nodes: {}
edges: []
"#
            .to_string(),
        },
        InMemorySourceInput {
            source_id: "b/root.yaml".to_string(),
            source_label: "root-b-row".to_string(),
            content: r#"
kind: cluster
id: root_b
version: "1.0.0"
nodes: {}
edges: []
"#
            .to_string(),
        },
    ];

    let discovery = discover_in_memory_cluster_tree("b/root.yaml", &sources, &[])
        .expect("discover root from source id");
    assert_eq!(discovery.root.id, "root_b");
}

#[test]
fn discover_cluster_tree_parses_and_returns_root_from_root_path() {
    let temp_root = make_temp_dir("filesystem_root_mismatch");
    let root_a_path = temp_root.join("a.yaml");
    let root_b_path = temp_root.join("b.yaml");

    fs::write(
        &root_a_path,
        r#"
kind: cluster
id: root_a
version: "1.0.0"
nodes: {}
edges: []
"#,
    )
    .expect("write root a");
    fs::write(
        &root_b_path,
        r#"
kind: cluster
id: root_b
version: "1.0.0"
nodes: {}
edges: []
"#,
    )
    .expect("write root b");

    let discovery = discover_cluster_tree(&root_b_path, &[]).expect("discover root from root path");
    assert_eq!(discovery.root.id, "root_b");

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn discover_cluster_tree_uses_referring_filesystem_dir_for_nested_lookup() {
    let temp_root = make_temp_dir("filesystem_referrer_scope");
    let a_dir = temp_root.join("a");
    let b_dir = temp_root.join("b");
    fs::create_dir_all(&a_dir).expect("create a dir");
    fs::create_dir_all(&b_dir).expect("create b dir");

    let root_a_path = a_dir.join("root.yaml");
    let root_b_path = b_dir.join("root.yaml");
    let shared_a_path = a_dir.join("shared.yaml");
    let shared_b_path = b_dir.join("shared.yaml");

    let root_content = r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: shared@1.0.0
edges: []
"#;

    fs::write(&root_a_path, root_content).expect("write root a");
    fs::write(&root_b_path, root_content).expect("write root b");
    fs::write(
        &shared_a_path,
        r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#,
    )
    .expect("write shared a");
    fs::write(&shared_b_path, "not: [valid: yaml").expect("write shared b");

    let discovery_a = discover_cluster_tree(&root_a_path, &[]).expect("discovery for root a");
    assert_eq!(discovery_a.root.id, "root_graph");
    assert_eq!(
        discovery_a
            .cluster_sources
            .get(&("shared".to_string(), "1.0.0".parse().expect("version"))),
        Some(&fs::canonicalize(&shared_a_path).expect("canonical shared a"))
    );
    assert_eq!(
        discovery_a
            .cluster_diagnostic_labels
            .get(&("shared".to_string(), "1.0.0".parse().expect("version"))),
        Some(&shared_a_path.display().to_string())
    );

    let err = discover_cluster_tree(&root_b_path, &[])
        .expect_err("root b should resolve through b/shared.yaml and fail there");
    assert!(err
        .to_string()
        .contains(&shared_b_path.display().to_string()));

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn discover_cluster_tree_skips_directory_candidates_and_uses_real_file() {
    let temp_root = make_temp_dir("filesystem_skip_directory_candidate");
    let cluster_dir = temp_root.join("clusters");
    fs::create_dir_all(&cluster_dir).expect("create cluster dir");

    let graph_path = temp_root.join("graph.yaml");
    let directory_candidate = temp_root.join("shared.yaml");
    let real_candidate = cluster_dir.join("shared.yaml");

    fs::write(
        &graph_path,
        r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: shared@1.0.0
edges: []
"#,
    )
    .expect("write root graph");
    fs::create_dir_all(&directory_candidate).expect("create directory-shaped candidate");
    fs::write(
        &real_candidate,
        r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#,
    )
    .expect("write real shared cluster");

    let discovery =
        discover_cluster_tree(&graph_path, &[]).expect("discovery should skip directory candidate");
    let key = ("shared".to_string(), "1.0.0".parse().expect("version"));
    assert_eq!(
        discovery.cluster_sources.get(&key),
        Some(&fs::canonicalize(&real_candidate).expect("canonical real candidate"))
    );
    assert_eq!(
        discovery.cluster_diagnostic_labels.get(&key),
        Some(&real_candidate.display().to_string())
    );

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn discover_cluster_tree_returns_cluster_discovery_fields() {
    let temp_root = make_temp_dir("filesystem_cluster_discovery_fields");
    let cluster_dir = temp_root.join("clusters");
    fs::create_dir_all(&cluster_dir).expect("create cluster dir");

    let graph_path = temp_root.join("graph.yaml");
    let shared_path = cluster_dir.join("shared.yaml");

    fs::write(
        &graph_path,
        r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: shared@1.0.0
edges: []
"#,
    )
    .expect("write root graph");
    fs::write(
        &shared_path,
        r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#,
    )
    .expect("write shared cluster");

    let discovery = discover_cluster_tree(&graph_path, &[]).expect("discover cluster tree");
    let key = ("shared".to_string(), "1.0.0".parse().expect("version"));
    assert_eq!(discovery.root.id, "root_graph");
    assert!(discovery.clusters.contains_key(&key));
    assert_eq!(
        discovery.cluster_sources.get(&key),
        Some(&fs::canonicalize(&shared_path).expect("canonical shared path"))
    );
    assert_eq!(
        discovery.cluster_diagnostic_labels.get(&key),
        Some(&shared_path.display().to_string())
    );

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn load_cluster_tree_returns_nested_cluster_definitions() {
    let temp_root = make_temp_dir("filesystem_load_cluster_tree_fields");
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
    cluster: shared@1.0.0
edges: []
"#,
    )
    .expect("write root graph");
    fs::write(
        cluster_dir.join("shared.yaml"),
        r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#,
    )
    .expect("write shared cluster");

    let tree = load_cluster_tree(&graph_path, &[]).expect("load cluster tree");
    let key = ("shared".to_string(), "1.0.0".parse().expect("version"));
    assert_eq!(
        tree.get(&key).expect("shared cluster should be present").id,
        "shared"
    );

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
fn load_graph_sources_skips_directory_candidates_and_continues_to_real_file() {
    let temp_root = make_temp_dir("bundle_skip_directory_candidate");
    let cluster_dir = temp_root.join("clusters");
    fs::create_dir_all(&cluster_dir).expect("create cluster dir");

    let graph_path = temp_root.join("graph.yaml");
    let directory_candidate = temp_root.join("shared.yaml");
    let real_candidate = cluster_dir.join("shared.yaml");

    fs::write(
        &graph_path,
        r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: shared@1.0.0
edges: []
"#,
    )
    .expect("write root graph");
    fs::create_dir_all(&directory_candidate).expect("create directory-shaped candidate");
    fs::write(
        &real_candidate,
        r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#,
    )
    .expect("write real shared cluster");

    let bundle =
        load_graph_sources(&graph_path, &[]).expect("bundle load should skip directory candidate");
    let canonical_graph_path = fs::canonicalize(&graph_path).expect("canonical graph path");
    let canonical_real_candidate =
        fs::canonicalize(&real_candidate).expect("canonical real candidate");

    assert_eq!(bundle.discovered_files.len(), 2);
    assert!(bundle.discovered_files.contains(&canonical_graph_path));
    assert!(bundle.discovered_files.contains(&canonical_real_candidate));
    assert!(bundle.source_map.contains_key(&canonical_real_candidate));
    assert!(!bundle.source_map.contains_key(&directory_candidate));

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
    assert!(message.contains("graph id is 'actual_cluster'"));
    assert!(message.contains("filename must match the decoded graph id field"));
    assert!(message.contains("rename the file to 'expected_cluster.yaml'"));
    assert!(message.contains("referenced by node 'nested'"));

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn load_in_memory_graph_sources_missing_cluster_reports_logical_search_scope_and_label() {
    let search_roots = vec!["search".to_string()];
    let sources = vec![InMemorySourceInput {
        source_id: "graphs/root.yaml".to_string(),
        source_label: "root-row".to_string(),
        content: r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: missing_cluster@1.0.0
edges: []
"#
        .to_string(),
    }];

    let err = load_in_memory_graph_sources("graphs/root.yaml", &sources, &search_roots)
        .expect_err("missing in-memory cluster must error");
    let message = err.to_string();
    assert!(message.contains("logical source paths ending in 'missing_cluster.yaml'"));
    assert!(message.contains("graphs/missing_cluster.yaml"));
    assert!(message.contains("graphs/clusters/missing_cluster.yaml"));
    assert!(message.contains("search/missing_cluster.yaml"));
    assert!(message.contains("search/clusters/missing_cluster.yaml"));
    assert!(message.contains("source_id must end in 'missing_cluster.yaml'"));
    assert!(message.contains("referenced by node 'nested' in 'root-row'"));
    assert!(!message.contains("rename the file"));
}

#[test]
fn load_in_memory_graph_sources_cluster_id_mismatch_uses_label_first_logical_path_text() {
    let search_roots = vec!["graphs".to_string()];
    let sources = vec![
        InMemorySourceInput {
            source_id: "graphs/root.yaml".to_string(),
            source_label: "root-row".to_string(),
            content: r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: expected_cluster@1.0.0
edges: []
"#
            .to_string(),
        },
        InMemorySourceInput {
            source_id: "graphs/expected_cluster.yaml".to_string(),
            source_label: "candidate-row".to_string(),
            content: r#"
kind: cluster
id: actual_cluster
version: "1.0.0"
nodes: {}
edges: []
"#
            .to_string(),
        },
    ];

    let err = load_in_memory_graph_sources("graphs/root.yaml", &sources, &search_roots)
        .expect_err("in-memory id mismatch must error");
    let message = err.to_string();
    assert!(message.contains(
        "opened in-memory source 'candidate-row' (source_id 'graphs/expected_cluster.yaml')"
    ));
    assert!(message.contains("expected_cluster@1.0.0"));
    assert!(message.contains("graph id is 'actual_cluster'"));
    assert!(message
        .contains("graph reference, source_id, and graph id must agree on 'expected_cluster'"));
    assert!(message.contains(
        "change the graph id to 'expected_cluster' or change the graph reference and source_id to match"
    ));
    assert!(message.contains("referenced by node 'nested' in 'root-row'"));
    assert!(!message.contains("rename the file"));
}

#[test]
fn discover_in_memory_cluster_tree_duplicate_definition_mentions_labels_not_files() {
    let search_roots = vec!["a".to_string(), "b".to_string()];
    let sources = vec![
        InMemorySourceInput {
            source_id: "graph.yaml".to_string(),
            source_label: "root-row".to_string(),
            content: r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: shared@1.0.0
edges: []
"#
            .to_string(),
        },
        InMemorySourceInput {
            source_id: "a/shared.yaml".to_string(),
            source_label: "alpha-row".to_string(),
            content: r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#
            .to_string(),
        },
        InMemorySourceInput {
            source_id: "b/shared.yaml".to_string(),
            source_label: "beta-row".to_string(),
            content: r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#
            .to_string(),
        },
    ];

    let err = load_in_memory_graph_sources("graph.yaml", &sources, &search_roots)
        .expect_err("duplicate in-memory definitions must error");
    let message = err.to_string();
    assert!(message.contains("defined by multiple sources"));
    assert!(message.contains("'alpha-row' (source_id 'a/shared.yaml')"));
    assert!(message.contains("'beta-row' (source_id 'b/shared.yaml')"));
    assert!(!message.contains("multiple files"));
}

#[test]
fn discover_in_memory_cluster_tree_cycle_reports_circular_reference() {
    let sources = vec![
        InMemorySourceInput {
            source_id: "graph.yaml".to_string(),
            source_label: "root-row".to_string(),
            content: r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: alpha@1.0.0
edges: []
"#
            .to_string(),
        },
        InMemorySourceInput {
            source_id: "alpha.yaml".to_string(),
            source_label: "alpha-row".to_string(),
            content: r#"
kind: cluster
id: alpha
version: "1.0.0"
nodes:
  nested:
    cluster: beta@1.0.0
edges: []
"#
            .to_string(),
        },
        InMemorySourceInput {
            source_id: "beta.yaml".to_string(),
            source_label: "beta-row".to_string(),
            content: r#"
kind: cluster
id: beta
version: "1.0.0"
nodes:
  nested:
    cluster: alpha@1.0.0
edges: []
"#
            .to_string(),
        },
    ];

    let err = discover_in_memory_cluster_tree("graph.yaml", &sources, &[])
        .expect_err("in-memory cycle must error");
    let message = err.to_string();
    assert!(message.contains("circular cluster reference"));
    assert!(message.contains("alpha-row") || message.contains("beta-row"));
}

#[test]
fn load_graph_sources_duplicate_definition_reports_multiple_files() {
    let temp_root = make_temp_dir("filesystem_duplicate_definition");
    let a_dir = temp_root.join("a");
    let b_dir = temp_root.join("b");
    fs::create_dir_all(&a_dir).expect("create a dir");
    fs::create_dir_all(&b_dir).expect("create b dir");

    let graph_path = temp_root.join("graph.yaml");
    fs::write(
        &graph_path,
        r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: shared@1.0.0
edges: []
"#,
    )
    .expect("write graph");
    for dir in [&a_dir, &b_dir] {
        fs::write(
            dir.join("shared.yaml"),
            r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#,
        )
        .expect("write shared cluster");
    }

    let err = load_graph_sources(&graph_path, &[a_dir.clone(), b_dir.clone()])
        .expect_err("filesystem duplicate definitions must error");
    let message = err.to_string();
    assert!(message.contains("defined by multiple files"));
    assert!(message.contains(
        &fs::canonicalize(a_dir.join("shared.yaml"))
            .expect("canonical a")
            .display()
            .to_string()
    ));
    assert!(message.contains(
        &fs::canonicalize(b_dir.join("shared.yaml"))
            .expect("canonical b")
            .display()
            .to_string()
    ));
    assert!(!message.contains("multiple sources"));

    let _ = fs::remove_dir_all(&temp_root);
}

#[test]
fn load_graph_sources_cycle_reports_circular_reference() {
    let temp_root = make_temp_dir("filesystem_cycle");
    let cluster_dir = temp_root.join("clusters");
    fs::create_dir_all(&cluster_dir).expect("create cluster dir");

    let graph_path = temp_root.join("graph.yaml");
    let loop_path = cluster_dir.join("loop.yaml");

    fs::write(
        &graph_path,
        r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: loop@1.0.0
edges: []
"#,
    )
    .expect("write root graph");
    fs::write(
        &loop_path,
        r#"
kind: cluster
id: loop
version: "1.0.0"
nodes:
  again:
    cluster: loop@1.0.0
edges: []
"#,
    )
    .expect("write looping cluster");

    let err = load_graph_sources(&graph_path, &[]).expect_err("cycle must error");
    let message = err.to_string();
    assert!(message.contains("circular cluster reference"));
    assert!(message.contains(&loop_path.display().to_string()));

    let _ = fs::remove_dir_all(&temp_root);
}

#[cfg(unix)]
#[test]
fn load_graph_sources_dedupes_filesystem_canonical_aliases() {
    use std::os::unix::fs::symlink;

    let temp_root = make_temp_dir("filesystem_canonical_alias_dedupe");
    let real_dir = temp_root.join("real");
    let alias_dir = temp_root.join("alias");
    fs::create_dir_all(&real_dir).expect("create real dir");

    let graph_path = temp_root.join("graph.yaml");
    let real_shared_path = real_dir.join("shared.yaml");
    symlink(&real_dir, &alias_dir).expect("create alias symlink");

    fs::write(
        &graph_path,
        r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  nested:
    cluster: shared@1.0.0
edges: []
"#,
    )
    .expect("write root graph");
    fs::write(
        &real_shared_path,
        r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#,
    )
    .expect("write shared cluster");

    let discovery = discover_cluster_tree(&graph_path, &[alias_dir.clone(), real_dir.clone()])
        .expect("discover");
    let key = ("shared".to_string(), "1.0.0".parse().expect("version"));
    let canonical_shared = fs::canonicalize(&real_shared_path).expect("canonical shared");
    assert_eq!(discovery.cluster_sources.get(&key), Some(&canonical_shared));
    assert_eq!(
        discovery.cluster_diagnostic_labels.get(&key),
        Some(&alias_dir.join("shared.yaml").display().to_string())
    );

    let bundle = load_graph_sources(&graph_path, &[alias_dir, real_dir]).expect("load bundle");
    assert_eq!(bundle.discovered_files.len(), 2);
    assert!(bundle.source_map.contains_key(&canonical_shared));

    let _ = fs::remove_dir_all(&temp_root);
}
