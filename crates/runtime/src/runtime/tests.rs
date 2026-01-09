use std::collections::HashMap;

use crate::action;
use crate::catalog::{build_core_catalog, core_registries};
use crate::cluster::{
    ExpandedEndpoint, ExpandedGraph, ExpandedNode, InputMetadata, OutputMetadata, PrimitiveCatalog,
    PrimitiveKind, PrimitiveMetadata, ValueType,
};
use crate::compute::implementations::{Add, ConstNumber};
use crate::compute::PrimitiveRegistry as ComputeRegistry;
use crate::runtime::run;
use crate::runtime::types::{
    ExecError, ExecutionContext, Registries, RuntimeValue, ValidationError,
};
use crate::source::{SourceKind, SourcePrimitive, SourcePrimitiveManifest, SourceRegistry};
use crate::trigger::TriggerRegistry;

#[derive(Default)]
struct TestCatalog {
    metadata: HashMap<(String, String), PrimitiveMetadata>,
}

impl PrimitiveCatalog for TestCatalog {
    fn get(&self, id: &str, version: &String) -> Option<PrimitiveMetadata> {
        self.metadata
            .get(&(id.to_string(), version.clone()))
            .cloned()
    }
}

fn add_metadata() -> PrimitiveMetadata {
    let mut outputs = HashMap::new();
    outputs.insert(
        "result".to_string(),
        OutputMetadata {
            value_type: ValueType::Number,
            cardinality: crate::cluster::Cardinality::Single,
        },
    );

    PrimitiveMetadata {
        kind: PrimitiveKind::Compute,
        inputs: vec![
            InputMetadata {
                name: "a".to_string(),
                value_type: ValueType::Number,
                required: true,
            },
            InputMetadata {
                name: "b".to_string(),
                value_type: ValueType::Number,
                required: true,
            },
        ],
        outputs,
        parameters: Vec::new(),
    }
}

fn source_metadata() -> PrimitiveMetadata {
    let mut outputs = HashMap::new();
    outputs.insert(
        "out".to_string(),
        OutputMetadata {
            value_type: ValueType::Number,
            cardinality: crate::cluster::Cardinality::Single,
        },
    );

    PrimitiveMetadata {
        kind: PrimitiveKind::Source,
        inputs: Vec::new(),
        outputs,
        parameters: Vec::new(),
    }
}

#[derive(Clone)]
struct ConstSource {
    manifest: SourcePrimitiveManifest,
    value: f64,
}

impl ConstSource {
    fn new(id: &str, value: f64) -> Self {
        Self {
            manifest: SourcePrimitiveManifest {
                id: id.to_string(),
                version: "v1".to_string(),
                kind: SourceKind::Source,
                inputs: vec![],
                outputs: vec![crate::source::OutputSpec {
                    name: "out".to_string(),
                    value_type: crate::common::ValueType::Number,
                }],
                parameters: vec![],
                execution: crate::source::ExecutionSpec {
                    deterministic: true,
                    cadence: crate::source::Cadence::Continuous,
                },
                state: crate::source::StateSpec { allowed: false },
                side_effects: false,
            },
            value,
        }
    }
}

impl SourcePrimitive for ConstSource {
    fn manifest(&self) -> &SourcePrimitiveManifest {
        &self.manifest
    }

    fn produce(
        &self,
        _parameters: &HashMap<String, crate::source::ParameterValue>,
    ) -> HashMap<String, crate::common::Value> {
        HashMap::from([("out".to_string(), crate::common::Value::Number(self.value))])
    }
}

