use std::collections::HashMap;

use crate::common::{Value, ValueType};

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
    pub execution: ExecutionSpec,
    pub state: StateSpec,
    pub side_effects: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SourceValidationError {
    WrongKind {
        expected: SourceKind,
        got: SourceKind,
    },
    InputsNotAllowed,
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
    UndeclaredParameter {
        node: String,
        parameter: String,
    },
    UndeclaredOutput {
        primitive: String,
        output: String,
    },
    MissingDeclaredOutput {
        primitive: String,
        output: String,
    },
    InvalidOutputType {
        output: String,
        expected: ValueType,
        got: ValueType,
    },
    UnknownPrimitive(String),
    MissingNode(String),
    MissingOutput {
        node: String,
        output: String,
    },
    OutputsRequired,
}

pub trait SourcePrimitive {
    fn manifest(&self) -> &SourcePrimitiveManifest;

    fn produce(&self, parameters: &HashMap<String, ParameterValue>) -> HashMap<String, Value>;
}

pub use implementations::{boolean, number, string, BooleanSource, NumberSource, StringSource};
pub use registry::SourceRegistry;

#[cfg(test)]
mod tests;
