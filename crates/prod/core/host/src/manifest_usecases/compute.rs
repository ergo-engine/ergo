//! manifest_usecases::compute
//!
//! Purpose:
//! - Parse and normalize file-backed compute manifests into typed runtime
//!   compute manifests.
//!
//! Owns:
//! - Compute raw DTOs, compute-specific parse errors, compute parameter aliases,
//!   default parsing, and lowering into the typed runtime manifest.
//!
//! Does not own:
//! - Host manifest dispatch, summary projection, or CLI rendering.
//!
//! Connects to:
//! - `manifest_usecases.rs`, which dispatches `kind: compute` manifests here
//!   while preserving the public host manifest API.
//! - `ergo_runtime::compute`, which consumes the lowered typed manifest and
//!   validates the result after host parsing.
//!
//! Safety notes:
//! - The file-backed compute surface still aliases `int` parameters to numeric
//!   runtime values and keeps `CMP-*` rule ownership local to this module.
//! - The tiny inline test at the bottom stays here deliberately because it
//!   protects the private alias parser directly rather than broader host
//!   behavior.

use std::borrow::Cow;

use ergo_runtime::common::{
    doc_anchor_for_rule, ErrorInfo, Phase, PrimitiveKind, RuleViolation, Value, ValueType,
};
use ergo_runtime::compute::{
    Cadence as ComputeCadence, Cardinality as ComputeCardinality, ComputePrimitiveManifest,
    ErrorType as ComputeErrorType, ExecutionSpec as ComputeExecutionSpec,
    ParameterSpec as ComputeParameterSpec, StateSpec as ComputeStateSpec,
};
use serde::Deserialize;

use super::common::parse_value_type;
use super::parse_error_violation;

pub(super) fn parse_manifest(
    source_label: &str,
    value: serde_json::Value,
) -> Result<ComputePrimitiveManifest, RuleViolation> {
    let raw = serde_json::from_value::<RawComputeManifest>(value).map_err(|err| {
        parse_error_violation(format!("parse compute manifest '{}': {err}", source_label))
    })?;
    raw_to_compute_manifest(raw).map_err(RuleViolation::from)
}

fn parse_compute_parameter_type(
    parameter: &str,
    input: &str,
) -> Result<ValueType, ComputeParseError> {
    match input.to_ascii_lowercase().as_str() {
        "int" | "number" => Ok(ValueType::Number),
        "bool" | "boolean" => Ok(ValueType::Bool),
        "string" => Ok(ValueType::String),
        "series" => Ok(ValueType::Series),
        other => Err(ComputeParseError::InvalidParameterType {
            parameter: parameter.to_string(),
            got: other.to_string(),
        }),
    }
}

#[derive(Debug)]
enum ComputeParseError {
    WrongKind {
        got: String,
    },
    InvalidInputType {
        input: String,
        got: String,
    },
    InvalidOutputType {
        output: String,
        got: String,
    },
    InvalidInputCardinality {
        input: String,
        got: String,
    },
    InvalidParameterType {
        parameter: String,
        got: String,
    },
    InvalidParameterDefault {
        parameter: String,
        expected: ValueType,
        reason: String,
    },
    InvalidCadence {
        got: String,
    },
    Internal {
        summary: String,
        path: Option<&'static str>,
        fix: Option<&'static str>,
    },
}