#[test]
fn unified_runtime_executes_compute_graph() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "src1".to_string(),
        ExpandedNode {
            runtime_id: "src1".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "const1".to_string(),
                version: "v1".to_string(),
            },
            parameters: HashMap::new(),
        },
    );
    nodes.insert(
        "src2".to_string(),
        ExpandedNode {
            runtime_id: "src2".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "const2".to_string(),
                version: "v1".to_string(),
            },
            parameters: HashMap::new(),
        },
    );
    nodes.insert(
        "add1".to_string(),
        ExpandedNode {
            runtime_id: "add1".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "add".to_string(),
                version: "v1".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    let edges = vec![
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src1".to_string(),
                port_name: "out".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "add1".to_string(),
                port_name: "a".to_string(),
            },
        },
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src2".to_string(),
                port_name: "out".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "add1".to_string(),
                port_name: "b".to_string(),
            },
        },
    ];

    let expanded = ExpandedGraph {
        nodes,
        edges,
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "sum".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "add1".to_string(),
                port_name: "result".to_string(),
            },
        }],
    };

    let mut catalog = TestCatalog::default();
    catalog
        .metadata
        .insert(("add".to_string(), "v1".to_string()), add_metadata());
    catalog
        .metadata
        .insert(("const1".to_string(), "v1".to_string()), source_metadata());
    catalog
        .metadata
        .insert(("const2".to_string(), "v1".to_string()), source_metadata());

    let mut compute_registry = ComputeRegistry::new();
    compute_registry.register(Box::new(Add::new())).unwrap();

    let mut source_registry = SourceRegistry::new();
    source_registry
        .register(Box::new(ConstSource::new("const1", 3.0)))
        .unwrap();
    source_registry
        .register(Box::new(ConstSource::new("const2", 4.0)))
        .unwrap();

    let registries = Registries {
        sources: &source_registry,
        computes: &compute_registry,
        triggers: &TriggerRegistry::new(),
        actions: &crate::action::ActionRegistry::new(),
    };

    let ctx = ExecutionContext;

    let report = run(&expanded, &catalog, &registries, &ctx).unwrap();
    assert_eq!(report.outputs.get("sum"), Some(&RuntimeValue::Number(7.0)));
}

#[test]
fn parameters_flow_into_compute_execution() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "const_number".to_string(),
        ExpandedNode {
            runtime_id: "const_number".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "const_number".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([(
                "value".to_string(),
                crate::cluster::ParameterValue::Number(4.5),
            )]),
        },
    );

    let expanded = ExpandedGraph {
        nodes,
        edges: Vec::new(),
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "out".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "const_number".to_string(),
                port_name: "value".to_string(),
            },
        }],
    };

    let mut catalog = TestCatalog::default();
    catalog.metadata.insert(
        ("const_number".to_string(), "0.1.0".to_string()),
        PrimitiveMetadata {
            kind: PrimitiveKind::Compute,
            inputs: vec![],
            outputs: HashMap::from([(
                "value".to_string(),
                OutputMetadata {
                    value_type: ValueType::Number,
                    cardinality: crate::cluster::Cardinality::Single,
                },
            )]),
            parameters: Vec::new(),
        },
    );

    let mut compute_registry = ComputeRegistry::new();
    compute_registry
        .register(Box::new(ConstNumber::new()))
        .unwrap();

    let registries = Registries {
        sources: &SourceRegistry::new(),
        computes: &compute_registry,
        triggers: &TriggerRegistry::new(),
        actions: &action::ActionRegistry::new(),
    };

    let ctx = ExecutionContext;

    let report = run(&expanded, &catalog, &registries, &ctx).unwrap();
    assert_eq!(report.outputs.get("out"), Some(&RuntimeValue::Number(4.5)));
}

#[test]
fn hello_world_graph_executes_with_core_catalog_and_registries() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "src_a".to_string(),
        ExpandedNode {
            runtime_id: "src_a".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "number_source".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([(
                "value".to_string(),
                crate::cluster::ParameterValue::Number(3.0),
            )]),
        },
    );
    nodes.insert(
        "src_b".to_string(),
        ExpandedNode {
            runtime_id: "src_b".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "number_source".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([(
                "value".to_string(),
                crate::cluster::ParameterValue::Number(1.0),
            )]),
        },
    );
    nodes.insert(
        "gt1".to_string(),
        ExpandedNode {
            runtime_id: "gt1".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "gt".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );
    nodes.insert(
        "emit".to_string(),
        ExpandedNode {
            runtime_id: "emit".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "emit_if_true".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );
    nodes.insert(
        "act".to_string(),
        ExpandedNode {
            runtime_id: "act".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "ack_action".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([(
                "accept".to_string(),
                crate::cluster::ParameterValue::Bool(true),
            )]),
        },
    );

    let edges = vec![
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src_a".to_string(),
                port_name: "value".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "gt1".to_string(),
                port_name: "a".to_string(),
            },
        },
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src_b".to_string(),
                port_name: "value".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "gt1".to_string(),
                port_name: "b".to_string(),
            },
        },
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "gt1".to_string(),
                port_name: "result".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "emit".to_string(),
                port_name: "input".to_string(),
            },
        },
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "emit".to_string(),
                port_name: "event".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "act".to_string(),
                port_name: "event".to_string(),
            },
        },
    ];

    let expanded = ExpandedGraph {
        nodes,
        edges,
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "action_outcome".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "act".to_string(),
                port_name: "outcome".to_string(),
            },
        }],
    };

    let catalog = build_core_catalog();
    let registries = core_registries().unwrap();
    let registries = Registries {
        sources: &registries.sources,
        computes: &registries.computes,
        triggers: &registries.triggers,
        actions: &registries.actions,
    };

    let ctx = ExecutionContext;

    let report = run(&expanded, &catalog, &registries, &ctx).unwrap();
    assert_eq!(
        report.outputs.get("action_outcome"),
        Some(&RuntimeValue::Event(
            crate::runtime::types::RuntimeEvent::Action(crate::action::ActionOutcome::Completed)
        ))
    );
}

