//! manifest_usecases::trigger
//!
//! Purpose:
//! - Parse and normalize file-backed trigger manifests into typed runtime
//!   trigger manifests.
//!
//! Owns:
//! - Trigger raw DTOs, trigger-specific parse errors, parameter/default
//!   parsing, and trigger lowering into the typed runtime manifest.
//!
//! Does not own:
//! - Host manifest dispatch, summary projection, or composition routing.
//!
//! Connects to:
//! - `manifest_usecases.rs`, which dispatches `kind: trigger` manifests here
//!   while preserving the public host manifest API.
//! - `ergo_runtime::trigger`, which consumes the lowered typed manifest and
//!   validates trigger-specific registration semantics after host parsing.
//!
//! Safety notes:
//! - Trigger-specific parameter/default parsing stays local so `TRG-*` rule
//!   ownership and trigger-only cardinality/output constraints remain explicit.
//! - This module preserves the current file-backed trigger contract, including
//!   event output expectations and single-vs-multiple input cardinality checks.

use std::borrow::Cow;

use ergo_runtime::common::{doc_anchor_for_rule, ErrorInfo, Phase, RuleViolation};
use ergo_runtime::trigger::{
    Cadence as TriggerCadence, Cardinality as TriggerCardinality,
    ExecutionSpec as TriggerExecutionSpec, ParameterSpec as TriggerParameterSpec,
    ParameterType as TriggerParameterType, ParameterValue as TriggerParameterValue, TriggerKind,
    TriggerPrimitiveManifest, TriggerValueType,
};
use serde::Deserialize;

use super::common::parse_int_value;
use super::parse_error_violation;

pub(super) fn parse_manifest(
    source_label: &str,
    value: serde_json::Value,
) -> Result<TriggerPrimitiveManifest, RuleViolation> {
    let raw = serde_json::from_value::<RawTriggerManifest>(value).map_err(|err| {
        parse_error_violation(format!("parse trigger manifest '{}': {err}", source_label))
    })?;
    raw_to_trigger_manifest(raw).map_err(RuleViolation::from)
}

#[derive(Debug)]
enum TriggerParseError {
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
    InvalidParameterDefault {
        parameter: String,
        expected: TriggerParameterType,
        reason: String,
    },
    Internal {
        summary: String,
        path: Option<&'static str>,
        fix: Option<&'static str>,
    },
}

