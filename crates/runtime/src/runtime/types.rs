use std::collections::HashMap;

use crate::action::{ActionRegistry, ActionValidationError};
use crate::cluster::{InputMetadata, OutputMetadata, PrimitiveKind, ValueType};
use crate::common::Value;
use crate::compute::ComputeError;
use crate::compute::PrimitiveRegistry as ComputeRegistry;
use crate::source::SourceRegistry;
use crate::trigger::TriggerRegistry;

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeEvent {
    Trigger(crate::trigger::TriggerEvent),
    Action(crate::action::ActionOutcome),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeValue {
    Number(f64),
    Series(Vec<f64>),
    Bool(bool),
    Event(RuntimeEvent),
    String(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedNode {
    pub runtime_id: String,
    pub impl_id: String,
    pub version: String,
    pub kind: PrimitiveKind,
    /// Input metadata is used for validation only (required + type checks).
    pub inputs: Vec<InputMetadata>,
    pub outputs: HashMap<String, OutputMetadata>,
    pub parameters: HashMap<String, crate::cluster::ParameterValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedEdge {
    pub from: Endpoint,
    pub to: Endpoint,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Endpoint {
    NodePort { node_id: String, port_name: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedGraph {
    pub nodes: HashMap<String, ValidatedNode>,
    pub edges: Vec<ValidatedEdge>,
    pub topo_order: Vec<String>,
    pub boundary_outputs: Vec<crate::cluster::OutputPortSpec>,
}

#[derive(Debug)]
pub enum ValidationError {
    CycleDetected,
    UnknownNode(String),
    MissingPrimitive {
        id: String,
        version: String,
    },
    InvalidEdgeKind {
        from: PrimitiveKind,
        to: PrimitiveKind,
    },
    MissingRequiredInput {
        node: String,
        input: String,
    },
    MissingInputMetadata {
        node: String,
        input: String,
    },
    TypeMismatch {
        from: String,
        output: String,
        to: String,
        input: String,
        expected: ValueType,
        got: ValueType,
    },
    ActionNotGated(String),
    MissingOutputMetadata {
        node: String,
        output: String,
    },
    ExternalInputNotAllowed {
        name: String,
    },
    /// V.MULTI-EDGE: Multiple edges targeting the same input port.
    /// All inputs currently have Cardinality::Single; fan-in is not supported.
    MultipleInboundEdges {
        node: String,
        input: String,
    },
}

#[derive(Debug)]
pub enum ExecError {
    UnknownPrimitive {
        id: String,
        version: String,
    },
    TypeConversionFailed {
        node: String,
        port: String,
    },
    ParameterTypeConversionFailed {
        node: String,
        parameter: String,
    },
    /// X.11: Int parameter value exceeds f64 exact representation range (|i| > 2^53).
    ParameterOutOfRange {
        node: String,
        parameter: String,
        value: i64,
    },
    ComputeFailed {
        node: String,
        id: String,
        version: String,
        error: ComputeError,
    },
    NonFiniteOutput {
        node: String,
        port: String,
    },
    ActionExecutionFailed(ActionValidationError),
    MissingOutput {
        node: String,
        output: String,
    },
    MissingNode {
        node: String,
    },
}

/// Execution context for runtime invocation.
/// Adapter-provided context values are exposed to context-aware sources.
#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    values: HashMap<String, Value>,
}

impl ExecutionContext {
    pub fn from_values(values: HashMap<String, Value>) -> Self {
        Self { values }
    }

    pub fn value(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }
}

pub struct Registries<'a> {
    pub sources: &'a SourceRegistry,
    pub computes: &'a ComputeRegistry,
    pub triggers: &'a TriggerRegistry,
    pub actions: &'a ActionRegistry,
}

#[derive(Debug)]
pub struct ExecutionReport {
    pub outputs: HashMap<String, RuntimeValue>,
}

impl RuntimeValue {
    pub fn value_type(&self) -> ValueType {
        match self {
            RuntimeValue::Number(_) => ValueType::Number,
            RuntimeValue::Series(_) => ValueType::Series,
            RuntimeValue::Bool(_) => ValueType::Bool,
            RuntimeValue::Event(_) => ValueType::Event,
            RuntimeValue::String(_) => ValueType::String,
        }
    }
}

impl ValidatedNode {
    pub fn required_inputs(&self) -> impl Iterator<Item = &InputMetadata> {
        self.inputs.iter().filter(|i| i.required)
    }
}
