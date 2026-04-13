//! manifest_usecases::source
//!
//! Purpose:
//! - Lower raw file-backed source manifests into typed runtime source manifests
//!   for the host manifest ingress surface.
//!
//! Owns:
//! - Source-specific raw DTOs, parse errors, kind/cadence/default parsing, and
//!   default-parameter projection used during adapter composition checks.
//!
//! Does not own:
//! - The public host entrypoints in `manifest_usecases.rs`.
//! - Runtime source registration semantics enforced by `SourceRegistry`.
//!
//! Connects to:
//! - `manifest_usecases.rs`, which dispatches `kind: source` manifests here and
//!   exposes host-facing validation/composition usecases.
//! - `ergo_runtime::source`, which consumes the lowered typed manifest.
//!
//! Safety notes:
//! - This module preserves the current file-surface aliases like
//!   `boolean` -> `Bool`.
//! - Source manifests still reject inputs on the file-backed path and keep the
//!   current `continuous` cadence-only contract.

use std::borrow::Cow;
use std::collections::HashMap;

use ergo_runtime::cluster::ParameterValue as ClusterParameterValue;
use ergo_runtime::common::{doc_anchor_for_rule, ErrorInfo, Phase, RuleViolation};
use ergo_runtime::source::{
    ContextRequirement, ExecutionSpec as SourceExecutionSpec, ParameterSpec as SourceParameterSpec,
    ParameterType as SourceParameterType, ParameterValue as SourceParameterValue, SourceKind,
    SourcePrimitiveManifest, SourceRequires, StateSpec as SourceStateSpec,
};
use serde::Deserialize;

use super::common::{parse_int_value, parse_value_type};

pub(super) fn parse_manifest(
    source_label: &str,
    value: serde_json::Value,
) -> Result<SourcePrimitiveManifest, RuleViolation> {
    let raw = serde_json::from_value::<RawSourceManifest>(value).map_err(|err| RuleViolation {
        rule_id: "INTERNAL",
        phase: Phase::Registration,
        doc_anchor: doc_anchor_for_rule("INTERNAL"),
        summary: Cow::Owned(format!("parse source manifest '{}': {err}", source_label)),
        path: None,
        fix: None,
    })?;
    raw_to_source_manifest(raw).map_err(RuleViolation::from)
}

pub(super) fn default_params_for_composition(
    manifest: &SourcePrimitiveManifest,
) -> HashMap<String, ClusterParameterValue> {
    manifest
        .parameters
        .iter()
        .filter_map(|param| {
            param
                .default
                .as_ref()
                .map(|value| (param.name.clone(), source_param_value_to_cluster(value)))
        })
        .collect()
}

fn source_param_value_to_cluster(value: &SourceParameterValue) -> ClusterParameterValue {
    match value {
        SourceParameterValue::Int(v) => ClusterParameterValue::Int(*v),
        SourceParameterValue::Number(v) => ClusterParameterValue::Number(*v),
        SourceParameterValue::Bool(v) => ClusterParameterValue::Bool(*v),
        SourceParameterValue::String(v) => ClusterParameterValue::String(v.clone()),
        SourceParameterValue::Enum(v) => ClusterParameterValue::Enum(v.clone()),
    }
}

#[derive(Debug)]
enum SourceParseError {
    WrongKind {
        got: String,
    },
    InputsNotAllowed,
    InvalidOutputType {
        output: String,
        got: String,
    },
    InvalidCadence {
        got: String,
    },
    InvalidParameterDefault {
        parameter: String,
        expected: SourceParameterType,
        reason: String,
    },
    Internal {
        summary: String,
        path: Option<&'static str>,
        fix: Option<&'static str>,
    },
}

