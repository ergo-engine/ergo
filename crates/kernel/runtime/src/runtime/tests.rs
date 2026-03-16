use std::collections::HashMap;

use crate::action;
use crate::catalog::{build_core_catalog, core_registries};
use crate::cluster::{
    ExpandedEndpoint, ExpandedGraph, ExpandedNode, InputMetadata, OutputMetadata, PrimitiveCatalog,
    PrimitiveKind, PrimitiveMetadata, ValueType,
};
use crate::common::ErrorInfo;
use crate::compute::implementations::{Add, ConstNumber, Divide};
use crate::compute::PrimitiveRegistry as ComputeRegistry;
use crate::compute::{ComputePrimitive, ComputePrimitiveManifest};
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
    source_metadata_with_type(ValueType::Number)
}

fn source_metadata_with_type(value_type: ValueType) -> PrimitiveMetadata {
    let mut outputs = HashMap::new();
    outputs.insert(
        "out".to_string(),
        OutputMetadata {
            value_type,
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

fn compute_metadata_with_input_type(value_type: ValueType) -> PrimitiveMetadata {
    let mut outputs = HashMap::new();
    outputs.insert(
        "out".to_string(),
        OutputMetadata {
            value_type: ValueType::Number,
            cardinality: crate::cluster::Cardinality::Single,
        },
    );

    PrimitiveMetadata {
        kind: PrimitiveKind::Compute,
        inputs: vec![InputMetadata {
            name: "in".to_string(),
            value_type,
            required: true,
        }],
        outputs,
        parameters: Vec::new(),
    }
}

fn trigger_metadata_with_optional_input_type(value_type: ValueType) -> PrimitiveMetadata {
    let mut outputs = HashMap::new();
    outputs.insert(
        "event".to_string(),
        OutputMetadata {
            value_type: ValueType::Event,
            cardinality: crate::cluster::Cardinality::Single,
        },
    );

    PrimitiveMetadata {
        kind: PrimitiveKind::Trigger,
        inputs: vec![InputMetadata {
            name: "input".to_string(),
            value_type,
            required: false,
        }],
        outputs,
        parameters: Vec::new(),
    }
}

fn action_metadata_with_gate_and_payload(payload_type: ValueType) -> PrimitiveMetadata {
    PrimitiveMetadata {
        kind: PrimitiveKind::Action,
        inputs: vec![
            InputMetadata {
                name: "event".to_string(),
                value_type: ValueType::Event,
                required: false,
            },
            InputMetadata {
                name: "value".to_string(),
                value_type: payload_type,
                required: true,
            },
        ],
        outputs: HashMap::from([(
            "outcome".to_string(),
            OutputMetadata {
                value_type: ValueType::Event,
                cardinality: crate::cluster::Cardinality::Single,
            },
        )]),
        parameters: Vec::new(),
    }
}

#[derive(Clone)]
struct ConstSource {
    manifest: SourcePrimitiveManifest,
    value: f64,
}

#[derive(Clone)]
struct MissingOutputCompute {
    manifest: ComputePrimitiveManifest,
}

impl MissingOutputCompute {
    fn new(id: &str) -> Self {
        Self {
            manifest: ComputePrimitiveManifest {
                id: id.to_string(),
                version: "0.1.0".to_string(),
                kind: crate::common::PrimitiveKind::Compute,
                inputs: vec![crate::compute::InputSpec {
                    name: "in".to_string(),
                    value_type: crate::common::ValueType::Number,
                    required: true,
                    cardinality: crate::compute::Cardinality::Single,
                }],
                outputs: vec![crate::compute::OutputSpec {
                    name: "out".to_string(),
                    value_type: crate::common::ValueType::Number,
                }],
                parameters: vec![],
                execution: crate::compute::ExecutionSpec {
                    deterministic: true,
                    cadence: crate::compute::Cadence::Continuous,
                    may_error: false,
                },
                errors: crate::compute::ErrorSpec {
                    allowed: false,
                    types: vec![],
                    deterministic: true,
                },
                state: crate::compute::StateSpec {
                    allowed: false,
                    resettable: false,
                    description: None,
                },
                side_effects: false,
            },
        }
    }
}

impl ComputePrimitive for MissingOutputCompute {
    fn manifest(&self) -> &ComputePrimitiveManifest {
        &self.manifest
    }

    fn compute(
        &self,
        _inputs: &HashMap<String, crate::common::Value>,
        _parameters: &HashMap<String, crate::common::Value>,
        _state: Option<&mut crate::compute::PrimitiveState>,
    ) -> Result<HashMap<String, crate::common::Value>, crate::compute::ComputeError> {
        Ok(HashMap::new())
    }
}

impl ConstSource {
    fn new(id: &str, value: f64) -> Self {
        Self {
            manifest: SourcePrimitiveManifest {
                id: id.to_string(),
                version: "0.1.0".to_string(),
                kind: SourceKind::Source,
                inputs: vec![],
                outputs: vec![crate::source::OutputSpec {
                    name: "out".to_string(),
                    value_type: crate::common::ValueType::Number,
                }],
                parameters: vec![],
                requires: crate::source::SourceRequires { context: vec![] },
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
        _ctx: &ExecutionContext,
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
                requested_version: "v1".to_string(),
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
                requested_version: "v1".to_string(),
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
                requested_version: "v1".to_string(),
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

    let ctx = ExecutionContext::default();

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
                requested_version: "0.1.0".to_string(),
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

    let ctx = ExecutionContext::default();

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
                requested_version: "0.1.0".to_string(),
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
                requested_version: "0.1.0".to_string(),
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
                requested_version: "0.1.0".to_string(),
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
                requested_version: "0.1.0".to_string(),
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
                requested_version: "0.1.0".to_string(),
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

    let ctx = ExecutionContext::default();

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
                requested_version: "0.1.0".to_string(),
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
                requested_version: "0.1.0".to_string(),
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
                requested_version: "0.1.0".to_string(),
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
                requested_version: "0.1.0".to_string(),
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
                requested_version: "0.1.0".to_string(),
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
    let err = result.unwrap_err();
    assert_eq!(err.rule_id(), "V.3");
    assert_eq!(err.path().as_deref(), Some("$.edges"));
    match err {
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
                requested_version: "0.1.0".to_string(),
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
                requested_version: "0.1.0".to_string(),
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
                requested_version: "0.1.0".to_string(),
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
                requested_version: "0.1.0".to_string(),
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
                requested_version: "0.1.0".to_string(),
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

    let ctx = ExecutionContext::default();

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
fn emit_if_event_and_true_runtime_emits_and_suppresses_with_trigger_event_input() {
    fn run_with_condition(condition: bool) -> RuntimeValue {
        let mut nodes = HashMap::new();
        nodes.insert(
            "event_signal".to_string(),
            ExpandedNode {
                runtime_id: "event_signal".to_string(),
                authoring_path: vec![],
                implementation: crate::cluster::ImplementationInstance {
                    impl_id: "const_bool".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([(
                    "value".to_string(),
                    crate::cluster::ParameterValue::Bool(true),
                )]),
            },
        );
        nodes.insert(
            "condition_signal".to_string(),
            ExpandedNode {
                runtime_id: "condition_signal".to_string(),
                authoring_path: vec![],
                implementation: crate::cluster::ImplementationInstance {
                    impl_id: "const_bool".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([(
                    "value".to_string(),
                    crate::cluster::ParameterValue::Bool(condition),
                )]),
            },
        );
        nodes.insert(
            "emit_source_event".to_string(),
            ExpandedNode {
                runtime_id: "emit_source_event".to_string(),
                authoring_path: vec![],
                implementation: crate::cluster::ImplementationInstance {
                    impl_id: "emit_if_true".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::new(),
            },
        );
        nodes.insert(
            "gate_event".to_string(),
            ExpandedNode {
                runtime_id: "gate_event".to_string(),
                authoring_path: vec![],
                implementation: crate::cluster::ImplementationInstance {
                    impl_id: "emit_if_event_and_true".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::new(),
            },
        );
        nodes.insert(
            "ack".to_string(),
            ExpandedNode {
                runtime_id: "ack".to_string(),
                authoring_path: vec![],
                implementation: crate::cluster::ImplementationInstance {
                    impl_id: "ack_action".to_string(),
                    requested_version: "0.1.0".to_string(),
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
                    node_id: "event_signal".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "emit_source_event".to_string(),
                    port_name: "input".to_string(),
                },
            },
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "emit_source_event".to_string(),
                    port_name: "event".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "gate_event".to_string(),
                    port_name: "event".to_string(),
                },
            },
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "condition_signal".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "gate_event".to_string(),
                    port_name: "condition".to_string(),
                },
            },
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "gate_event".to_string(),
                    port_name: "event".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "ack".to_string(),
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
                    node_id: "ack".to_string(),
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

        let report = run(
            &expanded,
            &catalog,
            &registries,
            &ExecutionContext::default(),
        )
        .expect("runtime execution should succeed");

        report
            .outputs
            .get("action_outcome")
            .cloned()
            .expect("action_outcome must be present")
    }

    let emitted_outcome = run_with_condition(true);
    assert_eq!(
        emitted_outcome,
        RuntimeValue::Event(crate::runtime::types::RuntimeEvent::Action(
            crate::action::ActionOutcome::Completed
        ))
    );

    let suppressed_outcome = run_with_condition(false);
    assert_eq!(
        suppressed_outcome,
        RuntimeValue::Event(crate::runtime::types::RuntimeEvent::Action(
            crate::action::ActionOutcome::Skipped
        ))
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
                requested_version: "v1".to_string(),
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
    assert_eq!(err.rule_id(), "D.2");
    assert_eq!(err.path().as_deref(), Some("$.nodes"));
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
                requested_version: "v1".to_string(),
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
    assert_eq!(err.rule_id(), "D.2");
    assert_eq!(err.path().as_deref(), Some("$.nodes"));
    match err {
        ValidationError::MissingOutputMetadata { node, output } => {
            assert_eq!(node, "src1");
            assert_eq!(output, "does_not_exist");
        }
        other => panic!("expected MissingOutputMetadata, got {:?}", other),
    }
}

#[test]
fn validate_rejects_cycle_detected() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "a".to_string(),
        ExpandedNode {
            runtime_id: "a".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "c1".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );
    nodes.insert(
        "b".to_string(),
        ExpandedNode {
            runtime_id: "b".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "c2".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    // a.out -> b.in and b.out -> a.in forms a cycle.
    let edges = vec![
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "a".to_string(),
                port_name: "out".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "b".to_string(),
                port_name: "in".to_string(),
            },
        },
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "b".to_string(),
                port_name: "out".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "a".to_string(),
                port_name: "in".to_string(),
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
    catalog.metadata.insert(
        ("c1".to_string(), "0.1.0".to_string()),
        compute_metadata_with_input_type(ValueType::Number),
    );
    catalog.metadata.insert(
        ("c2".to_string(), "0.1.0".to_string()),
        compute_metadata_with_input_type(ValueType::Number),
    );

    let err = crate::runtime::validate(&expanded, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "V.1");
    assert_eq!(err.path().as_deref(), Some("$.edges"));
    assert!(matches!(err, ValidationError::CycleDetected));
}

#[test]
fn validate_rejects_source_event_edge_to_action() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "src".to_string(),
        ExpandedNode {
            runtime_id: "src".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "src".to_string(),
                requested_version: "0.1.0".to_string(),
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
                impl_id: "act".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    // Source -> Action is only allowed for scalar payload inputs; event inputs
    // must be gated by Trigger outputs.
    let edges = vec![crate::cluster::ExpandedEdge {
        from: ExpandedEndpoint::NodePort {
            node_id: "src".to_string(),
            port_name: "out".to_string(),
        },
        to: ExpandedEndpoint::NodePort {
            node_id: "act".to_string(),
            port_name: "event".to_string(),
        },
    }];

    let expanded = ExpandedGraph {
        nodes,
        edges,
        boundary_inputs: Vec::new(),
        boundary_outputs: Vec::new(),
    };

    let mut catalog = TestCatalog::default();
    catalog.metadata.insert(
        ("src".to_string(), "0.1.0".to_string()),
        source_metadata_with_type(ValueType::Event),
    );
    catalog.metadata.insert(
        ("act".to_string(), "0.1.0".to_string()),
        PrimitiveMetadata {
            kind: PrimitiveKind::Action,
            inputs: vec![InputMetadata {
                name: "event".to_string(),
                value_type: ValueType::Event,
                required: true,
            }],
            outputs: HashMap::from([(
                "outcome".to_string(),
                OutputMetadata {
                    value_type: ValueType::Event,
                    cardinality: crate::cluster::Cardinality::Single,
                },
            )]),
            parameters: Vec::new(),
        },
    );

    let err = crate::runtime::validate(&expanded, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "V.2");
    assert_eq!(err.path().as_deref(), Some("$.edges"));
    assert!(matches!(err, ValidationError::InvalidEdgeKind { .. }));
}

#[test]
fn validate_allows_source_scalar_payload_to_action_when_gated() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "src".to_string(),
        ExpandedNode {
            runtime_id: "src".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "source".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );
    nodes.insert(
        "trg".to_string(),
        ExpandedNode {
            runtime_id: "trg".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "trigger".to_string(),
                requested_version: "0.1.0".to_string(),
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
                impl_id: "action".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    let edges = vec![
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src".to_string(),
                port_name: "out".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "act".to_string(),
                port_name: "value".to_string(),
            },
        },
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "trg".to_string(),
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
        boundary_outputs: Vec::new(),
    };

    let mut catalog = TestCatalog::default();
    catalog.metadata.insert(
        ("source".to_string(), "0.1.0".to_string()),
        source_metadata_with_type(ValueType::Number),
    );
    catalog.metadata.insert(
        ("trigger".to_string(), "0.1.0".to_string()),
        trigger_metadata_with_optional_input_type(ValueType::Bool),
    );
    catalog.metadata.insert(
        ("action".to_string(), "0.1.0".to_string()),
        action_metadata_with_gate_and_payload(ValueType::Number),
    );

    crate::runtime::validate(&expanded, &catalog)
        .expect("source scalar payload edge into action should be valid when gated");
}

#[test]
fn validate_allows_compute_scalar_payload_to_action_when_gated() {
    let mut nodes = HashMap::new();
    for (id, impl_id) in [
        ("src_num", "source_num"),
        ("cmp", "compute"),
        ("trg", "trigger"),
        ("act", "action"),
    ] {
        nodes.insert(
            id.to_string(),
            ExpandedNode {
                runtime_id: id.to_string(),
                authoring_path: vec![],
                implementation: crate::cluster::ImplementationInstance {
                    impl_id: impl_id.to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::new(),
            },
        );
    }

    let edges = vec![
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src_num".to_string(),
                port_name: "out".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "cmp".to_string(),
                port_name: "in".to_string(),
            },
        },
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "cmp".to_string(),
                port_name: "out".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "act".to_string(),
                port_name: "value".to_string(),
            },
        },
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "trg".to_string(),
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
        boundary_outputs: Vec::new(),
    };

    let mut catalog = TestCatalog::default();
    catalog.metadata.insert(
        ("source_num".to_string(), "0.1.0".to_string()),
        source_metadata_with_type(ValueType::Number),
    );
    catalog.metadata.insert(
        ("compute".to_string(), "0.1.0".to_string()),
        compute_metadata_with_input_type(ValueType::Number),
    );
    catalog.metadata.insert(
        ("trigger".to_string(), "0.1.0".to_string()),
        trigger_metadata_with_optional_input_type(ValueType::Bool),
    );
    catalog.metadata.insert(
        ("action".to_string(), "0.1.0".to_string()),
        action_metadata_with_gate_and_payload(ValueType::Number),
    );

    crate::runtime::validate(&expanded, &catalog)
        .expect("compute scalar payload edge into action should be valid when gated");
}

#[test]
fn validate_allows_source_series_payload_to_action_when_gated() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "src".to_string(),
        ExpandedNode {
            runtime_id: "src".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "source_series".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );
    nodes.insert(
        "trg".to_string(),
        ExpandedNode {
            runtime_id: "trg".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "trigger".to_string(),
                requested_version: "0.1.0".to_string(),
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
                impl_id: "action".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    let edges = vec![
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src".to_string(),
                port_name: "out".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "act".to_string(),
                port_name: "value".to_string(),
            },
        },
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "trg".to_string(),
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
        boundary_outputs: Vec::new(),
    };

    let mut catalog = TestCatalog::default();
    catalog.metadata.insert(
        ("source_series".to_string(), "0.1.0".to_string()),
        source_metadata_with_type(ValueType::Series),
    );
    catalog.metadata.insert(
        ("trigger".to_string(), "0.1.0".to_string()),
        trigger_metadata_with_optional_input_type(ValueType::Bool),
    );
    catalog.metadata.insert(
        ("action".to_string(), "0.1.0".to_string()),
        action_metadata_with_gate_and_payload(ValueType::Series),
    );

    crate::runtime::validate(&expanded, &catalog)
        .expect("source series payload edge into action should be valid when gated");
}

#[test]
fn validate_allows_compute_series_payload_to_action_when_gated() {
    let mut nodes = HashMap::new();
    for (id, impl_id) in [
        ("src_series", "source_series"),
        ("cmp", "compute_series"),
        ("trg", "trigger"),
        ("act", "action"),
    ] {
        nodes.insert(
            id.to_string(),
            ExpandedNode {
                runtime_id: id.to_string(),
                authoring_path: vec![],
                implementation: crate::cluster::ImplementationInstance {
                    impl_id: impl_id.to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::new(),
            },
        );
    }

    let edges = vec![
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src_series".to_string(),
                port_name: "out".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "cmp".to_string(),
                port_name: "in".to_string(),
            },
        },
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "cmp".to_string(),
                port_name: "out".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "act".to_string(),
                port_name: "value".to_string(),
            },
        },
        crate::cluster::ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "trg".to_string(),
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
        boundary_outputs: Vec::new(),
    };

    let mut catalog = TestCatalog::default();
    catalog.metadata.insert(
        ("source_series".to_string(), "0.1.0".to_string()),
        source_metadata_with_type(ValueType::Series),
    );
    catalog.metadata.insert(
        ("compute_series".to_string(), "0.1.0".to_string()),
        PrimitiveMetadata {
            kind: PrimitiveKind::Compute,
            inputs: vec![InputMetadata {
                name: "in".to_string(),
                value_type: ValueType::Series,
                required: true,
            }],
            outputs: HashMap::from([(
                "out".to_string(),
                OutputMetadata {
                    value_type: ValueType::Series,
                    cardinality: crate::cluster::Cardinality::Single,
                },
            )]),
            parameters: Vec::new(),
        },
    );
    catalog.metadata.insert(
        ("trigger".to_string(), "0.1.0".to_string()),
        trigger_metadata_with_optional_input_type(ValueType::Bool),
    );
    catalog.metadata.insert(
        ("action".to_string(), "0.1.0".to_string()),
        action_metadata_with_gate_and_payload(ValueType::Series),
    );

    crate::runtime::validate(&expanded, &catalog)
        .expect("compute series payload edge into action should be valid when gated");
}

#[test]
fn validate_rejects_compute_event_edge_to_action_event_input() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "cmp".to_string(),
        ExpandedNode {
            runtime_id: "cmp".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "compute".to_string(),
                requested_version: "0.1.0".to_string(),
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
                impl_id: "action".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    let edges = vec![crate::cluster::ExpandedEdge {
        from: ExpandedEndpoint::NodePort {
            node_id: "cmp".to_string(),
            port_name: "out".to_string(),
        },
        to: ExpandedEndpoint::NodePort {
            node_id: "act".to_string(),
            port_name: "event".to_string(),
        },
    }];

    let expanded = ExpandedGraph {
        nodes,
        edges,
        boundary_inputs: Vec::new(),
        boundary_outputs: Vec::new(),
    };

    let mut catalog = TestCatalog::default();
    catalog.metadata.insert(
        ("compute".to_string(), "0.1.0".to_string()),
        PrimitiveMetadata {
            kind: PrimitiveKind::Compute,
            inputs: vec![],
            outputs: HashMap::from([(
                "out".to_string(),
                OutputMetadata {
                    value_type: ValueType::Event,
                    cardinality: crate::cluster::Cardinality::Single,
                },
            )]),
            parameters: Vec::new(),
        },
    );
    catalog.metadata.insert(
        ("action".to_string(), "0.1.0".to_string()),
        PrimitiveMetadata {
            kind: PrimitiveKind::Action,
            inputs: vec![InputMetadata {
                name: "event".to_string(),
                value_type: ValueType::Event,
                required: false,
            }],
            outputs: HashMap::from([(
                "outcome".to_string(),
                OutputMetadata {
                    value_type: ValueType::Event,
                    cardinality: crate::cluster::Cardinality::Single,
                },
            )]),
            parameters: Vec::new(),
        },
    );

    let err = crate::runtime::validate(&expanded, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "V.2");
    assert!(matches!(err, ValidationError::InvalidEdgeKind { .. }));
}

#[test]
fn validate_rejects_action_with_scalar_payload_edge_but_no_trigger_gate() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "src".to_string(),
        ExpandedNode {
            runtime_id: "src".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "source".to_string(),
                requested_version: "0.1.0".to_string(),
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
                impl_id: "action".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    let edges = vec![crate::cluster::ExpandedEdge {
        from: ExpandedEndpoint::NodePort {
            node_id: "src".to_string(),
            port_name: "out".to_string(),
        },
        to: ExpandedEndpoint::NodePort {
            node_id: "act".to_string(),
            port_name: "value".to_string(),
        },
    }];

    let expanded = ExpandedGraph {
        nodes,
        edges,
        boundary_inputs: Vec::new(),
        boundary_outputs: Vec::new(),
    };

    let mut catalog = TestCatalog::default();
    catalog.metadata.insert(
        ("source".to_string(), "0.1.0".to_string()),
        source_metadata_with_type(ValueType::Number),
    );
    catalog.metadata.insert(
        ("action".to_string(), "0.1.0".to_string()),
        action_metadata_with_gate_and_payload(ValueType::Number),
    );

    let err = crate::runtime::validate(&expanded, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "V.5");
    assert!(matches!(err, ValidationError::ActionNotGated(node) if node == "act"));
}

#[test]
fn validate_rejects_external_input_endpoint() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "cmp".to_string(),
        ExpandedNode {
            runtime_id: "cmp".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "compute".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    let edges = vec![crate::cluster::ExpandedEdge {
        from: ExpandedEndpoint::NodePort {
            node_id: "cmp".to_string(),
            port_name: "out".to_string(),
        },
        to: ExpandedEndpoint::ExternalInput {
            name: "external".to_string(),
        },
    }];

    let expanded = ExpandedGraph {
        nodes,
        edges,
        boundary_inputs: Vec::new(),
        boundary_outputs: Vec::new(),
    };

    let mut catalog = TestCatalog::default();
    catalog.metadata.insert(
        ("compute".to_string(), "0.1.0".to_string()),
        compute_metadata_with_input_type(ValueType::Number),
    );

    let err = crate::runtime::validate(&expanded, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "E.3");
    assert_eq!(err.path().as_deref(), Some("$.edges"));
    assert!(matches!(
        err,
        ValidationError::ExternalInputNotAllowed { .. }
    ));
}

#[test]
fn validate_rejects_missing_primitive_metadata() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "cmp".to_string(),
        ExpandedNode {
            runtime_id: "cmp".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "missing_compute".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    let expanded = ExpandedGraph {
        nodes,
        edges: Vec::new(),
        boundary_inputs: Vec::new(),
        boundary_outputs: Vec::new(),
    };

    let catalog = TestCatalog::default();

    let err = crate::runtime::validate(&expanded, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "V.8");
    assert_eq!(err.path().as_deref(), Some("$.nodes"));
    assert!(matches!(
        err,
        ValidationError::MissingPrimitive { id, version }
            if id == "missing_compute" && version == "0.1.0"
    ));
}

/// ACT-12: Actions must be gated by trigger events.
#[test]
fn act_12_action_not_gated_rejected() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "act".to_string(),
        ExpandedNode {
            runtime_id: "act".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "test_action".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    let expanded = ExpandedGraph {
        nodes,
        edges: Vec::new(),
        boundary_inputs: Vec::new(),
        boundary_outputs: Vec::new(),
    };

    let mut catalog = TestCatalog::default();
    catalog.metadata.insert(
        ("test_action".to_string(), "0.1.0".to_string()),
        PrimitiveMetadata {
            kind: PrimitiveKind::Action,
            inputs: vec![InputMetadata {
                name: "event".to_string(),
                value_type: ValueType::Event,
                required: false,
            }],
            outputs: HashMap::from([(
                "outcome".to_string(),
                OutputMetadata {
                    value_type: ValueType::Event,
                    cardinality: crate::cluster::Cardinality::Single,
                },
            )]),
            parameters: Vec::new(),
        },
    );

    let err = crate::runtime::validate(&expanded, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "V.5");
    assert_eq!(err.path().as_deref(), Some("$.edges"));
    match err {
        ValidationError::ActionNotGated(node) => assert_eq!(node, "act"),
        other => panic!("expected ActionNotGated, got {:?}", other),
    }
}

#[test]
fn comp_4_source_output_type_mismatch_rejected() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "src".to_string(),
        ExpandedNode {
            runtime_id: "src".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "source".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );
    nodes.insert(
        "cmp".to_string(),
        ExpandedNode {
            runtime_id: "cmp".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "compute".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    let edges = vec![crate::cluster::ExpandedEdge {
        from: ExpandedEndpoint::NodePort {
            node_id: "src".to_string(),
            port_name: "out".to_string(),
        },
        to: ExpandedEndpoint::NodePort {
            node_id: "cmp".to_string(),
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
    catalog.metadata.insert(
        ("source".to_string(), "0.1.0".to_string()),
        source_metadata_with_type(ValueType::Bool),
    );
    catalog.metadata.insert(
        ("compute".to_string(), "0.1.0".to_string()),
        compute_metadata_with_input_type(ValueType::Number),
    );

    let err = crate::runtime::validate(&expanded, &catalog).unwrap_err();
    assert_eq!(err.rule_id(), "V.4");
    assert_eq!(err.path().as_deref(), Some("$.edges"));
    match err {
        ValidationError::TypeMismatch { expected, got, .. } => {
            assert_eq!(expected, ValueType::Number);
            assert_eq!(got, ValueType::Bool);
        }
        other => panic!("expected TypeMismatch, got {:?}", other),
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

    let ctx = ExecutionContext::default();

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
                requested_version: "0.1.0".to_string(),
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

    let ctx = ExecutionContext::default();

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
                requested_version: "0.1.0".to_string(),
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

    let ctx = ExecutionContext::default();

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
                requested_version: "0.1.0".to_string(),
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

    let ctx = ExecutionContext::default();

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
                requested_version: "v1".to_string(),
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
                requested_version: "v1".to_string(),
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
                requested_version: "v1".to_string(),
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
    assert_eq!(err.rule_id(), "V.7");
    assert_eq!(err.path().as_deref(), Some("$.edges"));
    match err {
        ValidationError::MultipleInboundEdges { node, input } => {
            assert_eq!(node, "add1");
            assert_eq!(input, "a");
        }
        other => panic!("expected MultipleInboundEdges, got {:?}", other),
    }
}

/// Source with required context key, empty context → MissingRequiredContextKey.
#[test]
fn context_key_missing_rejected() {
    // Build a source that requires context key "x" of type Number.
    #[derive(Clone)]
    struct RequiresXSource {
        manifest: SourcePrimitiveManifest,
    }

    impl SourcePrimitive for RequiresXSource {
        fn manifest(&self) -> &SourcePrimitiveManifest {
            &self.manifest
        }

        fn produce(
            &self,
            _parameters: &HashMap<String, crate::source::ParameterValue>,
            _ctx: &ExecutionContext,
        ) -> HashMap<String, crate::common::Value> {
            HashMap::from([("out".to_string(), crate::common::Value::Number(0.0))])
        }
    }

    let src = RequiresXSource {
        manifest: SourcePrimitiveManifest {
            id: "req_x".to_string(),
            version: "0.1.0".to_string(),
            kind: SourceKind::Source,
            inputs: vec![],
            outputs: vec![crate::source::OutputSpec {
                name: "out".to_string(),
                value_type: crate::common::ValueType::Number,
            }],
            parameters: vec![],
            requires: crate::source::SourceRequires {
                context: vec![crate::source::ContextRequirement {
                    name: "x".to_string(),
                    ty: crate::common::ValueType::Number,
                    required: true,
                }],
            },
            execution: crate::source::ExecutionSpec {
                deterministic: true,
                cadence: crate::source::Cadence::Continuous,
            },
            state: crate::source::StateSpec { allowed: false },
            side_effects: false,
        },
    };

    let mut source_registry = SourceRegistry::new();
    source_registry.register(Box::new(src)).unwrap();

    let graph = crate::runtime::types::ValidatedGraph {
        nodes: HashMap::from([(
            "s".to_string(),
            crate::runtime::types::ValidatedNode {
                runtime_id: "s".to_string(),
                impl_id: "req_x".to_string(),
                version: "0.1.0".to_string(),
                kind: PrimitiveKind::Source,
                inputs: vec![],
                outputs: HashMap::from([(
                    "out".to_string(),
                    OutputMetadata {
                        value_type: ValueType::Number,
                        cardinality: crate::cluster::Cardinality::Single,
                    },
                )]),
                parameters: HashMap::new(),
            },
        )]),
        edges: vec![],
        topo_order: vec!["s".to_string()],
        boundary_outputs: vec![],
    };

    let registries = Registries {
        sources: &source_registry,
        computes: &ComputeRegistry::new(),
        triggers: &TriggerRegistry::new(),
        actions: &crate::action::ActionRegistry::new(),
    };

    // Empty context — required key "x" is missing.
    let ctx = ExecutionContext::default();
    let err = crate::runtime::execute(&graph, &registries, &ctx).unwrap_err();
    assert_eq!(err.rule_id(), "SRC-10");
    assert_eq!(err.path().as_deref(), Some("$.context.x"));
    match err {
        ExecError::MissingRequiredContextKey { node, key } => {
            assert_eq!(node, "s");
            assert_eq!(key, "x");
        }
        other => panic!("expected MissingRequiredContextKey, got {:?}", other),
    }
}

/// Source with required Number context key, context provides String → ContextKeyTypeMismatch.
#[test]
fn context_key_type_mismatch_rejected() {
    #[derive(Clone)]
    struct RequiresXSource {
        manifest: SourcePrimitiveManifest,
    }

    impl SourcePrimitive for RequiresXSource {
        fn manifest(&self) -> &SourcePrimitiveManifest {
            &self.manifest
        }

        fn produce(
            &self,
            _parameters: &HashMap<String, crate::source::ParameterValue>,
            _ctx: &ExecutionContext,
        ) -> HashMap<String, crate::common::Value> {
            HashMap::from([("out".to_string(), crate::common::Value::Number(0.0))])
        }
    }

    let src = RequiresXSource {
        manifest: SourcePrimitiveManifest {
            id: "req_x".to_string(),
            version: "0.1.0".to_string(),
            kind: SourceKind::Source,
            inputs: vec![],
            outputs: vec![crate::source::OutputSpec {
                name: "out".to_string(),
                value_type: crate::common::ValueType::Number,
            }],
            parameters: vec![],
            requires: crate::source::SourceRequires {
                context: vec![crate::source::ContextRequirement {
                    name: "x".to_string(),
                    ty: crate::common::ValueType::Number,
                    required: true,
                }],
            },
            execution: crate::source::ExecutionSpec {
                deterministic: true,
                cadence: crate::source::Cadence::Continuous,
            },
            state: crate::source::StateSpec { allowed: false },
            side_effects: false,
        },
    };

    let mut source_registry = SourceRegistry::new();
    source_registry.register(Box::new(src)).unwrap();

    let graph = crate::runtime::types::ValidatedGraph {
        nodes: HashMap::from([(
            "s".to_string(),
            crate::runtime::types::ValidatedNode {
                runtime_id: "s".to_string(),
                impl_id: "req_x".to_string(),
                version: "0.1.0".to_string(),
                kind: PrimitiveKind::Source,
                inputs: vec![],
                outputs: HashMap::from([(
                    "out".to_string(),
                    OutputMetadata {
                        value_type: ValueType::Number,
                        cardinality: crate::cluster::Cardinality::Single,
                    },
                )]),
                parameters: HashMap::new(),
            },
        )]),
        edges: vec![],
        topo_order: vec!["s".to_string()],
        boundary_outputs: vec![],
    };

    let registries = Registries {
        sources: &source_registry,
        computes: &ComputeRegistry::new(),
        triggers: &TriggerRegistry::new(),
        actions: &crate::action::ActionRegistry::new(),
    };

    // Provide a String value where Number is required.
    let ctx = ExecutionContext::from_values(HashMap::from([(
        "x".to_string(),
        crate::common::Value::String("not_a_number".to_string()),
    )]));

    let err = crate::runtime::execute(&graph, &registries, &ctx).unwrap_err();
    assert_eq!(err.rule_id(), "SRC-11");
    assert_eq!(err.path().as_deref(), Some("$.context.x"));
    match err {
        ExecError::ContextKeyTypeMismatch {
            node,
            key,
            expected,
            got,
        } => {
            assert_eq!(node, "s");
            assert_eq!(key, "x");
            assert_eq!(expected, crate::common::ValueType::Number);
            assert_eq!(got, crate::common::ValueType::String);
        }
        other => panic!("expected ContextKeyTypeMismatch, got {:?}", other),
    }
}

/// Source with optional context key, empty context → executes successfully.
#[test]
fn optional_context_key_missing_allowed() {
    #[derive(Clone)]
    struct OptionalXSource {
        manifest: SourcePrimitiveManifest,
    }

    impl SourcePrimitive for OptionalXSource {
        fn manifest(&self) -> &SourcePrimitiveManifest {
            &self.manifest
        }

        fn produce(
            &self,
            _parameters: &HashMap<String, crate::source::ParameterValue>,
            _ctx: &ExecutionContext,
        ) -> HashMap<String, crate::common::Value> {
            HashMap::from([("out".to_string(), crate::common::Value::Number(42.0))])
        }
    }

    let src = OptionalXSource {
        manifest: SourcePrimitiveManifest {
            id: "opt_x".to_string(),
            version: "0.1.0".to_string(),
            kind: SourceKind::Source,
            inputs: vec![],
            outputs: vec![crate::source::OutputSpec {
                name: "out".to_string(),
                value_type: crate::common::ValueType::Number,
            }],
            parameters: vec![],
            requires: crate::source::SourceRequires {
                context: vec![crate::source::ContextRequirement {
                    name: "x".to_string(),
                    ty: crate::common::ValueType::Number,
                    required: false,
                }],
            },
            execution: crate::source::ExecutionSpec {
                deterministic: true,
                cadence: crate::source::Cadence::Continuous,
            },
            state: crate::source::StateSpec { allowed: false },
            side_effects: false,
        },
    };

    let mut source_registry = SourceRegistry::new();
    source_registry.register(Box::new(src)).unwrap();

    let graph = crate::runtime::types::ValidatedGraph {
        nodes: HashMap::from([(
            "s".to_string(),
            crate::runtime::types::ValidatedNode {
                runtime_id: "s".to_string(),
                impl_id: "opt_x".to_string(),
                version: "0.1.0".to_string(),
                kind: PrimitiveKind::Source,
                inputs: vec![],
                outputs: HashMap::from([(
                    "out".to_string(),
                    OutputMetadata {
                        value_type: ValueType::Number,
                        cardinality: crate::cluster::Cardinality::Single,
                    },
                )]),
                parameters: HashMap::new(),
            },
        )]),
        edges: vec![],
        topo_order: vec!["s".to_string()],
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "result".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "s".to_string(),
                port_name: "out".to_string(),
            },
        }],
    };

    let registries = Registries {
        sources: &source_registry,
        computes: &ComputeRegistry::new(),
        triggers: &TriggerRegistry::new(),
        actions: &crate::action::ActionRegistry::new(),
    };

    // Empty context — optional key "x" is missing, should still succeed.
    let ctx = ExecutionContext::default();
    let report = crate::runtime::execute(&graph, &registries, &ctx).unwrap();
    assert_eq!(
        report.outputs.get("result"),
        Some(&RuntimeValue::Number(42.0))
    );
}

/// CMP-11: Compute must produce all declared outputs on success.
#[test]
fn cmp_11_missing_output_fails() {
    let mut source_registry = SourceRegistry::new();
    source_registry
        .register(Box::new(ConstSource::new("src", 1.0)))
        .unwrap();

    let mut compute_registry = ComputeRegistry::new();
    compute_registry
        .register(Box::new(MissingOutputCompute::new("missing_output")))
        .unwrap();

    let graph = crate::runtime::types::ValidatedGraph {
        nodes: HashMap::from([
            (
                "src".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "src".to_string(),
                    impl_id: "src".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Source,
                    inputs: vec![],
                    outputs: HashMap::from([(
                        "out".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Number,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::new(),
                },
            ),
            (
                "cmp".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "cmp".to_string(),
                    impl_id: "missing_output".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Compute,
                    inputs: vec![InputMetadata {
                        name: "in".to_string(),
                        value_type: ValueType::Number,
                        required: true,
                    }],
                    outputs: HashMap::from([(
                        "out".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Number,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::new(),
                },
            ),
        ]),
        edges: vec![crate::runtime::types::ValidatedEdge {
            from: crate::runtime::types::Endpoint::NodePort {
                node_id: "src".to_string(),
                port_name: "out".to_string(),
            },
            to: crate::runtime::types::Endpoint::NodePort {
                node_id: "cmp".to_string(),
                port_name: "in".to_string(),
            },
        }],
        topo_order: vec!["src".to_string(), "cmp".to_string()],
        boundary_outputs: vec![],
    };

    let registries = Registries {
        sources: &source_registry,
        computes: &compute_registry,
        triggers: &TriggerRegistry::new(),
        actions: &crate::action::ActionRegistry::new(),
    };

    let ctx = ExecutionContext::default();
    let err = crate::runtime::execute(&graph, &registries, &ctx).unwrap_err();
    assert_eq!(err.rule_id(), "CMP-11");
    assert_eq!(err.path().as_deref(), Some("$.nodes.cmp.outputs.out"));
    match err {
        ExecError::MissingOutput { node, output } => {
            assert_eq!(node, "cmp");
            assert_eq!(output, "out");
        }
        other => panic!("expected MissingOutput, got {:?}", other),
    }
}

/// CMP-12: Compute errors surface as ComputeFailed and produce no outputs.
#[test]
fn cmp_12_compute_error_fails() {
    let mut source_registry = SourceRegistry::new();
    source_registry
        .register(Box::new(ConstSource::new("src_a", 1.0)))
        .unwrap();
    source_registry
        .register(Box::new(ConstSource::new("src_b", 0.0)))
        .unwrap();

    let mut compute_registry = ComputeRegistry::new();
    compute_registry.register(Box::new(Divide::new())).unwrap();

    let graph = crate::runtime::types::ValidatedGraph {
        nodes: HashMap::from([
            (
                "src_a".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "src_a".to_string(),
                    impl_id: "src_a".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Source,
                    inputs: vec![],
                    outputs: HashMap::from([(
                        "out".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Number,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::new(),
                },
            ),
            (
                "src_b".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "src_b".to_string(),
                    impl_id: "src_b".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Source,
                    inputs: vec![],
                    outputs: HashMap::from([(
                        "out".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Number,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::new(),
                },
            ),
            (
                "cmp".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "cmp".to_string(),
                    impl_id: "divide".to_string(),
                    version: "0.2.0".to_string(),
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
                    outputs: HashMap::from([(
                        "result".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Number,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::new(),
                },
            ),
        ]),
        edges: vec![
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "src_a".to_string(),
                    port_name: "out".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "cmp".to_string(),
                    port_name: "a".to_string(),
                },
            },
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "src_b".to_string(),
                    port_name: "out".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "cmp".to_string(),
                    port_name: "b".to_string(),
                },
            },
        ],
        topo_order: vec!["src_a".to_string(), "src_b".to_string(), "cmp".to_string()],
        boundary_outputs: vec![],
    };

    let registries = Registries {
        sources: &source_registry,
        computes: &compute_registry,
        triggers: &TriggerRegistry::new(),
        actions: &action::ActionRegistry::new(),
    };

    let ctx = ExecutionContext::default();
    let err = crate::runtime::execute(&graph, &registries, &ctx).unwrap_err();
    assert_eq!(err.rule_id(), "CMP-12");
    assert_eq!(err.path().as_deref(), Some("$.nodes.cmp"));
    match err {
        ExecError::ComputeFailed { node, error, .. } => {
            assert_eq!(node, "cmp");
            assert!(matches!(
                error,
                crate::compute::ComputeError::DivisionByZero
            ));
        }
        other => panic!("expected ComputeFailed, got {:?}", other),
    }
}

/// Source with $key context requirement, parameter resolves key name from context.
#[test]
fn execute_source_precheck_resolves_dollar_key() {
    #[derive(Clone)]
    struct DollarKeySource {
        manifest: SourcePrimitiveManifest,
    }

    impl SourcePrimitive for DollarKeySource {
        fn manifest(&self) -> &SourcePrimitiveManifest {
            &self.manifest
        }

        fn produce(
            &self,
            _parameters: &HashMap<String, crate::source::ParameterValue>,
            ctx: &ExecutionContext,
        ) -> HashMap<String, crate::common::Value> {
            // The source reads from whatever context key the parameter resolves to.
            // For test purposes, just return a fixed value to confirm execution succeeded.
            let _ = ctx;
            HashMap::from([("out".to_string(), crate::common::Value::Number(99.0))])
        }
    }

    let src = DollarKeySource {
        manifest: SourcePrimitiveManifest {
            id: "dollar_key_src".to_string(),
            version: "0.1.0".to_string(),
            kind: SourceKind::Source,
            inputs: vec![],
            outputs: vec![crate::source::OutputSpec {
                name: "out".to_string(),
                value_type: crate::common::ValueType::Number,
            }],
            parameters: vec![crate::source::ParameterSpec {
                name: "key".to_string(),
                value_type: crate::source::ParameterType::String,
                default: None,
                bounds: None,
            }],
            requires: crate::source::SourceRequires {
                context: vec![crate::source::ContextRequirement {
                    name: "$key".to_string(),
                    ty: crate::common::ValueType::Number,
                    required: true,
                }],
            },
            execution: crate::source::ExecutionSpec {
                deterministic: true,
                cadence: crate::source::Cadence::Continuous,
            },
            state: crate::source::StateSpec { allowed: false },
            side_effects: false,
        },
    };

    let mut source_registry = SourceRegistry::new();
    source_registry.register(Box::new(src)).unwrap();

    let graph = crate::runtime::types::ValidatedGraph {
        nodes: HashMap::from([(
            "s".to_string(),
            crate::runtime::types::ValidatedNode {
                runtime_id: "s".to_string(),
                impl_id: "dollar_key_src".to_string(),
                version: "0.1.0".to_string(),
                kind: PrimitiveKind::Source,
                inputs: vec![],
                outputs: HashMap::from([(
                    "out".to_string(),
                    OutputMetadata {
                        value_type: ValueType::Number,
                        cardinality: crate::cluster::Cardinality::Single,
                    },
                )]),
                parameters: HashMap::from([(
                    "key".to_string(),
                    crate::cluster::ParameterValue::String("sample_key".to_string()),
                )]),
            },
        )]),
        edges: vec![],
        topo_order: vec!["s".to_string()],
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "result".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "s".to_string(),
                port_name: "out".to_string(),
            },
        }],
    };

    let registries = Registries {
        sources: &source_registry,
        computes: &ComputeRegistry::new(),
        triggers: &TriggerRegistry::new(),
        actions: &crate::action::ActionRegistry::new(),
    };

    // Context provides the resolved key "sample_key" with correct type.
    let ctx = ExecutionContext::from_values(HashMap::from([(
        "sample_key".to_string(),
        crate::common::Value::Number(42.0),
    )]));

    let report = crate::runtime::execute(&graph, &registries, &ctx).unwrap();
    assert_eq!(
        report.outputs.get("result"),
        Some(&RuntimeValue::Number(99.0))
    );
}

/// Source with $key context requirement, resolved key missing from context → error.
#[test]
fn execute_source_dollar_key_missing_context_rejected() {
    #[derive(Clone)]
    struct DollarKeySource {
        manifest: SourcePrimitiveManifest,
    }

    impl SourcePrimitive for DollarKeySource {
        fn manifest(&self) -> &SourcePrimitiveManifest {
            &self.manifest
        }

        fn produce(
            &self,
            _parameters: &HashMap<String, crate::source::ParameterValue>,
            _ctx: &ExecutionContext,
        ) -> HashMap<String, crate::common::Value> {
            HashMap::from([("out".to_string(), crate::common::Value::Number(0.0))])
        }
    }

    let src = DollarKeySource {
        manifest: SourcePrimitiveManifest {
            id: "dollar_key_src2".to_string(),
            version: "0.1.0".to_string(),
            kind: SourceKind::Source,
            inputs: vec![],
            outputs: vec![crate::source::OutputSpec {
                name: "out".to_string(),
                value_type: crate::common::ValueType::Number,
            }],
            parameters: vec![crate::source::ParameterSpec {
                name: "key".to_string(),
                value_type: crate::source::ParameterType::String,
                default: None,
                bounds: None,
            }],
            requires: crate::source::SourceRequires {
                context: vec![crate::source::ContextRequirement {
                    name: "$key".to_string(),
                    ty: crate::common::ValueType::Number,
                    required: true,
                }],
            },
            execution: crate::source::ExecutionSpec {
                deterministic: true,
                cadence: crate::source::Cadence::Continuous,
            },
            state: crate::source::StateSpec { allowed: false },
            side_effects: false,
        },
    };

    let mut source_registry = SourceRegistry::new();
    source_registry.register(Box::new(src)).unwrap();

    let graph = crate::runtime::types::ValidatedGraph {
        nodes: HashMap::from([(
            "s".to_string(),
            crate::runtime::types::ValidatedNode {
                runtime_id: "s".to_string(),
                impl_id: "dollar_key_src2".to_string(),
                version: "0.1.0".to_string(),
                kind: PrimitiveKind::Source,
                inputs: vec![],
                outputs: HashMap::from([(
                    "out".to_string(),
                    OutputMetadata {
                        value_type: ValueType::Number,
                        cardinality: crate::cluster::Cardinality::Single,
                    },
                )]),
                parameters: HashMap::from([(
                    "key".to_string(),
                    crate::cluster::ParameterValue::String("sample_key".to_string()),
                )]),
            },
        )]),
        edges: vec![],
        topo_order: vec!["s".to_string()],
        boundary_outputs: vec![],
    };

    let registries = Registries {
        sources: &source_registry,
        computes: &ComputeRegistry::new(),
        triggers: &TriggerRegistry::new(),
        actions: &crate::action::ActionRegistry::new(),
    };

    // Empty context — resolved key "sample_key" is not present.
    let ctx = ExecutionContext::default();
    let err = crate::runtime::execute(&graph, &registries, &ctx).unwrap_err();
    assert_eq!(err.rule_id(), "SRC-10");
    match err {
        ExecError::MissingRequiredContextKey { node, key } => {
            assert_eq!(node, "s");
            assert_eq!(key, "sample_key");
        }
        other => panic!("expected MissingRequiredContextKey, got {:?}", other),
    }
}

/// Optional $key with missing parameter fails at execution precheck.
/// Guards the optional early-return regression at the execution layer:
/// resolution must run before `if !req.required { continue; }`.
#[test]
fn execute_source_precheck_optional_dollar_key_missing_parameter_rejected() {
    #[derive(Clone)]
    struct OptDollarKeySource {
        manifest: SourcePrimitiveManifest,
    }

    impl SourcePrimitive for OptDollarKeySource {
        fn manifest(&self) -> &SourcePrimitiveManifest {
            &self.manifest
        }

        fn produce(
            &self,
            _parameters: &HashMap<String, crate::source::ParameterValue>,
            _ctx: &ExecutionContext,
        ) -> HashMap<String, crate::common::Value> {
            HashMap::from([("out".to_string(), crate::common::Value::Number(0.0))])
        }
    }

    let src = OptDollarKeySource {
        manifest: SourcePrimitiveManifest {
            id: "opt_dollar_key_src".to_string(),
            version: "0.1.0".to_string(),
            kind: SourceKind::Source,
            inputs: vec![],
            outputs: vec![crate::source::OutputSpec {
                name: "out".to_string(),
                value_type: crate::common::ValueType::Number,
            }],
            parameters: vec![crate::source::ParameterSpec {
                name: "key".to_string(),
                value_type: crate::source::ParameterType::String,
                default: None,
                bounds: None,
            }],
            requires: crate::source::SourceRequires {
                context: vec![crate::source::ContextRequirement {
                    name: "$key".to_string(),
                    ty: crate::common::ValueType::Number,
                    required: false,
                }],
            },
            execution: crate::source::ExecutionSpec {
                deterministic: true,
                cadence: crate::source::Cadence::Continuous,
            },
            state: crate::source::StateSpec { allowed: false },
            side_effects: false,
        },
    };

    let mut source_registry = SourceRegistry::new();
    source_registry.register(Box::new(src)).unwrap();

    let graph = crate::runtime::types::ValidatedGraph {
        nodes: HashMap::from([(
            "s".to_string(),
            crate::runtime::types::ValidatedNode {
                runtime_id: "s".to_string(),
                impl_id: "opt_dollar_key_src".to_string(),
                version: "0.1.0".to_string(),
                kind: PrimitiveKind::Source,
                inputs: vec![],
                outputs: HashMap::from([(
                    "out".to_string(),
                    OutputMetadata {
                        value_type: ValueType::Number,
                        cardinality: crate::cluster::Cardinality::Single,
                    },
                )]),
                // No "key" parameter provided — $key cannot resolve.
                parameters: HashMap::new(),
            },
        )]),
        edges: vec![],
        topo_order: vec!["s".to_string()],
        boundary_outputs: vec![],
    };

    let registries = Registries {
        sources: &source_registry,
        computes: &ComputeRegistry::new(),
        triggers: &TriggerRegistry::new(),
        actions: &crate::action::ActionRegistry::new(),
    };

    let ctx = ExecutionContext::default();
    let err = crate::runtime::execute(&graph, &registries, &ctx).unwrap_err();
    // Resolution failure is mapped to MissingRequiredContextKey with the raw binding name.
    match err {
        ExecError::MissingRequiredContextKey { node, key } => {
            assert_eq!(node, "s");
            assert_eq!(key, "$key");
        }
        other => panic!(
            "expected MissingRequiredContextKey for unresolved optional $key, got {:?}",
            other
        ),
    }
}

// ---- Effect routing infrastructure tests ----

/// Test-only action primitive that declares one write: reads from_input "value" and writes to key.
#[derive(Clone)]
struct WriteAction {
    manifest: crate::action::ActionPrimitiveManifest,
}

impl WriteAction {
    fn new(id: &str, write_key: &str, from_input: &str) -> Self {
        Self {
            manifest: crate::action::ActionPrimitiveManifest {
                id: id.to_string(),
                version: "0.1.0".to_string(),
                kind: crate::action::ActionKind::Action,
                inputs: vec![
                    crate::action::InputSpec {
                        name: "event".to_string(),
                        value_type: crate::action::ActionValueType::Event,
                        required: true,
                        cardinality: crate::action::Cardinality::Single,
                    },
                    crate::action::InputSpec {
                        name: from_input.to_string(),
                        value_type: crate::action::ActionValueType::Number,
                        required: true,
                        cardinality: crate::action::Cardinality::Single,
                    },
                ],
                outputs: vec![crate::action::OutputSpec {
                    name: "outcome".to_string(),
                    value_type: crate::action::ActionValueType::Event,
                }],
                parameters: vec![],
                effects: crate::action::ActionEffects {
                    writes: vec![crate::action::ActionWriteSpec {
                        name: write_key.to_string(),
                        value_type: crate::common::ValueType::Number,
                        from_input: from_input.to_string(),
                    }],
                    intents: vec![],
                },
                execution: crate::action::ExecutionSpec {
                    deterministic: true,
                    retryable: false,
                },
                state: crate::action::StateSpec { allowed: false },
                side_effects: true,
            },
        }
    }
}

impl crate::action::ActionPrimitive for WriteAction {
    fn manifest(&self) -> &crate::action::ActionPrimitiveManifest {
        &self.manifest
    }

    fn execute(
        &self,
        _inputs: &HashMap<String, crate::action::ActionValue>,
        _parameters: &HashMap<String, crate::action::ParameterValue>,
    ) -> HashMap<String, crate::action::ActionValue> {
        HashMap::from([(
            "outcome".to_string(),
            crate::action::ActionValue::Event(crate::action::ActionOutcome::Completed),
        )])
    }
}

#[derive(Clone)]
struct IntentAction {
    manifest: crate::action::ActionPrimitiveManifest,
}

impl IntentAction {
    fn intent_only(id: &str, intent_kind: &str) -> Self {
        Self {
            manifest: crate::action::ActionPrimitiveManifest {
                id: id.to_string(),
                version: "0.1.0".to_string(),
                kind: crate::action::ActionKind::Action,
                inputs: vec![
                    crate::action::InputSpec {
                        name: "event".to_string(),
                        value_type: crate::action::ActionValueType::Event,
                        required: true,
                        cardinality: crate::action::Cardinality::Single,
                    },
                    crate::action::InputSpec {
                        name: "symbol".to_string(),
                        value_type: crate::action::ActionValueType::String,
                        required: true,
                        cardinality: crate::action::Cardinality::Single,
                    },
                    crate::action::InputSpec {
                        name: "qty".to_string(),
                        value_type: crate::action::ActionValueType::Number,
                        required: true,
                        cardinality: crate::action::Cardinality::Single,
                    },
                ],
                outputs: vec![crate::action::OutputSpec {
                    name: "outcome".to_string(),
                    value_type: crate::action::ActionValueType::Event,
                }],
                parameters: vec![],
                effects: crate::action::ActionEffects {
                    writes: vec![],
                    intents: vec![crate::action::IntentSpec {
                        name: intent_kind.to_string(),
                        fields: vec![
                            crate::action::IntentFieldSpec {
                                name: "symbol".to_string(),
                                value_type: crate::common::ValueType::String,
                                from_input: Some("symbol".to_string()),
                                from_param: None,
                            },
                            crate::action::IntentFieldSpec {
                                name: "qty".to_string(),
                                value_type: crate::common::ValueType::Number,
                                from_input: Some("qty".to_string()),
                                from_param: None,
                            },
                        ],
                        mirror_writes: vec![],
                    }],
                },
                execution: crate::action::ExecutionSpec {
                    deterministic: true,
                    retryable: false,
                },
                state: crate::action::StateSpec { allowed: false },
                side_effects: true,
            },
        }
    }

    fn mixed(id: &str, intent_kind: &str) -> Self {
        Self {
            manifest: crate::action::ActionPrimitiveManifest {
                id: id.to_string(),
                version: "0.1.0".to_string(),
                kind: crate::action::ActionKind::Action,
                inputs: vec![
                    crate::action::InputSpec {
                        name: "event".to_string(),
                        value_type: crate::action::ActionValueType::Event,
                        required: true,
                        cardinality: crate::action::Cardinality::Single,
                    },
                    crate::action::InputSpec {
                        name: "symbol".to_string(),
                        value_type: crate::action::ActionValueType::String,
                        required: true,
                        cardinality: crate::action::Cardinality::Single,
                    },
                    crate::action::InputSpec {
                        name: "qty".to_string(),
                        value_type: crate::action::ActionValueType::Number,
                        required: true,
                        cardinality: crate::action::Cardinality::Single,
                    },
                ],
                outputs: vec![crate::action::OutputSpec {
                    name: "outcome".to_string(),
                    value_type: crate::action::ActionValueType::Event,
                }],
                parameters: vec![],
                effects: crate::action::ActionEffects {
                    writes: vec![crate::action::ActionWriteSpec {
                        name: "order_qty".to_string(),
                        value_type: crate::common::ValueType::Number,
                        from_input: "qty".to_string(),
                    }],
                    intents: vec![crate::action::IntentSpec {
                        name: intent_kind.to_string(),
                        fields: vec![
                            crate::action::IntentFieldSpec {
                                name: "symbol".to_string(),
                                value_type: crate::common::ValueType::String,
                                from_input: Some("symbol".to_string()),
                                from_param: None,
                            },
                            crate::action::IntentFieldSpec {
                                name: "qty".to_string(),
                                value_type: crate::common::ValueType::Number,
                                from_input: Some("qty".to_string()),
                                from_param: None,
                            },
                        ],
                        mirror_writes: vec![crate::action::IntentMirrorWriteSpec {
                            name: "last_symbol".to_string(),
                            value_type: crate::common::ValueType::String,
                            from_field: "symbol".to_string(),
                        }],
                    }],
                },
                execution: crate::action::ExecutionSpec {
                    deterministic: true,
                    retryable: false,
                },
                state: crate::action::StateSpec { allowed: false },
                side_effects: true,
            },
        }
    }
}

impl crate::action::ActionPrimitive for IntentAction {
    fn manifest(&self) -> &crate::action::ActionPrimitiveManifest {
        &self.manifest
    }

    fn execute(
        &self,
        _inputs: &HashMap<String, crate::action::ActionValue>,
        _parameters: &HashMap<String, crate::action::ParameterValue>,
    ) -> HashMap<String, crate::action::ActionValue> {
        HashMap::from([(
            "outcome".to_string(),
            crate::action::ActionValue::Event(crate::action::ActionOutcome::Completed),
        )])
    }
}

fn intent_action_graph(action_impl_id: &str) -> crate::runtime::types::ValidatedGraph {
    crate::runtime::types::ValidatedGraph {
        nodes: HashMap::from([
            (
                "src_bool".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "src_bool".to_string(),
                    impl_id: "boolean_source".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Source,
                    inputs: vec![],
                    outputs: HashMap::from([(
                        "value".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Bool,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::Bool(true),
                    )]),
                },
            ),
            (
                "src_symbol".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "src_symbol".to_string(),
                    impl_id: "string_source".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Source,
                    inputs: vec![],
                    outputs: HashMap::from([(
                        "value".to_string(),
                        OutputMetadata {
                            value_type: ValueType::String,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::String("EURUSD".to_string()),
                    )]),
                },
            ),
            (
                "src_qty".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "src_qty".to_string(),
                    impl_id: "number_source".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Source,
                    inputs: vec![],
                    outputs: HashMap::from([(
                        "value".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Number,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::Number(100.0),
                    )]),
                },
            ),
            (
                "emit".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "emit".to_string(),
                    impl_id: "emit_if_true".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Trigger,
                    inputs: vec![InputMetadata {
                        name: "input".to_string(),
                        value_type: ValueType::Bool,
                        required: true,
                    }],
                    outputs: HashMap::from([(
                        "event".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Event,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::new(),
                },
            ),
            (
                "act".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "act".to_string(),
                    impl_id: action_impl_id.to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Action,
                    inputs: vec![
                        InputMetadata {
                            name: "event".to_string(),
                            value_type: ValueType::Event,
                            required: true,
                        },
                        InputMetadata {
                            name: "symbol".to_string(),
                            value_type: ValueType::String,
                            required: true,
                        },
                        InputMetadata {
                            name: "qty".to_string(),
                            value_type: ValueType::Number,
                            required: true,
                        },
                    ],
                    outputs: HashMap::from([(
                        "outcome".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Event,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::new(),
                },
            ),
        ]),
        edges: vec![
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "src_bool".to_string(),
                    port_name: "value".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "input".to_string(),
                },
            },
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "event".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "act".to_string(),
                    port_name: "event".to_string(),
                },
            },
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "src_symbol".to_string(),
                    port_name: "value".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "act".to_string(),
                    port_name: "symbol".to_string(),
                },
            },
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "src_qty".to_string(),
                    port_name: "value".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "act".to_string(),
                    port_name: "qty".to_string(),
                },
            },
        ],
        topo_order: vec![
            "src_bool".to_string(),
            "src_symbol".to_string(),
            "src_qty".to_string(),
            "emit".to_string(),
            "act".to_string(),
        ],
        boundary_outputs: vec![],
    }
}

/// Action with one declared write emits one ActionEffect with value sourced from from_input.
#[test]
fn effect_action_one_write_emits_effect() {
    let core_regs = crate::catalog::core_registries().unwrap();

    let mut act_reg = crate::action::ActionRegistry::new();
    act_reg
        .register(Box::new(WriteAction::new(
            "test_write_action",
            "price",
            "value",
        )))
        .unwrap();

    let mut bool_source = SourceRegistry::new();
    bool_source
        .register(Box::new(crate::source::BooleanSource::new()))
        .unwrap();
    bool_source
        .register(Box::new(crate::source::NumberSource::new()))
        .unwrap();

    let graph = crate::runtime::types::ValidatedGraph {
        nodes: HashMap::from([
            (
                "src_bool".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "src_bool".to_string(),
                    impl_id: "boolean_source".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Source,
                    inputs: vec![],
                    outputs: HashMap::from([(
                        "value".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Bool,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::Bool(true),
                    )]),
                },
            ),
            (
                "src_num".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "src_num".to_string(),
                    impl_id: "number_source".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Source,
                    inputs: vec![],
                    outputs: HashMap::from([(
                        "value".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Number,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::Number(42.0),
                    )]),
                },
            ),
            (
                "emit".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "emit".to_string(),
                    impl_id: "emit_if_true".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Trigger,
                    inputs: vec![InputMetadata {
                        name: "input".to_string(),
                        value_type: ValueType::Bool,
                        required: true,
                    }],
                    outputs: HashMap::from([(
                        "event".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Event,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::new(),
                },
            ),
            (
                "act".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "act".to_string(),
                    impl_id: "test_write_action".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Action,
                    inputs: vec![
                        InputMetadata {
                            name: "event".to_string(),
                            value_type: ValueType::Event,
                            required: true,
                        },
                        InputMetadata {
                            name: "value".to_string(),
                            value_type: ValueType::Number,
                            required: true,
                        },
                    ],
                    outputs: HashMap::from([(
                        "outcome".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Event,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::new(),
                },
            ),
        ]),
        edges: vec![
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "src_bool".to_string(),
                    port_name: "value".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "input".to_string(),
                },
            },
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "event".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "act".to_string(),
                    port_name: "event".to_string(),
                },
            },
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "src_num".to_string(),
                    port_name: "value".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "act".to_string(),
                    port_name: "value".to_string(),
                },
            },
        ],
        topo_order: vec![
            "src_bool".to_string(),
            "src_num".to_string(),
            "emit".to_string(),
            "act".to_string(),
        ],
        boundary_outputs: vec![],
    };

    let regs = Registries {
        sources: &bool_source,
        computes: &core_regs.computes,
        triggers: &core_regs.triggers,
        actions: &act_reg,
    };

    let ctx = ExecutionContext::default();
    let report = crate::runtime::execute(&graph, &regs, &ctx).unwrap();

    // Verify effects
    assert_eq!(
        report.effects.len(),
        1,
        "action with one write should emit one effect"
    );
    let effect = &report.effects[0];
    assert_eq!(effect.kind, "set_context");
    assert_eq!(effect.writes.len(), 1);
    assert_eq!(effect.writes[0].key, "price");
    assert_eq!(effect.writes[0].value, crate::common::Value::Number(42.0));
}