#[test]
fn validation_fails_on_missing_required_input() {
    // Same graph as hello_world but with edge src_a -> gt1 removed
    // This should cause validation to fail: gt1 is missing required input "a"
    let mut nodes = HashMap::new();
    nodes.insert(
        "src_a".to_string(),
        ExpandedNode {
            runtime_id: "src_a".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "number_source".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([(
                "value".to_string(),
                crate::cluster::ParameterValue::Number(3.0),
            )]),
        },
    );
    nodes.insert(
        "src_b".to_string(),
        ExpandedNode {
            runtime_id: "src_b".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "number_source".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([(
                "value".to_string(),
                crate::cluster::ParameterValue::Number(1.0),
            )]),
        },
    );
    nodes.insert(
        "gt1".to_string(),
        ExpandedNode {
            runtime_id: "gt1".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "gt".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );
    nodes.insert(
        "emit".to_string(),
        ExpandedNode {
            runtime_id: "emit".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "emit_if_true".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );
    nodes.insert(
        "act".to_string(),
        ExpandedNode {
            runtime_id: "act".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "ack_action".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([(
                "accept".to_string(),
                crate::cluster::ParameterValue::Bool(true),
            )]),
        },
    );

    // Missing edge: src_a -> gt1:a (first edge removed)
    let edges = vec![
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src_b".to_string(),
                port_name: "value".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "gt1".to_string(),
                port_name: "b".to_string(),
            },
        },
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "gt1".to_string(),
                port_name: "result".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "emit".to_string(),
                port_name: "input".to_string(),
            },
        },
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "emit".to_string(),
                port_name: "event".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "act".to_string(),
                port_name: "event".to_string(),
            },
        },
    ];

    let expanded = ExpandedGraph {
        nodes,
        edges,
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "action_outcome".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "act".to_string(),
                port_name: "outcome".to_string(),
            },
        }],
    };

    let catalog = build_core_catalog();

    // Validation should fail with MissingRequiredInput
    let result = crate::runtime::validate::validate(&expanded, &catalog);
    assert!(result.is_err(), "Expected validation to fail");
    match result.unwrap_err() {
        crate::runtime::types::ValidationError::MissingRequiredInput { node, input } => {
            assert_eq!(node, "gt1");
            assert_eq!(input, "a");
        }
        other => panic!("Expected MissingRequiredInput, got {:?}", other),
    }
}

