use super::*;
use crate::cluster::{
    ExpandedEdge, ImplementationInstance, InputMetadata, OutputMetadata, ParameterMetadata,
    PrimitiveMetadata,
};
use std::collections::HashMap;

#[derive(Default)]
struct TestCatalog {
    metadata: HashMap<(String, String), PrimitiveMetadata>,
}

impl TestCatalog {
    fn with_meta(mut self, id: &str, version: &str, meta: PrimitiveMetadata) -> Self {
        self.metadata
            .insert((id.to_string(), version.to_string()), meta);
        self
    }
}

impl PrimitiveCatalog for TestCatalog {
    fn get(&self, id: &str, version: &String) -> Option<PrimitiveMetadata> {
        self.metadata
            .get(&(id.to_string(), version.clone()))
            .cloned()
    }
}

fn sample_meta(output_name: &str) -> PrimitiveMetadata {
    PrimitiveMetadata {
        kind: PrimitiveKind::Compute,
        inputs: vec![InputMetadata {
            name: "in".to_string(),
            value_type: ValueType::Number,
            required: true,
        }],
        outputs: HashMap::from([(
            output_name.to_string(),
            OutputMetadata {
                value_type: ValueType::Number,
                cardinality: Cardinality::Single,
            },
        )]),
        parameters: vec![ParameterMetadata {
            name: "k".to_string(),
            ty: ParameterType::Number,
            default: Some(ParameterValue::Number(1.25)),
            required: false,
        }],
    }
}

fn sample_graph() -> ExpandedGraph {
    let node_a = ExpandedNode {
        runtime_id: "n1".to_string(),
        authoring_path: vec![],
        implementation: ImplementationInstance {
            impl_id: "prim".to_string(),
            requested_version: "^1".to_string(),
            version: "1.2.0".to_string(),
        },
        parameters: HashMap::from([("scale".to_string(), ParameterValue::Number(3.5))]),
    };
    let node_b = ExpandedNode {
        runtime_id: "n2".to_string(),
        authoring_path: vec![],
        implementation: ImplementationInstance {
            impl_id: "prim".to_string(),
            requested_version: "^1".to_string(),
            version: "1.2.0".to_string(),
        },
        parameters: HashMap::new(),
    };
    ExpandedGraph {
        nodes: HashMap::from([("n1".to_string(), node_a), ("n2".to_string(), node_b)]),
        edges: vec![
            ExpandedEdge {
                from: ExpandedEndpoint::ExternalInput {
                    name: "market".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "n1".to_string(),
                    port_name: "in".to_string(),
                },
            },
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "n1".to_string(),
                    port_name: "out".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "n2".to_string(),
                    port_name: "in".to_string(),
                },
            },
        ],
        boundary_inputs: vec![],
        boundary_outputs: vec![],
    }
}

#[test]
fn rpv1_is_stable_for_same_graph() {
    let graph = sample_graph();
    let catalog = TestCatalog::default().with_meta("prim", "1.2.0", sample_meta("out"));
    let a = compute_runtime_provenance(RuntimeProvenanceScheme::Rpv1, "g", &graph, &catalog)
        .expect("first provenance");
    let b = compute_runtime_provenance(RuntimeProvenanceScheme::Rpv1, "g", &graph, &catalog)
        .expect("second provenance");
    assert_eq!(a, b);
    assert!(a.starts_with("rpv1:sha256:"));
}

#[test]
fn rpv1_changes_when_used_metadata_changes() {
    let graph = sample_graph();
    let catalog_a = TestCatalog::default().with_meta("prim", "1.2.0", sample_meta("out"));
    let catalog_b = TestCatalog::default().with_meta("prim", "1.2.0", sample_meta("out2"));
    let a = compute_runtime_provenance(RuntimeProvenanceScheme::Rpv1, "g", &graph, &catalog_a)
        .expect("provenance A");
    let b = compute_runtime_provenance(RuntimeProvenanceScheme::Rpv1, "g", &graph, &catalog_b)
        .expect("provenance B");
    assert_ne!(a, b);
}

#[test]
fn rpv1_rejects_non_finite_float() {
    let mut graph = sample_graph();
    graph
        .nodes
        .get_mut("n1")
        .unwrap()
        .parameters
        .insert("scale".to_string(), ParameterValue::Number(f64::NAN));
    let catalog = TestCatalog::default().with_meta("prim", "1.2.0", sample_meta("out"));
    let err = compute_runtime_provenance(RuntimeProvenanceScheme::Rpv1, "g", &graph, &catalog)
        .expect_err("non-finite float should fail");
    assert!(matches!(err, RuntimeProvenanceError::NonFiniteFloat { .. }));
}

#[test]
fn rpv1_float_digest_regression() {
    let graph = sample_graph();
    let catalog = TestCatalog::default().with_meta("prim", "1.2.0", sample_meta("out"));
    let digest = compute_runtime_provenance(RuntimeProvenanceScheme::Rpv1, "g", &graph, &catalog)
        .expect("provenance");
    // Cross-platform determinism guard: serde_json + ryu should produce the same digest.
    assert_eq!(
        digest,
        "rpv1:sha256:458fdcc0a5786436cd2fedbbd21cfa6b71bdc63f6ca8e342cb5b7a163aacef0f"
    );
}

#[test]
fn rpv1_ignores_unrelated_catalog_entries() {
    let graph = sample_graph();
    let catalog_a = TestCatalog::default().with_meta("prim", "1.2.0", sample_meta("out"));
    let catalog_b = TestCatalog::default()
        .with_meta("prim", "1.2.0", sample_meta("out"))
        .with_meta(
            "unused",
            "9.9.9",
            PrimitiveMetadata {
                kind: PrimitiveKind::Action,
                inputs: Vec::new(),
                outputs: HashMap::new(),
                parameters: Vec::new(),
            },
        );
    let a = compute_runtime_provenance(RuntimeProvenanceScheme::Rpv1, "g", &graph, &catalog_a)
        .expect("provenance A");
    let b = compute_runtime_provenance(RuntimeProvenanceScheme::Rpv1, "g", &graph, &catalog_b)
        .expect("provenance B");
    assert_eq!(a, b);
}
