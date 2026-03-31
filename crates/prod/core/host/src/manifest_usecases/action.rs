//! manifest_usecases::action
//!
//! Purpose:
//! - Parse and normalize file-backed action manifests into typed runtime
//!   action manifests.
//!
//! Owns:
//! - Action raw DTOs, action-specific parse errors, action parameter/default
//!   parsing, writes-only file-surface normalization, and action lowering into
//!   the typed runtime manifest.
//! - Action default-parameter projection for adapter-composition checks.
//!
//! Does not own:
//! - Host manifest dispatch, summary projection, or adapter-compose routing.
//! - The richer runtime/custom intent manifest surface beyond the current
//!   file-backed writes-only path.
//!
//! Connects to:
//! - `manifest_usecases.rs`, which dispatches `kind: action` manifests here
//!   while preserving the public host API and host-owned error projection.
//! - `ergo_runtime::action`, which consumes the lowered typed manifest.
//! - `ergo_adapter` composition checks, which use this module's default
//!   parameter projection for file-backed action manifests.
//!
//! Safety notes:
//! - The file-backed action path still normalizes missing `effects` to empty
//!   writes with no intents; richer runtime intent surfaces remain out of scope.
//! - Action-specific default parsing stays local so `ACT-*` rule ownership and
//!   writes-only file-surface constraints do not drift behind a generic helper.

use std::borrow::Cow;
use std::collections::HashMap;

use ergo_runtime::action::{
    ActionEffects, ActionKind, ActionPrimitiveManifest, ActionValueType,
    ParameterType as ActionParameterType, ParameterValue as ActionParameterValue,
};
use ergo_runtime::cluster::ParameterValue as ClusterParameterValue;
use ergo_runtime::common::{doc_anchor_for_rule, ErrorInfo, Phase, RuleViolation};
use serde::Deserialize;

use super::common::{parse_int_value, parse_value_type};
use super::parse_error_violation;

pub(super) fn parse_manifest(
    source_label: &str,
    value: serde_json::Value,
) -> Result<ActionPrimitiveManifest, RuleViolation> {
    let raw = serde_json::from_value::<RawActionManifest>(value).map_err(|err| {
        parse_error_violation(format!("parse action manifest '{}': {err}", source_label))
    })?;
    raw_to_action_manifest(raw).map_err(RuleViolation::from)
}

pub(super) fn default_params_for_composition(
    manifest: &ActionPrimitiveManifest,
) -> HashMap<String, ClusterParameterValue> {
    manifest
        .parameters
        .iter()
        .filter_map(|param| {
            param
                .default
                .as_ref()
                .map(|value| (param.name.clone(), action_param_value_to_cluster(value)))
        })
        .collect()
}

fn action_param_value_to_cluster(value: &ActionParameterValue) -> ClusterParameterValue {
    match value {
        ActionParameterValue::Int(v) => ClusterParameterValue::Int(*v),
        ActionParameterValue::Number(v) => ClusterParameterValue::Number(*v),
        ActionParameterValue::Bool(v) => ClusterParameterValue::Bool(*v),
        ActionParameterValue::String(v) => ClusterParameterValue::String(v.clone()),
        ActionParameterValue::Enum(v) => ClusterParameterValue::Enum(v.clone()),
    }
}

#[derive(Debug)]
enum ActionParseError {
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
    InvalidWriteType {
        name: String,
        got: String,
    },
    InvalidParameterDefault {
        parameter: String,
        expected: ActionParameterType,
        reason: String,
    },
    Internal {
        summary: String,
        path: Option<&'static str>,
        fix: Option<&'static str>,
    },
}