#[test]
fn metadata_less_execute_rejects_intent_emitting_graph() {
    let core_regs = crate::catalog::core_registries().unwrap();
    let mut act_reg = crate::action::ActionRegistry::new();
    act_reg
        .register(Box::new(IntentAction::intent_only(
            "intent_only_action",
            "place_order",
        )))
        .unwrap();

    let graph = intent_action_graph("intent_only_action");
    let regs = Registries {
        sources: &core_regs.sources,
        computes: &core_regs.computes,
        triggers: &core_regs.triggers,
        actions: &act_reg,
    };
    let ctx = ExecutionContext::default();
    let err = crate::runtime::execute(&graph, &regs, &ctx).unwrap_err();

    match err {
        ExecError::IntentMetadataRequired { node } => assert_eq!(node, "act"),
        other => panic!("expected IntentMetadataRequired, got {other:?}"),
    }
}

#[test]
fn metadata_aware_execute_emits_intent_only_external_effect_kind() {
    let core_regs = crate::catalog::core_registries().unwrap();
    let mut act_reg = crate::action::ActionRegistry::new();
    act_reg
        .register(Box::new(IntentAction::intent_only(
            "intent_only_action",
            "place_order",
        )))
        .unwrap();

    let graph = intent_action_graph("intent_only_action");
    let regs = Registries {
        sources: &core_regs.sources,
        computes: &core_regs.computes,
        triggers: &core_regs.triggers,
        actions: &act_reg,
    };
    let ctx = ExecutionContext::default();
    let report =
        crate::runtime::execute_with_metadata(&graph, &regs, &ctx, "graph-1", "event-1").unwrap();

    assert_eq!(report.effects.len(), 1);
    let effect = &report.effects[0];
    assert_eq!(effect.kind, "place_order");
    assert!(effect.writes.is_empty());
    assert_eq!(effect.intents.len(), 1);
    assert_eq!(effect.intents[0].kind, "place_order");
    assert!(
        effect.intents[0].intent_id.starts_with("eid1:sha256:"),
        "intent_id should use eid1 derivation"
    );
}

