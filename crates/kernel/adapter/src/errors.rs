//! errors.rs — Adapter error types
//!
//! Purpose:
//! - Defines error types for adapter manifest validation, event
//!   binding, fixture construction, and composition failures.
//!
//! Owns:
//! - `InvalidAdapter`, `InvalidAdapterComposition`, `InvalidBinding`

use std::borrow::Cow;
use std::fmt;

use ergo_runtime::common::{doc_anchor_for_rule, ErrorInfo, Phase};
#[derive(Debug)]
pub enum InvalidAdapter {
    InvalidId {
        id: String,
    },
    InvalidVersion {
        version: String,
    },
    InvalidRuntimeCompatibility {
        version: String,
    },
    IncompatibleRuntime {
        required: String,
        actual: String,
    },
    ProvidesNothing,
    DuplicateContextKey {
        name: String,
        first_index: usize,
        second_index: usize,
    },
    InvalidContextKeyType {
        name: String,
        got: String,
        index: usize,
    },
    DuplicateEventKind {
        name: String,
        index: usize,
    },
    InvalidPayloadSchema {
        event: String,
        error: String,
        index: usize,
    },
    NoCaptureFormat,
    InvalidCaptureField {
        field: String,
        index: usize,
    },
    MissingWritableFlag {
        key: String,
        index: usize,
    },
    DuplicateEffectName {
        name: String,
        index: usize,
    },
    InvalidEffectSchema {
        effect: String,
        error: String,
        index: usize,
    },
    WritableWithoutSetContext {
        keys: Vec<String>,
    },
    WritableKeyNotCaptured {
        key: String,
        index: usize,
    },
    SetContextNotCaptured,
    WritableKeyRequired {
        key: String,
        index: usize,
    },
    RequiredEventFieldNotProvided {
        event: String,
        field: String,
        event_index: usize,
    },
    RequiredEventFieldTypeMismatch {
        event: String,
        field: String,
        expected: String,
        got: String,
        event_index: usize,
    },
    EventPayloadSchemaNotObject {
        event: String,
        event_index: usize,
    },
    UnsupportedEventFieldType {
        event: String,
        field: String,
        detail: String,
        event_index: usize,
    },
}

