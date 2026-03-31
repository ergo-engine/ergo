//! manifest_usecases
//!
//! Purpose:
//! - Host-owned file-manifest ingress for adapter, source, compute, trigger,
//!   and action manifests.
//! - Loads YAML/JSON, dispatches by `kind`, normalizes the current file-backed
//!   manifest surface into typed runtime/adapter manifests, and projects
//!   failures into host-facing rule violations.
//!
//! Owns:
//! - Lower-level `*_text`, `*_value`, and path entrypoints used by CLI and
//!   embedded host callers.
//! - File-surface normalization such as `boolean`/`bool` aliases, compute
//!   `int` parameter aliases, and action `effects` defaulting to empty writes
//!   with no intents on the file-backed path.
//!
//! Does not own:
//! - Primitive registration semantics in the runtime registries.
//! - Adapter composition semantics enforced in `ergo_adapter`.
//! - The richer runtime/custom action-intent manifest surface beyond the
//!   current file-backed writes-only subset.
//!
//! Connects to:
//! - `ergo_runtime` registries for typed primitive validation.
//! - `ergo_adapter` composition checks for source/action adapter compatibility.
//! - CLI `validate` / `check-compose` commands through the public host exports.
//!
//! Safety notes:
//! - Parse/IO/decode failures are projected as `INTERNAL` host rule violations
//!   and rendered downstream as-is.
//! - Unsupported compose targets currently map to a host-synthesized `COMP-1`
//!   violation to preserve the existing CLI contract.
//! - Private manifest-family pipelines now live in dedicated submodules so
//!   source/compute/trigger/action raw DTOs, parse errors, and alias/default
//!   logic stay grouped without moving the public host surface.

mod action;
mod common;
mod compute;
mod source;
mod trigger;

use std::borrow::Cow;
use std::path::Path;

use ergo_adapter::composition::{
    validate_action_adapter_composition, validate_source_adapter_composition,
};
use ergo_adapter::{AdapterManifest, AdapterProvides};
use ergo_runtime::action::{ActionPrimitiveManifest, ActionRegistry};
use ergo_runtime::common::{doc_anchor_for_rule, Phase, RuleViolation};
use ergo_runtime::compute::{ComputePrimitiveManifest, PrimitiveRegistry as ComputeRegistry};
use ergo_runtime::source::{SourcePrimitiveManifest, SourceRegistry};
use ergo_runtime::trigger::{TriggerPrimitiveManifest, TriggerRegistry};

#[cfg(test)]
mod tests;

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
                doc_anchor: doc_anchor_for_rule("COMP-1").to_string(),
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
    let parsed = parse_manifest_path(path).map_err(HostManifestError::from)?;
    let summary = parsed.summary().clone();
    validate_parsed(&parsed).map_err(HostManifestError::from)?;
    Ok(summary)
}

#[allow(clippy::result_large_err)]
pub fn validate_manifest_text(
    source_label: &str,
    content: &str,
) -> Result<ManifestSummary, HostManifestError> {
    let parsed = parse_manifest_text(source_label, content).map_err(HostManifestError::from)?;
    let summary = parsed.summary().clone();
    validate_parsed(&parsed).map_err(HostManifestError::from)?;
    Ok(summary)
}

#[allow(clippy::result_large_err)]
pub fn validate_manifest_value(
    source_label: &str,
    value: serde_json::Value,
) -> Result<ManifestSummary, HostManifestError> {
    let parsed = parse_manifest_value(source_label, value).map_err(HostManifestError::from)?;
    let summary = parsed.summary().clone();
    validate_parsed(&parsed).map_err(HostManifestError::from)?;
    Ok(summary)
}

#[allow(clippy::result_large_err)]
pub fn check_compose_paths(
    adapter_path: &Path,
    other_path: &Path,
) -> Result<(), HostManifestError> {
    let adapter_manifest = parse_adapter_manifest_path(adapter_path)
        .map_err(|msg| HostManifestError::RuleViolation(parse_error_violation(msg).into()))?;
    check_compose_with_adapter_manifest(adapter_manifest, parse_manifest_path(other_path))
}

#[allow(clippy::result_large_err)]
pub fn check_compose_text(
    adapter_label: &str,
    adapter_content: &str,
    other_label: &str,
    other_content: &str,
) -> Result<(), HostManifestError> {
    let adapter_manifest = parse_adapter_manifest_text(adapter_label, adapter_content)
        .map_err(|msg| HostManifestError::RuleViolation(parse_error_violation(msg).into()))?;
    check_compose_with_adapter_manifest(
        adapter_manifest,
        parse_manifest_text(other_label, other_content),
    )
}

#[allow(clippy::result_large_err)]
pub fn check_compose_values(
    adapter_label: &str,
    adapter_value: serde_json::Value,
    other_label: &str,
    other_value: serde_json::Value,
) -> Result<(), HostManifestError> {
    let adapter_manifest = parse_adapter_manifest_value(adapter_label, adapter_value)
        .map_err(|msg| HostManifestError::RuleViolation(parse_error_violation(msg).into()))?;
    check_compose_with_adapter_manifest(
        adapter_manifest,
        parse_manifest_value(other_label, other_value),
    )
}