/// R.7: Actions execute only when trigger event emitted.
/// When trigger emits NotEmitted, action should return Skipped (not execute).
#[test]
fn r7_action_skipped_when_trigger_not_emitted() {
    // Same structure as hello_world_graph, but with values that cause trigger to NOT emit.
    // src_a=1.0, src_b=3.0 means gt(1.0, 3.0) = false, so emit_if_true emits NotEmitted.
    let mut nodes = HashMap::new();
    nodes.insert(
        "src_a".to_string(),
        ExpandedNode {
            runtime_id: "src_a".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "number_source".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([(
                "value".to_string(),
                crate::cluster::ParameterValue::Number(1.0), // a < b
            )]),
        },
    );
    nodes.insert(
        "src_b".to_string(),
        ExpandedNode {
            runtime_id: "src_b".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "number_source".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([(
                "value".to_string(),
                crate::cluster::ParameterValue::Number(3.0), // a < b, so gt returns false
            )]),
        },
    );
    nodes.insert(
        "gt1".to_string(),
        ExpandedNode {
            runtime_id: "gt1".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "gt".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );
    nodes.insert(
        "emit".to_string(),
        ExpandedNode {
            runtime_id: "emit".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "emit_if_true".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );
    nodes.insert(
        "act".to_string(),
        ExpandedNode {
            runtime_id: "act".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "ack_action".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([(
                "accept".to_string(),
                crate::cluster::ParameterValue::Bool(true),
            )]),
        },
    );

    let edges = vec![
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src_a".to_string(),
                port_name: "value".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "gt1".to_string(),
                port_name: "a".to_string(),
            },
        },
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src_b".to_string(),
                port_name: "value".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "gt1".to_string(),
                port_name: "b".to_string(),
            },
        },
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "gt1".to_string(),
                port_name: "result".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "emit".to_string(),
                port_name: "input".to_string(),
            },
        },
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "emit".to_string(),
                port_name: "event".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "act".to_string(),
                port_name: "event".to_string(),
            },
        },
    ];

    let expanded = ExpandedGraph {
        nodes,
        edges,
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "action_outcome".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "act".to_string(),
                port_name: "outcome".to_string(),
            },
        }],
    };

    let catalog = build_core_catalog();
    let registries = core_registries().unwrap();
    let registries = Registries {
        sources: &registries.sources,
        computes: &registries.computes,
        triggers: &registries.triggers,
        actions: &registries.actions,
    };

    let ctx = ExecutionContext;

    let report = run(&expanded, &catalog, &registries, &ctx).unwrap();

    // R.7 enforcement: Action should be Skipped, not Attempted/Completed
    assert_eq!(
        report.outputs.get("action_outcome"),
        Some(&RuntimeValue::Event(
            crate::runtime::types::RuntimeEvent::Action(crate::action::ActionOutcome::Skipped)
        )),
        "R.7: Action must return Skipped when gating trigger emits NotEmitted"
    );
}

#[test]
fn validate_returns_error_when_edge_references_unknown_node() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "src1".to_string(),
        ExpandedNode {
            runtime_id: "src1".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "const1".to_string(),
                version: "v1".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    let edges = vec![crate::cluster::ExpandedEdge {
        from: ExpandedEndpoint::NodePort {
            node_id: "src1".to_string(),
            port_name: "out".to_string(),
        },
        to: ExpandedEndpoint::NodePort {
            node_id: "missing".to_string(),
            port_name: "in".to_string(),
        },
    }];

    let expanded = ExpandedGraph {
        nodes,
        edges,
        boundary_inputs: Vec::new(),
        boundary_outputs: Vec::new(),
    };

    let mut catalog = TestCatalog::default();
    catalog
        .metadata
        .insert(("const1".to_string(), "v1".to_string()), source_metadata());

    let err = crate::runtime::validate(&expanded, &catalog).unwrap_err();
    match err {
        ValidationError::UnknownNode(node) => assert_eq!(node, "missing"),
        other => panic!("expected UnknownNode, got {:?}", other),
    }
}

#[test]
fn validate_rejects_invalid_boundary_output_port() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "src1".to_string(),
        ExpandedNode {
            runtime_id: "src1".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "const1".to_string(),
                version: "v1".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    let expanded = ExpandedGraph {
        nodes,
        edges: Vec::new(),
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "missing".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "src1".to_string(),
                port_name: "does_not_exist".to_string(),
            },
        }],
    };

    let mut catalog = TestCatalog::default();
    catalog
        .metadata
        .insert(("const1".to_string(), "v1".to_string()), source_metadata());

    let err = crate::runtime::validate(&expanded, &catalog).unwrap_err();
    match err {
        ValidationError::MissingOutputMetadata { node, output } => {
            assert_eq!(node, "src1");
            assert_eq!(output, "does_not_exist");
        }
        other => panic!("expected MissingOutputMetadata, got {:?}", other),
    }
}

