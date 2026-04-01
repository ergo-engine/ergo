//! common::errors
//!
//! Purpose:
//! - Define the kernel-owned validation error taxonomy shared by compute-style
//!   primitive registration and catalog assembly.
//!
//! Owns:
//! - `ValidationError` as the typed authority for common compute registration
//!   failures.
//! - The `Display`/`Error` surface higher layers rely on for chaining instead of
//!   flattening these errors into strings.
//!
//! Does not own:
//! - Primitive-family wrapper errors in catalog assembly.
//! - Host or product-facing error descriptors.
//!
//! Connects to:
//! - `catalog.rs`, which wraps these failures in `CoreRegistrationError`.
//! - Compute validation callers that need one authoritative error surface.
//!
//! Safety notes:
//! - `Display` intentionally stays aligned with `ErrorInfo` summary/rule-id so
//!   registration diagnostics share one semantic authority.

use std::borrow::Cow;
use std::fmt;

use crate::common::value::{PrimitiveKind, ValueType};
use crate::common::{doc_anchor_for_rule, ErrorInfo, Phase};

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    InvalidId {
        id: String,
    },
    InvalidVersion {
        version: String,
    },
    WrongKind {
        expected: PrimitiveKind,
        got: PrimitiveKind,
    },
    /// X.7: Compute primitives must declare at least one input.
    NoInputsDeclared {
        primitive: String,
    },
    NoOutputsDeclared {
        primitive: String,
    },
    SideEffectsNotAllowed,
    NonDeterministicExecution,
    NonDeterministicErrors {
        primitive: String,
    },
    InvalidCadence {
        primitive: String,
    },
    InvalidInputCardinality {
        primitive: String,
        input: String,
        got: String,
    },
    DuplicateId(String),
    DuplicateInput {
        name: String,
        first_index: usize,
        second_index: usize,
    },
    DuplicateOutput {
        name: String,
        first_index: usize,
        second_index: usize,
    },
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
    MissingDeclaredOutput {
        primitive: String,
        output: String,
    },
    InvalidParameterType {
        parameter: String,
        expected: ValueType,
        got: ValueType,
    },
    StateNotResettable {
        primitive: String,
    },
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

