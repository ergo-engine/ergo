use std::collections::HashMap;

use crate::common::{PrimitiveKind, Value, ValueType};

pub mod implementations;
pub mod registry;

#[derive(Debug, Clone, PartialEq)]
pub enum Cadence {
    Continuous,
    Event,
}

#[derive(Debug, Clone)]
pub struct InputSpec {
    pub name: String,
    pub value_type: ValueType,
    pub required: bool,
}

#[derive(Debug, Clone)]
pub struct OutputSpec {
    pub name: String,
    pub value_type: ValueType,
}

#[derive(Debug, Clone)]
pub struct ParameterSpec {
    pub name: String,
    pub value_type: ValueType,
    pub default: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct ExecutionSpec {
    pub deterministic: bool,
    pub cadence: Cadence,
}

#[derive(Debug, Clone)]
pub struct StateSpec {
    pub stateful: bool,
    pub rolling_window: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct PrimitiveState {
    pub data: HashMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComputeError {
    DivisionByZero,
    NonFiniteResult,
}

impl std::fmt::Display for ComputeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComputeError::DivisionByZero => write!(f, "division by zero"),
            ComputeError::NonFiniteResult => write!(f, "non-finite result"),
        }
    }
}

impl std::error::Error for ComputeError {}

#[derive(Debug, Clone)]
pub struct ComputePrimitiveManifest {
    pub id: String,
    pub version: String,
    pub kind: PrimitiveKind,
    pub inputs: Vec<InputSpec>,
    pub outputs: Vec<OutputSpec>,
    pub parameters: Vec<ParameterSpec>,
    pub execution: ExecutionSpec,
    pub state: StateSpec,
    pub side_effects: bool,
}

pub trait ComputePrimitive {
    fn manifest(&self) -> &ComputePrimitiveManifest;

    fn compute(
        &self,
        inputs: &HashMap<String, Value>,
        parameters: &HashMap<String, Value>,
        state: Option<&mut PrimitiveState>,
    ) -> Result<HashMap<String, Value>, ComputeError>;
}

pub use implementations::{
    add, and, const_bool, const_number, divide, eq, gt, lt, multiply, negate, neq, not, or,
    safe_divide, select, subtract, Add, And, ConstBool, ConstNumber, Divide, Eq, Gt, Lt, Multiply,
    Negate, Neq, Not, Or, SafeDivide, Select, Subtract,
};
pub use registry::PrimitiveRegistry;

#[cfg(test)]
mod tests;
