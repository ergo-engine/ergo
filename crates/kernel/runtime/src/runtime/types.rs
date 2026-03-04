use std::borrow::Cow;
use std::collections::HashMap;

use crate::action::ActionRegistry;
use crate::cluster::{InputMetadata, OutputMetadata, PrimitiveKind, ValueType};
use crate::common::{ErrorInfo, Phase, Value};
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

impl ErrorInfo for ValidationError {
    fn rule_id(&self) -> &'static str {
        match self {
            Self::CycleDetected => "V.1",
            Self::InvalidEdgeKind { .. } => "V.2",
            Self::MissingRequiredInput { .. } => "V.3",
            Self::TypeMismatch { .. } => "V.4",
            Self::ActionNotGated(_) => "V.5",
            Self::MultipleInboundEdges { .. } => "V.7",
            Self::MissingPrimitive { .. } => "V.8",
            Self::UnknownNode(_)
            | Self::MissingInputMetadata { .. }
            | Self::MissingOutputMetadata { .. } => "D.2",
            Self::ExternalInputNotAllowed { .. } => "E.3",
        }
    }

    fn phase(&self) -> Phase {
        Phase::Composition
    }

    fn doc_anchor(&self) -> &'static str {
        match self.rule_id() {
            "D.2" => "STABLE/CLUSTER_SPEC.md#D.2",
            "E.3" => "STABLE/CLUSTER_SPEC.md#E.3",
            "V.1" => "STABLE/CLUSTER_SPEC.md#V.1",
            "V.2" => "STABLE/CLUSTER_SPEC.md#V.2",
            "V.3" => "STABLE/CLUSTER_SPEC.md#V.3",
            "V.4" => "STABLE/CLUSTER_SPEC.md#V.4",
            "V.5" => "STABLE/CLUSTER_SPEC.md#V.5",
            "V.7" => "STABLE/CLUSTER_SPEC.md#V.7",
            "V.8" => "STABLE/CLUSTER_SPEC.md#V.8",
            _ => "CANONICAL/PHASE_INVARIANTS.md",
        }
    }

    fn summary(&self) -> Cow<'static, str> {
        match self {
            Self::CycleDetected => Cow::Borrowed("Cycle detected in graph"),
            Self::UnknownNode(node) => Cow::Owned(format!("Unknown node '{}'", node)),
            Self::MissingPrimitive { id, version } => {
                Cow::Owned(format!("Missing primitive '{}@{}'", id, version))
            }
            Self::InvalidEdgeKind { from, to } => {
                Cow::Owned(format!("Invalid edge kind: {:?} -> {:?}", from, to))
            }
            Self::MissingRequiredInput { node, input } => Cow::Owned(format!(
                "Missing required input '{}' on node '{}'",
                input, node
            )),
            Self::MissingInputMetadata { node, input } => Cow::Owned(format!(
                "Missing input metadata '{}' on node '{}'",
                input, node
            )),
            Self::TypeMismatch {
                from,
                output,
                to,
                input,
                expected,
                got,
            } => Cow::Owned(format!(
                "Type mismatch {}.{} -> {}.{} (expected {:?}, got {:?})",
                from, output, to, input, expected, got
            )),
            Self::ActionNotGated(node) => {
                Cow::Owned(format!("Action '{}' is not gated by a trigger", node))
            }
            Self::MissingOutputMetadata { node, output } => Cow::Owned(format!(
                "Missing output metadata '{}' on node '{}'",
                output, node
            )),
            Self::ExternalInputNotAllowed { name } => Cow::Owned(format!(
                "External input '{}' is not allowed in execution graph",
                name
            )),
            Self::MultipleInboundEdges { node, input } => {
                Cow::Owned(format!("Multiple inbound edges to '{}.{}'", node, input))
            }
        }
    }

    fn path(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::CycleDetected => Some(Cow::Borrowed("$.edges")),
            Self::InvalidEdgeKind { .. } => Some(Cow::Borrowed("$.edges")),
            Self::MissingRequiredInput { .. } => Some(Cow::Borrowed("$.edges")),
            Self::TypeMismatch { .. } => Some(Cow::Borrowed("$.edges")),
            Self::ActionNotGated(_) => Some(Cow::Borrowed("$.edges")),
            Self::MultipleInboundEdges { .. } => Some(Cow::Borrowed("$.edges")),
            Self::ExternalInputNotAllowed { .. } => Some(Cow::Borrowed("$.edges")),
            Self::UnknownNode(_)
            | Self::MissingPrimitive { .. }
            | Self::MissingInputMetadata { .. }
            | Self::MissingOutputMetadata { .. } => Some(Cow::Borrowed("$.nodes")),
        }
    }

    fn fix(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::CycleDetected => Some(Cow::Borrowed("Remove the cycle in the graph")),
            Self::UnknownNode(_) => Some(Cow::Borrowed("Remove edges referencing missing nodes")),
            Self::MissingPrimitive { .. } => Some(Cow::Borrowed(
                "Register the referenced primitive implementation",
            )),
            Self::InvalidEdgeKind { .. } => Some(Cow::Borrowed("Remove the invalid edge")),
            Self::MissingRequiredInput { .. } => Some(Cow::Borrowed(
                "Connect the required input or mark it optional",
            )),
            Self::MissingInputMetadata { .. } => {
                Some(Cow::Borrowed("Ensure input metadata exists"))
            }
            Self::TypeMismatch { .. } => {
                Some(Cow::Borrowed("Ensure connected ports share the same type"))
            }
            Self::ActionNotGated(_) => Some(Cow::Borrowed("Gate the action with a trigger output")),
            Self::MissingOutputMetadata { .. } => {
                Some(Cow::Borrowed("Ensure output metadata exists"))
            }
            Self::ExternalInputNotAllowed { .. } => Some(Cow::Borrowed(
                "Remove external inputs; use source nodes instead",
            )),
            Self::MultipleInboundEdges { .. } => {
                Some(Cow::Borrowed("Allow only one inbound edge per input"))
            }
        }
    }
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
    MissingRequiredContextKey {
        node: String,
        key: String,
    },
    ContextKeyTypeMismatch {
        node: String,
        key: String,
        expected: crate::common::ValueType,
        got: crate::common::ValueType,
    },
    MissingOutput {
        node: String,
        output: String,
    },
    MissingNode {
        node: String,
    },
}