impl ErrorInfo for ValidationError {
    fn rule_id(&self) -> &'static str {
        match self {
            Self::InvalidId { .. } => "CMP-1",
            Self::InvalidVersion { .. } => "CMP-2",
            Self::WrongKind { .. } => "CMP-3",
            Self::NoInputsDeclared { .. } => "CMP-4",
            Self::DuplicateInput { .. } => "CMP-5",
            Self::NoOutputsDeclared { .. } => "CMP-6",
            Self::DuplicateOutput { .. } => "CMP-7",
            Self::SideEffectsNotAllowed => "CMP-8",
            Self::StateNotResettable { .. } => "CMP-9",
            Self::NonDeterministicErrors { .. } => "CMP-10",
            Self::InvalidInputType { .. } => "CMP-13",
            Self::InvalidInputCardinality { .. } => "CMP-14",
            Self::UnsupportedParameterType { .. } => "CMP-15",
            Self::InvalidCadence { .. } => "CMP-16",
            Self::NonDeterministicExecution => "CMP-17",
            Self::DuplicateId(_) => "CMP-18",
            Self::MissingDeclaredOutput { .. } => "CMP-11",
            Self::MissingOutput { .. } => "CMP-11",
            Self::InvalidOutputType { .. } => "CMP-20",
            Self::InvalidParameterType { .. } => "CMP-19",
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
            Self::InvalidId { id } => Cow::Owned(format!("Invalid compute ID: '{}'", id)),
            Self::InvalidVersion { version } => {
                Cow::Owned(format!("Invalid version: '{}'", version))
            }
            Self::WrongKind { expected, got } => Cow::Owned(format!(
                "Wrong kind: expected {:?}, got {:?}",
                expected, got
            )),
            Self::NoInputsDeclared { .. } => Cow::Borrowed("Compute has no inputs"),
            Self::NoOutputsDeclared { .. } => Cow::Borrowed("Compute has no outputs"),
            Self::SideEffectsNotAllowed => Cow::Borrowed("Compute has side effects"),
            Self::NonDeterministicExecution => {
                Cow::Borrowed("Compute execution must be deterministic")
            }
            Self::NonDeterministicErrors { .. } => {
                Cow::Borrowed("Compute errors must be deterministic when allowed")
            }
            Self::InvalidCadence { .. } => Cow::Borrowed("Compute cadence must be continuous"),
            Self::InvalidInputCardinality { input, got, .. } => Cow::Owned(format!(
                "Input '{}' has invalid cardinality '{}'",
                input, got
            )),
            Self::DuplicateId(_) => Cow::Borrowed("Duplicate compute ID: already registered"),
            Self::DuplicateInput { name, .. } => {
                Cow::Owned(format!("Duplicate input name: '{}'", name))
            }
            Self::DuplicateOutput { name, .. } => {
                Cow::Owned(format!("Duplicate output name: '{}'", name))
            }
            Self::InvalidInputType {
                input,
                expected,
                got,
            } => Cow::Owned(format!(
                "Input '{}' has invalid type: expected {:?}, got {:?}",
                input, expected, got
            )),
            Self::InvalidOutputType {
                output,
                expected,
                got,
            } => Cow::Owned(format!(
                "Output '{}' has invalid type: expected {:?}, got {:?}",
                output, expected, got
            )),
            Self::MissingDeclaredOutput { primitive, output } => Cow::Owned(format!(
                "Missing declared output '{}' for primitive '{}'",
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
            Self::StateNotResettable { .. } => {
                Cow::Borrowed("State must be resettable when allowed")
            }
            Self::MissingOutput { node, output } => {
                Cow::Owned(format!("Missing output '{}' on node '{}'", output, node))
            }
            Self::UnsupportedParameterType { parameter, got, .. } => Cow::Owned(format!(
                "Parameter '{}' has unsupported type {:?}",
                parameter, got
            )),
        }
    }

    fn path(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::InvalidId { .. } => Some(Cow::Borrowed("$.id")),
            Self::InvalidVersion { .. } => Some(Cow::Borrowed("$.version")),
            Self::WrongKind { .. } => Some(Cow::Borrowed("$.kind")),
            Self::NoInputsDeclared { .. } => Some(Cow::Borrowed("$.inputs")),
            Self::NoOutputsDeclared { .. } => Some(Cow::Borrowed("$.outputs")),
            Self::DuplicateId(_) => Some(Cow::Borrowed("$.id")),
            Self::DuplicateInput { second_index, .. } => {
                Some(Cow::Owned(format!("$.inputs[{}].name", second_index)))
            }
            Self::DuplicateOutput { second_index, .. } => {
                Some(Cow::Owned(format!("$.outputs[{}].name", second_index)))
            }
            Self::InvalidInputType { .. } => Some(Cow::Borrowed("$.inputs[].type")),
            Self::InvalidOutputType { .. } => Some(Cow::Borrowed("$.outputs[].type")),
            Self::InvalidInputCardinality { .. } => Some(Cow::Borrowed("$.inputs[].cardinality")),
            Self::SideEffectsNotAllowed => Some(Cow::Borrowed("$.side_effects")),
            Self::NonDeterministicExecution => Some(Cow::Borrowed("$.execution.deterministic")),
            Self::NonDeterministicErrors { .. } => Some(Cow::Borrowed("$.errors.deterministic")),
            Self::InvalidCadence { .. } => Some(Cow::Borrowed("$.execution.cadence")),
            Self::UnsupportedParameterType { .. } => Some(Cow::Borrowed("$.parameters[].type")),
            Self::InvalidParameterType { .. } => Some(Cow::Borrowed("$.parameters[].default")),
            Self::StateNotResettable { .. } => Some(Cow::Borrowed("$.state.resettable")),
            _ => None,
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
            Self::WrongKind { .. } => Some(Cow::Borrowed("Set kind: compute")),
            Self::NoInputsDeclared { .. } => Some(Cow::Borrowed("Add at least one input")),
            Self::NoOutputsDeclared { .. } => Some(Cow::Borrowed("Add at least one output")),
            Self::SideEffectsNotAllowed => Some(Cow::Borrowed("Set side_effects: false")),
            Self::NonDeterministicExecution => {
                Some(Cow::Borrowed("Set execution.deterministic: true"))
            }
            Self::NonDeterministicErrors { .. } => Some(Cow::Borrowed(
                "Set errors.deterministic: true or errors.allowed: false",
            )),
            Self::InvalidCadence { .. } => Some(Cow::Borrowed("Set cadence: continuous")),
            Self::InvalidInputCardinality { .. } => {
                Some(Cow::Borrowed("Set input cardinality to single"))
            }
            Self::DuplicateInput { name, .. } => Some(Cow::Owned(format!(
                "Rename input '{}' to a unique value",
                name
            ))),
            Self::DuplicateOutput { name, .. } => Some(Cow::Owned(format!(
                "Rename output '{}' to a unique value",
                name
            ))),
            Self::InvalidOutputType { .. } => Some(Cow::Borrowed(
                "Use a valid output type: number, bool, series, or string",
            )),
            Self::UnsupportedParameterType { parameter, .. } => Some(Cow::Owned(format!(
                "Change parameter '{}' type to int, number, or bool",
                parameter
            ))),
            Self::InvalidParameterType { parameter, .. } => Some(Cow::Owned(format!(
                "Change parameter '{}' default value to match the declared type",
                parameter
            ))),
            Self::StateNotResettable { .. } => {
                Some(Cow::Borrowed("Set state.resettable: true"))
            }
            _ => None,
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.summary(), self.rule_id())
    }
}