impl ErrorInfo for SourceParseError {
    fn rule_id(&self) -> &'static str {
        match self {
            Self::WrongKind { .. } => "SRC-3",
            Self::InputsNotAllowed => "SRC-4",
            Self::InvalidOutputType { .. } => "SRC-7",
            Self::InvalidCadence { .. } => "SRC-13",
            Self::InvalidParameterDefault { .. } => "SRC-15",
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
                Cow::Owned(format!("Wrong kind: expected source, got '{got}'"))
            }
            Self::InputsNotAllowed => Cow::Borrowed("Sources cannot declare inputs"),
            Self::InvalidOutputType { output, got } => {
                Cow::Owned(format!("Output '{output}' has invalid type '{got}'"))
            }
            Self::InvalidCadence { got } => {
                Cow::Owned(format!("Source cadence must be continuous (got '{got}')"))
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
            Self::InputsNotAllowed => Some(Cow::Borrowed("$.inputs")),
            Self::InvalidOutputType { .. } => Some(Cow::Borrowed("$.outputs[].type")),
            Self::InvalidCadence { .. } => Some(Cow::Borrowed("$.execution.cadence")),
            Self::InvalidParameterDefault { .. } => Some(Cow::Borrowed("$.parameters[].default")),
            Self::Internal { path, .. } => path.map(Cow::Borrowed),
        }
    }

    fn fix(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::WrongKind { .. } => Some(Cow::Borrowed("Set kind: source")),
            Self::InputsNotAllowed => Some(Cow::Borrowed("Remove inputs from source manifest")),
            Self::InvalidOutputType { .. } => Some(Cow::Borrowed(
                "Use a valid output type: number, bool, string, or series",
            )),
            Self::InvalidCadence { .. } => Some(Cow::Borrowed("Set cadence: continuous")),
            Self::InvalidParameterDefault { .. } => Some(Cow::Borrowed(
                "Change parameter default value to match the declared parameter type",
            )),
            Self::Internal { fix, .. } => fix.map(Cow::Borrowed),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawSourceManifest {
    id: String,
    version: String,
    kind: String,
    inputs: Vec<RawSourceInput>,
    outputs: Vec<RawSourceOutput>,
    #[serde(default)]
    parameters: Vec<RawSourceParameter>,
    #[serde(default)]
    requires: RawSourceRequires,
    execution: RawSourceExecution,
    state: RawSourceState,
    side_effects: bool,
}

/// Deserialization-only DTO for source manifest input declarations.
/// Fields are read during YAML parsing and mapped to kernel types;
/// the struct itself is never used after the conversion pass.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RawSourceInput {
    name: String,
    #[serde(rename = "type")]
    value_type: String,
    required: bool,
}

#[derive(Debug, Deserialize)]
struct RawSourceOutput {
    name: String,
    #[serde(rename = "type")]
    value_type: String,
}

