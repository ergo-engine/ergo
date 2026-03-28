//! discovery tests
//!
//! Purpose:
//! - Cover discovery-private traversal behavior that is hard to observe through the public loader
//!   API alone.
//!
//! Owns:
//! - Unit tests for `ClusterTreeBuilder` memoization and read/traversal behavior.
//!
//! Does not own:
//! - Public loader API coverage; that stays in `tests/loader_api.rs`.
//!
//! Safety notes:
//! - These tests protect the invariant that discovery reads and traverses a shared descendant
//!   source only once after it has completed a successful subtree walk.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use super::*;
use crate::decode::parse_graph_content;
use crate::resolver::{ClusterResolver, ResolvedSourceCandidate, ResolverResult};

fn filesystem_source(path: &str) -> SourceRef {
    let path = PathBuf::from(path);
    SourceRef::Filesystem {
        canonical_path: path.clone(),
        lexical_path: path,
    }
}

fn filesystem_candidate(path: &str) -> ResolvedSourceCandidate {
    ResolvedSourceCandidate {
        source_ref: filesystem_source(path),
        opened_label: path.to_string(),
    }
}

fn resolution_key(
    referring_source: Option<&SourceRef>,
    cluster_id: &str,
) -> (Option<String>, String) {
    (
        referring_source.map(SourceRef::opened_label),
        cluster_id.to_string(),
    )
}

#[derive(Debug)]
struct FakeResolver {
    resolutions: HashMap<(Option<String>, String), Vec<ResolvedSourceCandidate>>,
    contents: HashMap<SourceRef, String>,
    read_counts: RefCell<HashMap<SourceRef, usize>>,
}

impl FakeResolver {
    fn new() -> Self {
        Self {
            resolutions: HashMap::new(),
            contents: HashMap::new(),
            read_counts: RefCell::new(HashMap::new()),
        }
    }

    fn add_resolution(
        mut self,
        referring_source: Option<&SourceRef>,
        cluster_id: &str,
        candidates: Vec<ResolvedSourceCandidate>,
    ) -> Self {
        self.resolutions
            .insert(resolution_key(referring_source, cluster_id), candidates);
        self
    }

    fn add_content(mut self, source_ref: SourceRef, content: &str) -> Self {
        self.contents.insert(source_ref, content.to_string());
        self
    }

    fn read_count(&self, source_ref: &SourceRef) -> usize {
        *self.read_counts.borrow().get(source_ref).unwrap_or(&0)
    }
}

impl ClusterResolver for FakeResolver {
    fn resolve(
        &self,
        cluster_id: &str,
        referring_source: Option<&SourceRef>,
    ) -> Result<ResolverResult, LoaderError> {
        Ok(ResolverResult {
            found: self
                .resolutions
                .get(&resolution_key(referring_source, cluster_id))
                .cloned()
                .unwrap_or_default(),
            search_trace: Vec::new(),
        })
    }

    fn read(&self, source_ref: &SourceRef) -> Result<String, LoaderError> {
        let mut counts = self.read_counts.borrow_mut();
        *counts.entry(source_ref.clone()).or_insert(0) += 1;
        self.contents.get(source_ref).cloned().ok_or_else(|| {
            LoaderError::Discovery(LoaderDiscoveryError {
                message: format!("missing fake content for '{}'", source_ref.opened_label()),
            })
        })
    }
}

#[test]
fn cluster_tree_builder_reads_shared_descendant_once_in_a_diamond_graph() {
    let root_source = filesystem_source("/tmp/root.yaml");
    let left_source = filesystem_source("/tmp/left.yaml");
    let right_source = filesystem_source("/tmp/right.yaml");
    let shared_source = filesystem_source("/tmp/shared.yaml");

    let root = parse_graph_content(
        r#"
kind: cluster
id: root_graph
version: "1.0.0"
nodes:
  left_branch:
    cluster: left@1.0.0
  right_branch:
    cluster: right@1.0.0
edges: []
"#,
        &root_source.opened_label(),
    )
    .expect("parse root graph");

    let resolver = FakeResolver::new()
        .add_resolution(
            Some(&root_source),
            "left",
            vec![filesystem_candidate("/tmp/left.yaml")],
        )
        .add_resolution(
            Some(&root_source),
            "right",
            vec![filesystem_candidate("/tmp/right.yaml")],
        )
        .add_resolution(
            Some(&left_source),
            "shared",
            vec![filesystem_candidate("/tmp/shared.yaml")],
        )
        .add_resolution(
            Some(&right_source),
            "shared",
            vec![filesystem_candidate("/tmp/shared.yaml")],
        )
        .add_content(
            left_source.clone(),
            r#"
kind: cluster
id: left
version: "1.0.0"
nodes:
  shared_child:
    cluster: shared@1.0.0
edges: []
"#,
        )
        .add_content(
            right_source.clone(),
            r#"
kind: cluster
id: right
version: "1.0.0"
nodes:
  shared_child:
    cluster: shared@1.0.0
edges: []
"#,
        )
        .add_content(
            shared_source.clone(),
            r#"
kind: cluster
id: shared
version: "1.0.0"
nodes: {}
edges: []
"#,
        );

    let mut builder = ClusterTreeBuilder::new(&resolver);
    builder
        .visit(root_source.clone(), root)
        .expect("diamond discovery should succeed");

    assert_eq!(resolver.read_count(&left_source), 1);
    assert_eq!(resolver.read_count(&right_source), 1);
    assert_eq!(resolver.read_count(&shared_source), 1);
    assert!(builder
        .clusters
        .contains_key(&("shared".to_string(), "1.0.0".parse().expect("version"))));
}