impl std::error::Error for ValidationError {}

#[cfg(test)]
mod tests {
    use super::ValidationError;
    use crate::common::value::ValueType;
    use crate::common::ErrorInfo;

    #[test]
    fn cmp_15_remains_unsupported_parameter_type() {
        let err = ValidationError::UnsupportedParameterType {
            primitive: "p".to_string(),
            version: "1.0.0".to_string(),
            parameter: "x".to_string(),
            got: ValueType::String,
        };

        assert_eq!(err.rule_id(), "CMP-15");
        assert_eq!(
            err.doc_anchor(),
            "docs/primitives/compute.md#4-enforcement-mapping"
        );
        assert_eq!(err.path().as_deref(), Some("$.parameters[].type"));
        assert_eq!(
            err.fix().as_deref(),
            Some("Change parameter 'x' type to int, number, or bool")
        );
    }

    #[test]
    fn cmp_19_reserved_for_invalid_parameter_type() {
        let err = ValidationError::InvalidParameterType {
            parameter: "x".to_string(),
            expected: ValueType::Number,
            got: ValueType::String,
        };

        assert_eq!(err.rule_id(), "CMP-19");
        assert_eq!(
            err.doc_anchor(),
            "docs/primitives/compute.md#4-enforcement-mapping"
        );
        assert_eq!(err.path().as_deref(), Some("$.parameters[].default"));
        assert_eq!(
            err.fix().as_deref(),
            Some("Change parameter 'x' default value to match the declared type")
        );
    }

    #[test]
    fn cmp_20_reserved_for_invalid_output_type() {
        let err = ValidationError::InvalidOutputType {
            output: "out".to_string(),
            expected: ValueType::Number,
            got: ValueType::String,
        };

        assert_eq!(err.rule_id(), "CMP-20");
        assert_eq!(
            err.doc_anchor(),
            "docs/primitives/compute.md#4-enforcement-mapping"
        );
        assert_eq!(err.path().as_deref(), Some("$.outputs[].type"));
        assert_eq!(
            err.fix().as_deref(),
            Some("Use a valid output type: number, bool, series, or string")
        );
    }
}
