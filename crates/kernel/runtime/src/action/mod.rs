use std::borrow::Cow;
use std::collections::HashMap;

use crate::common::{ErrorInfo, Phase, ValueType};

pub mod implementations;
pub mod registry;

#[derive(Debug, Clone, PartialEq)]
pub enum ActionKind {
    Action,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionValueType {
    Event,
    Number,
    Series,
    Bool,
    String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionOutcome {
    Attempted,
    /// Action executed successfully and completed.
    /// NOTE: If serialization is introduced, add backward-compat serde alias for legacy "Filled".
    Completed,
    Rejected,
    Cancelled,
    Failed,
    /// Action was never attempted because gating trigger emitted NotEmitted.
    Skipped,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionValue {
    Event(ActionOutcome),
    Number(f64),
    Series(Vec<f64>),
    Bool(bool),
    String(String),
}

impl ActionValue {
    pub fn value_type(&self) -> ActionValueType {
        match self {
            ActionValue::Event(_) => ActionValueType::Event,
            ActionValue::Number(_) => ActionValueType::Number,
            ActionValue::Series(_) => ActionValueType::Series,
            ActionValue::Bool(_) => ActionValueType::Bool,
            ActionValue::String(_) => ActionValueType::String,
        }
    }

    pub fn as_event(&self) -> Option<&ActionOutcome> {
        match self {
            ActionValue::Event(e) => Some(e),
            _ => None,
        }
    }

    pub fn as_series(&self) -> Option<&Vec<f64>> {
        match self {
            ActionValue::Series(series) => Some(series),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParameterType {
    Int,
    Number,
    Bool,
    String,
    Enum,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParameterValue {
    Int(i64),
    Number(f64),
    Bool(bool),
    String(String),
    Enum(String),
}

impl ParameterValue {
    pub fn value_type(&self) -> ParameterType {
        match self {
            ParameterValue::Int(_) => ParameterType::Int,
            ParameterValue::Number(_) => ParameterType::Number,
            ParameterValue::Bool(_) => ParameterType::Bool,
            ParameterValue::String(_) => ParameterType::String,
            ParameterValue::Enum(_) => ParameterType::Enum,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cardinality {
    Single,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputSpec {
    pub name: String,
    pub value_type: ActionValueType,
    pub required: bool,
    pub cardinality: Cardinality,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputSpec {
    pub name: String,
    pub value_type: ActionValueType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParameterSpec {
    pub name: String,
    pub value_type: ParameterType,
    pub default: Option<ParameterValue>,
    pub required: bool,
    pub bounds: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionWriteSpec {
    pub name: String,
    pub value_type: ValueType,
    pub from_input: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ActionEffects {
    pub writes: Vec<ActionWriteSpec>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionSpec {
    pub deterministic: bool,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateSpec {
    pub allowed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionPrimitiveManifest {
    pub id: String,
    pub version: String,
    pub kind: ActionKind,
    pub inputs: Vec<InputSpec>,
    pub outputs: Vec<OutputSpec>,
    pub parameters: Vec<ParameterSpec>,
    pub effects: ActionEffects,
    pub execution: ExecutionSpec,
    pub state: StateSpec,
    pub side_effects: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ActionState {
    pub data: HashMap<String, ActionValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionValidationError {
    InvalidId {
        id: String,
    },
    InvalidVersion {
        version: String,
    },
    WrongKind {
        expected: ActionKind,
        got: ActionKind,
    },
    SideEffectsRequired,
    NonDeterministicExecution,
    RetryNotAllowed,
    StateNotAllowed,
    DuplicateId(String),
    DuplicateInput {
        name: String,
        first_index: usize,
        second_index: usize,
    },
    EventInputRequired,
    DuplicateWriteName {
        name: String,
        first_index: usize,
        second_index: usize,
    },
    InvalidWriteType {
        name: String,
        got: ValueType,
    },
    InvalidInputType {
        input: String,
        expected: ActionValueType,
        got: ActionValueType,
    },
    OutputNotOutcome {
        name: String,
        index: usize,
    },
    InvalidOutputType {
        output: String,
        expected: ActionValueType,
        got: ActionValueType,
    },
    UndeclaredOutput {
        primitive: String,
        output: String,
    },
    InvalidParameterType {
        parameter: String,
        expected: ParameterType,
        got: ParameterType,
    },
    UnboundWriteKeyReference {
        name: String,
        referenced_param: String,
    },
    WriteKeyReferenceNotString {
        name: String,
        referenced_param: String,
    },
    WriteFromInputNotFound {
        write_name: String,
        from_input: String,
    },
    WriteFromInputTypeMismatch {
        write_name: String,
        from_input: String,
        expected: ValueType,
        found: ActionValueType,
    },
}

impl ErrorInfo for ActionValidationError {
    fn rule_id(&self) -> &'static str {
        match self {
            Self::InvalidId { .. } => "ACT-1",
            Self::InvalidVersion { .. } => "ACT-2",
            Self::WrongKind { .. } => "ACT-3",
            Self::EventInputRequired => "ACT-4",
            Self::DuplicateInput { .. } => "ACT-5",
            Self::InvalidInputType { .. } => "ACT-6",
            Self::UndeclaredOutput { .. } => "ACT-7",
            Self::OutputNotOutcome { .. } => "ACT-8",
            Self::InvalidOutputType { .. } => "ACT-9",
            Self::StateNotAllowed => "ACT-10",
            Self::SideEffectsRequired => "ACT-11",
            Self::DuplicateWriteName { .. } => "ACT-14",
            Self::InvalidWriteType { .. } => "ACT-15",
            Self::RetryNotAllowed => "ACT-16",
            Self::NonDeterministicExecution => "ACT-17",
            Self::DuplicateId(_) => "ACT-18",
            Self::InvalidParameterType { .. } => "ACT-19",
            Self::UnboundWriteKeyReference { .. } => "ACT-20",
            Self::WriteKeyReferenceNotString { .. } => "ACT-21",
            Self::WriteFromInputNotFound { .. } => "ACT-22",
            Self::WriteFromInputTypeMismatch { .. } => "ACT-23",
        }
    }

    fn phase(&self) -> Phase {
        Phase::Registration
    }

    fn doc_anchor(&self) -> &'static str {
        match self.rule_id() {
            "ACT-1" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-1",
            "ACT-2" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-2",
            "ACT-3" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-3",
            "ACT-4" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-4",
            "ACT-5" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-5",
            "ACT-6" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-6",
            "ACT-7" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-7",
            "ACT-8" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-8",
            "ACT-9" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-9",
            "ACT-10" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-10",
            "ACT-11" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-11",
            "ACT-14" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-14",
            "ACT-15" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-15",
            "ACT-16" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-16",
            "ACT-17" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-17",
            "ACT-18" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-18",
            "ACT-19" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-19",
            "ACT-20" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-20",
            "ACT-21" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-21",
            "ACT-22" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-22",
            "ACT-23" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-23",
            _ => "CANONICAL/PHASE_INVARIANTS.md",
        }
    }

    fn summary(&self) -> Cow<'static, str> {
        match self {
            Self::InvalidId { id } => Cow::Owned(format!("Invalid action ID: '{}'", id)),
            Self::InvalidVersion { version } => {
                Cow::Owned(format!("Invalid version: '{}'", version))
            }
            Self::WrongKind { expected, got } => Cow::Owned(format!(
                "Wrong kind: expected {:?}, got {:?}",
                expected, got
            )),
            Self::SideEffectsRequired => Cow::Borrowed("Actions must declare side effects"),
            Self::NonDeterministicExecution => {
                Cow::Borrowed("Action execution must be deterministic")
            }
            Self::RetryNotAllowed => Cow::Borrowed("Action retryable must be false"),
            Self::StateNotAllowed => Cow::Borrowed("Action state is not allowed"),
            Self::DuplicateId(_) => Cow::Borrowed("Duplicate action ID: already registered"),
            Self::DuplicateInput { name, .. } => {
                Cow::Owned(format!("Duplicate input name: '{}'", name))
            }
            Self::EventInputRequired => Cow::Borrowed("Action requires at least one event input"),
            Self::DuplicateWriteName { name, .. } => {
                Cow::Owned(format!("Duplicate write name: '{}'", name))
            }
            Self::InvalidWriteType { name, got } => {
                Cow::Owned(format!("Write '{}' has invalid type {:?}", name, got))
            }
            Self::InvalidInputType {
                input,
                expected,
                got,
            } => Cow::Owned(format!(
                "Input '{}' has invalid type: expected {:?}, got {:?}",
                input, expected, got
            )),
            Self::OutputNotOutcome { name, .. } => Cow::Owned(format!(
                "Action output must be named 'outcome', got '{}'",
                name
            )),
            Self::InvalidOutputType {
                output,
                expected,
                got,
            } => Cow::Owned(format!(
                "Output '{}' has invalid type: expected {:?}, got {:?}",
                output, expected, got
            )),
            Self::UndeclaredOutput { primitive, output } => Cow::Owned(format!(
                "Undeclared output '{}' on primitive '{}'",
                output, primitive
            )),
            Self::InvalidParameterType {
                parameter,
                expected,
                got,
            } => Cow::Owned(format!(
                "Parameter '{}' has invalid type: expected {:?}, got {:?}",
                parameter, expected, got
            )),
            Self::UnboundWriteKeyReference {
                name,
                referenced_param,
            } => Cow::Owned(format!(
                "Write key '{}' references undefined parameter '{}'",
                name, referenced_param
            )),
            Self::WriteKeyReferenceNotString {
                name,
                referenced_param,
            } => Cow::Owned(format!(
                "Write key '{}' references parameter '{}' which is not String type",
                name, referenced_param
            )),
            Self::WriteFromInputNotFound {
                write_name,
                from_input,
            } => Cow::Owned(format!(
                "Write '{}' references undeclared input '{}'",
                write_name, from_input
            )),
            Self::WriteFromInputTypeMismatch {
                write_name,
                from_input,
                expected,
                found,
            } => Cow::Owned(format!(
                "Write '{}' type {:?} does not match input '{}' type {:?}",
                write_name, expected, from_input, found
            )),
        }
    }

    fn path(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::InvalidId { .. } => Some(Cow::Borrowed("$.id")),
            Self::InvalidVersion { .. } => Some(Cow::Borrowed("$.version")),
            Self::DuplicateId(_) => Some(Cow::Borrowed("$.id")),
            Self::WrongKind { .. } => Some(Cow::Borrowed("$.kind")),
            Self::EventInputRequired => Some(Cow::Borrowed("$.inputs")),
            Self::DuplicateInput { second_index, .. } => {
                Some(Cow::Owned(format!("$.inputs[{}].name", second_index)))
            }
            Self::InvalidInputType { .. } => Some(Cow::Borrowed("$.inputs[].type")),
            Self::UndeclaredOutput { .. } => Some(Cow::Borrowed("$.outputs")),
            Self::OutputNotOutcome { index, .. } => {
                Some(Cow::Owned(format!("$.outputs[{}].name", index)))
            }
            Self::InvalidOutputType { .. } => Some(Cow::Borrowed("$.outputs[0].type")),
            Self::StateNotAllowed => Some(Cow::Borrowed("$.state.allowed")),
            Self::SideEffectsRequired => Some(Cow::Borrowed("$.side_effects")),
            Self::DuplicateWriteName { second_index, .. } => Some(Cow::Owned(format!(
                "$.effects.writes[{}].name",
                second_index
            ))),
            Self::InvalidWriteType { .. } => Some(Cow::Borrowed("$.effects.writes[].type")),
            Self::RetryNotAllowed => Some(Cow::Borrowed("$.execution.retryable")),
            Self::NonDeterministicExecution => Some(Cow::Borrowed("$.execution.deterministic")),
            Self::InvalidParameterType { .. } => Some(Cow::Borrowed("$.parameters[].default")),
            Self::UnboundWriteKeyReference { .. } => Some(Cow::Borrowed("$.effects.writes[].name")),
            Self::WriteKeyReferenceNotString { .. } => {
                Some(Cow::Borrowed("$.effects.writes[].name"))
            }
            Self::WriteFromInputNotFound { .. } => {
                Some(Cow::Borrowed("$.effects.writes[].from_input"))
            }
            Self::WriteFromInputTypeMismatch { .. } => {
                Some(Cow::Borrowed("$.effects.writes[].from_input"))
            }
        }
    }

    fn fix(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::InvalidId { .. } => Some(Cow::Borrowed(
                "ID must start with lowercase letter and contain only lowercase letters, digits, and underscores",
            )),
            Self::DuplicateId(_) => Some(Cow::Borrowed("Choose a unique ID not already registered")),
            Self::InvalidVersion { .. } => Some(Cow::Borrowed(
                "Version must be valid semver (e.g., '1.0.0')",
            )),
            Self::WrongKind { .. } => Some(Cow::Borrowed("Set kind: action")),
            Self::EventInputRequired => Some(Cow::Borrowed("Add at least one event input")),
            Self::DuplicateInput { name, .. } => Some(Cow::Owned(format!(
                "Rename input '{}' to a unique value",
                name
            ))),
            Self::InvalidInputType { .. } => Some(Cow::Borrowed(
                "Use a valid input type: event, number, series, bool, or string",
            )),
            Self::UndeclaredOutput { .. } => Some(Cow::Borrowed("Declare a single outcome output")),
            Self::OutputNotOutcome { .. } => Some(Cow::Borrowed("Rename output to 'outcome'")),
            Self::InvalidOutputType { .. } => Some(Cow::Borrowed("Output type must be event")),
            Self::StateNotAllowed => Some(Cow::Borrowed("Set state.allowed: false")),
            Self::SideEffectsRequired => Some(Cow::Borrowed("Set side_effects: true")),
            Self::DuplicateWriteName { name, .. } => Some(Cow::Owned(format!(
                "Rename write '{}' to a unique value",
                name
            ))),
            Self::InvalidWriteType { .. } => Some(Cow::Borrowed(
                "Write types must be Number, Series, Bool, or String",
            )),
            Self::RetryNotAllowed => Some(Cow::Borrowed("Set execution.retryable: false")),
            Self::NonDeterministicExecution => {
                Some(Cow::Borrowed("Set execution.deterministic: true"))
            }
            Self::InvalidParameterType { .. } => Some(Cow::Borrowed(
                "Change parameter default value to match the declared parameter type",
            )),
            Self::UnboundWriteKeyReference {
                referenced_param, ..
            } => Some(Cow::Owned(format!(
                "Add parameter '{}' to the action manifest",
                referenced_param
            ))),
            Self::WriteKeyReferenceNotString {
                referenced_param, ..
            } => Some(Cow::Owned(format!(
                "Change parameter '{}' type to String",
                referenced_param
            ))),
            Self::WriteFromInputNotFound { from_input, .. } => Some(Cow::Owned(format!(
                "Declare input '{}' in the action manifest inputs",
                from_input
            ))),
            Self::WriteFromInputTypeMismatch {
                from_input,
                expected,
                ..
            } => Some(Cow::Owned(format!(
                "Change input '{}' type to match write type {:?}, or use a scalar-typed input",
                from_input, expected
            ))),
        }
    }
}

pub trait ActionPrimitive {
    fn manifest(&self) -> &ActionPrimitiveManifest;

    fn execute(
        &self,
        inputs: &HashMap<String, ActionValue>,
        parameters: &HashMap<String, ParameterValue>,
    ) -> HashMap<String, ActionValue>;
}

pub use implementations::{
    AckAction, AnnotateAction, ContextSetBoolAction, ContextSetNumberAction,
    ContextSetSeriesAction, ContextSetStringAction,
};
pub use registry::ActionRegistry;

#[cfg(test)]
mod tests;