#[test]
fn execute_returns_error_when_topology_references_missing_node() {
    let graph = crate::runtime::types::ValidatedGraph {
        nodes: HashMap::new(),
        edges: Vec::new(),
        topo_order: vec!["ghost".to_string()],
        boundary_outputs: Vec::new(),
    };

    let sources = SourceRegistry::new();
    let computes = ComputeRegistry::new();
    let triggers = TriggerRegistry::new();
    let actions = crate::action::ActionRegistry::new();

    let registries = Registries {
        sources: &sources,
        computes: &computes,
        triggers: &triggers,
        actions: &actions,
    };

    let ctx = ExecutionContext;

    let err = crate::runtime::execute(&graph, &registries, &ctx).unwrap_err();
    match err {
        ExecError::MissingNode { node } => assert_eq!(node, "ghost"),
        other => panic!("expected ExecError::MissingNode, got {:?}", other),
    }
}

/// X.11: Int parameter at f64 exact range boundary (2^53) should succeed.
#[test]
fn int_parameter_within_f64_exact_range_allowed() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "const_number".to_string(),
        ExpandedNode {
            runtime_id: "const_number".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "const_number".to_string(),
                version: "0.1.0".to_string(),
            },
            // X.11: Use Int at exact boundary (2^53)
            parameters: HashMap::from([(
                "value".to_string(),
                crate::cluster::ParameterValue::Int(9_007_199_254_740_992), // 2^53
            )]),
        },
    );

    let expanded = ExpandedGraph {
        nodes,
        edges: Vec::new(),
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "out".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "const_number".to_string(),
                port_name: "value".to_string(),
            },
        }],
    };

    let mut catalog = TestCatalog::default();
    catalog.metadata.insert(
        ("const_number".to_string(), "0.1.0".to_string()),
        PrimitiveMetadata {
            kind: PrimitiveKind::Compute,
            inputs: vec![],
            outputs: HashMap::from([(
                "value".to_string(),
                OutputMetadata {
                    value_type: ValueType::Number,
                    cardinality: crate::cluster::Cardinality::Single,
                },
            )]),
            parameters: Vec::new(),
        },
    );

    let mut compute_registry = ComputeRegistry::new();
    compute_registry
        .register(Box::new(ConstNumber::new()))
        .unwrap();

    let registries = Registries {
        sources: &SourceRegistry::new(),
        computes: &compute_registry,
        triggers: &TriggerRegistry::new(),
        actions: &action::ActionRegistry::new(),
    };

    let ctx = ExecutionContext;

    let report = run(&expanded, &catalog, &registries, &ctx).unwrap();
    // 2^53 converts exactly to f64
    assert_eq!(
        report.outputs.get("out"),
        Some(&RuntimeValue::Number(9_007_199_254_740_992.0))
    );
}

/// X.11: Int parameter exceeding f64 exact range (2^53 + 1) should fail.
#[test]
fn int_parameter_out_of_range_rejected() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "const_number".to_string(),
        ExpandedNode {
            runtime_id: "const_number".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "const_number".to_string(),
                version: "0.1.0".to_string(),
            },
            // X.11: Use Int beyond exact range (2^53 + 1)
            parameters: HashMap::from([(
                "value".to_string(),
                crate::cluster::ParameterValue::Int(9_007_199_254_740_993), // 2^53 + 1
            )]),
        },
    );

    let expanded = ExpandedGraph {
        nodes,
        edges: Vec::new(),
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "out".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "const_number".to_string(),
                port_name: "value".to_string(),
            },
        }],
    };

    let mut catalog = TestCatalog::default();
    catalog.metadata.insert(
        ("const_number".to_string(), "0.1.0".to_string()),
        PrimitiveMetadata {
            kind: PrimitiveKind::Compute,
            inputs: vec![],
            outputs: HashMap::from([(
                "value".to_string(),
                OutputMetadata {
                    value_type: ValueType::Number,
                    cardinality: crate::cluster::Cardinality::Single,
                },
            )]),
            parameters: Vec::new(),
        },
    );

    let mut compute_registry = ComputeRegistry::new();
    compute_registry
        .register(Box::new(ConstNumber::new()))
        .unwrap();

    let registries = Registries {
        sources: &SourceRegistry::new(),
        computes: &compute_registry,
        triggers: &TriggerRegistry::new(),
        actions: &action::ActionRegistry::new(),
    };

    let ctx = ExecutionContext;

    let result = run(&expanded, &catalog, &registries, &ctx);
    assert!(
        result.is_err(),
        "Expected execution to fail for out-of-range Int"
    );

    match result.unwrap_err() {
        crate::runtime::RuntimeError::Execution(ExecError::ParameterOutOfRange {
            node,
            parameter,
            value,
        }) => {
            assert_eq!(node, "const_number");
            assert_eq!(parameter, "value");
            assert_eq!(value, 9_007_199_254_740_993);
        }
        other => panic!("Expected ParameterOutOfRange, got {:?}", other),
    }
}

