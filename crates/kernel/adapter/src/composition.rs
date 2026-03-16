use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;

use ergo_runtime::action::ActionEffects;
use ergo_runtime::common::{resolve_manifest_name, ErrorInfo, Phase, ValueType};
pub use ergo_runtime::source::{ContextRequirement, SourceRequires};

use crate::provides::AdapterProvides;

#[derive(Debug)]
pub enum CompositionError {
    MissingContextKey {
        key: String,
        index: usize,
    },
    ContextTypeMismatch {
        key: String,
        expected: String,
        got: String,
        index: usize,
    },
    UnsupportedCaptureFormat {
        version: String,
    },
    WriteTargetNotProvided {
        key: String,
        index: usize,
    },
    WriteTargetNotWritable {
        key: String,
        index: usize,
    },
    WriteTypeMismatch {
        key: String,
        expected: String,
        got: String,
        index: usize,
    },
    MissingSetContextEffect,
    MissingIntentEffect {
        kind: String,
        index: usize,
    },
    ManifestNameResolutionFailed {
        binding: String,
        index: usize,
        context: &'static str,
    },
}

impl ErrorInfo for CompositionError {
    fn rule_id(&self) -> &'static str {
        match self {
            Self::MissingContextKey { .. } => "COMP-1",
            Self::ContextTypeMismatch { .. } => "COMP-2",
            Self::UnsupportedCaptureFormat { .. } => "COMP-3",
            Self::WriteTargetNotProvided { .. } => "COMP-11",
            Self::WriteTargetNotWritable { .. } => "COMP-12",
            Self::WriteTypeMismatch { .. } => "COMP-13",
            Self::MissingSetContextEffect => "COMP-14",
            Self::MissingIntentEffect { .. } => "COMP-17",
            Self::ManifestNameResolutionFailed { .. } => "COMP-16",
        }
    }

    fn phase(&self) -> Phase {
        Phase::Composition
    }

    fn doc_anchor(&self) -> &'static str {
        match self {
            Self::MissingContextKey { .. } => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#COMP-1",
            Self::ContextTypeMismatch { .. } => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#COMP-2",
            Self::UnsupportedCaptureFormat { .. } => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#COMP-3",
            Self::WriteTargetNotProvided { .. } => "STABLE/PRIMITIVE_MANIFESTS/action.md#COMP-11",
            Self::WriteTargetNotWritable { .. } => "STABLE/PRIMITIVE_MANIFESTS/action.md#COMP-12",
            Self::WriteTypeMismatch { .. } => "STABLE/PRIMITIVE_MANIFESTS/action.md#COMP-13",
            Self::MissingSetContextEffect => "STABLE/PRIMITIVE_MANIFESTS/action.md#COMP-14",
            Self::MissingIntentEffect { .. } => "STABLE/PRIMITIVE_MANIFESTS/action.md#COMP-17",
            Self::ManifestNameResolutionFailed { .. } => "CANONICAL/PHASE_INVARIANTS.md#COMP-16",
        }
    }

    fn summary(&self) -> Cow<'static, str> {
        match self {
            Self::MissingContextKey { key, .. } => Cow::Owned(format!(
                "Required context key '{}' not provided by adapter",
                key
            )),
            Self::ContextTypeMismatch {
                key, expected, got, ..
            } => Cow::Owned(format!(
                "Context key '{}' type mismatch: expected '{}', got '{}'",
                key, expected, got
            )),
            Self::UnsupportedCaptureFormat { version } => {
                Cow::Owned(format!("Unsupported capture format version: '{}'", version))
            }
            Self::WriteTargetNotProvided { key, .. } => Cow::Owned(format!(
                "Action write target '{}' not provided by adapter",
                key
            )),
            Self::WriteTargetNotWritable { key, .. } => Cow::Owned(format!(
                "Action write target '{}' is not writable in adapter",
                key
            )),
            Self::WriteTypeMismatch {
                key, expected, got, ..
            } => Cow::Owned(format!(
                "Action write target '{}' type mismatch: expected '{}', got '{}'",
                key, expected, got
            )),
            Self::MissingSetContextEffect => {
                Cow::Borrowed("Adapter does not accept set_context effect required for writes")
            }
            Self::MissingIntentEffect { kind, .. } => Cow::Owned(format!(
                "Adapter does not accept intent effect kind '{}' required by action manifest",
                kind
            )),
            Self::ManifestNameResolutionFailed { binding, .. } => Cow::Owned(format!(
                "Failed to resolve parameter-bound manifest name '{}'",
                binding
            )),
        }
    }

    fn path(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::MissingContextKey { index, .. } => {
                Some(Cow::Owned(format!("$.requires.context[{}].name", index)))
            }
            Self::ContextTypeMismatch { index, .. } => {
                Some(Cow::Owned(format!("$.requires.context[{}].type", index)))
            }
            Self::UnsupportedCaptureFormat { .. } => {
                Some(Cow::Borrowed("$.capture.format_version"))
            }
            Self::WriteTargetNotProvided { index, .. } => {
                Some(Cow::Owned(format!("$.effects.writes[{}].name", index)))
            }
            Self::WriteTargetNotWritable { index, .. } => {
                Some(Cow::Owned(format!("$.effects.writes[{}].name", index)))
            }
            Self::WriteTypeMismatch { index, .. } => {
                Some(Cow::Owned(format!("$.effects.writes[{}].type", index)))
            }
            Self::MissingSetContextEffect => Some(Cow::Borrowed("$.effects.writes")),
            Self::MissingIntentEffect { index, .. } => {
                Some(Cow::Owned(format!("$.effects.intents[{}].name", index)))
            }
            Self::ManifestNameResolutionFailed { index, context, .. } => {
                Some(Cow::Owned(format!("$.{context}[{index}].name")))
            }
        }
    }

    fn fix(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::MissingContextKey { key, .. } => Some(Cow::Owned(format!(
                "Add context key '{}' to the adapter's context_keys",
                key
            ))),
            Self::ContextTypeMismatch { key, expected, .. } => Some(Cow::Owned(format!(
                "Change type of '{}' in adapter's context_keys to '{}'",
                key, expected
            ))),
            Self::UnsupportedCaptureFormat { .. } => {
                Some(Cow::Borrowed("Use a supported capture format version: 1"))
            }
            Self::WriteTargetNotProvided { key, .. } => Some(Cow::Owned(format!(
                "Add context key '{}' to the adapter's context_keys",
                key
            ))),
            Self::WriteTargetNotWritable { key, .. } => Some(Cow::Owned(format!(
                "Mark context key '{}' as writable in the adapter manifest",
                key
            ))),
            Self::WriteTypeMismatch { key, expected, .. } => Some(Cow::Owned(format!(
                "Change type of '{}' in adapter's context_keys to '{}'",
                key, expected
            ))),
            Self::MissingSetContextEffect => Some(Cow::Borrowed(
                "Add 'set_context' to adapter accepts.effects",
            )),
            Self::MissingIntentEffect { kind, .. } => Some(Cow::Owned(format!(
                "Add '{}' to adapter accepts.effects",
                kind
            ))),
            Self::ManifestNameResolutionFailed { binding, .. } => Some(Cow::Owned(format!(
                "Ensure parameter referenced by '{}' exists and is a String type",
                binding
            ))),
        }
    }
}

