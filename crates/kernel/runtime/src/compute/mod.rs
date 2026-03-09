use std::collections::HashMap;

use crate::common::{PrimitiveKind, Value, ValueType};

pub mod implementations;
pub mod registry;

#[derive(Debug, Clone, PartialEq)]
pub enum Cadence {
    Continuous,
    Event,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cardinality {
    Single,
    Multiple,
}

#[derive(Debug, Clone)]
pub struct InputSpec {
    pub name: String,
    pub value_type: ValueType,
    pub required: bool,
    pub cardinality: Cardinality,
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
    pub required: bool,
    pub bounds: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExecutionSpec {
    pub deterministic: bool,
    pub cadence: Cadence,
    pub may_error: bool,
}

#[derive(Debug, Clone)]
pub struct ErrorSpec {
    pub allowed: bool,
    pub types: Vec<ErrorType>,
    pub deterministic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorType {
    DivisionByZero,
    NonFiniteResult,
    InvalidParameter,
}

#[derive(Debug, Clone)]
pub struct StateSpec {
    pub allowed: bool,
    pub resettable: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PrimitiveState {
    pub data: HashMap<String, Value>,
}

/// Errors that can occur during compute primitive execution.
///
/// These represent semantic failures in computation, not infrastructure failures.
/// They map to `ErrKind::SemanticError` and are non-retryable.
///
/// See: B.2 in PHASE_INVARIANTS.md
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComputeError {
    /// B.2: Division by zero is undefined.
    DivisionByZero,
    /// B.2: Result overflowed to infinity or produced NaN.
    NonFiniteResult,
    /// Parameter value violated primitive constraints.
    InvalidParameter { parameter: String, reason: String },
}

impl std::fmt::Display for ComputeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComputeError::DivisionByZero => write!(f, "division by zero"),
            ComputeError::NonFiniteResult => write!(f, "non-finite result"),
            ComputeError::InvalidParameter { parameter, reason } => {
                write!(f, "invalid parameter '{}': {}", parameter, reason)
            }
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
    pub errors: ErrorSpec,
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
    add, and, append, const_bool, const_number, divide, eq, gt, len, lt, mean, multiply, negate,
    neq, not, or, safe_divide, select, subtract, sum, window, Add, And, Append, ConstBool,
    ConstNumber, Divide, Eq, Gt, Len, Lt, Mean, Multiply, Negate, Neq, Not, Or, SafeDivide, Select,
    Subtract, Sum, Window,
};
pub use registry::PrimitiveRegistry;

#[cfg(test)]
mod tests;