impl ErrorInfo for ExecError {
    fn rule_id(&self) -> &'static str {
        match self {
            Self::TypeConversionFailed { .. } => "V.4",
            Self::ParameterTypeConversionFailed { .. } => "I.4",
            Self::MissingOutput { .. } => "CMP-11",
            Self::ComputeFailed { .. } => "CMP-12",
            Self::NonFiniteOutput { .. } => "NUM-FINITE-1",
            Self::ParameterOutOfRange { .. } => "X.11",
            Self::MissingRequiredContextKey { .. } => "SRC-10",
            Self::ContextKeyTypeMismatch { .. } => "SRC-11",
            Self::UnknownPrimitive { .. } => "INTERNAL",
            Self::MissingNode { .. } => "INTERNAL",
        }
    }

    fn phase(&self) -> Phase {
        Phase::Execution
    }

    fn doc_anchor(&self) -> &'static str {
        match self.rule_id() {
            "CMP-11" => "STABLE/PRIMITIVE_MANIFESTS/compute.md#CMP-11",
            "CMP-12" => "STABLE/PRIMITIVE_MANIFESTS/compute.md#CMP-12",
            "SRC-10" => "STABLE/PRIMITIVE_MANIFESTS/source.md#SRC-10",
            "SRC-11" => "STABLE/PRIMITIVE_MANIFESTS/source.md#SRC-11",
            "V.4" => "STABLE/CLUSTER_SPEC.md#V.4",
            "I.4" => "STABLE/CLUSTER_SPEC.md#I.4",
            "NUM-FINITE-1" => "CANONICAL/PHASE_INVARIANTS.md#NUM-FINITE-1",
            "X.11" => "CANONICAL/PHASE_INVARIANTS.md#X.11",
            _ => "CANONICAL/PHASE_INVARIANTS.md",
        }
    }

    fn summary(&self) -> Cow<'static, str> {
        match self {
            Self::UnknownPrimitive { id, version } => {
                Cow::Owned(format!("Unknown primitive '{}@{}'", id, version))
            }
            Self::TypeConversionFailed { node, port } => {
                Cow::Owned(format!("Type conversion failed at '{}.{}'", node, port))
            }
            Self::ParameterTypeConversionFailed { node, parameter } => Cow::Owned(format!(
                "Parameter type conversion failed at '{}.{}'",
                node, parameter
            )),
            Self::ParameterOutOfRange {
                node,
                parameter,
                value,
            } => Cow::Owned(format!(
                "Parameter '{}.{}' out of range (value {})",
                node, parameter, value
            )),
            Self::ComputeFailed {
                node,
                id,
                version,
                error,
            } => Cow::Owned(format!(
                "Compute '{}' ({}@{}) failed: {:?}",
                node, id, version, error
            )),
            Self::NonFiniteOutput { node, port } => {
                Cow::Owned(format!("Non-finite numeric output at '{}.{}'", node, port))
            }
            Self::MissingRequiredContextKey { node, key } => Cow::Owned(format!(
                "Missing required context key '{}' for source node '{}'",
                key, node
            )),
            Self::ContextKeyTypeMismatch {
                node,
                key,
                expected,
                got,
            } => Cow::Owned(format!(
                "Context key '{}' type mismatch for source node '{}': expected {:?}, got {:?}",
                key, node, expected, got
            )),
            Self::MissingOutput { node, output } => Cow::Owned(format!(
                "Missing declared output '{}' on node '{}'",
                output, node
            )),
            Self::MissingNode { node } => Cow::Owned(format!("Missing node '{}'", node)),
        }
    }

    fn path(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::MissingRequiredContextKey { key, .. }
            | Self::ContextKeyTypeMismatch { key, .. } => {
                Some(Cow::Owned(format!("$.context.{}", key)))
            }
            Self::ComputeFailed { node, .. } => Some(Cow::Owned(format!("$.nodes.{}", node))),
            Self::ParameterOutOfRange {
                node, parameter, ..
            } => Some(Cow::Owned(format!(
                "$.nodes.{}.parameters.{}",
                node, parameter
            ))),
            Self::NonFiniteOutput { node, port } => {
                Some(Cow::Owned(format!("$.nodes.{}.outputs.{}", node, port)))
            }
            Self::MissingOutput { node, output } => {
                Some(Cow::Owned(format!("$.nodes.{}.outputs.{}", node, output)))
            }
            _ => None,
        }
    }

    fn fix(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::MissingRequiredContextKey { key, .. } => Some(Cow::Owned(format!(
                "Provide required context key '{}' via adapter, or mark it required: false in the source manifest",
                key
            ))),
            Self::ContextKeyTypeMismatch { key, expected, .. } => Some(Cow::Owned(format!(
                "Provide context key '{}' with type {:?}",
                key, expected
            ))),
            Self::MissingOutput { output, .. } => Some(Cow::Owned(format!(
                "Ensure the compute implementation produces output '{}' on success",
                output
            ))),
            Self::ComputeFailed { .. } => Some(Cow::Borrowed(
                "Handle the compute error or adjust inputs/parameters to avoid it",
            )),
            Self::NonFiniteOutput { .. } => Some(Cow::Borrowed(
                "Ensure all numeric outputs are finite (not NaN/inf)",
            )),
            Self::ParameterOutOfRange { .. } => Some(Cow::Borrowed(
                "Use an Int parameter within f64 exact range (|i| <= 2^53)",
            )),
            _ => None,
        }
    }
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
    pub effects: Vec<crate::common::ActionEffect>,
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