#[allow(clippy::result_large_err)]
fn check_compose_with_adapter_manifest(
    adapter_manifest: AdapterManifest,
    other: Result<ParsedManifest, RuleViolation>,
) -> Result<(), HostManifestError> {
    ergo_adapter::validate_adapter(&adapter_manifest)
        .map_err(RuleViolation::from)
        .map_err(HostManifestError::from)?;
    let adapter_provides = AdapterProvides::from_manifest(&adapter_manifest);

    let other = other.map_err(HostManifestError::from)?;
    match other {
        ParsedManifest::Source { manifest, .. } => {
            let params = source::default_params_for_composition(&manifest);
            validate_source_adapter_composition(&manifest.requires, &adapter_provides, &params)
                .map_err(RuleViolation::from)
                .map_err(HostManifestError::from)
        }
        ParsedManifest::Action { manifest, .. } => {
            let params = action::default_params_for_composition(&manifest);
            validate_action_adapter_composition(&manifest.effects, &adapter_provides, &params)
                .map_err(RuleViolation::from)
                .map_err(HostManifestError::from)
        }
        parsed => Err(HostManifestError::UnsupportedComposeTargetKind {
            kind: parsed.summary().kind.clone(),
        }),
    }
}

fn parse_adapter_manifest_path(path: &Path) -> Result<AdapterManifest, String> {
    let data = std::fs::read_to_string(path)
        .map_err(|err| format!("read adapter manifest '{}': {err}", path.display()))?;
    parse_adapter_manifest_text(&path.display().to_string(), &data)
}

fn parse_adapter_manifest_text(
    source_label: &str,
    content: &str,
) -> Result<AdapterManifest, String> {
    let value = serde_yaml::from_str::<serde_json::Value>(content)
        .map_err(|err| format!("parse adapter manifest '{}': {err}", source_label))?;
    parse_adapter_manifest_value(source_label, value)
}

fn parse_adapter_manifest_value(
    source_label: &str,
    value: serde_json::Value,
) -> Result<AdapterManifest, String> {
    serde_json::from_value::<AdapterManifest>(value)
        .map_err(|err| format!("decode adapter manifest '{}': {err}", source_label))
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

fn parse_manifest_path(path: &Path) -> Result<ParsedManifest, RuleViolation> {
    let value = load_manifest_value_from_path(path).map_err(parse_error_violation)?;
    parse_manifest_value(&path.display().to_string(), value)
}

fn parse_manifest_text(source_label: &str, content: &str) -> Result<ParsedManifest, RuleViolation> {
    let value =
        load_manifest_value_from_text(source_label, content).map_err(parse_error_violation)?;
    parse_manifest_value(source_label, value)
}

fn parse_manifest_value(
    source_label: &str,
    value: serde_json::Value,
) -> Result<ParsedManifest, RuleViolation> {
    let kind = value
        .get("kind")
        .and_then(|v| v.as_str())
        .map(|kind| kind.to_ascii_lowercase())
        .ok_or_else(|| {
            parse_error_violation(format!(
                "manifest '{}' is missing 'kind' field",
                source_label
            ))
        })?;

    match kind.as_str() {
        "adapter" => {
            let manifest = serde_json::from_value::<AdapterManifest>(value).map_err(|err| {
                parse_error_violation(format!("parse adapter manifest '{}': {err}", source_label))
            })?;
            let summary = ManifestSummary {
                kind: "adapter".to_string(),
                id: manifest.id.clone(),
                version: manifest.version.clone(),
            };
            Ok(ParsedManifest::Adapter { summary, manifest })
        }
        "source" => {
            let manifest = source::parse_manifest(source_label, value)?;
            let summary = ManifestSummary {
                kind: "source".to_string(),
                id: manifest.id.clone(),
                version: manifest.version.clone(),
            };
            Ok(ParsedManifest::Source { summary, manifest })
        }
        "compute" => {
            let manifest = compute::parse_manifest(source_label, value)?;
            let summary = ManifestSummary {
                kind: "compute".to_string(),
                id: manifest.id.clone(),
                version: manifest.version.clone(),
            };
            Ok(ParsedManifest::Compute { summary, manifest })
        }
        "trigger" => {
            let manifest = trigger::parse_manifest(source_label, value)?;
            let summary = ManifestSummary {
                kind: "trigger".to_string(),
                id: manifest.id.clone(),
                version: manifest.version.clone(),
            };
            Ok(ParsedManifest::Trigger { summary, manifest })
        }
        "action" => {
            let manifest = action::parse_manifest(source_label, value)?;
            let summary = ManifestSummary {
                kind: "action".to_string(),
                id: manifest.id.clone(),
                version: manifest.version.clone(),
            };
            Ok(ParsedManifest::Action { summary, manifest })
        }
        other => Err(parse_error_violation(format!(
            "unknown manifest kind '{}' in '{}'",
            other, source_label
        ))),
    }
}

fn load_manifest_value_from_path(path: &Path) -> Result<serde_json::Value, String> {
    let data = std::fs::read_to_string(path)
        .map_err(|err| format!("read manifest '{}': {err}", path.display()))?;
    load_manifest_value_from_text(&path.display().to_string(), &data)
}

fn load_manifest_value_from_text(
    source_label: &str,
    content: &str,
) -> Result<serde_json::Value, String> {
    serde_yaml::from_str::<serde_json::Value>(content)
        .map_err(|err| format!("parse manifest '{}': {err}", source_label))
}

fn parse_error_violation(message: String) -> RuleViolation {
    RuleViolation {
        rule_id: "INTERNAL",
        phase: Phase::Registration,
        doc_anchor: doc_anchor_for_rule("INTERNAL"),
        summary: Cow::Owned(message),
        path: None,
        fix: None,
    }
}

fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Registration => "registration",
        Phase::Composition => "composition",
        Phase::Execution => "execution",
    }
}