#[test]
fn metadata_aware_execute_emits_canonical_internal_then_external_effects() {
    let core_regs = crate::catalog::core_registries().unwrap();
    let mut act_reg = crate::action::ActionRegistry::new();
    act_reg
        .register(Box::new(IntentAction::mixed(
            "intent_mixed_action",
            "place_order",
        )))
        .unwrap();

    let graph = intent_action_graph("intent_mixed_action");
    let regs = Registries {
        sources: &core_regs.sources,
        computes: &core_regs.computes,
        triggers: &core_regs.triggers,
        actions: &act_reg,
    };
    let ctx = ExecutionContext::default();
    let report =
        crate::runtime::execute_with_metadata(&graph, &regs, &ctx, "graph-1", "event-2").unwrap();

    assert_eq!(report.effects.len(), 2);

    let internal = &report.effects[0];
    assert_eq!(internal.kind, "set_context");
    assert!(internal.intents.is_empty());
    assert_eq!(internal.writes.len(), 2);
    let write_map: HashMap<_, _> = internal
        .writes
        .iter()
        .map(|write| (write.key.as_str(), &write.value))
        .collect();
    assert_eq!(
        write_map.get("order_qty"),
        Some(&&crate::common::Value::Number(100.0))
    );
    assert_eq!(
        write_map.get("last_symbol"),
        Some(&&crate::common::Value::String("EURUSD".to_string()))
    );

    let external = &report.effects[1];
    assert_eq!(external.kind, "place_order");
    assert!(external.writes.is_empty());
    assert_eq!(external.intents.len(), 1);
    assert_eq!(external.intents[0].kind, external.kind);
}