impl ErrorInfo for ActionParseError {
    fn rule_id(&self) -> &'static str {
        match self {
            Self::WrongKind { .. } => "ACT-3",
            Self::InvalidInputType { .. } => "ACT-6",
            Self::InvalidOutputType { .. } => "ACT-9",
            Self::InvalidWriteType { .. } => "ACT-15",
            Self::InvalidParameterDefault { .. } => "ACT-19",
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
                Cow::Owned(format!("Wrong kind: expected action, got '{got}'"))
            }
            Self::InvalidInputType { input, got } => {
                Cow::Owned(format!("Input '{input}' has invalid type '{got}'"))
            }
            Self::InvalidOutputType { output, got } => {
                Cow::Owned(format!("Output '{output}' has invalid type '{got}'"))
            }
            Self::InvalidWriteType { name, got } => {
                Cow::Owned(format!("Write '{name}' has invalid type '{got}'"))
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
            Self::InvalidWriteType { .. } => Some(Cow::Borrowed("$.effects.writes[].type")),
            Self::InvalidParameterDefault { .. } => Some(Cow::Borrowed("$.parameters[].default")),
            Self::Internal { path, .. } => path.map(Cow::Borrowed),
        }
    }

    fn fix(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::WrongKind { .. } => Some(Cow::Borrowed("Set kind: action")),
            Self::InvalidInputType { .. } => Some(Cow::Borrowed(
                "Use a valid input type: event, number, series, bool, or string",
            )),
            Self::InvalidOutputType { .. } => Some(Cow::Borrowed("Output type must be event")),
            Self::InvalidWriteType { .. } => Some(Cow::Borrowed(
                "Write types must be Number, Series, Bool, or String",
            )),
            Self::InvalidParameterDefault { .. } => Some(Cow::Borrowed(
                "Change parameter default value to match the declared parameter type",
            )),
            Self::Internal { fix, .. } => fix.map(Cow::Borrowed),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawActionManifest {
    id: String,
    version: String,
    kind: String,
    inputs: Vec<RawActionInput>,
    outputs: Vec<RawActionOutput>,
    #[serde(default)]
    parameters: Vec<RawActionParameter>,
    #[serde(default)]
    effects: RawActionEffects,
    execution: RawActionExecution,
    state: RawActionState,
    side_effects: bool,
}

#[derive(Debug, Deserialize)]
struct RawActionInput {
    name: String,
    #[serde(rename = "type")]
    value_type: String,
    required: bool,
    #[serde(default)]
    cardinality: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawActionOutput {
    name: String,
    #[serde(rename = "type")]
    value_type: String,
}

#[derive(Debug, Deserialize)]
struct RawActionParameter {
    name: String,
    #[serde(rename = "type")]
    value_type: String,
    default: Option<serde_json::Value>,
    required: bool,
    bounds: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RawActionEffects {
    #[serde(default)]
    writes: Vec<RawActionWrite>,
}

#[derive(Debug, Deserialize)]
struct RawActionWrite {
    name: String,
    #[serde(rename = "type")]
    value_type: String,
    from_input: String,
}

#[derive(Debug, Deserialize)]
struct RawActionExecution {
    deterministic: bool,
    retryable: bool,
}

#[derive(Debug, Deserialize)]
struct RawActionState {
    allowed: bool,
}

fn raw_to_action_manifest(
    raw: RawActionManifest,
) -> Result<ActionPrimitiveManifest, ActionParseError> {
    let kind = parse_action_kind(&raw.kind)?;
    let inputs = raw
        .inputs
        .into_iter()
        .map(|input| {
            let value_type = parse_action_value_type(&input.value_type).ok_or_else(|| {
                ActionParseError::InvalidInputType {
                    input: input.name.clone(),
                    got: input.value_type.clone(),
                }
            })?;
            let cardinality = parse_action_cardinality(input.cardinality.as_deref())?;
            Ok(ergo_runtime::action::InputSpec {
                name: input.name,
                value_type,
                required: input.required,
                cardinality,
            })
        })
        .collect::<Result<Vec<_>, ActionParseError>>()?;

    let outputs = raw
        .outputs
        .into_iter()
        .map(|output| {
            let value_type = parse_action_value_type(&output.value_type).ok_or_else(|| {
                ActionParseError::InvalidOutputType {
                    output: output.name.clone(),
                    got: output.value_type.clone(),
                }
            })?;
            Ok(ergo_runtime::action::OutputSpec {
                name: output.name,
                value_type,
            })
        })
        .collect::<Result<Vec<_>, ActionParseError>>()?;

    let parameters = raw
        .parameters
        .into_iter()
        .map(|param| {
            let value_type = parse_action_parameter_type(&param.value_type).ok_or_else(|| {
                ActionParseError::Internal {
                    summary: format!("invalid parameter type '{}'", param.value_type),
                    path: Some("$.parameters[].type"),
                    fix: Some("Use a valid parameter type: int, number, bool, string, or enum"),
                }
            })?;
            let default =
                parse_action_parameter_default(&param.name, value_type.clone(), param.default)?;
            Ok(ergo_runtime::action::ParameterSpec {
                name: param.name,
                value_type,
                default,
                required: param.required,
                bounds: param.bounds,
            })
        })
        .collect::<Result<Vec<_>, ActionParseError>>()?;

    let effects = ActionEffects {
        writes: raw
            .effects
            .writes
            .into_iter()
            .map(|write| {
                let value_type = parse_value_type(&write.value_type).ok_or_else(|| {
                    ActionParseError::InvalidWriteType {
                        name: write.name.clone(),
                        got: write.value_type.clone(),
                    }
                })?;
                Ok(ergo_runtime::action::ActionWriteSpec {
                    name: write.name,
                    value_type,
                    from_input: write.from_input,
                })
            })
            .collect::<Result<Vec<_>, ActionParseError>>()?,
        intents: vec![],
    };

    let execution = ergo_runtime::action::ExecutionSpec {
        deterministic: raw.execution.deterministic,
        retryable: raw.execution.retryable,
    };

    let state = ergo_runtime::action::StateSpec {
        allowed: raw.state.allowed,
    };

    Ok(ActionPrimitiveManifest {
        id: raw.id,
        version: raw.version,
        kind,
        inputs,
        outputs,
        parameters,
        effects,
        execution,
        state,
        side_effects: raw.side_effects,
    })
}

fn parse_action_parameter_type(input: &str) -> Option<ActionParameterType> {
    match input.to_ascii_lowercase().as_str() {
        "int" => Some(ActionParameterType::Int),
        "number" => Some(ActionParameterType::Number),
        "bool" | "boolean" => Some(ActionParameterType::Bool),
        "string" => Some(ActionParameterType::String),
        "enum" => Some(ActionParameterType::Enum),
        _ => None,
    }
}

// This stays family-local even though the shape is similar across manifest
// families because the action parameter enum, writes-only surface, and `ACT-*`
// error ownership differ from the other pipelines.
fn parse_action_parameter_default(
    parameter: &str,
    value_type: ActionParameterType,
    raw: Option<serde_json::Value>,
) -> Result<Option<ActionParameterValue>, ActionParseError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    let value = match value_type {
        ActionParameterType::Int => {
            ActionParameterValue::Int(parse_int_value(&raw).map_err(|reason| {
                ActionParseError::InvalidParameterDefault {
                    parameter: parameter.to_string(),
                    expected: value_type.clone(),
                    reason,
                }
            })?)
        }
        ActionParameterType::Number => {
            ActionParameterValue::Number(raw.as_f64().ok_or_else(|| {
                ActionParseError::InvalidParameterDefault {
                    parameter: parameter.to_string(),
                    expected: value_type.clone(),
                    reason: "expected numeric default".to_string(),
                }
            })?)
        }
        ActionParameterType::Bool => {
            ActionParameterValue::Bool(raw.as_bool().ok_or_else(|| {
                ActionParseError::InvalidParameterDefault {
                    parameter: parameter.to_string(),
                    expected: value_type.clone(),
                    reason: "expected boolean default".to_string(),
                }
            })?)
        }
        ActionParameterType::String => ActionParameterValue::String(
            raw.as_str()
                .ok_or_else(|| ActionParseError::InvalidParameterDefault {
                    parameter: parameter.to_string(),
                    expected: value_type.clone(),
                    reason: "expected string default".to_string(),
                })?
                .to_string(),
        ),
        ActionParameterType::Enum => ActionParameterValue::Enum(
            raw.as_str()
                .ok_or_else(|| ActionParseError::InvalidParameterDefault {
                    parameter: parameter.to_string(),
                    expected: value_type.clone(),
                    reason: "expected enum default".to_string(),
                })?
                .to_string(),
        ),
    };
    Ok(Some(value))
}

fn parse_action_kind(input: &str) -> Result<ActionKind, ActionParseError> {
    match input.to_ascii_lowercase().as_str() {
        "action" => Ok(ActionKind::Action),
        other => Err(ActionParseError::WrongKind {
            got: other.to_string(),
        }),
    }
}

fn parse_action_cardinality(
    input: Option<&str>,
) -> Result<ergo_runtime::action::Cardinality, ActionParseError> {
    match input.map(|s| s.to_ascii_lowercase()) {
        None => Ok(ergo_runtime::action::Cardinality::Single),
        Some(value) if value == "single" => Ok(ergo_runtime::action::Cardinality::Single),
        Some(other) => Err(ActionParseError::Internal {
            summary: format!("invalid cardinality '{other}'"),
            path: Some("$.inputs[].cardinality"),
            fix: Some("Set input cardinality to single"),
        }),
    }
}

fn parse_action_value_type(input: &str) -> Option<ActionValueType> {
    match input.to_ascii_lowercase().as_str() {
        "event" => Some(ActionValueType::Event),
        "number" => Some(ActionValueType::Number),
        "series" => Some(ActionValueType::Series),
        "bool" | "boolean" => Some(ActionValueType::Bool),
        "string" => Some(ActionValueType::String),
        _ => None,
    }
}