#[cfg(test)]
mod tests {
    use super::{ExecError, ValidationError};
    use crate::cluster::PrimitiveKind;
    use crate::common::ErrorInfo;

    #[test]
    fn v8_missing_primitive_maps_to_v8() {
        let err = ValidationError::MissingPrimitive {
            id: "missing".to_string(),
            version: "0.1.0".to_string(),
        };

        assert_eq!(err.rule_id(), "V.8");
        assert_eq!(err.doc_anchor(), "STABLE/CLUSTER_SPEC.md#V.8");
    }

    #[test]
    fn exec_type_conversion_maps_to_v4() {
        let err = ExecError::TypeConversionFailed {
            node: "n".to_string(),
            port: "p".to_string(),
        };

        assert_eq!(err.rule_id(), "V.4");
        assert_eq!(err.doc_anchor(), "STABLE/CLUSTER_SPEC.md#V.4");
    }

    #[test]
    fn exec_parameter_type_conversion_maps_to_i4() {
        let err = ExecError::ParameterTypeConversionFailed {
            node: "n".to_string(),
            parameter: "x".to_string(),
        };

        assert_eq!(err.rule_id(), "I.4");
        assert_eq!(err.doc_anchor(), "STABLE/CLUSTER_SPEC.md#I.4");
    }

    #[test]
    fn exec_internal_missing_node_is_explicit() {
        let err = ExecError::MissingNode {
            node: "ghost".to_string(),
        };

        assert_eq!(err.rule_id(), "INTERNAL");
        assert_eq!(err.phase(), crate::common::Phase::Execution);
        assert_eq!(err.doc_anchor(), "CANONICAL/PHASE_INVARIANTS.md");
    }

    #[test]
    fn validation_known_rules_unchanged() {
        let err = ValidationError::InvalidEdgeKind {
            from: PrimitiveKind::Source,
            to: PrimitiveKind::Action,
        };
        assert_eq!(err.rule_id(), "V.2");
    }
}
