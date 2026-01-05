use std::collections::HashMap;

pub mod implementations;
pub mod registry;

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerKind {
    Trigger,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerValueType {
    Number,
    Series,
    Bool,
    Event,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerEvent {
    Emitted,
    NotEmitted,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerValue {
    Number(f64),
    Series(Vec<f64>),
    Bool(bool),
    Event(TriggerEvent),
}

impl TriggerValue {
    pub fn value_type(&self) -> TriggerValueType {
        match self {
            TriggerValue::Number(_) => TriggerValueType::Number,
            TriggerValue::Series(_) => TriggerValueType::Series,
            TriggerValue::Bool(_) => TriggerValueType::Bool,
            TriggerValue::Event(_) => TriggerValueType::Event,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            TriggerValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            TriggerValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_event(&self) -> Option<&TriggerEvent> {
        match self {
            TriggerValue::Event(e) => Some(e),
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
    Multiple,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputSpec {
    pub name: String,
    pub value_type: TriggerValueType,
    pub required: bool,
    pub cardinality: Cardinality,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputSpec {
    pub name: String,
    pub value_type: TriggerValueType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParameterSpec {
    pub name: String,
    pub value_type: ParameterType,
    pub default: Option<ParameterValue>,
    pub bounds: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cadence {
    Continuous,
    Event,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionSpec {
    pub deterministic: bool,
    pub cadence: Cadence,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateSpec {
    pub allowed: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TriggerPrimitiveManifest {
    pub id: String,
    pub version: String,
    pub kind: TriggerKind,
    pub inputs: Vec<InputSpec>,
    pub outputs: Vec<OutputSpec>,
    pub parameters: Vec<ParameterSpec>,
    pub execution: ExecutionSpec,
    pub state: StateSpec,
    pub side_effects: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerValidationError {
    WrongKind {
        expected: TriggerKind,
        got: TriggerKind,
    },
    SideEffectsNotAllowed,
    NonDeterministicExecution,
    DuplicateId(String),
    InvalidInputType {
        input: String,
        expected: TriggerValueType,
        got: TriggerValueType,
    },
    InvalidOutputType {
        output: String,
        expected: TriggerValueType,
        got: TriggerValueType,
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
    /// TRG-STATE-1: Triggers must be stateless.
    StatefulTriggerNotAllowed {
        trigger_id: String,
    },
}

/// A trigger primitive that evaluates inputs and emits events.
///
/// # TRG-STATE-1: Stateless Triggers
///
/// Triggers are stateless across runs. The runtime API intentionally does not
/// support persisted trigger state. Implementations may use ephemeral
/// evaluation-local memory only (e.g., stack variables within `evaluate()`),
/// but no state may be preserved, observed, or depended upon between
/// invocations.
///
/// Temporal patterns requiring memory (once, latch, debounce, count) must be
/// implemented as clusters where state flows through graph structure or
/// environment, not trigger internals.
pub trait TriggerPrimitive: Send + Sync {
    fn manifest(&self) -> &TriggerPrimitiveManifest;

    fn evaluate(
        &self,
        inputs: &HashMap<String, TriggerValue>,
        parameters: &HashMap<String, ParameterValue>,
    ) -> HashMap<String, TriggerValue>;
}

pub use implementations::emit_if_true::EmitIfTrue;
pub use registry::TriggerRegistry;