#[derive(Debug, Deserialize)]
struct RawSourceParameter {
    name: String,
    #[serde(rename = "type")]
    value_type: String,
    default: Option<serde_json::Value>,
    bounds: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RawSourceRequires {
    #[serde(default)]
    context: Vec<RawContextRequirement>,
}

#[derive(Debug, Deserialize)]
struct RawContextRequirement {
    name: String,
    #[serde(rename = "type")]
    value_type: String,
    required: bool,
}

#[derive(Debug, Deserialize)]
struct RawSourceExecution {
    deterministic: bool,
    cadence: String,
}

#[derive(Debug, Deserialize)]
struct RawSourceState {
    allowed: bool,
}

fn raw_to_source_manifest(
    raw: RawSourceManifest,
) -> Result<SourcePrimitiveManifest, SourceParseError> {
    let kind = parse_source_kind(&raw.kind)?;
    if !raw.inputs.is_empty() {
        return Err(SourceParseError::InputsNotAllowed);
    }
    let inputs = Vec::new();

    let outputs = raw
        .outputs
        .into_iter()
        .map(|output| {
            let value_type = parse_value_type(&output.value_type).ok_or_else(|| {
                SourceParseError::InvalidOutputType {
                    output: output.name.clone(),
                    got: output.value_type.clone(),
                }
            })?;
            Ok(ergo_runtime::source::OutputSpec {
                name: output.name,
                value_type,
            })
        })
        .collect::<Result<Vec<_>, SourceParseError>>()?;

    let parameters = raw
        .parameters
        .into_iter()
        .map(|param| {
            let value_type = parse_parameter_type(&param.value_type).ok_or_else(|| {
                SourceParseError::Internal {
                    summary: format!("invalid parameter type '{}'", param.value_type),
                    path: Some("$.parameters[].type"),
                    fix: Some("Use a valid parameter type: int, number, bool, string, or enum"),
                }
            })?;
            let default = parse_parameter_default(&param.name, value_type.clone(), param.default)?;
            Ok(SourceParameterSpec {
                name: param.name,
                value_type,
                default,
                bounds: param.bounds,
            })
        })
        .collect::<Result<Vec<_>, SourceParseError>>()?;

    let requires = SourceRequires {
        context: raw
            .requires
            .context
            .into_iter()
            .map(|req| {
                let ty = parse_value_type(&req.value_type).ok_or_else(|| {
                    SourceParseError::Internal {
                        summary: format!("invalid context type '{}'", req.value_type),
                        path: Some("$.requires.context[].type"),
                        fix: Some("Use a valid context type: number, bool, string, or series"),
                    }
                })?;
                Ok(ContextRequirement {
                    name: req.name,
                    ty,
                    required: req.required,
                })
            })
            .collect::<Result<Vec<_>, SourceParseError>>()?,
    };

    let execution = SourceExecutionSpec {
        deterministic: raw.execution.deterministic,
        cadence: parse_source_cadence(&raw.execution.cadence)?,
    };

    let state = SourceStateSpec {
        allowed: raw.state.allowed,
    };

    Ok(SourcePrimitiveManifest {
        id: raw.id,
        version: raw.version,
        kind,
        inputs,
        outputs,
        parameters,
        requires,
        execution,
        state,
        side_effects: raw.side_effects,
    })
}

fn parse_parameter_type(input: &str) -> Option<SourceParameterType> {
    match input.to_ascii_lowercase().as_str() {
        "int" => Some(SourceParameterType::Int),
        "number" => Some(SourceParameterType::Number),
        "bool" | "boolean" => Some(SourceParameterType::Bool),
        "string" => Some(SourceParameterType::String),
        "enum" => Some(SourceParameterType::Enum),
        _ => None,
    }
}

fn parse_source_kind(input: &str) -> Result<SourceKind, SourceParseError> {
    match input.to_ascii_lowercase().as_str() {
        "source" => Ok(SourceKind::Source),
        other => Err(SourceParseError::WrongKind {
            got: other.to_string(),
        }),
    }
}

fn parse_source_cadence(input: &str) -> Result<ergo_runtime::source::Cadence, SourceParseError> {
    match input.to_ascii_lowercase().as_str() {
        "continuous" => Ok(ergo_runtime::source::Cadence::Continuous),
        other => Err(SourceParseError::InvalidCadence {
            got: other.to_string(),
        }),
    }
}

// This stays family-local even though the shape is similar across manifest
// families because source defaults lower onto source-specific parameter enums
// and preserve `SRC-*` mismatch ownership here.
fn parse_parameter_default(
    parameter: &str,
    value_type: SourceParameterType,
    raw: Option<serde_json::Value>,
) -> Result<Option<SourceParameterValue>, SourceParseError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    let value = match value_type {
        SourceParameterType::Int => {
            SourceParameterValue::Int(parse_int_value(&raw).map_err(|reason| {
                SourceParseError::InvalidParameterDefault {
                    parameter: parameter.to_string(),
                    expected: value_type.clone(),
                    reason,
                }
            })?)
        }
        SourceParameterType::Number => {
            SourceParameterValue::Number(raw.as_f64().ok_or_else(|| {
                SourceParseError::InvalidParameterDefault {
                    parameter: parameter.to_string(),
                    expected: value_type.clone(),
                    reason: "expected numeric default".to_string(),
                }
            })?)
        }
        SourceParameterType::Bool => {
            SourceParameterValue::Bool(raw.as_bool().ok_or_else(|| {
                SourceParseError::InvalidParameterDefault {
                    parameter: parameter.to_string(),
                    expected: value_type.clone(),
                    reason: "expected boolean default".to_string(),
                }
            })?)
        }
        SourceParameterType::String => SourceParameterValue::String(
            raw.as_str()
                .ok_or_else(|| SourceParseError::InvalidParameterDefault {
                    parameter: parameter.to_string(),
                    expected: value_type.clone(),
                    reason: "expected string default".to_string(),
                })?
                .to_string(),
        ),
        SourceParameterType::Enum => SourceParameterValue::Enum(
            raw.as_str()
                .ok_or_else(|| SourceParseError::InvalidParameterDefault {
                    parameter: parameter.to_string(),
                    expected: value_type.clone(),
                    reason: "expected enum default".to_string(),
                })?
                .to_string(),
        ),
    };
    Ok(Some(value))
}