/// Action with no writes emits no effects.
#[test]
fn effect_action_no_writes_emits_no_effects() {
    let core_regs = crate::catalog::core_registries().unwrap();

    // Use hello_world-style graph with ack_action (no writes)
    let graph = crate::runtime::types::ValidatedGraph {
        nodes: HashMap::from([
            (
                "src_bool".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "src_bool".to_string(),
                    impl_id: "boolean_source".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Source,
                    inputs: vec![],
                    outputs: HashMap::from([(
                        "value".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Bool,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::Bool(true),
                    )]),
                },
            ),
            (
                "emit".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "emit".to_string(),
                    impl_id: "emit_if_true".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Trigger,
                    inputs: vec![InputMetadata {
                        name: "input".to_string(),
                        value_type: ValueType::Bool,
                        required: true,
                    }],
                    outputs: HashMap::from([(
                        "event".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Event,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::new(),
                },
            ),
            (
                "act".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "act".to_string(),
                    impl_id: "ack_action".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Action,
                    inputs: vec![InputMetadata {
                        name: "event".to_string(),
                        value_type: ValueType::Event,
                        required: true,
                    }],
                    outputs: HashMap::from([(
                        "outcome".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Event,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::from([(
                        "accept".to_string(),
                        crate::cluster::ParameterValue::Bool(true),
                    )]),
                },
            ),
        ]),
        edges: vec![
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "src_bool".to_string(),
                    port_name: "value".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "input".to_string(),
                },
            },
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "event".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "act".to_string(),
                    port_name: "event".to_string(),
                },
            },
        ],
        topo_order: vec![
            "src_bool".to_string(),
            "emit".to_string(),
            "act".to_string(),
        ],
        boundary_outputs: vec![],
    };

    let regs = Registries {
        sources: &core_regs.sources,
        computes: &core_regs.computes,
        triggers: &core_regs.triggers,
        actions: &core_regs.actions,
    };

    let ctx = ExecutionContext::default();
    let report = crate::runtime::execute(&graph, &regs, &ctx).unwrap();
    assert!(
        report.effects.is_empty(),
        "action with no writes should emit no effects"
    );
}

