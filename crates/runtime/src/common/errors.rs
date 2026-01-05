use crate::common::value::{PrimitiveKind, ValueType};

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    WrongKind {
        expected: PrimitiveKind,
        got: PrimitiveKind,
    },
    /// X.7: Compute primitives must declare at least one input.
    NoInputsDeclared {
        primitive: String,
    },
    SideEffectsNotAllowed,
    NonDeterministicExecution,
    DuplicateId(String),
    InvalidInputType {
        input: String,
        expected: ValueType,
        got: ValueType,
    },
    InvalidOutputType {
        output: String,
        expected: ValueType,
        got: ValueType,
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
        expected: ValueType,
        got: ValueType,
    },
    UnknownPrimitive(String),
    CycleDetected,
    MissingNode(String),
    MissingOutput {
        node: String,
        output: String,
    },
    /// X.10: Compute parameters must not be Series type.
    UnsupportedParameterType {
        primitive: String,
        version: String,
        parameter: String,
        got: ValueType,
    },
}