impl fmt::Display for CompositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.rule_id(), self.summary())
    }
}

impl std::error::Error for CompositionError {}

/// Supported capture format versions.
const SUPPORTED_CAPTURE_VERSIONS: &[&str] = &["1"];

/// Validate that an adapter provides what a source requires.
/// COMP-1: Required context keys must exist in adapter.
/// COMP-2: Context key types must match.
/// COMP-16: Parameter-bound manifest names ($key) must resolve.
pub fn validate_source_adapter_composition(
    source: &SourceRequires,
    adapter: &AdapterProvides,
    parameters: &HashMap<String, ergo_runtime::cluster::ParameterValue>,
) -> Result<(), CompositionError> {
    for (index, req) in source.context.iter().enumerate() {
        // COMP-16: Resolve $key bindings before required check so optional
        // parameter-bound keys are still resolved.
        let resolved_name = resolve_manifest_name(&req.name, parameters).map_err(|_| {
            CompositionError::ManifestNameResolutionFailed {
                binding: req.name.clone(),
                index,
                context: "requires.context",
            }
        })?;

        let provided = match adapter.context.get(&resolved_name) {
            Some(p) => p,
            None => {
                if !req.required {
                    continue;
                }

                // COMP-1: Check key exists (required only)
                return Err(CompositionError::MissingContextKey {
                    key: resolved_name,
                    index,
                });
            }
        };

        // COMP-2: Check types match
        let provided_ty = match parse_value_type(&provided.ty) {
            Some(ty) => ty,
            None => {
                return Err(CompositionError::ContextTypeMismatch {
                    key: resolved_name,
                    expected: value_type_name(&req.ty).to_string(),
                    got: provided.ty.clone(),
                    index,
                });
            }
        };

        if req.ty != provided_ty {
            return Err(CompositionError::ContextTypeMismatch {
                key: resolved_name,
                expected: value_type_name(&req.ty).to_string(),
                got: provided.ty.clone(),
                index,
            });
        }
    }
    Ok(())
}