/// Skipped action (trigger not emitted) produces no effects.
#[test]
fn effect_skipped_action_emits_no_effects() {
    let core_regs = crate::catalog::core_registries().unwrap();

    let mut act_reg = crate::action::ActionRegistry::new();
    act_reg
        .register(Box::new(WriteAction::new(
            "test_write_action2",
            "price",
            "value",
        )))
        .unwrap();

    // Source produces false -> emit_if_true emits NotEmitted -> action skipped
    let graph = crate::runtime::types::ValidatedGraph {
        nodes: HashMap::from([
            (
                "src_bool".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "src_bool".to_string(),
                    impl_id: "boolean_source".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Source,
                    inputs: vec![],
                    outputs: HashMap::from([(
                        "value".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Bool,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::Bool(false), // false -> NotEmitted
                    )]),
                },
            ),
            (
                "src_num".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "src_num".to_string(),
                    impl_id: "number_source".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Source,
                    inputs: vec![],
                    outputs: HashMap::from([(
                        "value".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Number,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::Number(42.0),
                    )]),
                },
            ),
            (
                "emit".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "emit".to_string(),
                    impl_id: "emit_if_true".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Trigger,
                    inputs: vec![InputMetadata {
                        name: "input".to_string(),
                        value_type: ValueType::Bool,
                        required: true,
                    }],
                    outputs: HashMap::from([(
                        "event".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Event,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::new(),
                },
            ),
            (
                "act".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "act".to_string(),
                    impl_id: "test_write_action2".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Action,
                    inputs: vec![
                        InputMetadata {
                            name: "event".to_string(),
                            value_type: ValueType::Event,
                            required: true,
                        },
                        InputMetadata {
                            name: "value".to_string(),
                            value_type: ValueType::Number,
                            required: true,
                        },
                    ],
                    outputs: HashMap::from([(
                        "outcome".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Event,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::new(),
                },
            ),
        ]),
        edges: vec![
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "src_bool".to_string(),
                    port_name: "value".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "input".to_string(),
                },
            },
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "event".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "act".to_string(),
                    port_name: "event".to_string(),
                },
            },
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "src_num".to_string(),
                    port_name: "value".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "act".to_string(),
                    port_name: "value".to_string(),
                },
            },
        ],
        topo_order: vec![
            "src_bool".to_string(),
            "src_num".to_string(),
            "emit".to_string(),
            "act".to_string(),
        ],
        boundary_outputs: vec![],
    };

    let regs = Registries {
        sources: &core_regs.sources,
        computes: &core_regs.computes,
        triggers: &core_regs.triggers,
        actions: &act_reg,
    };

    let ctx = ExecutionContext::default();
    let report = crate::runtime::execute(&graph, &regs, &ctx).unwrap();
    assert!(
        report.effects.is_empty(),
        "skipped action should emit no effects"
    );
}

