use super::*;
use crate::common::ErrorInfo;
struct TestLoader {
    clusters: HashMap<(String, Version), ClusterDefinition>,
}

impl TestLoader {
    fn new() -> Self {
        Self {
            clusters: HashMap::new(),
        }
    }

    fn with_cluster(mut self, def: ClusterDefinition) -> Self {
        self.clusters
            .insert((def.id.clone(), def.version.clone()), def);
        self
    }
}

impl ClusterLoader for TestLoader {
    fn load(&self, id: &str, version: &Version) -> Option<ClusterDefinition> {
        self.clusters
            .get(&(id.to_string(), version.clone()))
            .cloned()
    }
}

impl ClusterVersionIndex for TestLoader {
    fn available_versions(&self, id: &str) -> Vec<Version> {
        let mut versions = self
            .clusters
            .keys()
            .filter_map(|(candidate_id, version)| {
                if candidate_id == id {
                    Some(version.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        versions.sort();
        versions
    }
}

fn empty_parameters() -> Vec<ParameterSpec> {
    Vec::new()
}

fn meta(kind: PrimitiveKind, outputs: &[(&str, ValueType)]) -> PrimitiveMetadata {
    let outputs_map = outputs
        .iter()
        .map(|(name, ty)| {
            (
                name.to_string(),
                OutputMetadata {
                    value_type: ty.clone(),
                    cardinality: Cardinality::Single,
                },
            )
        })
        .collect();
    PrimitiveMetadata {
        kind,
        inputs: Vec::new(),
        outputs: outputs_map,
        parameters: Vec::new(),
    }
}

/// A.1: Helper to create metadata with parameter specs
fn meta_with_params(
    kind: PrimitiveKind,
    outputs: &[(&str, ValueType)],
    params: Vec<ParameterMetadata>,
) -> PrimitiveMetadata {
    let outputs_map = outputs
        .iter()
        .map(|(name, ty)| {
            (
                name.to_string(),
                OutputMetadata {
                    value_type: ty.clone(),
                    cardinality: Cardinality::Single,
                },
            )
        })
        .collect();
    PrimitiveMetadata {
        kind,
        inputs: Vec::new(),
        outputs: outputs_map,
        parameters: params,
    }
}

#[derive(Default)]
struct TestCatalog {
    metadata: HashMap<(String, Version), PrimitiveMetadata>,
}

impl TestCatalog {
    fn with_metadata(mut self, id: &str, version: &str, meta: PrimitiveMetadata) -> Self {
        self.metadata
            .insert((id.to_string(), version.to_string()), meta);
        self
    }
}

impl PrimitiveCatalog for TestCatalog {
    fn get(&self, id: &str, version: &Version) -> Option<PrimitiveMetadata> {
        self.metadata
            .get(&(id.to_string(), version.clone()))
            .cloned()
    }
}

impl PrimitiveVersionIndex for TestCatalog {
    fn available_versions(&self, id: &str) -> Vec<Version> {
        let mut versions = self
            .metadata
            .keys()
            .filter_map(|(candidate_id, version)| {
                if candidate_id == id {
                    Some(version.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        versions.sort();
        versions
    }
}

#[test]
fn expands_primitive_cluster() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "p1".to_string(),
        NodeInstance {
            id: "p1".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let cluster = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();
    let expanded = expand(&cluster, &loader, &catalog).unwrap();

    assert_eq!(expanded.nodes.len(), 1);
    assert!(expanded.edges.is_empty());

    let node = expanded.nodes.values().next().unwrap();
    assert_eq!(
        node.authoring_path,
        vec![("root".to_string(), "p1".to_string())]
    );
    assert_eq!(node.implementation.impl_id, "prim");
}

#[test]
fn expands_nested_cluster_and_rewires_inputs() {
    let mut inner_nodes = HashMap::new();
    inner_nodes.insert(
        "leaf".to_string(),
        NodeInstance {
            id: "leaf".to_string(),
            kind: NodeKind::Impl {
                impl_id: "leaf_prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let inner = ClusterDefinition {
        id: "inner".to_string(),
        version: "1.0.0".to_string(),
        nodes: inner_nodes,
        edges: vec![Edge {
            from: OutputRef {
                node_id: "in".to_string(),
                port_name: "out".to_string(),
            },
            to: InputRef {
                node_id: "leaf".to_string(),
                port_name: "input".to_string(),
            },
        }],
        input_ports: vec![InputPortSpec {
            name: "in_port".to_string(),
            maps_to: GraphInputPlaceholder {
                name: "in".to_string(),
                ty: ValueType::Number,
                required: true,
            },
        }],
        output_ports: vec![OutputPortSpec {
            name: "out_port".to_string(),
            maps_to: OutputRef {
                node_id: "leaf".to_string(),
                port_name: "out".to_string(),
            },
        }],
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let mut outer_nodes = HashMap::new();
    outer_nodes.insert(
        "src".to_string(),
        NodeInstance {
            id: "src".to_string(),
            kind: NodeKind::Impl {
                impl_id: "src_prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );
    outer_nodes.insert(
        "nested".to_string(),
        NodeInstance {
            id: "nested".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "inner".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );
    outer_nodes.insert(
        "sink".to_string(),
        NodeInstance {
            id: "sink".to_string(),
            kind: NodeKind::Impl {
                impl_id: "sink_prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let outer = ClusterDefinition {
        id: "outer".to_string(),
        version: "1.0.0".to_string(),
        nodes: outer_nodes,
        edges: vec![
            Edge {
                from: OutputRef {
                    node_id: "src".to_string(),
                    port_name: "emit".to_string(),
                },
                to: InputRef {
                    node_id: "nested".to_string(),
                    port_name: "in_port".to_string(),
                },
            },
            Edge {
                from: OutputRef {
                    node_id: "nested".to_string(),
                    port_name: "out_port".to_string(),
                },
                to: InputRef {
                    node_id: "sink".to_string(),
                    port_name: "input".to_string(),
                },
            },
        ],
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new().with_cluster(inner);
    let catalog = TestCatalog::default();
    let expanded = expand(&outer, &loader, &catalog).unwrap();

    assert_eq!(expanded.nodes.len(), 3);

    let mut external_edges = Vec::new();
    let mut node_edges = Vec::new();
    for edge in expanded.edges {
        match (&edge.from, &edge.to) {
            (ExpandedEndpoint::ExternalInput { .. }, _)
            | (_, ExpandedEndpoint::ExternalInput { .. }) => external_edges.push(edge),
            _ => node_edges.push(edge),
        }
    }

    assert!(external_edges.is_empty());
    assert_eq!(node_edges.len(), 2);
}

#[test]
fn infers_source_like_signature() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "s".to_string(),
        NodeInstance {
            id: "s".to_string(),
            kind: NodeKind::Impl {
                impl_id: "source".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let cluster = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: vec![OutputPortSpec {
            name: "out".to_string(),
            maps_to: OutputRef {
                node_id: "s".to_string(),
                port_name: "value".to_string(),
            },
        }],
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();
    let expanded = expand(&cluster, &loader, &catalog).unwrap();

    let catalog = TestCatalog::default().with_metadata(
        "source",
        "1.0.0",
        meta(PrimitiveKind::Source, &[("value", ValueType::Number)]),
    );

    let sig = infer_signature(&expanded, &catalog).unwrap();

    assert_eq!(sig.kind, BoundaryKind::SourceLike);
    assert!(sig.is_origin);
    assert_eq!(sig.outputs.len(), 1);
    assert!(sig.outputs[0].wireable);
    assert_eq!(sig.outputs[0].ty, ValueType::Number);
}

#[test]
fn infers_action_like_signature_when_outputs_not_wireable() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "a".to_string(),
        NodeInstance {
            id: "a".to_string(),
            kind: NodeKind::Impl {
                impl_id: "action".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let cluster = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: vec![OutputPortSpec {
            name: "outcome".to_string(),
            maps_to: OutputRef {
                node_id: "a".to_string(),
                port_name: "outcome".to_string(),
            },
        }],
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();
    let expanded = expand(&cluster, &loader, &catalog).unwrap();

    let catalog = TestCatalog::default().with_metadata(
        "action",
        "1.0.0",
        meta(PrimitiveKind::Action, &[("outcome", ValueType::Event)]),
    );

    let sig = infer_signature(&expanded, &catalog).unwrap();

    assert_eq!(sig.kind, BoundaryKind::ActionLike);
    assert!(sig.has_side_effects);
    assert!(!sig.outputs[0].wireable);
}

#[test]
fn infers_trigger_like_signature_with_event_output() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "t".to_string(),
        NodeInstance {
            id: "t".to_string(),
            kind: NodeKind::Impl {
                impl_id: "trigger".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let cluster = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: vec![InputPortSpec {
            name: "in".to_string(),
            maps_to: GraphInputPlaceholder {
                name: "in".to_string(),
                ty: ValueType::Number,
                required: true,
            },
        }],
        output_ports: vec![OutputPortSpec {
            name: "out".to_string(),
            maps_to: OutputRef {
                node_id: "t".to_string(),
                port_name: "emitted".to_string(),
            },
        }],
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();
    let expanded = expand(&cluster, &loader, &catalog).unwrap();

    let catalog = TestCatalog::default().with_metadata(
        "trigger",
        "1.0.0",
        meta(PrimitiveKind::Trigger, &[("emitted", ValueType::Event)]),
    );

    let sig = infer_signature(&expanded, &catalog).unwrap();

    assert_eq!(sig.kind, BoundaryKind::TriggerLike);
    assert!(!sig.is_origin);
    assert!(sig.outputs[0].wireable);
}

#[test]
fn infers_compute_like_signature() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "c".to_string(),
        NodeInstance {
            id: "c".to_string(),
            kind: NodeKind::Impl {
                impl_id: "compute".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let cluster = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: vec![InputPortSpec {
            name: "in".to_string(),
            maps_to: GraphInputPlaceholder {
                name: "in".to_string(),
                ty: ValueType::Number,
                required: true,
            },
        }],
        output_ports: vec![OutputPortSpec {
            name: "out".to_string(),
            maps_to: OutputRef {
                node_id: "c".to_string(),
                port_name: "value".to_string(),
            },
        }],
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();
    let expanded = expand(&cluster, &loader, &catalog).unwrap();

    let catalog = TestCatalog::default().with_metadata(
        "compute",
        "1.0.0",
        meta(PrimitiveKind::Compute, &[("value", ValueType::Number)]),
    );

    let sig = infer_signature(&expanded, &catalog).unwrap();

    assert_eq!(sig.kind, BoundaryKind::ComputeLike);
    assert!(!sig.is_origin);
    assert!(!sig.has_side_effects);
}

/// F.1 invariant test: Input ports must never be wireable (CLUSTER_SPEC.md §3.2)
#[test]
fn input_ports_are_never_wireable() {
    // Setup: Create a cluster with input ports
    let mut nodes = HashMap::new();
    nodes.insert(
        "c".to_string(),
        NodeInstance {
            id: "c".to_string(),
            kind: NodeKind::Impl {
                impl_id: "compute".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let cluster = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: vec![
            InputPortSpec {
                name: "input_a".to_string(),
                maps_to: GraphInputPlaceholder {
                    name: "input_a".to_string(),
                    ty: ValueType::Number,
                    required: true,
                },
            },
            InputPortSpec {
                name: "input_b".to_string(),
                maps_to: GraphInputPlaceholder {
                    name: "input_b".to_string(),
                    ty: ValueType::Series,
                    required: false,
                },
            },
        ],
        output_ports: vec![OutputPortSpec {
            name: "out".to_string(),
            maps_to: OutputRef {
                node_id: "c".to_string(),
                port_name: "value".to_string(),
            },
        }],
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();
    let expanded = expand(&cluster, &loader, &catalog).unwrap();

    let catalog = TestCatalog::default().with_metadata(
        "compute",
        "1.0.0",
        meta(PrimitiveKind::Compute, &[("value", ValueType::Number)]),
    );

    let sig = infer_signature(&expanded, &catalog).unwrap();

    // F.1: Input ports must never be wireable
    assert!(
        sig.inputs.iter().all(|p| !p.wireable),
        "Invariant F.1 violated: Input ports must never be wireable"
    );

    // Verify we actually tested multiple inputs
    assert_eq!(
        sig.inputs.len(),
        2,
        "Test should verify multiple input ports"
    );
}

/// E.3 invariant test: ExternalInput must not appear as edge sink after expansion
#[test]
fn external_input_cannot_be_edge_sink() {
    // Setup: Create a cluster with an edge targeting a non-existent node
    // This will cause ExternalInput to appear as edge sink, violating E.3
    let mut nodes = HashMap::new();
    nodes.insert(
        "source_node".to_string(),
        NodeInstance {
            id: "source_node".to_string(),
            kind: NodeKind::Impl {
                impl_id: "source".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    // Edge targets "nonexistent_node" which doesn't exist in nodes
    // This will resolve to ExternalInput as the sink, violating E.3
    let cluster = ClusterDefinition {
        id: "malformed".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: vec![Edge {
            from: OutputRef {
                node_id: "source_node".to_string(),
                port_name: "out".to_string(),
            },
            to: InputRef {
                node_id: "nonexistent_node".to_string(),
                port_name: "in".to_string(),
            },
        }],
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();
    // This should return an InvariantViolation error for E.3
    let result = expand(&cluster, &loader, &catalog);
    let err = result.unwrap_err();
    assert_eq!(
        err.rule_id(),
        "E.3",
        "expected E.3 invariant violation, got: {}",
        err.summary()
    );
}

/// D.11 invariant test: Declared wireability cannot exceed inferred wireability
#[test]
fn declared_wireability_cannot_exceed_inferred() {
    // Setup: Create cluster with Action output (inferred wireable: false)
    let mut nodes = HashMap::new();
    nodes.insert(
        "action_node".to_string(),
        NodeInstance {
            id: "action_node".to_string(),
            kind: NodeKind::Impl {
                impl_id: "action".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let cluster = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: vec![OutputPortSpec {
            name: "outcome".to_string(),
            maps_to: OutputRef {
                node_id: "action_node".to_string(),
                port_name: "outcome".to_string(),
            },
        }],
        parameters: empty_parameters(),
        declared_signature: Some(Signature {
            kind: BoundaryKind::ActionLike,
            inputs: Vec::new(),
            outputs: vec![PortSpec {
                name: "outcome".to_string(),
                ty: ValueType::Event,
                cardinality: Cardinality::Single,
                wireable: true, // D.11 violation: cannot grant wireability
            }],
            has_side_effects: true,
            is_origin: false,
        }),
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default().with_metadata(
        "action",
        "1.0.0",
        meta(PrimitiveKind::Action, &[("outcome", ValueType::Event)]),
    );

    let err = expand(&cluster, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "D.10");
    assert_eq!(err.path().as_deref(), Some("$.declared_signature"));
    assert!(
        matches!(
            err,
            ExpandError::DeclaredSignatureInvalid(
                ClusterValidationError::WireabilityExceedsInferred { ref port_name }
            ) if port_name == "outcome"
        ),
        "Declared signature must not exceed inferred wireability"
    );
}

#[test]
fn validate_declared_signature_rejects_wireability_grant() {
    let inferred = Signature {
        kind: BoundaryKind::ActionLike,
        inputs: Vec::new(),
        outputs: vec![PortSpec {
            name: "outcome".to_string(),
            ty: ValueType::Event,
            cardinality: Cardinality::Single,
            wireable: false,
        }],
        has_side_effects: true,
        is_origin: false,
    };

    let declared = Signature {
        kind: BoundaryKind::ActionLike,
        inputs: Vec::new(),
        outputs: vec![PortSpec {
            name: "outcome".to_string(),
            ty: ValueType::Event,
            cardinality: Cardinality::Single,
            wireable: true,
        }],
        has_side_effects: true,
        is_origin: false,
    };

    let result = validate_declared_signature(&declared, &inferred);

    assert!(matches!(
        result,
        Err(ClusterValidationError::WireabilityExceedsInferred { port_name })
            if port_name == "outcome"
    ));
}

#[test]
fn duplicate_input_ports_rejected() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "impl".to_string(),
        NodeInstance {
            id: "impl".to_string(),
            kind: NodeKind::Impl {
                impl_id: "compute".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let cluster = ClusterDefinition {
        id: "dup_inputs".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: vec![
            InputPortSpec {
                name: "in".to_string(),
                maps_to: GraphInputPlaceholder {
                    name: "in_a".to_string(),
                    ty: ValueType::Number,
                    required: true,
                },
            },
            InputPortSpec {
                name: "in".to_string(),
                maps_to: GraphInputPlaceholder {
                    name: "in_b".to_string(),
                    ty: ValueType::Number,
                    required: true,
                },
            },
        ],
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();
    let err = expand(&cluster, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "D.5");
    assert_eq!(err.path().as_deref(), Some("$.input_ports"));
    assert!(matches!(
        err,
        ExpandError::DuplicateInputPort { name } if name == "in"
    ));
}

#[test]
fn duplicate_output_ports_rejected() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "impl".to_string(),
        NodeInstance {
            id: "impl".to_string(),
            kind: NodeKind::Impl {
                impl_id: "compute".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let cluster = ClusterDefinition {
        id: "dup_outputs".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: vec![
            OutputPortSpec {
                name: "out".to_string(),
                maps_to: OutputRef {
                    node_id: "impl".to_string(),
                    port_name: "value".to_string(),
                },
            },
            OutputPortSpec {
                name: "out".to_string(),
                maps_to: OutputRef {
                    node_id: "impl".to_string(),
                    port_name: "value".to_string(),
                },
            },
        ],
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();
    let err = expand(&cluster, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "D.6");
    assert_eq!(err.path().as_deref(), Some("$.output_ports"));
    assert!(matches!(
        err,
        ExpandError::DuplicateOutputPort { name } if name == "out"
    ));
}

#[test]
fn duplicate_parameters_rejected() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "impl".to_string(),
        NodeInstance {
            id: "impl".to_string(),
            kind: NodeKind::Impl {
                impl_id: "compute".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let cluster = ClusterDefinition {
        id: "dup_params".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: vec![
            ParameterSpec {
                name: "p".to_string(),
                ty: ParameterType::Number,
                default: None,
                required: true,
            },
            ParameterSpec {
                name: "p".to_string(),
                ty: ParameterType::Number,
                default: None,
                required: true,
            },
        ],
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();
    let err = expand(&cluster, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "D.9");
    assert_eq!(err.path().as_deref(), Some("$.parameters"));
    assert!(matches!(
        err,
        ExpandError::DuplicateParameter { name } if name == "p"
    ));
}

#[test]
fn parameter_default_type_mismatch_rejected() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "impl".to_string(),
        NodeInstance {
            id: "impl".to_string(),
            kind: NodeKind::Impl {
                impl_id: "compute".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let cluster = ClusterDefinition {
        id: "bad_default".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: vec![ParameterSpec {
            name: "flag".to_string(),
            ty: ParameterType::Bool,
            default: Some(ParameterDefault::Literal(ParameterValue::Number(1.0))),
            required: false,
        }],
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();
    let err = expand(&cluster, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "D.8");
    assert_eq!(err.path().as_deref(), Some("$.parameters"));
    assert!(matches!(
        err,
        ExpandError::ParameterDefaultTypeMismatch {
            name,
            expected,
            got
        } if name == "flag" && expected == ParameterType::Bool && got == ParameterType::Number
    ));
}

/// I.3: Required parameter with no default and no binding must be rejected
#[test]
fn required_parameter_missing_rejected() {
    // Inner cluster has a required parameter with no default
    let mut inner_nodes = HashMap::new();
    inner_nodes.insert(
        "leaf".to_string(),
        NodeInstance {
            id: "leaf".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let inner = ClusterDefinition {
        id: "inner".to_string(),
        version: "1.0.0".to_string(),
        nodes: inner_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: vec![ParameterSpec {
            name: "required_param".to_string(),
            ty: ParameterType::Number,
            default: None, // No default
            required: true,
        }],
        declared_signature: None,
    };

    // Outer cluster instantiates inner without providing the required binding
    let mut outer_nodes = HashMap::new();
    outer_nodes.insert(
        "nested".to_string(),
        NodeInstance {
            id: "nested".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "inner".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(), // No binding provided
        },
    );

    let outer = ClusterDefinition {
        id: "outer".to_string(),
        version: "1.0.0".to_string(),
        nodes: outer_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: Vec::new(),
        declared_signature: None,
    };

    let loader = TestLoader::new().with_cluster(inner);
    let catalog = TestCatalog::default();
    let err = expand(&outer, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "I.3");
    assert_eq!(err.path().as_deref(), Some("$.nodes"));
    assert!(matches!(
        err,
        ExpandError::MissingRequiredParameter {
            cluster_id,
            parameter
        } if cluster_id == "inner" && parameter == "required_param"
    ));
}

/// I.4: Literal binding with wrong type must be rejected
#[test]
fn parameter_binding_type_mismatch_rejected() {
    // Inner cluster expects a Number parameter
    let mut inner_nodes = HashMap::new();
    inner_nodes.insert(
        "leaf".to_string(),
        NodeInstance {
            id: "leaf".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let inner = ClusterDefinition {
        id: "inner".to_string(),
        version: "1.0.0".to_string(),
        nodes: inner_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: vec![ParameterSpec {
            name: "num_param".to_string(),
            ty: ParameterType::Number,
            default: None,
            required: true,
        }],
        declared_signature: None,
    };

    // Outer cluster provides a Bool when Number is expected
    let mut outer_nodes = HashMap::new();
    outer_nodes.insert(
        "nested".to_string(),
        NodeInstance {
            id: "nested".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "inner".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "num_param".to_string(),
                ParameterBinding::Literal {
                    value: ParameterValue::Bool(true), // Wrong type!
                },
            )]),
        },
    );

    let outer = ClusterDefinition {
        id: "outer".to_string(),
        version: "1.0.0".to_string(),
        nodes: outer_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: Vec::new(),
        declared_signature: None,
    };

    let loader = TestLoader::new().with_cluster(inner);
    let catalog = TestCatalog::default();
    let err = expand(&outer, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "I.4");
    assert_eq!(err.path().as_deref(), Some("$.nodes"));
    assert!(matches!(
        err,
        ExpandError::ParameterBindingTypeMismatch {
            cluster_id,
            parameter,
            expected,
            got
        } if cluster_id == "inner"
            && parameter == "num_param"
            && expected == ParameterType::Number
            && got == ParameterType::Bool
    ));
}

/// I.5: Exposed binding referencing nonexistent parent parameter must be rejected
#[test]
fn exposed_parameter_not_in_parent_rejected() {
    // Inner cluster has a parameter that will be exposed
    let mut inner_nodes = HashMap::new();
    inner_nodes.insert(
        "leaf".to_string(),
        NodeInstance {
            id: "leaf".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let inner = ClusterDefinition {
        id: "inner".to_string(),
        version: "1.0.0".to_string(),
        nodes: inner_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: vec![ParameterSpec {
            name: "inner_param".to_string(),
            ty: ParameterType::Number,
            default: None,
            required: true,
        }],
        declared_signature: None,
    };

    // Outer cluster exposes to a parent parameter that doesn't exist
    let mut outer_nodes = HashMap::new();
    outer_nodes.insert(
        "nested".to_string(),
        NodeInstance {
            id: "nested".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "inner".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "inner_param".to_string(),
                ParameterBinding::Exposed {
                    parent_param: "nonexistent_param".to_string(), // Doesn't exist!
                },
            )]),
        },
    );

    let outer = ClusterDefinition {
        id: "outer".to_string(),
        version: "1.0.0".to_string(),
        nodes: outer_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: Vec::new(), // No parameters defined!
        declared_signature: None,
    };

    let loader = TestLoader::new().with_cluster(inner);
    let catalog = TestCatalog::default();
    let err = expand(&outer, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "I.5");
    assert_eq!(err.path().as_deref(), Some("$.nodes"));
    assert!(matches!(
        err,
        ExpandError::ExposedParameterNotFound {
            cluster_id,
            parameter,
            referenced
        } if cluster_id == "inner"
            && parameter == "inner_param"
            && referenced == "nonexistent_param"
    ));
}

/// I.4: Exposed binding with incompatible type must be rejected
#[test]
fn exposed_parameter_type_mismatch_rejected() {
    // Inner cluster expects a Number parameter
    let mut inner_nodes = HashMap::new();
    inner_nodes.insert(
        "leaf".to_string(),
        NodeInstance {
            id: "leaf".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let inner = ClusterDefinition {
        id: "inner".to_string(),
        version: "1.0.0".to_string(),
        nodes: inner_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: vec![ParameterSpec {
            name: "threshold".to_string(),
            ty: ParameterType::Number, // Expects Number
            default: None,
            required: true,
        }],
        declared_signature: None,
    };

    // Outer cluster has an Int parameter but inner expects Number
    let mut outer_nodes = HashMap::new();
    outer_nodes.insert(
        "nested".to_string(),
        NodeInstance {
            id: "nested".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "inner".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "threshold".to_string(),
                ParameterBinding::Exposed {
                    parent_param: "count".to_string(), // Exposes Int as Number
                },
            )]),
        },
    );

    let outer = ClusterDefinition {
        id: "outer".to_string(),
        version: "1.0.0".to_string(),
        nodes: outer_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: vec![ParameterSpec {
            name: "count".to_string(),
            ty: ParameterType::Int, // Int, not Number!
            default: None,
            required: true,
        }],
        declared_signature: None,
    };

    let loader = TestLoader::new().with_cluster(inner);
    let catalog = TestCatalog::default();
    let err = expand(&outer, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "I.4");
    assert_eq!(err.path().as_deref(), Some("$.nodes"));
    assert!(matches!(
        err,
        ExpandError::ExposedParameterTypeMismatch {
            cluster_id,
            parameter,
            expected,
            got
        } if cluster_id == "inner"
            && parameter == "threshold"
            && expected == ParameterType::Number
            && got == ParameterType::Int
    ));
}

/// PR-B: Exposed binding propagates through nested clusters to leaf primitive
#[test]
fn exposed_binding_propagates_to_leaf_primitive() {
    // Structure:
    //   Parent cluster (param "threshold": Number)
    //     └── Nested cluster "middle" (param "inner_t" exposed to "threshold")
    //           └── Leaf impl (param "leaf_t" exposed to "inner_t")
    //
    // Instantiate parent with threshold = Literal(7.0)
    // Assert: leaf impl receives parameters = { "leaf_t": Number(7.0) }

    // Innermost cluster with a leaf primitive that exposes a parameter
    let mut inner_nodes = HashMap::new();
    inner_nodes.insert(
        "leaf".to_string(),
        NodeInstance {
            id: "leaf".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "leaf_t".to_string(),
                ParameterBinding::Exposed {
                    parent_param: "inner_t".to_string(),
                },
            )]),
        },
    );

    let inner = ClusterDefinition {
        id: "inner".to_string(),
        version: "1.0.0".to_string(),
        nodes: inner_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: vec![ParameterSpec {
            name: "inner_t".to_string(),
            ty: ParameterType::Number,
            default: None,
            required: true,
        }],
        declared_signature: None,
    };

    // Middle cluster that instantiates inner with an exposed binding
    let mut middle_nodes = HashMap::new();
    middle_nodes.insert(
        "nested_inner".to_string(),
        NodeInstance {
            id: "nested_inner".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "inner".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "inner_t".to_string(),
                ParameterBinding::Exposed {
                    parent_param: "middle_t".to_string(),
                },
            )]),
        },
    );

    let middle = ClusterDefinition {
        id: "middle".to_string(),
        version: "1.0.0".to_string(),
        nodes: middle_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: vec![ParameterSpec {
            name: "middle_t".to_string(),
            ty: ParameterType::Number,
            default: None,
            required: true,
        }],
        declared_signature: None,
    };

    // Outer cluster that provides a literal binding
    let mut outer_nodes = HashMap::new();
    outer_nodes.insert(
        "nested_middle".to_string(),
        NodeInstance {
            id: "nested_middle".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "middle".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "middle_t".to_string(),
                ParameterBinding::Literal {
                    value: ParameterValue::Number(7.0),
                },
            )]),
        },
    );

    let outer = ClusterDefinition {
        id: "outer".to_string(),
        version: "1.0.0".to_string(),
        nodes: outer_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: Vec::new(),
        declared_signature: None,
    };

    let loader = TestLoader::new().with_cluster(inner).with_cluster(middle);
    let catalog = TestCatalog::default();
    let expanded = expand(&outer, &loader, &catalog).unwrap();

    // Verify the leaf primitive received the propagated value
    assert_eq!(expanded.nodes.len(), 1);
    let leaf_node = expanded.nodes.values().next().unwrap();
    assert_eq!(leaf_node.implementation.impl_id, "prim");
    assert_eq!(
        leaf_node.parameters.get("leaf_t"),
        Some(&ParameterValue::Number(7.0))
    );
}

/// PR-B: Unresolved Exposed binding on primitive node is rejected
#[test]
fn unresolved_exposed_binding_rejected() {
    // Structure:
    //   Parent cluster (NO params)
    //     └── Leaf impl (param "x" exposed to "nonexistent")
    //
    // Expand should fail with UnresolvedExposedBinding

    let mut nodes = HashMap::new();
    nodes.insert(
        "leaf".to_string(),
        NodeInstance {
            id: "leaf".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "x".to_string(),
                ParameterBinding::Exposed {
                    parent_param: "nonexistent".to_string(),
                },
            )]),
        },
    );

    let cluster = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: Vec::new(), // No parameters!
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();
    let err = expand(&cluster, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "I.3");
    assert_eq!(err.path().as_deref(), Some("$.nodes"));
    assert!(matches!(
        err,
        ExpandError::UnresolvedExposedBinding {
            node_id,
            parameter,
            referenced
        } if node_id == "leaf"
            && parameter == "x"
            && referenced == "nonexistent"
    ));
}

/// A.1: Default parameter value propagates to leaf when no binding provided
#[test]
fn defaulted_parameter_propagates_to_leaf() {
    // Structure:
    //   Root cluster
    //     └── Leaf impl "prim" with param "threshold" (default 42.0, no binding)
    //
    // Expected: The expanded leaf node gets parameters = { "threshold": 42.0 }

    let mut nodes = HashMap::new();
    nodes.insert(
        "leaf".to_string(),
        NodeInstance {
            id: "leaf".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(), // No binding provided
        },
    );

    let cluster = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();

    // Catalog with primitive that has a default parameter
    let catalog = TestCatalog::default().with_metadata(
        "prim",
        "1.0.0",
        meta_with_params(
            PrimitiveKind::Compute,
            &[("out", ValueType::Number)],
            vec![ParameterMetadata {
                name: "threshold".to_string(),
                ty: ParameterType::Number,
                default: Some(ParameterValue::Number(42.0)),
                required: false,
            }],
        ),
    );

    let expanded = expand(&cluster, &loader, &catalog).unwrap();

    assert_eq!(expanded.nodes.len(), 1);
    let leaf = expanded.nodes.values().next().unwrap();
    assert_eq!(
        leaf.parameters.get("threshold"),
        Some(&ParameterValue::Number(42.0)),
        "Default parameter value should be applied"
    );
}

/// A.1: Explicit binding overrides default parameter value
#[test]
fn explicit_binding_overrides_default() {
    // Structure:
    //   Root cluster
    //     └── Leaf impl "prim" with param "threshold" (default 42.0, literal binding 99.0)
    //
    // Expected: The expanded leaf node gets parameters = { "threshold": 99.0 }

    let mut nodes = HashMap::new();
    nodes.insert(
        "leaf".to_string(),
        NodeInstance {
            id: "leaf".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "threshold".to_string(),
                ParameterBinding::Literal {
                    value: ParameterValue::Number(99.0),
                },
            )]),
        },
    );

    let cluster = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();

    // Catalog with primitive that has a default parameter
    let catalog = TestCatalog::default().with_metadata(
        "prim",
        "1.0.0",
        meta_with_params(
            PrimitiveKind::Compute,
            &[("out", ValueType::Number)],
            vec![ParameterMetadata {
                name: "threshold".to_string(),
                ty: ParameterType::Number,
                default: Some(ParameterValue::Number(42.0)),
                required: false,
            }],
        ),
    );

    let expanded = expand(&cluster, &loader, &catalog).unwrap();

    assert_eq!(expanded.nodes.len(), 1);
    let leaf = expanded.nodes.values().next().unwrap();
    assert_eq!(
        leaf.parameters.get("threshold"),
        Some(&ParameterValue::Number(99.0)),
        "Explicit binding should override default"
    );
}

/// A.1: Missing required parameter with no default still rejected
#[test]
fn missing_required_param_no_default_rejected() {
    // Structure:
    //   Root cluster
    //     └── Leaf impl "prim" with required param "threshold" (no default, no binding)
    //
    // Expected: Expansion fails with MissingRequiredParameter

    let mut nodes = HashMap::new();
    nodes.insert(
        "leaf".to_string(),
        NodeInstance {
            id: "leaf".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(), // No binding provided
        },
    );

    let cluster = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();

    // Catalog with primitive that has a required parameter (no default)
    let catalog = TestCatalog::default().with_metadata(
        "prim",
        "1.0.0",
        meta_with_params(
            PrimitiveKind::Compute,
            &[("out", ValueType::Number)],
            vec![ParameterMetadata {
                name: "threshold".to_string(),
                ty: ParameterType::Number,
                default: None,
                required: true,
            }],
        ),
    );

    let err = expand(&cluster, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "I.3");
    assert_eq!(err.path().as_deref(), Some("$.nodes"));
    assert!(matches!(
        err,
        ExpandError::MissingRequiredParameter {
            cluster_id,
            parameter
        } if cluster_id == "leaf"
            && parameter == "threshold"
    ));
}

/// A.1: Cluster parameter default propagates to nested cluster expansion
#[test]
fn cluster_parameter_default_propagates_to_nested() {
    // Structure:
    //   Outer cluster
    //     └── Inner cluster (has param "threshold" with default 42.0, no binding from outer)
    //           └── Leaf impl "prim" (param "leaf_t" exposed to "threshold")
    //
    // Expected: Leaf impl gets parameters = { "leaf_t": 42.0 }

    // Inner cluster: has a parameter with default, and a leaf that exposes it
    let mut inner_nodes = HashMap::new();
    inner_nodes.insert(
        "leaf".to_string(),
        NodeInstance {
            id: "leaf".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "leaf_t".to_string(),
                ParameterBinding::Exposed {
                    parent_param: "threshold".to_string(),
                },
            )]),
        },
    );

    let inner = ClusterDefinition {
        id: "inner".to_string(),
        version: "1.0.0".to_string(),
        nodes: inner_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: vec![ParameterSpec {
            name: "threshold".to_string(),
            ty: ParameterType::Number,
            default: Some(ParameterDefault::Literal(ParameterValue::Number(42.0))), // Default value
            required: false,
        }],
        declared_signature: None,
    };

    // Outer cluster: instantiates inner without providing a binding for "threshold"
    let mut outer_nodes = HashMap::new();
    outer_nodes.insert(
        "nested".to_string(),
        NodeInstance {
            id: "nested".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "inner".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(), // No binding - should use default
        },
    );

    let outer = ClusterDefinition {
        id: "outer".to_string(),
        version: "1.0.0".to_string(),
        nodes: outer_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new().with_cluster(inner);
    let catalog = TestCatalog::default();

    let expanded = expand(&outer, &loader, &catalog).unwrap();

    assert_eq!(expanded.nodes.len(), 1);
    let leaf = expanded.nodes.values().next().unwrap();
    assert_eq!(
        leaf.parameters.get("leaf_t"),
        Some(&ParameterValue::Number(42.0)),
        "Cluster parameter default should propagate to nested leaf"
    );
}

/// C.1: Expansion must assign deterministic runtime_ids for identical cluster definitions
#[test]
fn expansion_runtime_ids_deterministic() {
    // Create a cluster with multiple nodes (names chosen to differ in HashMap iteration order)
    let mut nodes = HashMap::new();
    for name in ["zebra", "alpha", "mike", "charlie", "bravo"] {
        nodes.insert(
            name.to_string(),
            NodeInstance {
                id: name.to_string(),
                kind: NodeKind::Impl {
                    impl_id: "prim".to_string(),
                    version: "1.0.0".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );
    }

    let cluster = ClusterDefinition {
        id: "test".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();

    // Expand multiple times and verify runtime_ids are identical
    let expanded1 = expand(&cluster, &loader, &catalog).unwrap();
    let expanded2 = expand(&cluster, &loader, &catalog).unwrap();
    let expanded3 = expand(&cluster, &loader, &catalog).unwrap();

    // Collect (authoring_id, runtime_id) pairs sorted by authoring_id
    fn collect_id_pairs(graph: &ExpandedGraph) -> Vec<(String, String)> {
        let mut pairs: Vec<_> = graph
            .nodes
            .values()
            .map(|n| {
                let authoring_id = n.authoring_path.last().unwrap().1.clone();
                (authoring_id, n.runtime_id.clone())
            })
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        pairs
    }

    let pairs1 = collect_id_pairs(&expanded1);
    let pairs2 = collect_id_pairs(&expanded2);
    let pairs3 = collect_id_pairs(&expanded3);

    assert_eq!(
        pairs1, pairs2,
        "Runtime IDs must be deterministic across expansions"
    );
    assert_eq!(
        pairs2, pairs3,
        "Runtime IDs must be deterministic across expansions"
    );

    // Verify that nodes are assigned in alphabetical order by authoring_id
    // (alpha=n0, bravo=n1, charlie=n2, mike=n3, zebra=n4)
    let expected_order = ["alpha", "bravo", "charlie", "mike", "zebra"];
    for (i, name) in expected_order.iter().enumerate() {
        let expected_runtime_id = format!("n{}", i);
        let actual = pairs1.iter().find(|(auth, _)| auth == *name).unwrap();
        assert_eq!(
            actual.1, expected_runtime_id,
            "Node '{}' should have runtime_id '{}', got '{}'",
            name, expected_runtime_id, actual.1
        );
    }
}

/// D.4: Boundary output referencing unmapped node must fail expansion
#[test]
fn unmapped_boundary_output_rejected() {
    // Create cluster with output port referencing non-existent node
    let mut nodes = HashMap::new();
    nodes.insert(
        "real_node".to_string(),
        NodeInstance {
            id: "real_node".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let cluster = ClusterDefinition {
        id: "test".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: vec![OutputPortSpec {
            name: "out".to_string(),
            maps_to: OutputRef {
                node_id: "nonexistent_node".to_string(), // This node doesn't exist
                port_name: "value".to_string(),
            },
        }],
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();

    let err = expand(&cluster, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "D.4");
    assert_eq!(err.path().as_deref(), Some("$.output_ports"));
    assert!(matches!(
        err,
        ExpandError::UnmappedBoundaryOutput {
            port_name,
            node_id
        } if port_name == "out" && node_id == "nonexistent_node"
    ));
}

/// D.4: Nested cluster output port referencing unmapped node must fail expansion
#[test]
fn nested_output_mapping_failure_rejected() {
    // Inner cluster has output port referencing non-existent internal node
    let mut inner_nodes = HashMap::new();
    inner_nodes.insert(
        "leaf".to_string(),
        NodeInstance {
            id: "leaf".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let inner = ClusterDefinition {
        id: "inner".to_string(),
        version: "1.0.0".to_string(),
        nodes: inner_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: vec![OutputPortSpec {
            name: "out".to_string(),
            maps_to: OutputRef {
                node_id: "ghost_node".to_string(), // This node doesn't exist in inner
                port_name: "value".to_string(),
            },
        }],
        parameters: empty_parameters(),
        declared_signature: None,
    };

    // Outer cluster instantiates inner
    let mut outer_nodes = HashMap::new();
    outer_nodes.insert(
        "nested".to_string(),
        NodeInstance {
            id: "nested".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "inner".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let outer = ClusterDefinition {
        id: "outer".to_string(),
        version: "1.0.0".to_string(),
        nodes: outer_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new().with_cluster(inner);
    let catalog = TestCatalog::default();

    let err = expand(&outer, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "D.4");
    assert_eq!(err.path().as_deref(), Some("$.nodes"));
    assert!(matches!(
        err,
        ExpandError::UnmappedNestedOutput {
            cluster_id,
            port_name
        } if cluster_id == "nested" && port_name == "out"
    ));
}

/// I.7: Binding referencing undeclared primitive parameter must be rejected
#[test]
fn undeclared_primitive_parameter_binding_rejected() {
    // Primitive declares parameter "value", but the node binds "valeu" (typo)
    let mut nodes = HashMap::new();
    nodes.insert(
        "leaf".to_string(),
        NodeInstance {
            id: "leaf".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "valeu".to_string(), // Typo!
                ParameterBinding::Literal {
                    value: ParameterValue::Number(42.0),
                },
            )]),
        },
    );

    let cluster = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default().with_metadata(
        "prim",
        "1.0.0",
        meta_with_params(
            PrimitiveKind::Compute,
            &[("out", ValueType::Number)],
            vec![ParameterMetadata {
                name: "value".to_string(),
                ty: ParameterType::Number,
                default: Some(ParameterValue::Number(0.0)),
                required: false,
            }],
        ),
    );

    let err = expand(&cluster, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "I.7");
    assert_eq!(err.path().as_deref(), Some("$.nodes"));
    assert!(matches!(
        err,
        ExpandError::UndeclaredParameter {
            node_id,
            parameter
        } if node_id == "leaf" && parameter == "valeu"
    ));
}

/// I.7: Binding referencing undeclared nested cluster parameter must be rejected
#[test]
fn undeclared_cluster_parameter_binding_rejected() {
    // Inner cluster declares parameter "threshold", outer binds "threshhold" (typo)
    let mut inner_nodes = HashMap::new();
    inner_nodes.insert(
        "leaf".to_string(),
        NodeInstance {
            id: "leaf".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let inner = ClusterDefinition {
        id: "inner".to_string(),
        version: "1.0.0".to_string(),
        nodes: inner_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: vec![ParameterSpec {
            name: "threshold".to_string(),
            ty: ParameterType::Number,
            default: Some(ParameterDefault::Literal(ParameterValue::Number(0.0))),
            required: false,
        }],
        declared_signature: None,
    };

    let mut outer_nodes = HashMap::new();
    outer_nodes.insert(
        "nested".to_string(),
        NodeInstance {
            id: "nested".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "inner".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "threshhold".to_string(), // Typo!
                ParameterBinding::Literal {
                    value: ParameterValue::Number(5.0),
                },
            )]),
        },
    );

    let outer = ClusterDefinition {
        id: "outer".to_string(),
        version: "1.0.0".to_string(),
        nodes: outer_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: Vec::new(),
        declared_signature: None,
    };

    let loader = TestLoader::new().with_cluster(inner);
    let catalog = TestCatalog::default();
    let err = expand(&outer, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "I.7");
    assert_eq!(err.path().as_deref(), Some("$.nodes"));
    assert!(matches!(
        err,
        ExpandError::UndeclaredParameter {
            node_id,
            parameter
        } if node_id == "inner" && parameter == "threshhold"
    ));
}

#[test]
fn missing_nested_cluster_rejected() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "nested".to_string(),
        NodeInstance {
            id: "nested".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "missing".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let cluster = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();

    let err = expand(&cluster, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "E.9");
    assert_eq!(err.path().as_deref(), Some("$.nodes"));
    assert!(matches!(
        err,
        ExpandError::MissingCluster { id, version } if id == "missing" && version == "1.0.0"
    ));
}

#[test]
fn resolves_primitive_semver_constraint_to_highest_satisfying() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "p1".to_string(),
        NodeInstance {
            id: "p1".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "^1.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let cluster = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default()
        .with_metadata(
            "prim",
            "1.0.0",
            meta(PrimitiveKind::Compute, &[("out", ValueType::Number)]),
        )
        .with_metadata(
            "prim",
            "1.3.0",
            meta(PrimitiveKind::Compute, &[("out", ValueType::Number)]),
        )
        .with_metadata(
            "prim",
            "2.0.0",
            meta(PrimitiveKind::Compute, &[("out", ValueType::Number)]),
        );

    let expanded = expand(&cluster, &loader, &catalog).expect("constraint should resolve");
    let node = expanded.nodes.values().next().expect("expanded node");
    assert_eq!(node.implementation.requested_version, "^1.0");
    assert_eq!(node.implementation.version, "1.3.0");
}

#[test]
fn rejects_invalid_version_selector_with_i6_error() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "p1".to_string(),
        NodeInstance {
            id: "p1".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "not-a-semver".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let cluster = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();
    let err = expand(&cluster, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "I.6");
    assert!(matches!(
        err,
        ExpandError::InvalidVersionSelector {
            target_kind: VersionTargetKind::Primitive,
            id,
            selector,
        } if id == "prim" && selector == "not-a-semver"
    ));
}

#[test]
fn rejects_unsatisfied_cluster_constraint_with_i6_error() {
    let inner = ClusterDefinition {
        id: "inner".to_string(),
        version: "2.0.0".to_string(),
        nodes: HashMap::from([(
            "leaf".to_string(),
            NodeInstance {
                id: "leaf".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "leaf".to_string(),
                    version: "1.0.0".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        )]),
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let outer = ClusterDefinition {
        id: "outer".to_string(),
        version: "1.0.0".to_string(),
        nodes: HashMap::from([(
            "nested".to_string(),
            NodeInstance {
                id: "nested".to_string(),
                kind: NodeKind::Cluster {
                    cluster_id: "inner".to_string(),
                    version: "^1.0".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        )]),
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new().with_cluster(inner);
    let catalog = TestCatalog::default();
    let err = expand(&outer, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "I.6");
    assert!(matches!(
        err,
        ExpandError::UnsatisfiedVersionConstraint {
            target_kind: VersionTargetKind::Cluster,
            id,
            selector,
            ..
        } if id == "inner" && selector == "^1.0"
    ));
}

#[test]
fn rejects_non_semver_available_versions_with_i6_error() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "p1".to_string(),
        NodeInstance {
            id: "p1".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "^1.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let cluster = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default().with_metadata(
        "prim",
        "legacy",
        meta(PrimitiveKind::Compute, &[("out", ValueType::Number)]),
    );
    let err = expand(&cluster, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "I.6");
    assert!(matches!(
        err,
        ExpandError::InvalidAvailableVersion {
            target_kind: VersionTargetKind::Primitive,
            id,
            version,
        } if id == "prim" && version == "legacy"
    ));
}

// ---- derive_key() function tests ----

#[test]
fn derive_key_deterministic() {
    let path = vec![("cluster_a".to_string(), "node_1".to_string())];
    let a = derive_key(&path, "slot");
    let b = derive_key(&path, "slot");
    assert_eq!(a, b);
}

#[test]
fn derive_key_different_paths_produce_different_keys() {
    let path_a = vec![("cluster_a".to_string(), "node_1".to_string())];
    let path_b = vec![("cluster_b".to_string(), "node_1".to_string())];
    assert_ne!(derive_key(&path_a, "slot"), derive_key(&path_b, "slot"));
}

#[test]
fn derive_key_different_slot_names_produce_different_keys() {
    let path = vec![("cluster_a".to_string(), "node_1".to_string())];
    assert_ne!(derive_key(&path, "slot_x"), derive_key(&path, "slot_y"));
}

#[test]
fn derive_key_injective_encoding_handles_reserved_chars() {
    // Identifiers containing # and / must produce distinct keys from
    // identifiers that happen to match after naive delimiter joining.
    let path_a = vec![("a#b".to_string(), "c/d".to_string())];
    let path_b = vec![("a".to_string(), "b/c".to_string())];
    assert_ne!(derive_key(&path_a, "slot"), derive_key(&path_b, "slot"));

    // Length-prefix ensures "ab" + "cd" != "a" + "bcd"
    let path_c = vec![("ab".to_string(), "cd".to_string())];
    let path_d = vec![("a".to_string(), "bcd".to_string())];
    assert_ne!(derive_key(&path_c, "slot"), derive_key(&path_d, "slot"));
}

#[test]
fn derive_key_empty_path_produces_defined_output() {
    let key = derive_key(&[], "slot");
    assert!(key.starts_with("__ergo/"));
    assert!(key.contains("slot"));
    // Must be deterministic
    assert_eq!(key, derive_key(&[], "slot"));
}

// ---- Expansion-time derive_key resolution tests ----

#[test]
fn expand_derive_key_default_resolves_to_string() {
    // Inner cluster has a parameter with DeriveKey default.
    // The leaf exposes the cluster param so we can verify the resolved value.
    let mut inner_nodes = HashMap::new();
    inner_nodes.insert(
        "leaf".to_string(),
        NodeInstance {
            id: "leaf".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "key".to_string(),
                ParameterBinding::Exposed {
                    parent_param: "state_key".to_string(),
                },
            )]),
        },
    );

    let inner = ClusterDefinition {
        id: "inner".to_string(),
        version: "1.0.0".to_string(),
        nodes: inner_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: vec![ParameterSpec {
            name: "state_key".to_string(),
            ty: ParameterType::String,
            default: Some(ParameterDefault::DeriveKey {
                slot_name: "has_fired".to_string(),
            }),
            required: false,
        }],
        declared_signature: None,
    };

    // Root cluster instantiates inner
    let mut root_nodes = HashMap::new();
    root_nodes.insert(
        "inst".to_string(),
        NodeInstance {
            id: "inst".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "inner".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let root = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes: root_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new().with_cluster(inner);
    let catalog = TestCatalog::default();
    let expanded = expand(&root, &loader, &catalog).unwrap();

    assert_eq!(expanded.nodes.len(), 1);
    let node = expanded.nodes.values().next().unwrap();
    let resolved = node
        .parameters
        .get("key")
        .expect("leaf should have 'key' param from exposed derive_key default");
    let expected = derive_key(&[("root".to_string(), "inst".to_string())], "has_fired");
    assert_eq!(
        resolved,
        &ParameterValue::String(expected),
        "derive_key default must resolve to exact derived key"
    );
}

#[test]
fn expand_same_cluster_twice_produces_different_derived_keys() {
    let mut inner_nodes = HashMap::new();
    inner_nodes.insert(
        "leaf".to_string(),
        NodeInstance {
            id: "leaf".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            // Expose the derive_key param as a primitive parameter
            parameter_bindings: HashMap::from([(
                "key".to_string(),
                ParameterBinding::Exposed {
                    parent_param: "state_key".to_string(),
                },
            )]),
        },
    );

    let inner = ClusterDefinition {
        id: "inner".to_string(),
        version: "1.0.0".to_string(),
        nodes: inner_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: vec![ParameterSpec {
            name: "state_key".to_string(),
            ty: ParameterType::String,
            default: Some(ParameterDefault::DeriveKey {
                slot_name: "has_fired".to_string(),
            }),
            required: false,
        }],
        declared_signature: None,
    };

    // Root cluster instantiates inner twice
    let mut root_nodes = HashMap::new();
    root_nodes.insert(
        "inst_a".to_string(),
        NodeInstance {
            id: "inst_a".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "inner".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );
    root_nodes.insert(
        "inst_b".to_string(),
        NodeInstance {
            id: "inst_b".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "inner".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let root = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes: root_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new().with_cluster(inner);
    let catalog = TestCatalog::default();
    let expanded = expand(&root, &loader, &catalog).unwrap();

    assert_eq!(expanded.nodes.len(), 2);
    let keys: Vec<String> = expanded
        .nodes
        .values()
        .filter_map(|n| n.parameters.get("key"))
        .map(|v| match v {
            ParameterValue::String(s) => s.clone(),
            _ => panic!("expected String parameter"),
        })
        .collect();
    assert_eq!(
        keys.len(),
        2,
        "both instances should have derived key params"
    );
    assert_ne!(
        keys[0], keys[1],
        "different instances must derive different keys"
    );
}

#[test]
fn expand_explicit_binding_overrides_derive_key_default() {
    let mut inner_nodes = HashMap::new();
    inner_nodes.insert(
        "leaf".to_string(),
        NodeInstance {
            id: "leaf".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "key".to_string(),
                ParameterBinding::Exposed {
                    parent_param: "state_key".to_string(),
                },
            )]),
        },
    );

    let inner = ClusterDefinition {
        id: "inner".to_string(),
        version: "1.0.0".to_string(),
        nodes: inner_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: vec![ParameterSpec {
            name: "state_key".to_string(),
            ty: ParameterType::String,
            default: Some(ParameterDefault::DeriveKey {
                slot_name: "has_fired".to_string(),
            }),
            required: false,
        }],
        declared_signature: None,
    };

    // Root instantiates inner with an explicit binding that overrides DeriveKey
    let mut root_nodes = HashMap::new();
    root_nodes.insert(
        "inst".to_string(),
        NodeInstance {
            id: "inst".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "inner".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "state_key".to_string(),
                ParameterBinding::Literal {
                    value: ParameterValue::String("explicit_override".to_string()),
                },
            )]),
        },
    );

    let root = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes: root_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new().with_cluster(inner);
    let catalog = TestCatalog::default();
    let expanded = expand(&root, &loader, &catalog).unwrap();

    let leaf = expanded.nodes.values().next().unwrap();
    assert_eq!(
        leaf.parameters.get("key"),
        Some(&ParameterValue::String("explicit_override".to_string())),
        "explicit binding must override DeriveKey default"
    );
}

#[test]
fn expand_derive_key_same_slot_aliasing_allowed() {
    // Two parameters with same slot_name derive the same key (aliasing)
    let path = vec![("cluster".to_string(), "node".to_string())];
    let key_a = derive_key(&path, "shared_slot");
    let key_b = derive_key(&path, "shared_slot");
    assert_eq!(
        key_a, key_b,
        "same slot_name at same path must produce same key"
    );
}

fn once_cluster_definition() -> ClusterDefinition {
    let mut nodes = HashMap::new();
    nodes.insert(
        "state_source".to_string(),
        NodeInstance {
            id: "state_source".to_string(),
            kind: NodeKind::Impl {
                impl_id: "context_bool_source".to_string(),
                version: "0.1.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "key".to_string(),
                ParameterBinding::Exposed {
                    parent_param: "state_key".to_string(),
                },
            )]),
        },
    );
    nodes.insert(
        "not_state".to_string(),
        NodeInstance {
            id: "not_state".to_string(),
            kind: NodeKind::Impl {
                impl_id: "not".to_string(),
                version: "0.1.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );
    nodes.insert(
        "gate".to_string(),
        NodeInstance {
            id: "gate".to_string(),
            kind: NodeKind::Impl {
                impl_id: "emit_if_event_and_true".to_string(),
                version: "0.1.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );
    nodes.insert(
        "true_value".to_string(),
        NodeInstance {
            id: "true_value".to_string(),
            kind: NodeKind::Impl {
                impl_id: "boolean_source".to_string(),
                version: "0.1.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "value".to_string(),
                ParameterBinding::Literal {
                    value: ParameterValue::Bool(true),
                },
            )]),
        },
    );
    nodes.insert(
        "set_state".to_string(),
        NodeInstance {
            id: "set_state".to_string(),
            kind: NodeKind::Impl {
                impl_id: "context_set_bool".to_string(),
                version: "0.1.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "key".to_string(),
                ParameterBinding::Exposed {
                    parent_param: "state_key".to_string(),
                },
            )]),
        },
    );

    ClusterDefinition {
        id: "once_cluster".to_string(),
        version: "0.1.0".to_string(),
        nodes,
        edges: vec![
            Edge {
                from: OutputRef {
                    node_id: "incoming_event".to_string(),
                    port_name: "event".to_string(),
                },
                to: InputRef {
                    node_id: "gate".to_string(),
                    port_name: "event".to_string(),
                },
            },
            Edge {
                from: OutputRef {
                    node_id: "state_source".to_string(),
                    port_name: "value".to_string(),
                },
                to: InputRef {
                    node_id: "not_state".to_string(),
                    port_name: "value".to_string(),
                },
            },
            Edge {
                from: OutputRef {
                    node_id: "not_state".to_string(),
                    port_name: "result".to_string(),
                },
                to: InputRef {
                    node_id: "gate".to_string(),
                    port_name: "condition".to_string(),
                },
            },
            Edge {
                from: OutputRef {
                    node_id: "gate".to_string(),
                    port_name: "event".to_string(),
                },
                to: InputRef {
                    node_id: "set_state".to_string(),
                    port_name: "event".to_string(),
                },
            },
            Edge {
                from: OutputRef {
                    node_id: "true_value".to_string(),
                    port_name: "value".to_string(),
                },
                to: InputRef {
                    node_id: "set_state".to_string(),
                    port_name: "value".to_string(),
                },
            },
        ],
        input_ports: vec![InputPortSpec {
            name: "event".to_string(),
            maps_to: GraphInputPlaceholder {
                name: "incoming_event".to_string(),
                ty: ValueType::Event,
                required: true,
            },
        }],
        output_ports: vec![OutputPortSpec {
            name: "event".to_string(),
            maps_to: OutputRef {
                node_id: "gate".to_string(),
                port_name: "event".to_string(),
            },
        }],
        parameters: vec![ParameterSpec {
            name: "state_key".to_string(),
            ty: ParameterType::String,
            default: Some(ParameterDefault::DeriveKey {
                slot_name: "has_fired".to_string(),
            }),
            required: false,
        }],
        declared_signature: None,
    }
}

#[test]
fn once_cluster_expands_with_unique_derived_keys_per_instance() {
    let once = once_cluster_definition();

    let mut root_nodes = HashMap::new();
    root_nodes.insert(
        "once_a".to_string(),
        NodeInstance {
            id: "once_a".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "once_cluster".to_string(),
                version: "0.1.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );
    root_nodes.insert(
        "once_b".to_string(),
        NodeInstance {
            id: "once_b".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "once_cluster".to_string(),
                version: "0.1.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let root = ClusterDefinition {
        id: "root".to_string(),
        version: "0.1.0".to_string(),
        nodes: root_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: vec![OutputPortSpec {
            name: "out".to_string(),
            maps_to: OutputRef {
                node_id: "once_a".to_string(),
                port_name: "event".to_string(),
            },
        }],
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new().with_cluster(once);
    let catalog = TestCatalog::default();
    let expanded = expand(&root, &loader, &catalog).unwrap();

    assert_eq!(expanded.nodes.len(), 10);
    assert!(
        expanded
            .nodes
            .values()
            .all(|node| node.implementation.impl_id != "once_cluster"),
        "clusters must compile away to primitive nodes only"
    );

    let mut keys = expanded
        .nodes
        .values()
        .filter(|node| node.implementation.impl_id == "context_set_bool")
        .map(|node| {
            node.parameters
                .get("key")
                .expect("context_set_bool must have resolved key parameter")
        })
        .map(|value| match value {
            ParameterValue::String(s) => s.clone(),
            _ => panic!("expected String key parameter"),
        })
        .collect::<Vec<_>>();

    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), 2, "each instance must derive a unique key");
    assert!(
        keys.iter().all(|key| key.starts_with("__ergo/")),
        "derived keys must use __ergo namespace"
    );

    let mapped = expanded
        .boundary_outputs
        .first()
        .expect("root output should exist");
    let output_node = expanded
        .nodes
        .get(&mapped.maps_to.node_id)
        .expect("mapped output node should exist");
    assert_eq!(output_node.implementation.impl_id, "emit_if_event_and_true");
}

#[test]
fn once_cluster_explicit_state_key_binding_overrides_derive_key() {
    let once = once_cluster_definition();

    let mut root_nodes = HashMap::new();
    root_nodes.insert(
        "once".to_string(),
        NodeInstance {
            id: "once".to_string(),
            kind: NodeKind::Cluster {
                cluster_id: "once_cluster".to_string(),
                version: "0.1.0".to_string(),
            },
            parameter_bindings: HashMap::from([(
                "state_key".to_string(),
                ParameterBinding::Literal {
                    value: ParameterValue::String("explicit_once_key".to_string()),
                },
            )]),
        },
    );

    let root = ClusterDefinition {
        id: "root".to_string(),
        version: "0.1.0".to_string(),
        nodes: root_nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: empty_parameters(),
        declared_signature: None,
    };

    let loader = TestLoader::new().with_cluster(once);
    let catalog = TestCatalog::default();
    let expanded = expand(&root, &loader, &catalog).unwrap();

    let mut found_state_sources = 0usize;
    let mut found_set_actions = 0usize;
    for node in expanded.nodes.values() {
        if node.implementation.impl_id == "context_bool_source" {
            found_state_sources += 1;
            assert_eq!(
                node.parameters.get("key"),
                Some(&ParameterValue::String("explicit_once_key".to_string()))
            );
        }
        if node.implementation.impl_id == "context_set_bool" {
            found_set_actions += 1;
            assert_eq!(
                node.parameters.get("key"),
                Some(&ParameterValue::String("explicit_once_key".to_string()))
            );
        }
    }

    assert_eq!(found_state_sources, 1);
    assert_eq!(found_set_actions, 1);
}

// ---- Validation tests (defense in depth) ----

#[test]
fn cluster_derive_key_on_non_string_param_rejected() {
    let cluster = ClusterDefinition {
        id: "bad".to_string(),
        version: "1.0.0".to_string(),
        nodes: HashMap::new(),
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: vec![ParameterSpec {
            name: "count".to_string(),
            ty: ParameterType::Number,
            default: Some(ParameterDefault::DeriveKey {
                slot_name: "slot".to_string(),
            }),
            required: false,
        }],
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();
    let err = expand(&cluster, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "D.8");
    assert!(matches!(
        err,
        ExpandError::ParameterDefaultTypeMismatch {
            expected: ParameterType::Number,
            got: ParameterType::String,
            ..
        }
    ));
}

#[test]
fn cluster_derive_key_on_string_param_accepted() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "p1".to_string(),
        NodeInstance {
            id: "p1".to_string(),
            kind: NodeKind::Impl {
                impl_id: "prim".to_string(),
                version: "1.0.0".to_string(),
            },
            parameter_bindings: HashMap::new(),
        },
    );

    let cluster = ClusterDefinition {
        id: "root".to_string(),
        version: "1.0.0".to_string(),
        nodes,
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: vec![ParameterSpec {
            name: "key".to_string(),
            ty: ParameterType::String,
            default: Some(ParameterDefault::DeriveKey {
                slot_name: "slot".to_string(),
            }),
            required: false,
        }],
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();
    assert!(expand(&cluster, &loader, &catalog).is_ok());
}

#[test]
fn cluster_derive_key_empty_slot_name_rejected() {
    let cluster = ClusterDefinition {
        id: "bad".to_string(),
        version: "1.0.0".to_string(),
        nodes: HashMap::new(),
        edges: Vec::new(),
        input_ports: Vec::new(),
        output_ports: Vec::new(),
        parameters: vec![ParameterSpec {
            name: "key".to_string(),
            ty: ParameterType::String,
            default: Some(ParameterDefault::DeriveKey {
                slot_name: String::new(),
            }),
            required: false,
        }],
        declared_signature: None,
    };

    let loader = TestLoader::new();
    let catalog = TestCatalog::default();
    let err = expand(&cluster, &loader, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "D.8");
    assert!(matches!(err, ExpandError::InvalidDeriveKeySlot { .. }));
}
