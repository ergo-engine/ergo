//! composition.rs — Adapter-to-runtime composition and binding
//!
//! Purpose:
//! - Composes a validated adapter manifest with runtime primitives,
//!   producing the `AdapterProvides` structure that the host uses
//!   to configure execution context and event routing.
//!
//! Owns:
//! - Mapping adapter context keys to runtime source inputs
//! - Mapping adapter event kinds to runtime trigger configurations

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt;

use ergo_runtime::action::{ActionEffects, IntentSpec};
use ergo_runtime::common::{
    doc_anchor_for_rule, resolve_manifest_name, ErrorInfo, Phase, ValueType,
};
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
    MissingIntentPayloadSchema {
        kind: String,
        index: usize,
    },
    IntentPayloadSchemaIncompatible {
        kind: String,
        index: usize,
        detail: String,
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
            Self::MissingIntentPayloadSchema { .. } => "COMP-18",
            Self::IntentPayloadSchemaIncompatible { .. } => "COMP-19",
            Self::ManifestNameResolutionFailed { .. } => "COMP-16",
        }
    }

    fn phase(&self) -> Phase {
        Phase::Composition
    }

    fn doc_anchor(&self) -> &'static str {
        doc_anchor_for_rule(self.rule_id())
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
            Self::MissingIntentPayloadSchema { kind, .. } => Cow::Owned(format!(
                "Adapter effect '{}' is missing payload_schema required for intent compatibility checks",
                kind
            )),
            Self::IntentPayloadSchemaIncompatible { kind, detail, .. } => Cow::Owned(format!(
                "Adapter payload_schema for intent kind '{}' is incompatible: {}",
                kind, detail
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
            Self::MissingIntentPayloadSchema { index, .. }
            | Self::IntentPayloadSchemaIncompatible { index, .. } => {
                Some(Cow::Owned(format!("$.effects.intents[{}].fields", index)))
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
            Self::MissingIntentPayloadSchema { kind, .. } => Some(Cow::Owned(format!(
                "Add payload_schema for '{}' under adapter accepts.effects",
                kind
            ))),
            Self::IntentPayloadSchemaIncompatible { .. } => Some(Cow::Borrowed(
                "Adjust accepts.effects payload_schema to match the intent fields/types declared by the action manifest",
            )),
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
    let has_mirror_writes = effects
        .intents
        .iter()
        .any(|intent| !intent.mirror_writes.is_empty());

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

    if (!effects.writes.is_empty() || has_mirror_writes) && !adapter.effects.contains("set_context")
    {
        return Err(CompositionError::MissingSetContextEffect);
    }

    for (index, intent) in effects.intents.iter().enumerate() {
        if !adapter.effects.contains(&intent.name) {
            return Err(CompositionError::MissingIntentEffect {
                kind: intent.name.clone(),
                index,
            });
        }

        let payload_schema = adapter.effect_schemas.get(&intent.name).ok_or_else(|| {
            CompositionError::MissingIntentPayloadSchema {
                kind: intent.name.clone(),
                index,
            }
        })?;
        validate_intent_schema_compatibility(intent, payload_schema).map_err(|detail| {
            CompositionError::IntentPayloadSchemaIncompatible {
                kind: intent.name.clone(),
                index,
                detail,
            }
        })?;
    }

    Ok(())
}

fn validate_intent_schema_compatibility(
    intent: &IntentSpec,
    payload_schema: &serde_json::Value,
) -> Result<(), String> {
    let schema = payload_schema
        .as_object()
        .ok_or_else(|| "payload_schema must be a JSON object".to_string())?;

    if let Some(keyword) = unsupported_schema_keyword(schema) {
        return Err(format!("unsupported JSON Schema keyword '{}'", keyword));
    }

    let schema_type = schema
        .get("type")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "payload_schema.type must be present and set to 'object'".to_string())?;
    if schema_type != "object" {
        return Err(format!(
            "payload_schema.type must be 'object', found '{}'",
            schema_type
        ));
    }

    let properties = schema
        .get("properties")
        .and_then(|value| value.as_object())
        .ok_or_else(|| "payload_schema.properties must be present and be an object".to_string())?;

    let field_names: HashSet<&str> = intent
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();

    if let Some(required) = schema.get("required") {
        let required = required
            .as_array()
            .ok_or_else(|| "payload_schema.required must be an array of field names".to_string())?;
        for item in required {
            let required_name = item
                .as_str()
                .ok_or_else(|| "payload_schema.required entries must be strings".to_string())?;
            if !field_names.contains(required_name) {
                return Err(format!(
                    "required field '{}' is not declared in intent.fields",
                    required_name
                ));
            }
        }
    }

    for field in &intent.fields {
        let property_schema = properties.get(&field.name).ok_or_else(|| {
            format!(
                "intent field '{}' is missing from payload_schema.properties",
                field.name
            )
        })?;
        validate_field_schema_compatibility(&field.value_type, property_schema, &field.name)?;
    }

    Ok(())
}

fn validate_field_schema_compatibility(
    field_type: &ValueType,
    property_schema: &serde_json::Value,
    field_name: &str,
) -> Result<(), String> {
    let property = property_schema
        .as_object()
        .ok_or_else(|| format!("field '{}' schema must be a JSON object", field_name))?;
    if let Some(keyword) = unsupported_schema_keyword(property) {
        return Err(format!(
            "field '{}' uses unsupported JSON Schema keyword '{}'",
            field_name, keyword
        ));
    }

    match field_type {
        ValueType::Number | ValueType::Bool | ValueType::String => {
            let expected = value_type_to_json_type(field_type);
            let actual = property
                .get("type")
                .and_then(|value| value.as_str())
                .ok_or_else(|| {
                    format!(
                        "field '{}' schema must declare type '{}'",
                        field_name, expected
                    )
                })?;
            if actual != expected {
                return Err(format!(
                    "field '{}' expected JSON type '{}', found '{}'",
                    field_name, expected, actual
                ));
            }
        }
        ValueType::Series => {
            let actual = property
                .get("type")
                .and_then(|value| value.as_str())
                .ok_or_else(|| {
                    format!("field '{}' schema must declare type 'array'", field_name)
                })?;
            if actual != "array" {
                return Err(format!(
                    "field '{}' expected JSON type 'array', found '{}'",
                    field_name, actual
                ));
            }
            let items = property
                .get("items")
                .and_then(|value| value.as_object())
                .ok_or_else(|| {
                    format!(
                        "field '{}' array schema must define object 'items'",
                        field_name
                    )
                })?;
            if let Some(keyword) = unsupported_schema_keyword(items) {
                return Err(format!(
                    "field '{}' array items use unsupported JSON Schema keyword '{}'",
                    field_name, keyword
                ));
            }
            let item_type = items
                .get("type")
                .and_then(|value| value.as_str())
                .ok_or_else(|| {
                    format!(
                        "field '{}' array items must declare type 'number'",
                        field_name
                    )
                })?;
            if item_type != "number" {
                return Err(format!(
                    "field '{}' array items expected type 'number', found '{}'",
                    field_name, item_type
                ));
            }
        }
    }
    Ok(())
}

fn value_type_to_json_type(value_type: &ValueType) -> &'static str {
    match value_type {
        ValueType::Number => "number",
        ValueType::Bool => "boolean",
        ValueType::String => "string",
        ValueType::Series => "array",
    }
}

fn unsupported_schema_keyword(schema: &serde_json::Map<String, serde_json::Value>) -> Option<&str> {
    [
        "$ref", "oneOf", "anyOf", "allOf", "not", "if", "then", "else",
    ]
    .iter()
    .copied()
    .find(|keyword| schema.contains_key(*keyword))
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
