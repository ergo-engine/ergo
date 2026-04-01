//! trigger
//!
//! Purpose:
//! - Define kernel trigger primitive types, manifests, validation errors, and
//!   registry helpers.
//!
//! Owns:
//! - `TriggerValidationError` as the typed registration failure surface for
//!   trigger primitives.
//! - Trigger type metadata and registry-facing trigger declarations.
//!
//! Does not own:
//! - Catalog-level wrapper errors or host-facing rendering.
//! - Trigger execution orchestration outside kernel registration.
//!
//! Connects to:
//! - `catalog.rs`, which wraps trigger registration failures.
//! - Trigger implementations under `implementations/`.
//!
//! Safety notes:
//! - `Display` stays aligned with `ErrorInfo` so trigger registration meaning is
//!   not duplicated across layers.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;

use crate::common::{doc_anchor_for_rule, ErrorInfo, Phase};

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
    pub required: bool,
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
    InvalidId {
        id: String,
    },
    InvalidVersion {
        version: String,
    },
    WrongKind {
        expected: TriggerKind,
        got: TriggerKind,
    },
    NoInputsDeclared {
        trigger: String,
    },
    DuplicateInput {
        name: String,
        first_index: usize,
        second_index: usize,
    },
    SideEffectsNotAllowed,
    NonDeterministicExecution,
    DuplicateId(String),
    TriggerWrongOutputCount {
        got: usize,
    },
    InvalidInputCardinality {
        input: String,
        got: String,
    },
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
    InvalidParameterType {
        parameter: String,
        expected: ParameterType,
        got: ParameterType,
    },
    /// TRG-STATE-1: Triggers must be stateless.
    StatefulTriggerNotAllowed {
        trigger_id: String,
    },
}

