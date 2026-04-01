//! action
//!
//! Purpose:
//! - Define kernel action primitive types, manifests, validation errors, and
//!   registry helpers.
//!
//! Owns:
//! - `ActionValidationError` as the typed registration failure surface for
//!   action primitives.
//! - Action type metadata and registry-facing action declarations.
//!
//! Does not own:
//! - Catalog-level wrapper errors or host-facing diagnostics.
//! - Runtime execution/orchestration semantics outside action registration.
//!
//! Connects to:
//! - `catalog.rs`, which wraps action registration failures.
//! - Action implementations under `implementations/`.
//!
//! Safety notes:
//! - `Display` uses the `ErrorInfo` authority so action rule ids and summaries do
//!   not drift from the kernel meaning they already own.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;

use crate::common::{doc_anchor_for_rule, ErrorInfo, Phase, ValueType};

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

#[derive(Debug, Clone, PartialEq)]
pub struct IntentFieldSpec {
    pub name: String,
    pub value_type: ValueType,
    pub from_input: Option<String>,
    pub from_param: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntentMirrorWriteSpec {
    pub name: String,
    pub value_type: ValueType,
    pub from_field: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntentSpec {
    pub name: String,
    pub fields: Vec<IntentFieldSpec>,
    pub mirror_writes: Vec<IntentMirrorWriteSpec>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ActionEffects {
    pub writes: Vec<ActionWriteSpec>,
    pub intents: Vec<IntentSpec>,
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
    DuplicateIntentName {
        name: String,
        first_index: usize,
        second_index: usize,
    },
    DuplicateIntentFieldName {
        intent_name: String,
        field_name: String,
        first_index: usize,
        second_index: usize,
    },
    IntentFieldMissingSource {
        intent_name: String,
        field_name: String,
    },
    IntentFieldMultipleSources {
        intent_name: String,
        field_name: String,
    },
    IntentFieldFromInputNotFound {
        intent_name: String,
        field_name: String,
        from_input: String,
    },
    IntentFieldFromInputTypeMismatch {
        intent_name: String,
        field_name: String,
        from_input: String,
        expected: ValueType,
        found: ActionValueType,
    },
    IntentFieldFromParamNotFound {
        intent_name: String,
        field_name: String,
        from_param: String,
    },
    IntentFieldFromParamTypeMismatch {
        intent_name: String,
        field_name: String,
        from_param: String,
        expected: ValueType,
        found: ParameterType,
    },
    MirrorWriteFromFieldNotFound {
        intent_name: String,
        write_name: String,
        from_field: String,
    },
    MirrorWriteTypeMismatch {
        intent_name: String,
        write_name: String,
        from_field: String,
        expected: ValueType,
        found: ValueType,
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
            Self::DuplicateIntentName { .. } => "ACT-24",
            Self::DuplicateIntentFieldName { .. } => "ACT-25",
            Self::IntentFieldMissingSource { .. } => "ACT-26",
            Self::IntentFieldMultipleSources { .. } => "ACT-27",
            Self::IntentFieldFromInputNotFound { .. } => "ACT-28",
            Self::IntentFieldFromInputTypeMismatch { .. } => "ACT-29",
            Self::IntentFieldFromParamNotFound { .. } => "ACT-30",
            Self::IntentFieldFromParamTypeMismatch { .. } => "ACT-31",
            Self::MirrorWriteFromFieldNotFound { .. } => "ACT-32",
            Self::MirrorWriteTypeMismatch { .. } => "ACT-33",
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
            Self::DuplicateIntentName { name, .. } => {
                Cow::Owned(format!("Duplicate intent name: '{}'", name))
            }
            Self::DuplicateIntentFieldName {
                intent_name,
                field_name,
                ..
            } => Cow::Owned(format!(
                "Intent '{}' has duplicate field name '{}'",
                intent_name, field_name
            )),
            Self::IntentFieldMissingSource {
                intent_name,
                field_name,
            } => Cow::Owned(format!(
                "Intent '{}' field '{}' must set exactly one source (from_input or from_param)",
                intent_name, field_name
            )),
            Self::IntentFieldMultipleSources {
                intent_name,
                field_name,
            } => Cow::Owned(format!(
                "Intent '{}' field '{}' sets both from_input and from_param",
                intent_name, field_name
            )),
            Self::IntentFieldFromInputNotFound {
                intent_name,
                field_name,
                from_input,
            } => Cow::Owned(format!(
                "Intent '{}' field '{}' references undeclared input '{}'",
                intent_name, field_name, from_input
            )),
            Self::IntentFieldFromInputTypeMismatch {
                intent_name,
                field_name,
                from_input,
                expected,
                found,
            } => Cow::Owned(format!(
                "Intent '{}' field '{}' expects {:?} but input '{}' is {:?}",
                intent_name, field_name, expected, from_input, found
            )),
            Self::IntentFieldFromParamNotFound {
                intent_name,
                field_name,
                from_param,
            } => Cow::Owned(format!(
                "Intent '{}' field '{}' references undeclared parameter '{}'",
                intent_name, field_name, from_param
            )),
            Self::IntentFieldFromParamTypeMismatch {
                intent_name,
                field_name,
                from_param,
                expected,
                found,
            } => Cow::Owned(format!(
                "Intent '{}' field '{}' expects {:?} but parameter '{}' is {:?}",
                intent_name, field_name, expected, from_param, found
            )),
            Self::MirrorWriteFromFieldNotFound {
                intent_name,
                write_name,
                from_field,
            } => Cow::Owned(format!(
                "Intent '{}' mirror write '{}' references undeclared field '{}'",
                intent_name, write_name, from_field
            )),
            Self::MirrorWriteTypeMismatch {
                intent_name,
                write_name,
                from_field,
                expected,
                found,
            } => Cow::Owned(format!(
                "Intent '{}' mirror write '{}' type {:?} does not match field '{}' type {:?}",
                intent_name, write_name, expected, from_field, found
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
            Self::DuplicateIntentName { second_index, .. } => Some(Cow::Owned(format!(
                "$.effects.intents[{}].name",
                second_index
            ))),
            Self::DuplicateIntentFieldName {
                intent_name,
                second_index,
                ..
            } => Some(Cow::Owned(format!(
                "$.effects.intents[?(@.name==\"{}\")].fields[{}].name",
                intent_name, second_index
            ))),
            Self::IntentFieldMissingSource { intent_name, .. }
            | Self::IntentFieldMultipleSources { intent_name, .. } => Some(Cow::Owned(format!(
                "$.effects.intents[?(@.name==\"{}\")].fields[]",
                intent_name
            ))),
            Self::IntentFieldFromInputNotFound { intent_name, .. }
            | Self::IntentFieldFromInputTypeMismatch { intent_name, .. } => {
                Some(Cow::Owned(format!(
                    "$.effects.intents[?(@.name==\"{}\")].fields[].from_input",
                    intent_name
                )))
            }
            Self::IntentFieldFromParamNotFound { intent_name, .. }
            | Self::IntentFieldFromParamTypeMismatch { intent_name, .. } => {
                Some(Cow::Owned(format!(
                    "$.effects.intents[?(@.name==\"{}\")].fields[].from_param",
                    intent_name
                )))
            }
            Self::MirrorWriteFromFieldNotFound { intent_name, .. }
            | Self::MirrorWriteTypeMismatch { intent_name, .. } => Some(Cow::Owned(format!(
                "$.effects.intents[?(@.name==\"{}\")].mirror_writes[]",
                intent_name
            ))),
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
            Self::DuplicateIntentName { name, .. } => Some(Cow::Owned(format!(
                "Rename intent '{}' to a unique value",
                name
            ))),
            Self::DuplicateIntentFieldName { field_name, .. } => Some(Cow::Owned(format!(
                "Rename intent field '{}' to a unique value within its intent",
                field_name
            ))),
            Self::IntentFieldMissingSource { .. } => Some(Cow::Borrowed(
                "Set exactly one source on each intent field: from_input or from_param",
            )),
            Self::IntentFieldMultipleSources { .. } => Some(Cow::Borrowed(
                "Set only one source on each intent field: from_input or from_param",
            )),
            Self::IntentFieldFromInputNotFound { from_input, .. } => Some(Cow::Owned(format!(
                "Declare input '{}' in the action manifest inputs",
                from_input
            ))),
            Self::IntentFieldFromInputTypeMismatch {
                from_input,
                expected,
                ..
            } => Some(Cow::Owned(format!(
                "Change input '{}' type to match intent field type {:?}",
                from_input, expected
            ))),
            Self::IntentFieldFromParamNotFound { from_param, .. } => Some(Cow::Owned(format!(
                "Declare parameter '{}' in the action manifest parameters",
                from_param
            ))),
            Self::IntentFieldFromParamTypeMismatch {
                from_param,
                expected,
                ..
            } => Some(Cow::Owned(format!(
                "Change parameter '{}' type to match intent field type {:?}",
                from_param, expected
            ))),
            Self::MirrorWriteFromFieldNotFound { from_field, .. } => Some(Cow::Owned(format!(
                "Declare intent field '{}' before referencing it from mirror_writes",
                from_field
            ))),
            Self::MirrorWriteTypeMismatch {
                from_field,
                expected,
                ..
            } => Some(Cow::Owned(format!(
                "Change mirror write type to match field '{}' type {:?}",
                from_field, expected
            ))),
        }
    }
}

impl fmt::Display for ActionValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.summary(), self.rule_id())
    }
}

impl std::error::Error for ActionValidationError {}

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
