use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;

use ergo_adapter::composition::{
    validate_action_adapter_composition, validate_source_adapter_composition,
};
use ergo_adapter::{AdapterManifest, AdapterProvides};
use ergo_runtime::action::{
    ActionEffects, ActionKind, ActionPrimitiveManifest, ActionRegistry, ActionValueType,
    ParameterType as ActionParameterType, ParameterValue as ActionParameterValue,
};
use ergo_runtime::cluster::ParameterValue as ClusterParameterValue;
use ergo_runtime::common::{ErrorInfo, Phase, PrimitiveKind, RuleViolation, Value, ValueType};
use ergo_runtime::compute::{
    Cadence as ComputeCadence, Cardinality as ComputeCardinality, ComputePrimitiveManifest,
    ErrorType as ComputeErrorType, ExecutionSpec as ComputeExecutionSpec,
    ParameterSpec as ComputeParameterSpec, PrimitiveRegistry as ComputeRegistry,
    StateSpec as ComputeStateSpec,
};
use ergo_runtime::source::{
    ContextRequirement, ExecutionSpec as SourceExecutionSpec, ParameterSpec as SourceParameterSpec,
    ParameterType as SourceParameterType, ParameterValue as SourceParameterValue, SourceKind,
    SourcePrimitiveManifest, SourceRegistry, SourceRequires, StateSpec as SourceStateSpec,
};
use ergo_runtime::trigger::{
    Cadence as TriggerCadence, Cardinality as TriggerCardinality,
    ExecutionSpec as TriggerExecutionSpec, ParameterSpec as TriggerParameterSpec,
    ParameterType as TriggerParameterType, ParameterValue as TriggerParameterValue, TriggerKind,
    TriggerPrimitiveManifest, TriggerRegistry, TriggerValueType,
};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct ManifestSummary {
    pub kind: String,
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct HostRuleViolation {
    pub rule_id: String,
    pub phase: String,
    pub doc_anchor: String,
    pub summary: String,
    pub path: Option<String>,
    pub fix: Option<String>,
}

#[derive(Debug)]
pub enum HostManifestError {
    RuleViolation(HostRuleViolation),
    UnsupportedComposeTargetKind { kind: String },
}

impl From<RuleViolation> for HostManifestError {
    fn from(value: RuleViolation) -> Self {
        Self::RuleViolation(value.into())
    }
}

impl HostManifestError {
    pub fn into_rule_violation(self) -> HostRuleViolation {
        match self {
            Self::RuleViolation(violation) => violation,
            Self::UnsupportedComposeTargetKind { kind } => HostRuleViolation {
                rule_id: "COMP-1".to_string(),
                phase: "composition".to_string(),
                doc_anchor: "STABLE/PRIMITIVE_MANIFESTS/adapter.md#COMP-1".to_string(),
                summary: format!("unsupported manifest kind for composition: '{kind}'"),
                path: Some("$.kind".to_string()),
                fix: Some("Use a source or action manifest as the composition target".to_string()),
            },
        }
    }
}

impl From<RuleViolation> for HostRuleViolation {
    fn from(value: RuleViolation) -> Self {
        Self {
            rule_id: value.rule_id.to_string(),
            phase: phase_name(value.phase).to_string(),
            doc_anchor: value.doc_anchor.to_string(),
            summary: value.summary.into_owned(),
            path: value.path.map(|p| p.into_owned()),
            fix: value.fix.map(|f| f.into_owned()),
        }
    }
}

enum ParsedManifest {
    Adapter {
        summary: ManifestSummary,
        manifest: AdapterManifest,
    },
    Source {
        summary: ManifestSummary,
        manifest: SourcePrimitiveManifest,
    },
    Compute {
        summary: ManifestSummary,
        manifest: ComputePrimitiveManifest,
    },
    Trigger {
        summary: ManifestSummary,
        manifest: TriggerPrimitiveManifest,
    },
    Action {
        summary: ManifestSummary,
        manifest: ActionPrimitiveManifest,
    },
}

#[allow(clippy::result_large_err)]
pub fn validate_manifest_path(path: &Path) -> Result<ManifestSummary, HostManifestError> {
    let parsed = parse_manifest(path).map_err(HostManifestError::from)?;
    let summary = parsed.summary().clone();
    validate_parsed(&parsed).map_err(HostManifestError::from)?;
    Ok(summary)
}

#[allow(clippy::result_large_err)]
pub fn check_compose_paths(
    adapter_path: &Path,
    other_path: &Path,
) -> Result<(), HostManifestError> {
    let adapter_manifest = parse_adapter_manifest(adapter_path)
        .map_err(|msg| HostManifestError::RuleViolation(parse_error_violation(msg).into()))?;
    ergo_adapter::validate_adapter(&adapter_manifest)
        .map_err(RuleViolation::from)
        .map_err(HostManifestError::from)?;
    let adapter_provides = AdapterProvides::from_manifest(&adapter_manifest);

    let other = parse_manifest(other_path).map_err(HostManifestError::from)?;
    match other {
        ParsedManifest::Source { manifest, .. } => {
            let params = source_manifest_default_params(&manifest);
            validate_source_adapter_composition(&manifest.requires, &adapter_provides, &params)
                .map_err(RuleViolation::from)
                .map_err(HostManifestError::from)
        }
        ParsedManifest::Action { manifest, .. } => {
            let params = action_manifest_default_params(&manifest);
            validate_action_adapter_composition(&manifest.effects, &adapter_provides, &params)
                .map_err(RuleViolation::from)
                .map_err(HostManifestError::from)
        }
        _ => Err(HostManifestError::UnsupportedComposeTargetKind {
            kind: other.summary().kind.clone(),
        }),
    }
}

fn parse_adapter_manifest(path: &Path) -> Result<AdapterManifest, String> {
    let data = std::fs::read_to_string(path)
        .map_err(|err| format!("read adapter manifest '{}': {err}", path.display()))?;
    let value = serde_yaml::from_str::<serde_json::Value>(&data)
        .map_err(|err| format!("parse adapter manifest '{}': {err}", path.display()))?;
    serde_json::from_value::<AdapterManifest>(value)
        .map_err(|err| format!("decode adapter manifest '{}': {err}", path.display()))
}

fn source_manifest_default_params(
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

fn action_manifest_default_params(
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

fn source_param_value_to_cluster(value: &SourceParameterValue) -> ClusterParameterValue {
    match value {
        SourceParameterValue::Int(v) => ClusterParameterValue::Int(*v),
        SourceParameterValue::Number(v) => ClusterParameterValue::Number(*v),
        SourceParameterValue::Bool(v) => ClusterParameterValue::Bool(*v),
        SourceParameterValue::String(v) => ClusterParameterValue::String(v.clone()),
        SourceParameterValue::Enum(v) => ClusterParameterValue::Enum(v.clone()),
    }
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

fn validate_parsed(parsed: &ParsedManifest) -> Result<(), RuleViolation> {
    match parsed {
        ParsedManifest::Adapter { manifest, .. } => {
            ergo_adapter::validate_adapter(manifest).map_err(RuleViolation::from)
        }
        ParsedManifest::Source { manifest, .. } => {
            SourceRegistry::validate_manifest(manifest).map_err(RuleViolation::from)
        }
        ParsedManifest::Compute { manifest, .. } => {
            ComputeRegistry::validate_manifest(manifest).map_err(RuleViolation::from)
        }
        ParsedManifest::Trigger { manifest, .. } => {
            TriggerRegistry::validate_manifest(manifest).map_err(RuleViolation::from)
        }
        ParsedManifest::Action { manifest, .. } => {
            ActionRegistry::validate_manifest(manifest).map_err(RuleViolation::from)
        }
    }
}

impl ParsedManifest {
    fn summary(&self) -> &ManifestSummary {
        match self {
            ParsedManifest::Adapter { summary, .. }
            | ParsedManifest::Source { summary, .. }
            | ParsedManifest::Compute { summary, .. }
            | ParsedManifest::Trigger { summary, .. }
            | ParsedManifest::Action { summary, .. } => summary,
        }
    }
}

fn parse_manifest(path: &Path) -> Result<ParsedManifest, RuleViolation> {
    let value = load_manifest_value(path).map_err(parse_error_violation)?;
    let kind = value
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| parse_error_violation("manifest is missing 'kind' field".to_string()))?;

    match kind.to_ascii_lowercase().as_str() {
        "adapter" => {
            let manifest = serde_json::from_value::<AdapterManifest>(value)
                .map_err(|err| parse_error_violation(format!("parse adapter manifest: {err}")))?;
            let summary = ManifestSummary {
                kind: "adapter".to_string(),
                id: manifest.id.clone(),
                version: manifest.version.clone(),
            };
            Ok(ParsedManifest::Adapter { summary, manifest })
        }
        "source" => {
            let raw = serde_json::from_value::<RawSourceManifest>(value)
                .map_err(|err| parse_error_violation(format!("parse source manifest: {err}")))?;
            let manifest = raw_to_source_manifest(raw).map_err(RuleViolation::from)?;
            let summary = ManifestSummary {
                kind: "source".to_string(),
                id: manifest.id.clone(),
                version: manifest.version.clone(),
            };
            Ok(ParsedManifest::Source { summary, manifest })
        }
        "compute" => {
            let raw = serde_json::from_value::<RawComputeManifest>(value)
                .map_err(|err| parse_error_violation(format!("parse compute manifest: {err}")))?;
            let manifest = raw_to_compute_manifest(raw).map_err(RuleViolation::from)?;
            let summary = ManifestSummary {
                kind: "compute".to_string(),
                id: manifest.id.clone(),
                version: manifest.version.clone(),
            };
            Ok(ParsedManifest::Compute { summary, manifest })
        }
        "trigger" => {
            let raw = serde_json::from_value::<RawTriggerManifest>(value)
                .map_err(|err| parse_error_violation(format!("parse trigger manifest: {err}")))?;
            let manifest = raw_to_trigger_manifest(raw).map_err(RuleViolation::from)?;
            let summary = ManifestSummary {
                kind: "trigger".to_string(),
                id: manifest.id.clone(),
                version: manifest.version.clone(),
            };
            Ok(ParsedManifest::Trigger { summary, manifest })
        }
        "action" => {
            let raw = serde_json::from_value::<RawActionManifest>(value)
                .map_err(|err| parse_error_violation(format!("parse action manifest: {err}")))?;
            let manifest = raw_to_action_manifest(raw).map_err(RuleViolation::from)?;
            let summary = ManifestSummary {
                kind: "action".to_string(),
                id: manifest.id.clone(),
                version: manifest.version.clone(),
            };
            Ok(ParsedManifest::Action { summary, manifest })
        }
        other => Err(parse_error_violation(format!(
            "unknown manifest kind '{other}'"
        ))),
    }
}

fn load_manifest_value(path: &Path) -> Result<serde_json::Value, String> {
    let data = std::fs::read_to_string(path)
        .map_err(|err| format!("read manifest '{}': {err}", path.display()))?;
    serde_yaml::from_str::<serde_json::Value>(&data)
        .map_err(|err| format!("parse manifest '{}': {err}", path.display()))
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
        match self.rule_id() {
            "SRC-3" => "STABLE/PRIMITIVE_MANIFESTS/source.md#SRC-3",
            "SRC-4" => "STABLE/PRIMITIVE_MANIFESTS/source.md#SRC-4",
            "SRC-7" => "STABLE/PRIMITIVE_MANIFESTS/source.md#SRC-7",
            "SRC-13" => "STABLE/PRIMITIVE_MANIFESTS/source.md#SRC-13",
            "SRC-15" => "STABLE/PRIMITIVE_MANIFESTS/source.md#SRC-15",
            _ => "CANONICAL/PHASE_INVARIANTS.md",
        }
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
        match self.rule_id() {
            "CMP-3" => "STABLE/PRIMITIVE_MANIFESTS/compute.md#CMP-3",
            "CMP-13" => "STABLE/PRIMITIVE_MANIFESTS/compute.md#CMP-13",
            "CMP-20" => "STABLE/PRIMITIVE_MANIFESTS/compute.md#CMP-20",
            "CMP-14" => "STABLE/PRIMITIVE_MANIFESTS/compute.md#CMP-14",
            "CMP-15" => "STABLE/PRIMITIVE_MANIFESTS/compute.md#CMP-15",
            "CMP-19" => "STABLE/PRIMITIVE_MANIFESTS/compute.md#CMP-19",
            "CMP-16" => "STABLE/PRIMITIVE_MANIFESTS/compute.md#CMP-16",
            _ => "CANONICAL/PHASE_INVARIANTS.md",
        }
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
        match self.rule_id() {
            "TRG-3" => "STABLE/PRIMITIVE_MANIFESTS/trigger.md#TRG-3",
            "TRG-6" => "STABLE/PRIMITIVE_MANIFESTS/trigger.md#TRG-6",
            "TRG-8" => "STABLE/PRIMITIVE_MANIFESTS/trigger.md#TRG-8",
            "TRG-12" => "STABLE/PRIMITIVE_MANIFESTS/trigger.md#TRG-12",
            "TRG-14" => "STABLE/PRIMITIVE_MANIFESTS/trigger.md#TRG-14",
            _ => "CANONICAL/PHASE_INVARIANTS.md",
        }
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
        match self.rule_id() {
            "ACT-3" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-3",
            "ACT-6" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-6",
            "ACT-9" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-9",
            "ACT-15" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-15",
            "ACT-19" => "STABLE/PRIMITIVE_MANIFESTS/action.md#ACT-19",
            _ => "CANONICAL/PHASE_INVARIANTS.md",
        }
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

fn parse_error_violation(message: String) -> RuleViolation {
    RuleViolation {
        rule_id: "INTERNAL",
        phase: Phase::Registration,
        doc_anchor: "CANONICAL/PHASE_INVARIANTS.md",
        summary: Cow::Owned(message),
        path: None,
        fix: None,
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

fn parse_value_type(input: &str) -> Option<ValueType> {
    match input.to_ascii_lowercase().as_str() {
        "number" => Some(ValueType::Number),
        "bool" | "boolean" => Some(ValueType::Bool),
        "string" => Some(ValueType::String),
        "series" => Some(ValueType::Series),
        _ => None,
    }
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

fn parse_trigger_value_type(input: &str) -> Option<TriggerValueType> {
    match input.to_ascii_lowercase().as_str() {
        "event" => Some(TriggerValueType::Event),
        "number" => Some(TriggerValueType::Number),
        "bool" | "boolean" => Some(TriggerValueType::Bool),
        "series" => Some(TriggerValueType::Series),
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

fn parse_compute_kind(input: &str) -> Result<PrimitiveKind, ComputeParseError> {
    match input.to_ascii_lowercase().as_str() {
        "compute" => Ok(PrimitiveKind::Compute),
        other => Err(ComputeParseError::WrongKind {
            got: other.to_string(),
        }),
    }
}

fn parse_trigger_kind(input: &str) -> Result<TriggerKind, TriggerParseError> {
    match input.to_ascii_lowercase().as_str() {
        "trigger" => Ok(TriggerKind::Trigger),
        other => Err(TriggerParseError::WrongKind {
            got: other.to_string(),
        }),
    }
}

fn parse_action_kind(input: &str) -> Result<ActionKind, ActionParseError> {
    match input.to_ascii_lowercase().as_str() {
        "action" => Ok(ActionKind::Action),
        other => Err(ActionParseError::WrongKind {
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

fn parse_source_cadence(input: &str) -> Result<ergo_runtime::source::Cadence, SourceParseError> {
    match input.to_ascii_lowercase().as_str() {
        "continuous" => Ok(ergo_runtime::source::Cadence::Continuous),
        other => Err(SourceParseError::InvalidCadence {
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

fn parse_compute_error_type(input: &str) -> Option<ComputeErrorType> {
    match input.to_ascii_lowercase().as_str() {
        "divisionbyzero" | "division_by_zero" => Some(ComputeErrorType::DivisionByZero),
        "nonfiniteresult" | "non_finite_result" => Some(ComputeErrorType::NonFiniteResult),
        _ => None,
    }
}

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

fn parse_int_value(value: &serde_json::Value) -> Result<i64, String> {
    if let Some(num) = value.as_i64() {
        return Ok(num);
    }
    if let Some(num) = value.as_f64() {
        if num.fract() == 0.0 {
            return Ok(num as i64);
        }
    }
    Err("expected integer default".to_string())
}

fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Registration => "registration",
        Phase::Composition => "composition",
        Phase::Execution => "execution",
    }
}
