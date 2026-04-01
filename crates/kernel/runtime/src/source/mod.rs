//! source
//!
//! Purpose:
//! - Define kernel source primitive types, manifests, validation errors, and
//!   registry helpers.
//!
//! Owns:
//! - `SourceValidationError` as the typed registration/manifest failure surface
//!   for source primitives.
//! - Source type metadata and registry-facing source declarations.
//!
//! Does not own:
//! - Catalog-level wrapper errors or product-facing diagnostics.
//! - Host orchestration over validated source primitives.
//!
//! Connects to:
//! - `catalog.rs`, which wraps source registration failures.
//! - Source primitive implementations under `implementations/`.
//!
//! Safety notes:
//! - `Display` renders the `ErrorInfo` summary plus rule id so higher layers can
//!   chain source validation failures without inventing new wording.

use std::collections::HashMap;

use std::borrow::Cow;
use std::fmt;

use crate::common::{doc_anchor_for_rule, ErrorInfo, Phase, Value, ValueType};
use crate::runtime::ExecutionContext;

pub mod implementations;
pub mod registry;

#[derive(Debug, Clone, PartialEq)]
pub enum SourceKind {
    Source,
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
pub enum Cadence {
    Continuous,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputSpec {
    pub name: String,
    pub value_type: ValueType,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputSpec {
    pub name: String,
    pub value_type: ValueType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParameterSpec {
    pub name: String,
    pub value_type: ParameterType,
    pub default: Option<ParameterValue>,
    pub bounds: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceRequires {
    pub context: Vec<ContextRequirement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextRequirement {
    pub name: String,
    pub ty: ValueType,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionSpec {
    pub deterministic: bool,
    pub cadence: Cadence,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateSpec {
    pub allowed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourcePrimitiveManifest {
    pub id: String,
    pub version: String,
    pub kind: SourceKind,
    pub inputs: Vec<InputSpec>,
    pub outputs: Vec<OutputSpec>,
    pub parameters: Vec<ParameterSpec>,
    pub requires: SourceRequires,
    pub execution: ExecutionSpec,
    pub state: StateSpec,
    pub side_effects: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SourceValidationError {
    InvalidId {
        id: String,
    },
    InvalidVersion {
        version: String,
    },
    WrongKind {
        expected: SourceKind,
        got: SourceKind,
    },
    InputsNotAllowed,
    DuplicateOutput {
        name: String,
        first_index: usize,
        second_index: usize,
    },
    SideEffectsNotAllowed,
    NonDeterministicExecution,
    InvalidCadence,
    StateNotAllowed,
    DuplicateId(String),
    InvalidParameterType {
        parameter: String,
        expected: ParameterType,
        got: ParameterType,
    },
    InvalidOutputType {
        output: String,
        expected: ValueType,
        got: ValueType,
    },
    OutputsRequired,
    UnboundContextKeyReference {
        name: String,
        referenced_param: String,
    },
    ContextKeyReferenceNotString {
        name: String,
        referenced_param: String,
    },
}

impl ErrorInfo for SourceValidationError {
    fn rule_id(&self) -> &'static str {
        match self {
            Self::InvalidId { .. } => "SRC-1",
            Self::InvalidVersion { .. } => "SRC-2",
            Self::WrongKind { .. } => "SRC-3",
            Self::InputsNotAllowed => "SRC-4",
            Self::OutputsRequired => "SRC-5",
            Self::DuplicateOutput { .. } => "SRC-6",
            Self::InvalidOutputType { .. } => "SRC-7",
            Self::StateNotAllowed => "SRC-8",
            Self::SideEffectsNotAllowed => "SRC-9",
            Self::NonDeterministicExecution => "SRC-12",
            Self::InvalidCadence => "SRC-13",
            Self::DuplicateId(_) => "SRC-14",
            Self::InvalidParameterType { .. } => "SRC-15",
            Self::UnboundContextKeyReference { .. } => "SRC-16",
            Self::ContextKeyReferenceNotString { .. } => "SRC-17",
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
            Self::InvalidId { id } => Cow::Owned(format!("Invalid source ID: '{}'", id)),
            Self::InvalidVersion { version } => {
                Cow::Owned(format!("Invalid version: '{}'", version))
            }
            Self::WrongKind { expected, got } => Cow::Owned(format!(
                "Wrong kind: expected {:?}, got {:?}",
                expected, got
            )),
            Self::InputsNotAllowed => Cow::Borrowed("Sources cannot declare inputs"),
            Self::OutputsRequired => Cow::Borrowed("Source must declare at least one output"),
            Self::DuplicateOutput { name, .. } => {
                Cow::Owned(format!("Duplicate output name: '{}'", name))
            }
            Self::InvalidOutputType {
                output,
                expected,
                got,
            } => Cow::Owned(format!(
                "Output '{}' has invalid type: expected {:?}, got {:?}",
                output, expected, got
            )),
            Self::StateNotAllowed => Cow::Borrowed("Source state is not allowed"),
            Self::SideEffectsNotAllowed => Cow::Borrowed("Source side effects are not allowed"),
            Self::NonDeterministicExecution => {
                Cow::Borrowed("Source execution must be deterministic")
            }
            Self::InvalidCadence => Cow::Borrowed("Source cadence must be continuous"),
            Self::DuplicateId(_) => Cow::Borrowed("Duplicate source ID: already registered"),
            Self::InvalidParameterType {
                parameter,
                expected,
                got,
            } => Cow::Owned(format!(
                "Parameter '{}' has invalid type: expected {:?}, got {:?}",
                parameter, expected, got
            )),
            Self::UnboundContextKeyReference {
                name,
                referenced_param,
            } => Cow::Owned(format!(
                "Context key '{}' references undefined parameter '{}'",
                name, referenced_param
            )),
            Self::ContextKeyReferenceNotString {
                name,
                referenced_param,
            } => Cow::Owned(format!(
                "Context key '{}' references parameter '{}' which is not String type",
                name, referenced_param
            )),
        }
    }

    fn path(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::InvalidId { .. } => Some(Cow::Borrowed("$.id")),
            Self::InvalidVersion { .. } => Some(Cow::Borrowed("$.version")),
            Self::WrongKind { .. } => Some(Cow::Borrowed("$.kind")),
            Self::InputsNotAllowed => Some(Cow::Borrowed("$.inputs")),
            Self::OutputsRequired => Some(Cow::Borrowed("$.outputs")),
            Self::DuplicateOutput { second_index, .. } => {
                Some(Cow::Owned(format!("$.outputs[{}].name", second_index)))
            }
            Self::InvalidOutputType { .. } => Some(Cow::Borrowed("$.outputs[].type")),
            Self::StateNotAllowed => Some(Cow::Borrowed("$.state.allowed")),
            Self::SideEffectsNotAllowed => Some(Cow::Borrowed("$.side_effects")),
            Self::NonDeterministicExecution => Some(Cow::Borrowed("$.execution.deterministic")),
            Self::InvalidCadence => Some(Cow::Borrowed("$.execution.cadence")),
            Self::DuplicateId(_) => Some(Cow::Borrowed("$.id")),
            Self::InvalidParameterType { .. } => Some(Cow::Borrowed("$.parameters[].default")),
            Self::UnboundContextKeyReference { .. } => {
                Some(Cow::Borrowed("$.requires.context[].name"))
            }
            Self::ContextKeyReferenceNotString { .. } => {
                Some(Cow::Borrowed("$.requires.context[].name"))
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
            Self::WrongKind { .. } => Some(Cow::Borrowed("Set kind: source")),
            Self::InputsNotAllowed => Some(Cow::Borrowed("Remove inputs from source manifest")),
            Self::OutputsRequired => Some(Cow::Borrowed("Add at least one output")),
            Self::DuplicateOutput { name, .. } => Some(Cow::Owned(format!(
                "Rename output '{}' to a unique value",
                name
            ))),
            Self::InvalidOutputType { .. } => Some(Cow::Borrowed(
                "Use a valid output type: number, bool, string, or series",
            )),
            Self::StateNotAllowed => Some(Cow::Borrowed("Set state.allowed: false")),
            Self::SideEffectsNotAllowed => Some(Cow::Borrowed("Set side_effects: false")),
            Self::NonDeterministicExecution => {
                Some(Cow::Borrowed("Set execution.deterministic: true"))
            }
            Self::InvalidCadence => Some(Cow::Borrowed("Set cadence: continuous")),
            Self::InvalidParameterType { .. } => Some(Cow::Borrowed(
                "Change parameter default value to match the declared parameter type",
            )),
            Self::UnboundContextKeyReference {
                referenced_param, ..
            } => Some(Cow::Owned(format!(
                "Add parameter '{}' to the source manifest",
                referenced_param
            ))),
            Self::ContextKeyReferenceNotString {
                referenced_param, ..
            } => Some(Cow::Owned(format!(
                "Change parameter '{}' type to String",
                referenced_param
            ))),
        }
    }
}

impl fmt::Display for SourceValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.summary(), self.rule_id())
    }
}

impl std::error::Error for SourceValidationError {}

pub trait SourcePrimitive {
    fn manifest(&self) -> &SourcePrimitiveManifest;

    fn produce(
        &self,
        parameters: &HashMap<String, ParameterValue>,
        ctx: &ExecutionContext,
    ) -> HashMap<String, Value>;
}

pub use implementations::{
    boolean, context_bool, context_number, context_series, context_string, number, string,
    BooleanSource, ContextBoolSource, ContextNumberSource, ContextSeriesSource,
    ContextStringSource, NumberSource, StringSource,
};
pub use registry::SourceRegistry;

#[cfg(test)]
mod tests;