impl ErrorInfo for InvalidAdapter {
    fn rule_id(&self) -> &'static str {
        match self {
            Self::InvalidId { .. } => "ADP-1",
            Self::InvalidVersion { .. } => "ADP-2",
            Self::InvalidRuntimeCompatibility { .. } => "ADP-3",
            Self::IncompatibleRuntime { .. } => "ADP-3",
            Self::ProvidesNothing => "ADP-4",
            Self::DuplicateContextKey { .. } => "ADP-5",
            Self::InvalidContextKeyType { .. } => "ADP-6",
            Self::DuplicateEventKind { .. } => "ADP-7",
            Self::InvalidPayloadSchema { .. } => "ADP-8",
            Self::NoCaptureFormat => "ADP-9",
            Self::InvalidCaptureField { .. } => "ADP-10",
            Self::MissingWritableFlag { .. } => "ADP-11",
            Self::DuplicateEffectName { .. } => "ADP-12",
            Self::InvalidEffectSchema { .. } => "ADP-13",
            Self::WritableWithoutSetContext { .. } => "ADP-14",
            Self::WritableKeyNotCaptured { .. } => "ADP-15",
            Self::SetContextNotCaptured => "ADP-16",
            Self::WritableKeyRequired { .. } => "ADP-17",
            Self::RequiredEventFieldNotProvided { .. } => "ADP-18",
            Self::RequiredEventFieldTypeMismatch { .. } => "ADP-18",
            Self::EventPayloadSchemaNotObject { .. } => "ADP-19",
            Self::UnsupportedEventFieldType { .. } => "ADP-19",
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
            Self::InvalidId { id } => Cow::Owned(format!("Invalid adapter ID: '{}'", id)),
            Self::InvalidVersion { version } => {
                Cow::Owned(format!("Invalid version: '{}'", version))
            }
            Self::InvalidRuntimeCompatibility { version } => Cow::Owned(format!(
                "Invalid runtime_compatibility: '{}' (must be semver)",
                version
            )),
            Self::IncompatibleRuntime { required, actual } => {
                Cow::Owned(format!("runtime {} < required {}", actual, required))
            }
            Self::ProvidesNothing => Cow::Borrowed("Adapter provides no context keys or events"),
            Self::DuplicateContextKey { name, .. } => {
                Cow::Owned(format!("Duplicate context key: '{}'", name))
            }
            Self::InvalidContextKeyType { name, got, .. } => {
                Cow::Owned(format!("Context key '{}' has invalid type '{}'", name, got))
            }
            Self::DuplicateEventKind { name, .. } => {
                Cow::Owned(format!("Duplicate event kind: '{}'", name))
            }
            Self::InvalidPayloadSchema { event, error, .. } => Cow::Owned(format!(
                "Invalid payload schema for event '{}': {}",
                event, error
            )),
            Self::NoCaptureFormat => Cow::Borrowed("Capture format_version is empty or invalid"),
            Self::InvalidCaptureField { field, .. } => Cow::Owned(format!(
                "Capture field '{}' is not in CaptureFieldSet",
                field
            )),
            Self::MissingWritableFlag { key, .. } => Cow::Owned(format!(
                "Context key '{}' is missing the 'writable' field",
                key
            )),
            Self::DuplicateEffectName { name, .. } => {
                Cow::Owned(format!("Duplicate effect name: '{}'", name))
            }
            Self::InvalidEffectSchema { effect, error, .. } => Cow::Owned(format!(
                "Invalid payload schema for effect '{}': {}",
                effect, error
            )),
            Self::WritableWithoutSetContext { keys } => Cow::Owned(format!(
                "Writable keys {:?} declared but adapter has no set_context effect",
                keys
            )),
            Self::WritableKeyNotCaptured { key, .. } => Cow::Owned(format!(
                "Writable key '{}' must be captured for replay determinism",
                key
            )),
            Self::SetContextNotCaptured => {
                Cow::Borrowed("set_context effect must be captured when writable keys exist")
            }
            Self::WritableKeyRequired { key, .. } => {
                Cow::Owned(format!("Writable key '{}' cannot have required: true", key))
            }
            Self::RequiredEventFieldNotProvided { event, field, .. } => Cow::Owned(format!(
                "Required event field '{}.{}' is not declared in adapter context_keys",
                event, field
            )),
            Self::RequiredEventFieldTypeMismatch {
                event,
                field,
                expected,
                got,
                ..
            } => Cow::Owned(format!(
                "Required event field '{}.{}' type mismatch: context key is '{}', schema requires '{}'",
                event, field, got, expected
            )),
            Self::EventPayloadSchemaNotObject { event, .. } => Cow::Owned(format!(
                "Event '{}' payload_schema must be an object schema for context materialization",
                event
            )),
            Self::UnsupportedEventFieldType {
                event, field, detail, ..
            } => Cow::Owned(format!(
                "Event field '{}.{}' uses unsupported materialization type: {}",
                event, field, detail
            )),
        }
    }

    fn path(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::InvalidId { .. } => Some(Cow::Borrowed("$.id")),
            Self::InvalidVersion { .. } => Some(Cow::Borrowed("$.version")),
            Self::InvalidRuntimeCompatibility { .. } => {
                Some(Cow::Borrowed("$.runtime_compatibility"))
            }
            Self::IncompatibleRuntime { .. } => Some(Cow::Borrowed("$.runtime_compatibility")),
            Self::ProvidesNothing => None,
            Self::DuplicateContextKey { second_index, .. } => {
                Some(Cow::Owned(format!("$.context_keys[{}].name", second_index)))
            }
            Self::InvalidContextKeyType { index, .. } => {
                Some(Cow::Owned(format!("$.context_keys[{}].type", index)))
            }
            Self::DuplicateEventKind { index, .. } => {
                Some(Cow::Owned(format!("$.event_kinds[{}].name", index)))
            }
            Self::InvalidPayloadSchema { index, .. } => Some(Cow::Owned(format!(
                "$.event_kinds[{}].payload_schema",
                index
            ))),
            Self::NoCaptureFormat => Some(Cow::Borrowed("$.capture.format_version")),
            Self::InvalidCaptureField { index, .. } => {
                Some(Cow::Owned(format!("$.capture.fields[{}]", index)))
            }
            Self::MissingWritableFlag { index, .. } => {
                Some(Cow::Owned(format!("$.context_keys[{}].writable", index)))
            }
            Self::DuplicateEffectName { index, .. } => {
                Some(Cow::Owned(format!("$.accepts.effects[{}].name", index)))
            }
            Self::InvalidEffectSchema { index, .. } => Some(Cow::Owned(format!(
                "$.accepts.effects[{}].payload_schema",
                index
            ))),
            Self::WritableWithoutSetContext { .. } => Some(Cow::Borrowed("$.accepts.effects")),
            Self::WritableKeyNotCaptured { index, .. } => {
                Some(Cow::Owned(format!("$.context_keys[{}]", index)))
            }
            Self::SetContextNotCaptured => Some(Cow::Borrowed("$.capture.fields")),
            Self::WritableKeyRequired { index, .. } => {
                Some(Cow::Owned(format!("$.context_keys[{}]", index)))
            }
            Self::RequiredEventFieldNotProvided {
                event_index, field, ..
            } => Some(Cow::Owned(format!(
                "$.event_kinds[{}].payload_schema.properties.{}",
                event_index, field
            ))),
            Self::RequiredEventFieldTypeMismatch {
                event_index, field, ..
            } => Some(Cow::Owned(format!(
                "$.event_kinds[{}].payload_schema.properties.{}",
                event_index, field
            ))),
            Self::EventPayloadSchemaNotObject { event_index, .. } => Some(Cow::Owned(format!(
                "$.event_kinds[{}].payload_schema",
                event_index
            ))),
            Self::UnsupportedEventFieldType {
                event_index, field, ..
            } => Some(Cow::Owned(format!(
                "$.event_kinds[{}].payload_schema.properties.{}",
                event_index, field
            ))),
        }
    }

    fn fix(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::InvalidId { .. } => Some(Cow::Borrowed(
                "ID must start with lowercase letter, contain only lowercase letters, digits, and underscores (no hyphens)",
            )),
            Self::InvalidVersion { .. } => Some(Cow::Borrowed(
                "Version must be valid semver (e.g., '1.0.0')",
            )),
            Self::InvalidRuntimeCompatibility { .. } => Some(Cow::Borrowed(
                "runtime_compatibility must be valid semver (e.g., '0.1.0')",
            )),
            Self::IncompatibleRuntime { required, .. } => {
                Some(Cow::Owned(format!(
                    "upgrade runtime to {} or higher",
                    required
                )))
            }
            Self::ProvidesNothing => Some(Cow::Borrowed(
                "Add at least one context_key or event_kind",
            )),
            Self::DuplicateContextKey { name, .. } => Some(Cow::Owned(format!(
                "Rename '{}' to a unique value",
                name
            ))),
            Self::InvalidContextKeyType { got, .. } => Some(Cow::Owned(format!(
                "Type '{}' is not valid; use Number, Bool, String, or Series",
                got
            ))),
            Self::DuplicateEventKind { name, .. } => Some(Cow::Owned(format!(
                "Rename event kind '{}' to a unique value",
                name
            ))),
            Self::InvalidPayloadSchema { .. } => Some(Cow::Borrowed(
                "Provide a valid JSON Schema (Draft 2020-12)",
            )),
            Self::NoCaptureFormat => Some(Cow::Borrowed(
                "Set capture.format_version to a non-empty string",
            )),
            Self::InvalidCaptureField { field, .. } => Some(Cow::Owned(format!(
                "'{}' is not in CaptureFieldSet; valid selectors: event.<kind>, meta.adapter_id, meta.adapter_version, meta.timestamp",
                field
            ))),
            Self::MissingWritableFlag { key, .. } => Some(Cow::Owned(format!(
                "Add 'writable: true' or 'writable: false' to context key '{}'",
                key
            ))),
            Self::DuplicateEffectName { name, .. } => Some(Cow::Owned(format!(
                "Rename effect '{}' to a unique value",
                name
            ))),
            Self::InvalidEffectSchema { .. } => Some(Cow::Borrowed(
                "Provide a valid JSON Schema (Draft 2020-12)",
            )),
            Self::WritableWithoutSetContext { .. } => Some(Cow::Borrowed(
                "Add 'set_context' to accepts.effects when using writable keys",
            )),
            // ADP-15: Deferred until REP-SCOPE expansion
            Self::WritableKeyNotCaptured { .. } => None,
            // ADP-16: Deferred until REP-SCOPE expansion
            Self::SetContextNotCaptured => None,
            Self::WritableKeyRequired { key, .. } => Some(Cow::Owned(format!(
                "Set 'required: false' on writable key '{}'",
                key
            ))),
            Self::RequiredEventFieldNotProvided { field, .. } => Some(Cow::Owned(format!(
                "Add context key '{}' to context_keys with matching type",
                field
            ))),
            Self::RequiredEventFieldTypeMismatch {
                field, expected, ..
            } => Some(Cow::Owned(format!(
                "Change context key '{}' type to '{}'",
                field, expected
            ))),
            Self::EventPayloadSchemaNotObject { .. } => Some(Cow::Borrowed(
                "Set event payload_schema.type to 'object' and declare properties",
            )),
            Self::UnsupportedEventFieldType { .. } => Some(Cow::Borrowed(
                "Use field types that map to runtime values: number/integer, boolean, string, or array of numbers",
            )),
        }
    }
}

impl fmt::Display for InvalidAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.rule_id(), self.summary())
    }
}

impl std::error::Error for InvalidAdapter {}