impl ErrorInfo for ComputeParseError {
    fn rule_id(&self) -> &'static str {
        match self {
            Self::WrongKind { .. } => "CMP-3",
            Self::InvalidInputType { .. } => "CMP-13",
            Self::InvalidOutputType { .. } => "CMP-20",
            Self::InvalidInputCardinality { .. } => "CMP-14",
            Self::InvalidParameterType { .. } => "CMP-15",
            Self::InvalidParameterDefault { .. } => "CMP-19",
            Self::InvalidCadence { .. } => "CMP-16",
            Self::Internal { .. } => "INTERNAL",
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
            Self::WrongKind { got } => {
                Cow::Owned(format!("Wrong kind: expected compute, got '{got}'"))
            }
            Self::InvalidInputType { input, got } => {
                Cow::Owned(format!("Input '{input}' has invalid type '{got}'"))
            }
            Self::InvalidOutputType { output, got } => {
                Cow::Owned(format!("Output '{output}' has invalid type '{got}'"))
            }
            Self::InvalidInputCardinality { input, got } => {
                Cow::Owned(format!("Input '{input}' has invalid cardinality '{got}'"))
            }
            Self::InvalidParameterType { parameter, got } => Cow::Owned(format!(
                "Parameter '{parameter}' has unsupported type '{got}'"
            )),
            Self::InvalidParameterDefault {
                parameter,
                expected,
                reason,
            } => Cow::Owned(format!(
                "Parameter '{parameter}' default does not match {expected:?}: {reason}"
            )),
            Self::InvalidCadence { got } => {
                Cow::Owned(format!("Compute cadence must be continuous (got '{got}')"))
            }
            Self::Internal { summary, .. } => Cow::Owned(summary.clone()),
        }
    }

    fn path(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::WrongKind { .. } => Some(Cow::Borrowed("$.kind")),
            Self::InvalidInputType { .. } => Some(Cow::Borrowed("$.inputs[].type")),
            Self::InvalidOutputType { .. } => Some(Cow::Borrowed("$.outputs[].type")),
            Self::InvalidInputCardinality { .. } => Some(Cow::Borrowed("$.inputs[].cardinality")),
            Self::InvalidParameterType { .. } => Some(Cow::Borrowed("$.parameters[].type")),
            Self::InvalidParameterDefault { .. } => Some(Cow::Borrowed("$.parameters[].default")),
            Self::InvalidCadence { .. } => Some(Cow::Borrowed("$.execution.cadence")),
            Self::Internal { path, .. } => path.map(Cow::Borrowed),
        }
    }

    fn fix(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::WrongKind { .. } => Some(Cow::Borrowed("Set kind: compute")),
            Self::InvalidInputType { .. } => Some(Cow::Borrowed(
                "Use a valid input type: number, bool, or series",
            )),
            Self::InvalidOutputType { .. } => Some(Cow::Borrowed(
                "Use a valid output type: number, bool, series, or string",
            )),
            Self::InvalidInputCardinality { .. } => {
                Some(Cow::Borrowed("Set input cardinality to single"))
            }
            Self::InvalidParameterType { .. } => Some(Cow::Borrowed(
                "Change parameter type to int, number, or bool",
            )),
            Self::InvalidParameterDefault { .. } => Some(Cow::Borrowed(
                "Change parameter default value to match the declared type",
            )),
            Self::InvalidCadence { .. } => Some(Cow::Borrowed("Set cadence: continuous")),
            Self::Internal { fix, .. } => fix.map(Cow::Borrowed),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawComputeManifest {
    id: String,
    version: String,
    kind: String,
    inputs: Vec<RawComputeInput>,
    outputs: Vec<RawComputeOutput>,
    #[serde(default)]
    parameters: Vec<RawComputeParameter>,
    execution: RawComputeExecution,
    errors: RawComputeErrors,
    state: RawComputeState,
    side_effects: bool,
}

#[derive(Debug, Deserialize)]
struct RawComputeInput {
    name: String,
    #[serde(rename = "type")]
    value_type: String,
    required: bool,
    #[serde(default)]
    cardinality: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawComputeOutput {
    name: String,
    #[serde(rename = "type")]
    value_type: String,
}

#[derive(Debug, Deserialize)]
struct RawComputeParameter {
    name: String,
    #[serde(rename = "type")]
    value_type: String,
    default: Option<serde_json::Value>,
    required: bool,
    bounds: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawComputeExecution {
    deterministic: bool,
    cadence: String,
    may_error: bool,
}

#[derive(Debug, Deserialize)]
struct RawComputeErrors {
    allowed: bool,
    #[serde(default)]
    types: Vec<String>,
    deterministic: bool,
}

#[derive(Debug, Deserialize)]
struct RawComputeState {
    allowed: bool,
    resettable: bool,
    description: Option<String>,
}

fn raw_to_compute_manifest(
    raw: RawComputeManifest,
) -> Result<ComputePrimitiveManifest, ComputeParseError> {
    let kind = parse_compute_kind(&raw.kind)?;
    let inputs = raw
        .inputs
        .into_iter()
        .map(|input| {
            let value_type = parse_value_type(&input.value_type).ok_or_else(|| {
                ComputeParseError::InvalidInputType {
                    input: input.name.clone(),
                    got: input.value_type.clone(),
                }
            })?;
            let cardinality = parse_compute_cardinality(&input.name, input.cardinality.as_deref())?;
            Ok(ergo_runtime::compute::InputSpec {
                name: input.name,
                value_type,
                required: input.required,
                cardinality,
            })
        })
        .collect::<Result<Vec<_>, ComputeParseError>>()?;

    let outputs = raw
        .outputs
        .into_iter()
        .map(|output| {
            let value_type = parse_value_type(&output.value_type).ok_or_else(|| {
                ComputeParseError::InvalidOutputType {
                    output: output.name.clone(),
                    got: output.value_type.clone(),
                }
            })?;
            Ok(ergo_runtime::compute::OutputSpec {
                name: output.name,
                value_type,
            })
        })
        .collect::<Result<Vec<_>, ComputeParseError>>()?;

    let parameters = raw
        .parameters
        .into_iter()
        .map(|param| {
            let value_type = parse_compute_parameter_type(&param.name, &param.value_type)?;
            let default = parse_common_default(&param.name, value_type.clone(), param.default)?;
            Ok(ComputeParameterSpec {
                name: param.name,
                value_type,
                default,
                required: param.required,
                bounds: param.bounds,
            })
        })
        .collect::<Result<Vec<_>, ComputeParseError>>()?;

    let execution = ComputeExecutionSpec {
        deterministic: raw.execution.deterministic,
        cadence: parse_compute_cadence(&raw.execution.cadence)?,
        may_error: raw.execution.may_error,
    };

    let errors = ergo_runtime::compute::ErrorSpec {
        allowed: raw.errors.allowed,
        types: raw
            .errors
            .types
            .into_iter()
            .map(|ty| {
                parse_compute_error_type(&ty).ok_or_else(|| ComputeParseError::Internal {
                    summary: format!("invalid error type '{}'", ty),
                    path: Some("$.errors.types[]"),
                    fix: Some("Use a valid error type: DivisionByZero or NonFiniteResult"),
                })
            })
            .collect::<Result<Vec<_>, ComputeParseError>>()?,
        deterministic: raw.errors.deterministic,
    };

    let state = ComputeStateSpec {
        allowed: raw.state.allowed,
        resettable: raw.state.resettable,
        description: raw.state.description,
    };

    Ok(ComputePrimitiveManifest {
        id: raw.id,
        version: raw.version,
        kind,
        inputs,
        outputs,
        parameters,
        execution,
        errors,
        state,
        side_effects: raw.side_effects,
    })
}

fn parse_compute_kind(input: &str) -> Result<PrimitiveKind, ComputeParseError> {
    match input.to_ascii_lowercase().as_str() {
        "compute" => Ok(PrimitiveKind::Compute),
        other => Err(ComputeParseError::WrongKind {
            got: other.to_string(),
        }),
    }
}

fn parse_compute_cadence(input: &str) -> Result<ComputeCadence, ComputeParseError> {
    match input.to_ascii_lowercase().as_str() {
        "continuous" => Ok(ComputeCadence::Continuous),
        "event" => Ok(ComputeCadence::Event),
        other => Err(ComputeParseError::InvalidCadence {
            got: other.to_string(),
        }),
    }
}

fn parse_compute_cardinality(
    input_name: &str,
    input: Option<&str>,
) -> Result<ComputeCardinality, ComputeParseError> {
    match input.map(|s| s.to_ascii_lowercase()) {
        None => Ok(ComputeCardinality::Single),
        Some(value) if value == "single" => Ok(ComputeCardinality::Single),
        Some(value) if value == "multiple" => Ok(ComputeCardinality::Multiple),
        Some(other) => Err(ComputeParseError::InvalidInputCardinality {
            input: input_name.to_string(),
            got: other,
        }),
    }
}

fn parse_compute_error_type(input: &str) -> Option<ComputeErrorType> {
    match input.to_ascii_lowercase().as_str() {
        "divisionbyzero" | "division_by_zero" => Some(ComputeErrorType::DivisionByZero),
        "nonfiniteresult" | "non_finite_result" => Some(ComputeErrorType::NonFiniteResult),
        _ => None,
    }
}

// This stays family-local even though the shape is similar across manifest
// families because compute defaults lower into `Value`, preserve `int` ->
// `number` aliasing, and own `CMP-*` mismatch wording.
fn parse_common_default(
    parameter: &str,
    value_type: ValueType,
    raw: Option<serde_json::Value>,
) -> Result<Option<Value>, ComputeParseError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    let expected = value_type.clone();
    let value = match value_type {
        ValueType::Number => Value::Number(raw.as_f64().ok_or_else(|| {
            ComputeParseError::InvalidParameterDefault {
                parameter: parameter.to_string(),
                expected,
                reason: "expected numeric default".to_string(),
            }
        })?),
        ValueType::Bool => Value::Bool(raw.as_bool().ok_or_else(|| {
            ComputeParseError::InvalidParameterDefault {
                parameter: parameter.to_string(),
                expected,
                reason: "expected boolean default".to_string(),
            }
        })?),
        ValueType::String => Value::String(
            raw.as_str()
                .ok_or_else(|| ComputeParseError::InvalidParameterDefault {
                    parameter: parameter.to_string(),
                    expected,
                    reason: "expected string default".to_string(),
                })?
                .to_string(),
        ),
        ValueType::Series => {
            let array =
                raw.as_array()
                    .ok_or_else(|| ComputeParseError::InvalidParameterDefault {
                        parameter: parameter.to_string(),
                        expected: expected.clone(),
                        reason: "expected array default".to_string(),
                    })?;
            let mut values = Vec::with_capacity(array.len());
            for val in array {
                let num =
                    val.as_f64()
                        .ok_or_else(|| ComputeParseError::InvalidParameterDefault {
                            parameter: parameter.to_string(),
                            expected: expected.clone(),
                            reason: "expected numeric series default".to_string(),
                        })?;
                values.push(num);
            }
            Value::Series(values)
        }
    };
    Ok(Some(value))
}

// Keep this inline because it protects a private alias parser that is no longer
// intentionally exposed through the parent-module test seam.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_compute_parameter_type_accepts_int_alias_as_number() {
        let value_type =
            parse_compute_parameter_type("threshold", "int").expect("int alias should parse");

        assert_eq!(value_type, ValueType::Number);
    }
}