/// Action with multiple writes emits writes in manifest declaration order.
#[test]
fn effect_action_multiple_writes_emits_writes_in_declaration_order() {
    // Test-only action with two writes: "price" from "val_a", "volume" from "val_b"
    #[derive(Clone)]
    struct MultiWriteAction {
        manifest: crate::action::ActionPrimitiveManifest,
    }

    impl crate::action::ActionPrimitive for MultiWriteAction {
        fn manifest(&self) -> &crate::action::ActionPrimitiveManifest {
            &self.manifest
        }
        fn execute(
            &self,
            _inputs: &HashMap<String, crate::action::ActionValue>,
            _parameters: &HashMap<String, crate::action::ParameterValue>,
        ) -> HashMap<String, crate::action::ActionValue> {
            HashMap::from([(
                "outcome".to_string(),
                crate::action::ActionValue::Event(crate::action::ActionOutcome::Completed),
            )])
        }
    }

    let action = MultiWriteAction {
        manifest: crate::action::ActionPrimitiveManifest {
            id: "multi_write_action".to_string(),
            version: "0.1.0".to_string(),
            kind: crate::action::ActionKind::Action,
            inputs: vec![
                crate::action::InputSpec {
                    name: "event".to_string(),
                    value_type: crate::action::ActionValueType::Event,
                    required: true,
                    cardinality: crate::action::Cardinality::Single,
                },
                crate::action::InputSpec {
                    name: "val_a".to_string(),
                    value_type: crate::action::ActionValueType::Number,
                    required: true,
                    cardinality: crate::action::Cardinality::Single,
                },
                crate::action::InputSpec {
                    name: "val_b".to_string(),
                    value_type: crate::action::ActionValueType::Number,
                    required: true,
                    cardinality: crate::action::Cardinality::Single,
                },
            ],
            outputs: vec![crate::action::OutputSpec {
                name: "outcome".to_string(),
                value_type: crate::action::ActionValueType::Event,
            }],
            parameters: vec![],
            effects: crate::action::ActionEffects {
                writes: vec![
                    crate::action::ActionWriteSpec {
                        name: "price".to_string(),
                        value_type: crate::common::ValueType::Number,
                        from_input: "val_a".to_string(),
                    },
                    crate::action::ActionWriteSpec {
                        name: "volume".to_string(),
                        value_type: crate::common::ValueType::Number,
                        from_input: "val_b".to_string(),
                    },
                ],
                intents: vec![],
            },
            execution: crate::action::ExecutionSpec {
                deterministic: true,
                retryable: false,
            },
            state: crate::action::StateSpec { allowed: false },
            side_effects: true,
        },
    };

    let mut act_reg = crate::action::ActionRegistry::new();
    act_reg.register(Box::new(action)).unwrap();

    let core_regs = crate::catalog::core_registries().unwrap();

    let graph = crate::runtime::types::ValidatedGraph {
        nodes: HashMap::from([
            (
                "src_bool".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "src_bool".to_string(),
                    impl_id: "boolean_source".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Source,
                    inputs: vec![],
                    outputs: HashMap::from([(
                        "value".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Bool,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::Bool(true),
                    )]),
                },
            ),
            (
                "src_a".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "src_a".to_string(),
                    impl_id: "number_source".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Source,
                    inputs: vec![],
                    outputs: HashMap::from([(
                        "value".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Number,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::Number(100.0),
                    )]),
                },
            ),
            (
                "src_b".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "src_b".to_string(),
                    impl_id: "number_source".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Source,
                    inputs: vec![],
                    outputs: HashMap::from([(
                        "value".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Number,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::Number(500.0),
                    )]),
                },
            ),
            (
                "emit".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "emit".to_string(),
                    impl_id: "emit_if_true".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Trigger,
                    inputs: vec![InputMetadata {
                        name: "input".to_string(),
                        value_type: ValueType::Bool,
                        required: true,
                    }],
                    outputs: HashMap::from([(
                        "event".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Event,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::new(),
                },
            ),
            (
                "act".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "act".to_string(),
                    impl_id: "multi_write_action".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Action,
                    inputs: vec![
                        InputMetadata {
                            name: "event".to_string(),
                            value_type: ValueType::Event,
                            required: true,
                        },
                        InputMetadata {
                            name: "val_a".to_string(),
                            value_type: ValueType::Number,
                            required: true,
                        },
                        InputMetadata {
                            name: "val_b".to_string(),
                            value_type: ValueType::Number,
                            required: true,
                        },
                    ],
                    outputs: HashMap::from([(
                        "outcome".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Event,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::new(),
                },
            ),
        ]),
        edges: vec![
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "src_bool".to_string(),
                    port_name: "value".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "input".to_string(),
                },
            },
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "event".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "act".to_string(),
                    port_name: "event".to_string(),
                },
            },
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "src_a".to_string(),
                    port_name: "value".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "act".to_string(),
                    port_name: "val_a".to_string(),
                },
            },
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "src_b".to_string(),
                    port_name: "value".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "act".to_string(),
                    port_name: "val_b".to_string(),
                },
            },
        ],
        topo_order: vec![
            "src_bool".to_string(),
            "src_a".to_string(),
            "src_b".to_string(),
            "emit".to_string(),
            "act".to_string(),
        ],
        boundary_outputs: vec![],
    };

    let regs = Registries {
        sources: &core_regs.sources,
        computes: &core_regs.computes,
        triggers: &core_regs.triggers,
        actions: &act_reg,
    };

    let ctx = ExecutionContext::default();
    let report = crate::runtime::execute(&graph, &regs, &ctx).unwrap();

    assert_eq!(report.effects.len(), 1);
    let effect = &report.effects[0];
    assert_eq!(effect.writes.len(), 2, "two writes in declaration order");
    assert_eq!(effect.writes[0].key, "price");
    assert_eq!(effect.writes[0].value, crate::common::Value::Number(100.0));
    assert_eq!(effect.writes[1].key, "volume");
    assert_eq!(effect.writes[1].value, crate::common::Value::Number(500.0));
}