impl ErrorInfo for TriggerParseError {
    fn rule_id(&self) -> &'static str {
        match self {
            Self::WrongKind { .. } => "TRG-3",
            Self::InvalidInputType { .. } => "TRG-6",
            Self::InvalidOutputType { .. } => "TRG-8",
            Self::InvalidInputCardinality { .. } => "TRG-12",
            Self::InvalidParameterDefault { .. } => "TRG-14",
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
                Cow::Owned(format!("Wrong kind: expected trigger, got '{got}'"))
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
            Self::InvalidParameterDefault {
                parameter,
                expected,
                reason,
            } => Cow::Owned(format!(
                "Parameter '{parameter}' default does not match {expected:?}: {reason}"
            )),
            Self::Internal { summary, .. } => Cow::Owned(summary.clone()),
        }
    }

    fn path(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::WrongKind { .. } => Some(Cow::Borrowed("$.kind")),
            Self::InvalidInputType { .. } => Some(Cow::Borrowed("$.inputs[].type")),
            Self::InvalidOutputType { .. } => Some(Cow::Borrowed("$.outputs[0].type")),
            Self::InvalidInputCardinality { .. } => Some(Cow::Borrowed("$.inputs[].cardinality")),
            Self::InvalidParameterDefault { .. } => Some(Cow::Borrowed("$.parameters[].default")),
            Self::Internal { path, .. } => path.map(Cow::Borrowed),
        }
    }

    fn fix(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::WrongKind { .. } => Some(Cow::Borrowed("Set kind: trigger")),
            Self::InvalidInputType { .. } => Some(Cow::Borrowed(
                "Use a valid input type: number, bool, series, or event",
            )),
            Self::InvalidOutputType { .. } => Some(Cow::Borrowed("Output type must be event")),
            Self::InvalidInputCardinality { .. } => {
                Some(Cow::Borrowed("Set input cardinality to single"))
            }
            Self::InvalidParameterDefault { .. } => Some(Cow::Borrowed(
                "Change parameter default value to match the declared parameter type",
            )),
            Self::Internal { fix, .. } => fix.map(Cow::Borrowed),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawTriggerManifest {
    id: String,
    version: String,
    kind: String,
    inputs: Vec<RawTriggerInput>,
    outputs: Vec<RawTriggerOutput>,
    #[serde(default)]
    parameters: Vec<RawTriggerParameter>,
    execution: RawTriggerExecution,
    state: RawTriggerState,
    side_effects: bool,
}

#[derive(Debug, Deserialize)]
struct RawTriggerInput {
    name: String,
    #[serde(rename = "type")]
    value_type: String,
    required: bool,
    #[serde(default)]
    cardinality: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawTriggerOutput {
    name: String,
    #[serde(rename = "type")]
    value_type: String,
}

#[derive(Debug, Deserialize)]
struct RawTriggerParameter {
    name: String,
    #[serde(rename = "type")]
    value_type: String,
    default: Option<serde_json::Value>,
    required: bool,
    bounds: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawTriggerExecution {
    deterministic: bool,
    cadence: String,
}

#[derive(Debug, Deserialize)]
struct RawTriggerState {
    allowed: bool,
    description: Option<String>,
}

fn raw_to_trigger_manifest(
    raw: RawTriggerManifest,
) -> Result<TriggerPrimitiveManifest, TriggerParseError> {
    let kind = parse_trigger_kind(&raw.kind)?;
    let inputs = raw
        .inputs
        .into_iter()
        .map(|input| {
            let value_type = parse_trigger_value_type(&input.value_type).ok_or_else(|| {
                TriggerParseError::InvalidInputType {
                    input: input.name.clone(),
                    got: input.value_type.clone(),
                }
            })?;
            let cardinality = parse_trigger_cardinality(&input.name, input.cardinality.as_deref())?;
            Ok(ergo_runtime::trigger::InputSpec {
                name: input.name,
                value_type,
                required: input.required,
                cardinality,
            })
        })
        .collect::<Result<Vec<_>, TriggerParseError>>()?;

    let outputs = raw
        .outputs
        .into_iter()
        .map(|output| {
            let value_type = parse_trigger_value_type(&output.value_type).ok_or_else(|| {
                TriggerParseError::InvalidOutputType {
                    output: output.name.clone(),
                    got: output.value_type.clone(),
                }
            })?;
            Ok(ergo_runtime::trigger::OutputSpec {
                name: output.name,
                value_type,
            })
        })
        .collect::<Result<Vec<_>, TriggerParseError>>()?;

    let parameters = raw
        .parameters
        .into_iter()
        .map(|param| {
            let value_type = parse_trigger_parameter_type(&param.value_type).ok_or_else(|| {
                TriggerParseError::Internal {
                    summary: format!("invalid parameter type '{}'", param.value_type),
                    path: Some("$.parameters[].type"),
                    fix: Some("Use a valid parameter type: int, number, bool, string, or enum"),
                }
            })?;
            let default =
                parse_trigger_parameter_default(&param.name, value_type.clone(), param.default)?;
            Ok(TriggerParameterSpec {
                name: param.name,
                value_type,
                default,
                required: param.required,
                bounds: param.bounds,
            })
        })
        .collect::<Result<Vec<_>, TriggerParseError>>()?;

    let execution = TriggerExecutionSpec {
        deterministic: raw.execution.deterministic,
        cadence: parse_trigger_cadence(&raw.execution.cadence)?,
    };

    let state = ergo_runtime::trigger::StateSpec {
        allowed: raw.state.allowed,
        description: raw.state.description,
    };

    Ok(TriggerPrimitiveManifest {
        id: raw.id,
        version: raw.version,
        kind,
        inputs,
        outputs,
        parameters,
        execution,
        state,
        side_effects: raw.side_effects,
    })
}

fn parse_trigger_parameter_type(input: &str) -> Option<TriggerParameterType> {
    match input.to_ascii_lowercase().as_str() {
        "int" => Some(TriggerParameterType::Int),
        "number" => Some(TriggerParameterType::Number),
        "bool" | "boolean" => Some(TriggerParameterType::Bool),
        "string" => Some(TriggerParameterType::String),
        "enum" => Some(TriggerParameterType::Enum),
        _ => None,
    }
}

// This stays family-local even though the shape is similar across manifest
// families because trigger defaults map onto trigger-only parameter enums and
// keep `TRG-*` mismatch ownership local to this pipeline.
fn parse_trigger_parameter_default(
    parameter: &str,
    value_type: TriggerParameterType,
    raw: Option<serde_json::Value>,
) -> Result<Option<TriggerParameterValue>, TriggerParseError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    let value = match value_type {
        TriggerParameterType::Int => {
            TriggerParameterValue::Int(parse_int_value(&raw).map_err(|reason| {
                TriggerParseError::InvalidParameterDefault {
                    parameter: parameter.to_string(),
                    expected: value_type.clone(),
                    reason,
                }
            })?)
        }
        TriggerParameterType::Number => {
            TriggerParameterValue::Number(raw.as_f64().ok_or_else(|| {
                TriggerParseError::InvalidParameterDefault {
                    parameter: parameter.to_string(),
                    expected: value_type.clone(),
                    reason: "expected numeric default".to_string(),
                }
            })?)
        }
        TriggerParameterType::Bool => {
            TriggerParameterValue::Bool(raw.as_bool().ok_or_else(|| {
                TriggerParseError::InvalidParameterDefault {
                    parameter: parameter.to_string(),
                    expected: value_type.clone(),
                    reason: "expected boolean default".to_string(),
                }
            })?)
        }
        TriggerParameterType::String => TriggerParameterValue::String(
            raw.as_str()
                .ok_or_else(|| TriggerParseError::InvalidParameterDefault {
                    parameter: parameter.to_string(),
                    expected: value_type.clone(),
                    reason: "expected string default".to_string(),
                })?
                .to_string(),
        ),
        TriggerParameterType::Enum => TriggerParameterValue::Enum(
            raw.as_str()
                .ok_or_else(|| TriggerParseError::InvalidParameterDefault {
                    parameter: parameter.to_string(),
                    expected: value_type.clone(),
                    reason: "expected enum default".to_string(),
                })?
                .to_string(),
        ),
    };
    Ok(Some(value))
}

fn parse_trigger_kind(input: &str) -> Result<TriggerKind, TriggerParseError> {
    match input.to_ascii_lowercase().as_str() {
        "trigger" => Ok(TriggerKind::Trigger),
        other => Err(TriggerParseError::WrongKind {
            got: other.to_string(),
        }),
    }
}

fn parse_trigger_cadence(input: &str) -> Result<TriggerCadence, TriggerParseError> {
    match input.to_ascii_lowercase().as_str() {
        "continuous" => Ok(TriggerCadence::Continuous),
        "event" => Ok(TriggerCadence::Event),
        other => Err(TriggerParseError::Internal {
            summary: format!("invalid cadence '{other}'"),
            path: Some("$.execution.cadence"),
            fix: Some("Use a valid trigger cadence: continuous or event"),
        }),
    }
}

fn parse_trigger_cardinality(
    input_name: &str,
    input: Option<&str>,
) -> Result<TriggerCardinality, TriggerParseError> {
    match input.map(|s| s.to_ascii_lowercase()) {
        None => Ok(TriggerCardinality::Single),
        Some(value) if value == "single" => Ok(TriggerCardinality::Single),
        Some(value) if value == "multiple" => Ok(TriggerCardinality::Multiple),
        Some(other) => Err(TriggerParseError::InvalidInputCardinality {
            input: input_name.to_string(),
            got: other,
        }),
    }
}

fn parse_trigger_value_type(input: &str) -> Option<TriggerValueType> {
    match input.to_ascii_lowercase().as_str() {
        "event" => Some(TriggerValueType::Event),
        "number" => Some(TriggerValueType::Number),
        "bool" | "boolean" => Some(TriggerValueType::Bool),
        "series" => Some(TriggerValueType::Series),
        _ => None,
    }
}