/// X.11: i64::MIN must be rejected without panic (no .abs() overflow).
#[test]
fn int_parameter_i64_min_rejected() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "const_number".to_string(),
        ExpandedNode {
            runtime_id: "const_number".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "const_number".to_string(),
                version: "0.1.0".to_string(),
            },
            // X.11: i64::MIN would panic with .abs(), must reject gracefully
            parameters: HashMap::from([(
                "value".to_string(),
                crate::cluster::ParameterValue::Int(i64::MIN),
            )]),
        },
    );

    let expanded = ExpandedGraph {
        nodes,
        edges: Vec::new(),
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "out".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "const_number".to_string(),
                port_name: "value".to_string(),
            },
        }],
    };

    let mut catalog = TestCatalog::default();
    catalog.metadata.insert(
        ("const_number".to_string(), "0.1.0".to_string()),
        PrimitiveMetadata {
            kind: PrimitiveKind::Compute,
            inputs: vec![],
            outputs: HashMap::from([(
                "value".to_string(),
                OutputMetadata {
                    value_type: ValueType::Number,
                    cardinality: crate::cluster::Cardinality::Single,
                },
            )]),
            parameters: Vec::new(),
        },
    );

    let mut compute_registry = ComputeRegistry::new();
    compute_registry
        .register(Box::new(ConstNumber::new()))
        .unwrap();

    let registries = Registries {
        sources: &SourceRegistry::new(),
        computes: &compute_registry,
        triggers: &TriggerRegistry::new(),
        actions: &action::ActionRegistry::new(),
    };

    let ctx = ExecutionContext;

    // Must not panic, must return error
    let result = run(&expanded, &catalog, &registries, &ctx);
    assert!(result.is_err(), "Expected execution to fail for i64::MIN");

    match result.unwrap_err() {
        crate::runtime::RuntimeError::Execution(ExecError::ParameterOutOfRange {
            node,
            parameter,
            value,
        }) => {
            assert_eq!(node, "const_number");
            assert_eq!(parameter, "value");
            assert_eq!(value, i64::MIN);
        }
        other => panic!("Expected ParameterOutOfRange, got {:?}", other),
    }
}

/// V.MULTI-EDGE: Multiple edges targeting the same input port are rejected.
#[test]
fn validate_rejects_multiple_edges_to_same_input() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "src1".to_string(),
        ExpandedNode {
            runtime_id: "src1".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "const1".to_string(),
                version: "v1".to_string(),
            },
            parameters: HashMap::new(),
        },
    );
    nodes.insert(
        "src2".to_string(),
        ExpandedNode {
            runtime_id: "src2".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "const2".to_string(),
                version: "v1".to_string(),
            },
            parameters: HashMap::new(),
        },
    );
    nodes.insert(
        "add1".to_string(),
        ExpandedNode {
            runtime_id: "add1".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "add".to_string(),
                version: "v1".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    // Both sources wired to the same input port "a" on add1
    let edges = vec![
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src1".to_string(),
                port_name: "out".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "add1".to_string(),
                port_name: "a".to_string(),
            },
        },
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src2".to_string(),
                port_name: "out".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "add1".to_string(),
                port_name: "a".to_string(), // Same port as first edge!
            },
        },
    ];

    let expanded = ExpandedGraph {
        nodes,
        edges,
        boundary_inputs: Vec::new(),
        boundary_outputs: Vec::new(),
    };

    let mut catalog = TestCatalog::default();
    catalog
        .metadata
        .insert(("add".to_string(), "v1".to_string()), add_metadata());
    catalog
        .metadata
        .insert(("const1".to_string(), "v1".to_string()), source_metadata());
    catalog
        .metadata
        .insert(("const2".to_string(), "v1".to_string()), source_metadata());

    let err = crate::runtime::validate(&expanded, &catalog).unwrap_err();
    match err {
        ValidationError::MultipleInboundEdges { node, input } => {
            assert_eq!(node, "add1");
            assert_eq!(input, "a");
        }
        other => panic!("expected MultipleInboundEdges, got {:?}", other),
    }
}
