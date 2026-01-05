use std::collections::HashMap;

pub mod graph;
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
    Bool(bool),
    String(String),
}

impl ActionValue {
    pub fn value_type(&self) -> ActionValueType {
        match self {
            ActionValue::Event(_) => ActionValueType::Event,
            ActionValue::Number(_) => ActionValueType::Number,
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
    pub bounds: Option<String>,
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
    WrongKind {
        expected: ActionKind,
        got: ActionKind,
    },
    SideEffectsRequired,
    NonDeterministicExecution,
    RetryNotAllowed,
    StateNotAllowed,
    DuplicateId(String),
    EventInputRequired,
    InvalidInputType {
        input: String,
        expected: ActionValueType,
        got: ActionValueType,
    },
    InvalidOutputType {
        output: String,
        expected: ActionValueType,
        got: ActionValueType,
    },
    MissingRequiredInput(String),
    UndeclaredInput {
        node: String,
        input: String,
    },
    UndeclaredOutput {
        primitive: String,
        output: String,
    },
    MissingDeclaredOutput {
        primitive: String,
        output: String,
    },
    UndeclaredParameter {
        node: String,
        parameter: String,
    },
    InvalidParameterType {
        parameter: String,
        expected: ParameterType,
        got: ParameterType,
    },
    UnknownPrimitive(String),
    CycleDetected,
    MissingNode(String),
    MissingOutput {
        node: String,
        output: String,
    },
    ActionChainingNotAllowed {
        node: String,
        input: String,
    },
}

pub trait ActionPrimitive {
    fn manifest(&self) -> &ActionPrimitiveManifest;

    fn execute(
        &self,
        inputs: &HashMap<String, ActionValue>,
        parameters: &HashMap<String, ParameterValue>,
    ) -> HashMap<String, ActionValue>;
}

pub use graph::{ActionGraph, ActionNode, InputBinding, NodeOutputRef};
pub use implementations::{AckAction, AnnotateAction};
pub use registry::ActionRegistry;

#[cfg(test)]
mod tests;