impl ErrorInfo for TriggerValidationError {
    fn rule_id(&self) -> &'static str {
        match self {
            Self::InvalidId { .. } => "TRG-1",
            Self::InvalidVersion { .. } => "TRG-2",
            Self::WrongKind { .. } => "TRG-3",
            Self::NoInputsDeclared { .. } => "TRG-4",
            Self::DuplicateInput { .. } => "TRG-5",
            Self::InvalidInputType { .. } => "TRG-6",
            Self::TriggerWrongOutputCount { .. } => "TRG-7",
            Self::InvalidOutputType { .. } => "TRG-8",
            Self::StatefulTriggerNotAllowed { .. } => "TRG-9",
            Self::SideEffectsNotAllowed => "TRG-10",
            Self::NonDeterministicExecution => "TRG-11",
            Self::InvalidInputCardinality { .. } => "TRG-12",
            Self::DuplicateId(_) => "TRG-13",
            Self::InvalidParameterType { .. } => "TRG-14",
        }
    }

    fn phase(&self) -> Phase {
        Phase::Registration
    }

    fn doc_anchor(&self) -> &'static str {
        doc_anchor_for_rule(self.rule_id())
    }

    fn summary(&self) -> Cow<'static, str> {
        match self {
            Self::InvalidId { id } => Cow::Owned(format!("Invalid trigger ID: '{}'", id)),
            Self::InvalidVersion { version } => {
                Cow::Owned(format!("Invalid version: '{}'", version))
            }
            Self::WrongKind { expected, got } => Cow::Owned(format!(
                "Wrong kind: expected {:?}, got {:?}",
                expected, got
            )),
            Self::NoInputsDeclared { .. } => Cow::Borrowed("Trigger has no inputs"),
            Self::DuplicateInput { name, .. } => {
                Cow::Owned(format!("Duplicate input name: '{}'", name))
            }
            Self::InvalidInputType {
                input,
                expected,
                got,
            } => Cow::Owned(format!(
                "Input '{}' has invalid type: expected {:?}, got {:?}",
                input, expected, got
            )),
            Self::TriggerWrongOutputCount { got } => Cow::Owned(format!(
                "Trigger must declare exactly one output (got {})",
                got
            )),
            Self::InvalidOutputType {
                output,
                expected,
                got,
            } => Cow::Owned(format!(
                "Output '{}' has invalid type: expected {:?}, got {:?}",
                output, expected, got
            )),
            Self::StatefulTriggerNotAllowed { .. } => Cow::Borrowed("Trigger state is not allowed"),
            Self::SideEffectsNotAllowed => Cow::Borrowed("Trigger side effects are not allowed"),
            Self::NonDeterministicExecution => {
                Cow::Borrowed("Trigger execution must be deterministic")
            }
            Self::InvalidInputCardinality { input, got } => Cow::Owned(format!(
                "Input '{}' has invalid cardinality '{}'",
                input, got
            )),
            Self::DuplicateId(_) => Cow::Borrowed("Duplicate trigger ID: already registered"),
            Self::InvalidParameterType {
                parameter,
                expected,
                got,
            } => Cow::Owned(format!(
                "Parameter '{}' has invalid type: expected {:?}, got {:?}",
                parameter, expected, got
            )),
        }
    }

    fn path(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::InvalidId { .. } => Some(Cow::Borrowed("$.id")),
            Self::InvalidVersion { .. } => Some(Cow::Borrowed("$.version")),
            Self::DuplicateId(_) => Some(Cow::Borrowed("$.id")),
            Self::WrongKind { .. } => Some(Cow::Borrowed("$.kind")),
            Self::NoInputsDeclared { .. } => Some(Cow::Borrowed("$.inputs")),
            Self::DuplicateInput { second_index, .. } => {
                Some(Cow::Owned(format!("$.inputs[{}].name", second_index)))
            }
            Self::InvalidInputType { .. } => Some(Cow::Borrowed("$.inputs[].type")),
            Self::InvalidInputCardinality { .. } => Some(Cow::Borrowed("$.inputs[].cardinality")),
            Self::TriggerWrongOutputCount { .. } => Some(Cow::Borrowed("$.outputs")),
            Self::InvalidOutputType { .. } => Some(Cow::Borrowed("$.outputs[0].type")),
            Self::StatefulTriggerNotAllowed { .. } => Some(Cow::Borrowed("$.state.allowed")),
            Self::SideEffectsNotAllowed => Some(Cow::Borrowed("$.side_effects")),
            Self::NonDeterministicExecution => Some(Cow::Borrowed("$.execution.deterministic")),
            Self::InvalidParameterType { .. } => Some(Cow::Borrowed("$.parameters[].default")),
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
            Self::WrongKind { .. } => Some(Cow::Borrowed("Set kind: trigger")),
            Self::NoInputsDeclared { .. } => Some(Cow::Borrowed("Add at least one input")),
            Self::DuplicateInput { name, .. } => Some(Cow::Owned(format!(
                "Rename input '{}' to a unique value",
                name
            ))),
            Self::InvalidInputType { .. } => Some(Cow::Borrowed(
                "Use a valid input type: number, bool, series, or event",
            )),
            Self::TriggerWrongOutputCount { .. } => {
                Some(Cow::Borrowed("Declare exactly one output"))
            }
            Self::InvalidOutputType { .. } => {
                Some(Cow::Borrowed("Output type must be event"))
            }
            Self::StatefulTriggerNotAllowed { .. } => {
                Some(Cow::Borrowed("Set state.allowed: false"))
            }
            Self::SideEffectsNotAllowed => Some(Cow::Borrowed("Set side_effects: false")),
            Self::NonDeterministicExecution => {
                Some(Cow::Borrowed("Set execution.deterministic: true"))
            }
            Self::InvalidInputCardinality { .. } => {
                Some(Cow::Borrowed("Set input cardinality to single"))
            }
            Self::InvalidParameterType { .. } => Some(Cow::Borrowed(
                "Change parameter default value to match the declared parameter type",
            )),
        }
    }
}

impl fmt::Display for TriggerValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.summary(), self.rule_id())
    }
}

impl std::error::Error for TriggerValidationError {}

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

pub use implementations::emit_if_event_and_true::EmitIfEventAndTrue;
pub use implementations::emit_if_true::EmitIfTrue;
pub use registry::TriggerRegistry;