/// Action with $key write name resolves from node parameters.
#[test]
fn effect_action_write_key_dollar_binding_resolves_from_parameters() {
    let mut act_reg = crate::action::ActionRegistry::new();
    // WriteAction uses a single write with configurable key name
    let action = WriteAction::new("dollar_write_action", "$key", "value");
    // Need to add the "key" String parameter to the manifest
    let mut manifest = action.manifest.clone();
    manifest.parameters.push(crate::action::ParameterSpec {
        name: "key".to_string(),
        value_type: crate::action::ParameterType::String,
        default: None,
        required: true,
        bounds: None,
    });

    #[derive(Clone)]
    struct ParamWriteAction {
        manifest: crate::action::ActionPrimitiveManifest,
    }
    impl crate::action::ActionPrimitive for ParamWriteAction {
        fn manifest(&self) -> &crate::action::ActionPrimitiveManifest {
            &self.manifest
        }
        fn execute(
            &self,
            _inputs: &HashMap<String, crate::action::ActionValue>,
            _parameters: &HashMap<String, crate::action::ParameterValue>,
        ) -> HashMap<String, crate::action::ActionValue> {
            HashMap::from([(
                "outcome".to_string(),
                crate::action::ActionValue::Event(crate::action::ActionOutcome::Completed),
            )])
        }
    }
    act_reg
        .register(Box::new(ParamWriteAction { manifest }))
        .unwrap();

    let core_regs = crate::catalog::core_registries().unwrap();

    let graph = crate::runtime::types::ValidatedGraph {
        nodes: HashMap::from([
            (
                "src_bool".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "src_bool".to_string(),
                    impl_id: "boolean_source".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Source,
                    inputs: vec![],
                    outputs: HashMap::from([(
                        "value".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Bool,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::Bool(true),
                    )]),
                },
            ),
            (
                "src_num".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "src_num".to_string(),
                    impl_id: "number_source".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Source,
                    inputs: vec![],
                    outputs: HashMap::from([(
                        "value".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Number,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::Number(77.0),
                    )]),
                },
            ),
            (
                "emit".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "emit".to_string(),
                    impl_id: "emit_if_true".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Trigger,
                    inputs: vec![InputMetadata {
                        name: "input".to_string(),
                        value_type: ValueType::Bool,
                        required: true,
                    }],
                    outputs: HashMap::from([(
                        "event".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Event,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    parameters: HashMap::new(),
                },
            ),
            (
                "act".to_string(),
                crate::runtime::types::ValidatedNode {
                    runtime_id: "act".to_string(),
                    impl_id: "dollar_write_action".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Action,
                    inputs: vec![
                        InputMetadata {
                            name: "event".to_string(),
                            value_type: ValueType::Event,
                            required: true,
                        },
                        InputMetadata {
                            name: "value".to_string(),
                            value_type: ValueType::Number,
                            required: true,
                        },
                    ],
                    outputs: HashMap::from([(
                        "outcome".to_string(),
                        OutputMetadata {
                            value_type: ValueType::Event,
                            cardinality: crate::cluster::Cardinality::Single,
                        },
                    )]),
                    // Node parameter resolves $key -> "sample_key"
                    parameters: HashMap::from([(
                        "key".to_string(),
                        crate::cluster::ParameterValue::String("sample_key".to_string()),
                    )]),
                },
            ),
        ]),
        edges: vec![
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "src_bool".to_string(),
                    port_name: "value".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "input".to_string(),
                },
            },
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "event".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "act".to_string(),
                    port_name: "event".to_string(),
                },
            },
            crate::runtime::types::ValidatedEdge {
                from: crate::runtime::types::Endpoint::NodePort {
                    node_id: "src_num".to_string(),
                    port_name: "value".to_string(),
                },
                to: crate::runtime::types::Endpoint::NodePort {
                    node_id: "act".to_string(),
                    port_name: "value".to_string(),
                },
            },
        ],
        topo_order: vec![
            "src_bool".to_string(),
            "src_num".to_string(),
            "emit".to_string(),
            "act".to_string(),
        ],
        boundary_outputs: vec![],
    };

    let regs = Registries {
        sources: &core_regs.sources,
        computes: &core_regs.computes,
        triggers: &core_regs.triggers,
        actions: &act_reg,
    };

    let ctx = ExecutionContext::default();
    let report = crate::runtime::execute(&graph, &regs, &ctx).unwrap();

    assert_eq!(report.effects.len(), 1);
    assert_eq!(report.effects[0].writes.len(), 1);
    // Key should be resolved "sample_key", not literal "$key"
    assert_eq!(report.effects[0].writes[0].key, "sample_key");
    assert_eq!(
        report.effects[0].writes[0].value,
        crate::common::Value::Number(77.0)
    );
}

fn run_context_set_action_graph(
    action_impl_id: &str,
    payload_source_impl_id: &str,
    payload_source_value: crate::cluster::ParameterValue,
    key: &str,
) -> crate::runtime::types::ExecutionReport {
    let mut nodes = HashMap::new();
    nodes.insert(
        "gate_src".to_string(),
        ExpandedNode {
            runtime_id: "gate_src".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "const_bool".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([(
                "value".to_string(),
                crate::cluster::ParameterValue::Bool(true),
            )]),
        },
    );
    nodes.insert(
        "payload_src".to_string(),
        ExpandedNode {
            runtime_id: "payload_src".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: payload_source_impl_id.to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([("value".to_string(), payload_source_value)]),
        },
    );
    nodes.insert(
        "emit".to_string(),
        ExpandedNode {
            runtime_id: "emit".to_string(),
            authoring_path: vec![],
            implementation: crate::cluster::ImplementationInstance {
                impl_id: "emit_if_true".to_string(),
                requested_version: "0.1.0".to_string(),
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
                impl_id: action_impl_id.to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([(
                "key".to_string(),
                crate::cluster::ParameterValue::String(key.to_string()),
            )]),
        },
    );

    let expanded = ExpandedGraph {
        nodes,
        edges: vec![
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "gate_src".to_string(),
                    port_name: "value".to_string(),
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
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "payload_src".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "act".to_string(),
                    port_name: "value".to_string(),
                },
            },
        ],
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
    let core = core_registries().unwrap();
    let registries = Registries {
        sources: &core.sources,
        computes: &core.computes,
        triggers: &core.triggers,
        actions: &core.actions,
    };

    let report = run(
        &expanded,
        &catalog,
        &registries,
        &ExecutionContext::default(),
    )
    .unwrap();
    assert_eq!(
        report.outputs.get("action_outcome"),
        Some(&RuntimeValue::Event(
            crate::runtime::types::RuntimeEvent::Action(crate::action::ActionOutcome::Attempted),
        ))
    );
    report
}

#[test]
fn context_set_number_runtime_emits_effect_with_resolved_key_and_number_value() {
    let report = run_context_set_action_graph(
        "context_set_number",
        "number_source",
        crate::cluster::ParameterValue::Number(21.5),
        "fast_ema",
    );

    assert_eq!(report.effects.len(), 1);
    let effect = &report.effects[0];
    assert_eq!(effect.kind, "set_context");
    assert_eq!(effect.writes.len(), 1);
    assert_eq!(effect.writes[0].key, "fast_ema");
    assert_eq!(effect.writes[0].value, crate::common::Value::Number(21.5));
}

#[test]
fn context_set_bool_runtime_emits_effect_with_resolved_key_and_bool_value() {
    let report = run_context_set_action_graph(
        "context_set_bool",
        "boolean_source",
        crate::cluster::ParameterValue::Bool(false),
        "armed",
    );

    assert_eq!(report.effects.len(), 1);
    let effect = &report.effects[0];
    assert_eq!(effect.kind, "set_context");
    assert_eq!(effect.writes.len(), 1);
    assert_eq!(effect.writes[0].key, "armed");
    assert_eq!(effect.writes[0].value, crate::common::Value::Bool(false));
}

#[test]
fn context_set_string_runtime_emits_effect_with_resolved_key_and_string_value() {
    let report = run_context_set_action_graph(
        "context_set_string",
        "string_source",
        crate::cluster::ParameterValue::String("ready".to_string()),
        "status",
    );

    assert_eq!(report.effects.len(), 1);
    let effect = &report.effects[0];
    assert_eq!(effect.kind, "set_context");
    assert_eq!(effect.writes.len(), 1);
    assert_eq!(effect.writes[0].key, "status");
    assert_eq!(
        effect.writes[0].value,
        crate::common::Value::String("ready".to_string())
    );
}

#[test]
fn context_set_series_runtime_emits_effect_with_resolved_key_and_series_value() {
    let expanded = ExpandedGraph {
        nodes: HashMap::from([
            (
                "gate_src".to_string(),
                ExpandedNode {
                    runtime_id: "gate_src".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "const_bool".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::Bool(true),
                    )]),
                },
            ),
            (
                "series_src".to_string(),
                ExpandedNode {
                    runtime_id: "series_src".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "context_series_source".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::from([(
                        "key".to_string(),
                        crate::cluster::ParameterValue::String("samples".to_string()),
                    )]),
                },
            ),
            (
                "emit".to_string(),
                ExpandedNode {
                    runtime_id: "emit".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "emit_if_true".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::new(),
                },
            ),
            (
                "act".to_string(),
                ExpandedNode {
                    runtime_id: "act".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "context_set_series".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::from([(
                        "key".to_string(),
                        crate::cluster::ParameterValue::String("rolling".to_string()),
                    )]),
                },
            ),
        ]),
        edges: vec![
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "gate_src".to_string(),
                    port_name: "value".to_string(),
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
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "series_src".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "act".to_string(),
                    port_name: "value".to_string(),
                },
            },
        ],
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
    let core = core_registries().unwrap();
    let registries = Registries {
        sources: &core.sources,
        computes: &core.computes,
        triggers: &core.triggers,
        actions: &core.actions,
    };

    let ctx = ExecutionContext::from_values(HashMap::from([(
        "samples".to_string(),
        crate::common::Value::Series(vec![13.0, 21.0]),
    )]));

    let report = run(&expanded, &catalog, &registries, &ctx).unwrap();
    assert_eq!(
        report.outputs.get("action_outcome"),
        Some(&RuntimeValue::Event(
            crate::runtime::types::RuntimeEvent::Action(crate::action::ActionOutcome::Attempted),
        ))
    );
    assert_eq!(report.effects.len(), 1);
    let effect = &report.effects[0];
    assert_eq!(effect.kind, "set_context");
    assert_eq!(effect.writes.len(), 1);
    assert_eq!(effect.writes[0].key, "rolling");
    assert_eq!(
        effect.writes[0].value,
        crate::common::Value::Series(vec![13.0, 21.0])
    );
}

#[test]
fn context_number_source_runtime_reads_custom_key_parameter() {
    let expanded = ExpandedGraph {
        nodes: HashMap::from([(
            "src".to_string(),
            ExpandedNode {
                runtime_id: "src".to_string(),
                authoring_path: vec![],
                implementation: crate::cluster::ImplementationInstance {
                    impl_id: "context_number_source".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([(
                    "key".to_string(),
                    crate::cluster::ParameterValue::String("sample_key".to_string()),
                )]),
            },
        )]),
        edges: vec![],
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "out".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "src".to_string(),
                port_name: "value".to_string(),
            },
        }],
    };

    let catalog = build_core_catalog();
    let core = core_registries().unwrap();
    let registries = Registries {
        sources: &core.sources,
        computes: &core.computes,
        triggers: &core.triggers,
        actions: &core.actions,
    };

    let ctx = ExecutionContext::from_values(HashMap::from([(
        "sample_key".to_string(),
        crate::common::Value::Number(42.5),
    )]));

    let report = run(&expanded, &catalog, &registries, &ctx).unwrap();
    assert_eq!(report.outputs.get("out"), Some(&RuntimeValue::Number(42.5)));
}

#[test]
fn context_number_source_runtime_uses_default_key_when_parameter_omitted() {
    let expanded = ExpandedGraph {
        nodes: HashMap::from([(
            "src".to_string(),
            ExpandedNode {
                runtime_id: "src".to_string(),
                authoring_path: vec![],
                implementation: crate::cluster::ImplementationInstance {
                    impl_id: "context_number_source".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::new(),
            },
        )]),
        edges: vec![],
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "out".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "src".to_string(),
                port_name: "value".to_string(),
            },
        }],
    };

    let catalog = build_core_catalog();
    let core = core_registries().unwrap();
    let registries = Registries {
        sources: &core.sources,
        computes: &core.computes,
        triggers: &core.triggers,
        actions: &core.actions,
    };

    let ctx = ExecutionContext::from_values(HashMap::from([(
        "x".to_string(),
        crate::common::Value::Number(7.0),
    )]));

    let report = run(&expanded, &catalog, &registries, &ctx).unwrap();
    assert_eq!(report.outputs.get("out"), Some(&RuntimeValue::Number(7.0)));
}