/// COMP-3: Validate capture format version is supported.
pub fn validate_capture_format(version: &str) -> Result<(), CompositionError> {
    if !SUPPORTED_CAPTURE_VERSIONS.contains(&version) {
        return Err(CompositionError::UnsupportedCaptureFormat {
            version: version.to_string(),
        });
    }
    Ok(())
}

/// Validate that an adapter satisfies action write requirements.
/// COMP-11: Write targets exist in adapter context.
/// COMP-12: Write targets are writable.
/// COMP-13: Write target types match.
/// COMP-14: Writes require set_context effect acceptance.
/// COMP-16: Parameter-bound manifest names ($key) must resolve.
pub fn validate_action_adapter_composition(
    effects: &ActionEffects,
    adapter: &AdapterProvides,
    parameters: &HashMap<String, ergo_runtime::cluster::ParameterValue>,
) -> Result<(), CompositionError> {
    if effects.writes.is_empty() && effects.intents.is_empty() {
        return Ok(());
    }

    for (index, write) in effects.writes.iter().enumerate() {
        // COMP-16: Resolve $key bindings
        let resolved_name = resolve_manifest_name(&write.name, parameters).map_err(|_| {
            CompositionError::ManifestNameResolutionFailed {
                binding: write.name.clone(),
                index,
                context: "effects.writes",
            }
        })?;

        let provided = match adapter.context.get(&resolved_name) {
            Some(p) => p,
            None => {
                return Err(CompositionError::WriteTargetNotProvided {
                    key: resolved_name,
                    index,
                });
            }
        };

        if !provided.writable {
            return Err(CompositionError::WriteTargetNotWritable {
                key: resolved_name,
                index,
            });
        }

        let provided_ty = match parse_value_type(&provided.ty) {
            Some(ty) => ty,
            None => {
                return Err(CompositionError::WriteTypeMismatch {
                    key: resolved_name,
                    expected: value_type_name(&write.value_type).to_string(),
                    got: provided.ty.clone(),
                    index,
                });
            }
        };

        if provided_ty != write.value_type {
            return Err(CompositionError::WriteTypeMismatch {
                key: resolved_name,
                expected: value_type_name(&write.value_type).to_string(),
                got: provided.ty.clone(),
                index,
            });
        }
    }

    if !effects.writes.is_empty() && !adapter.effects.contains("set_context") {
        return Err(CompositionError::MissingSetContextEffect);
    }

    for (index, intent) in effects.intents.iter().enumerate() {
        if !adapter.effects.contains(&intent.name) {
            return Err(CompositionError::MissingIntentEffect {
                kind: intent.name.clone(),
                index,
            });
        }
    }

    Ok(())
}

fn parse_value_type(value: &str) -> Option<ValueType> {
    match value {
        "Number" => Some(ValueType::Number),
        "Series" => Some(ValueType::Series),
        "Bool" => Some(ValueType::Bool),
        "String" => Some(ValueType::String),
        _ => None,
    }
}

fn value_type_name(value: &ValueType) -> &'static str {
    match value {
        ValueType::Number => "Number",
        ValueType::Series => "Series",
        ValueType::Bool => "Bool",
        ValueType::String => "String",
    }
}