#[test]
fn context_bool_source_runtime_reads_custom_key_parameter() {
    let expanded = ExpandedGraph {
        nodes: HashMap::from([(
            "src".to_string(),
            ExpandedNode {
                runtime_id: "src".to_string(),
                authoring_path: vec![],
                implementation: crate::cluster::ImplementationInstance {
                    impl_id: "context_bool_source".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([(
                    "key".to_string(),
                    crate::cluster::ParameterValue::String("sample_key".to_string()),
                )]),
            },
        )]),
        edges: vec![],
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "out".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "src".to_string(),
                port_name: "value".to_string(),
            },
        }],
    };

    let catalog = build_core_catalog();
    let core = core_registries().unwrap();
    let registries = Registries {
        sources: &core.sources,
        computes: &core.computes,
        triggers: &core.triggers,
        actions: &core.actions,
    };

    let ctx = ExecutionContext::from_values(HashMap::from([(
        "sample_key".to_string(),
        crate::common::Value::Bool(true),
    )]));

    let report = run(&expanded, &catalog, &registries, &ctx).unwrap();
    assert_eq!(report.outputs.get("out"), Some(&RuntimeValue::Bool(true)));
}

#[test]
fn context_bool_source_runtime_uses_default_key_when_parameter_omitted() {
    let expanded = ExpandedGraph {
        nodes: HashMap::from([(
            "src".to_string(),
            ExpandedNode {
                runtime_id: "src".to_string(),
                authoring_path: vec![],
                implementation: crate::cluster::ImplementationInstance {
                    impl_id: "context_bool_source".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::new(),
            },
        )]),
        edges: vec![],
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "out".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "src".to_string(),
                port_name: "value".to_string(),
            },
        }],
    };

    let catalog = build_core_catalog();
    let core = core_registries().unwrap();
    let registries = Registries {
        sources: &core.sources,
        computes: &core.computes,
        triggers: &core.triggers,
        actions: &core.actions,
    };

    let ctx = ExecutionContext::from_values(HashMap::from([(
        "x".to_string(),
        crate::common::Value::Bool(true),
    )]));

    let report = run(&expanded, &catalog, &registries, &ctx).unwrap();
    assert_eq!(report.outputs.get("out"), Some(&RuntimeValue::Bool(true)));
}

#[test]
fn context_series_source_runtime_reads_custom_key_parameter() {
    let expanded = ExpandedGraph {
        nodes: HashMap::from([(
            "src".to_string(),
            ExpandedNode {
                runtime_id: "src".to_string(),
                authoring_path: vec![],
                implementation: crate::cluster::ImplementationInstance {
                    impl_id: "context_series_source".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([(
                    "key".to_string(),
                    crate::cluster::ParameterValue::String("sample_key".to_string()),
                )]),
            },
        )]),
        edges: vec![],
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "out".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "src".to_string(),
                port_name: "value".to_string(),
            },
        }],
    };

    let catalog = build_core_catalog();
    let core = core_registries().unwrap();
    let registries = Registries {
        sources: &core.sources,
        computes: &core.computes,
        triggers: &core.triggers,
        actions: &core.actions,
    };

    let ctx = ExecutionContext::from_values(HashMap::from([(
        "sample_key".to_string(),
        crate::common::Value::Series(vec![2.0, 4.0, 8.0]),
    )]));

    let report = run(&expanded, &catalog, &registries, &ctx).unwrap();
    assert_eq!(
        report.outputs.get("out"),
        Some(&RuntimeValue::Series(vec![2.0, 4.0, 8.0]))
    );
}

#[test]
fn context_series_source_runtime_uses_default_key_when_parameter_omitted() {
    let expanded = ExpandedGraph {
        nodes: HashMap::from([(
            "src".to_string(),
            ExpandedNode {
                runtime_id: "src".to_string(),
                authoring_path: vec![],
                implementation: crate::cluster::ImplementationInstance {
                    impl_id: "context_series_source".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::new(),
            },
        )]),
        edges: vec![],
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "out".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "src".to_string(),
                port_name: "value".to_string(),
            },
        }],
    };

    let catalog = build_core_catalog();
    let core = core_registries().unwrap();
    let registries = Registries {
        sources: &core.sources,
        computes: &core.computes,
        triggers: &core.triggers,
        actions: &core.actions,
    };

    let ctx = ExecutionContext::from_values(HashMap::from([(
        "x".to_string(),
        crate::common::Value::Series(vec![1.0, 1.5, 2.0]),
    )]));

    let report = run(&expanded, &catalog, &registries, &ctx).unwrap();
    assert_eq!(
        report.outputs.get("out"),
        Some(&RuntimeValue::Series(vec![1.0, 1.5, 2.0]))
    );
}

#[test]
fn series_stdlib_chain_persists_via_context_across_episodes() {
    let expanded = ExpandedGraph {
        nodes: HashMap::from([
            (
                "src_series".to_string(),
                ExpandedNode {
                    runtime_id: "src_series".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "context_series_source".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::from([(
                        "key".to_string(),
                        crate::cluster::ParameterValue::String("samples".to_string()),
                    )]),
                },
            ),
            (
                "src_num".to_string(),
                ExpandedNode {
                    runtime_id: "src_num".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "number_source".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::Number(5.0),
                    )]),
                },
            ),
            (
                "append".to_string(),
                ExpandedNode {
                    runtime_id: "append".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "append".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::new(),
                },
            ),
            (
                "window".to_string(),
                ExpandedNode {
                    runtime_id: "window".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "window".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::from([(
                        "size".to_string(),
                        crate::cluster::ParameterValue::Number(3.0),
                    )]),
                },
            ),
            (
                "mean".to_string(),
                ExpandedNode {
                    runtime_id: "mean".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "mean".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::new(),
                },
            ),
            (
                "gate".to_string(),
                ExpandedNode {
                    runtime_id: "gate".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "const_bool".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::Bool(true),
                    )]),
                },
            ),
            (
                "emit".to_string(),
                ExpandedNode {
                    runtime_id: "emit".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "emit_if_true".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::new(),
                },
            ),
            (
                "act".to_string(),
                ExpandedNode {
                    runtime_id: "act".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "context_set_series".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::from([(
                        "key".to_string(),
                        crate::cluster::ParameterValue::String("samples".to_string()),
                    )]),
                },
            ),
        ]),
        edges: vec![
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "src_series".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "append".to_string(),
                    port_name: "series".to_string(),
                },
            },
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "src_num".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "append".to_string(),
                    port_name: "value".to_string(),
                },
            },
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "append".to_string(),
                    port_name: "result".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "window".to_string(),
                    port_name: "series".to_string(),
                },
            },
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "window".to_string(),
                    port_name: "result".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "mean".to_string(),
                    port_name: "series".to_string(),
                },
            },
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "gate".to_string(),
                    port_name: "value".to_string(),
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
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "window".to_string(),
                    port_name: "result".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "act".to_string(),
                    port_name: "value".to_string(),
                },
            },
        ],
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![
            crate::cluster::OutputPortSpec {
                name: "window_out".to_string(),
                maps_to: crate::cluster::OutputRef {
                    node_id: "window".to_string(),
                    port_name: "result".to_string(),
                },
            },
            crate::cluster::OutputPortSpec {
                name: "mean_out".to_string(),
                maps_to: crate::cluster::OutputRef {
                    node_id: "mean".to_string(),
                    port_name: "result".to_string(),
                },
            },
        ],
    };

    let catalog = build_core_catalog();
    let core = core_registries().unwrap();
    let registries = Registries {
        sources: &core.sources,
        computes: &core.computes,
        triggers: &core.triggers,
        actions: &core.actions,
    };

    let first_ctx = ExecutionContext::from_values(HashMap::from([(
        "samples".to_string(),
        crate::common::Value::Series(vec![1.0, 2.0]),
    )]));
    let first = run(&expanded, &catalog, &registries, &first_ctx).unwrap();
    assert_eq!(
        first.outputs.get("window_out"),
        Some(&RuntimeValue::Series(vec![1.0, 2.0, 5.0]))
    );
    match first.outputs.get("mean_out") {
        Some(RuntimeValue::Number(mean)) => {
            assert!((*mean - (8.0 / 3.0)).abs() < 1e-9);
        }
        other => panic!("expected numeric mean_out, got {:?}", other),
    }
    assert_eq!(first.effects.len(), 1);
    assert_eq!(first.effects[0].writes.len(), 1);
    assert_eq!(first.effects[0].writes[0].key, "samples");
    assert_eq!(
        first.effects[0].writes[0].value,
        crate::common::Value::Series(vec![1.0, 2.0, 5.0])
    );

    let second_ctx = ExecutionContext::from_values(HashMap::from([(
        "samples".to_string(),
        first.effects[0].writes[0].value.clone(),
    )]));
    let second = run(&expanded, &catalog, &registries, &second_ctx).unwrap();
    assert_eq!(
        second.outputs.get("window_out"),
        Some(&RuntimeValue::Series(vec![2.0, 5.0, 5.0]))
    );
    assert_eq!(
        second.outputs.get("mean_out"),
        Some(&RuntimeValue::Number(4.0))
    );
    assert_eq!(
        second.effects[0].writes[0].value,
        crate::common::Value::Series(vec![2.0, 5.0, 5.0])
    );
}

#[test]
fn once_cluster_runtime_first_fire_then_suppresses_when_state_present() {
    let expanded = ExpandedGraph {
        nodes: HashMap::from([
            (
                "event_signal".to_string(),
                ExpandedNode {
                    runtime_id: "event_signal".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "const_bool".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::Bool(true),
                    )]),
                },
            ),
            (
                "emit_source_event".to_string(),
                ExpandedNode {
                    runtime_id: "emit_source_event".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "emit_if_true".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::new(),
                },
            ),
            (
                "state_source".to_string(),
                ExpandedNode {
                    runtime_id: "state_source".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "context_bool_source".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::from([(
                        "key".to_string(),
                        crate::cluster::ParameterValue::String("once_state".to_string()),
                    )]),
                },
            ),
            (
                "not_state".to_string(),
                ExpandedNode {
                    runtime_id: "not_state".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "not".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::new(),
                },
            ),
            (
                "gate_event".to_string(),
                ExpandedNode {
                    runtime_id: "gate_event".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "emit_if_event_and_true".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::new(),
                },
            ),
            (
                "set_value".to_string(),
                ExpandedNode {
                    runtime_id: "set_value".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "boolean_source".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::from([(
                        "value".to_string(),
                        crate::cluster::ParameterValue::Bool(true),
                    )]),
                },
            ),
            (
                "set_state".to_string(),
                ExpandedNode {
                    runtime_id: "set_state".to_string(),
                    authoring_path: vec![],
                    implementation: crate::cluster::ImplementationInstance {
                        impl_id: "context_set_bool".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::from([(
                        "key".to_string(),
                        crate::cluster::ParameterValue::String("once_state".to_string()),
                    )]),
                },
            ),
        ]),
        edges: vec![
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "event_signal".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "emit_source_event".to_string(),
                    port_name: "input".to_string(),
                },
            },
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "emit_source_event".to_string(),
                    port_name: "event".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "gate_event".to_string(),
                    port_name: "event".to_string(),
                },
            },
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "state_source".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "not_state".to_string(),
                    port_name: "value".to_string(),
                },
            },
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "not_state".to_string(),
                    port_name: "result".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "gate_event".to_string(),
                    port_name: "condition".to_string(),
                },
            },
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "gate_event".to_string(),
                    port_name: "event".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "set_state".to_string(),
                    port_name: "event".to_string(),
                },
            },
            crate::cluster::ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "set_value".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "set_state".to_string(),
                    port_name: "value".to_string(),
                },
            },
        ],
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![crate::cluster::OutputPortSpec {
            name: "event".to_string(),
            maps_to: crate::cluster::OutputRef {
                node_id: "gate_event".to_string(),
                port_name: "event".to_string(),
            },
        }],
    };

    let catalog = build_core_catalog();
    let core = core_registries().unwrap();
    let registries = Registries {
        sources: &core.sources,
        computes: &core.computes,
        triggers: &core.triggers,
        actions: &core.actions,
    };

    let first = run(
        &expanded,
        &catalog,
        &registries,
        &ExecutionContext::default(),
    )
    .unwrap();
    assert_eq!(
        first.outputs.get("event"),
        Some(&RuntimeValue::Event(
            crate::runtime::types::RuntimeEvent::Trigger(crate::trigger::TriggerEvent::Emitted),
        ))
    );
    assert_eq!(first.effects.len(), 1);
    assert_eq!(first.effects[0].kind, "set_context");
    assert_eq!(first.effects[0].writes.len(), 1);
    assert_eq!(first.effects[0].writes[0].key, "once_state");
    assert_eq!(
        first.effects[0].writes[0].value,
        crate::common::Value::Bool(true)
    );

    let second_ctx = ExecutionContext::from_values(HashMap::from([(
        "once_state".to_string(),
        crate::common::Value::Bool(true),
    )]));
    let second = run(&expanded, &catalog, &registries, &second_ctx).unwrap();
    assert_eq!(
        second.outputs.get("event"),
        Some(&RuntimeValue::Event(
            crate::runtime::types::RuntimeEvent::Trigger(crate::trigger::TriggerEvent::NotEmitted),
        ))
    );
    assert!(
        second.effects.is_empty(),
        "no context write should be emitted after state is already true"
    );
}
