use std::fmt;

use jsonschema::draft202012;

use crate::schema_materialization::{
    schema_properties, schema_property_to_context_type, schema_required_fields,
};
use crate::{AdapterProvides, EventId, EventPayload, EventTime, ExternalEvent, ExternalEventKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventBindingError {
    UnknownSemanticKind {
        kind: String,
    },
    InvalidSchema {
        kind: String,
        detail: String,
    },
    PayloadSchemaMismatch {
        kind: String,
        detail: String,
    },
    PayloadMustBeObject {
        kind: String,
    },
    MissingRequiredField {
        kind: String,
        field: String,
    },
    UnsupportedFieldType {
        kind: String,
        field: String,
        detail: String,
    },
    MissingContextProvision {
        kind: String,
        field: String,
    },
    ContextTypeMismatch {
        kind: String,
        field: String,
        expected: String,
        got: String,
    },
}

impl fmt::Display for EventBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSemanticKind { kind } => {
                write!(f, "unknown semantic event kind '{kind}'")
            }
            Self::InvalidSchema { kind, detail } => {
                write!(f, "invalid schema for semantic event kind '{kind}': {detail}")
            }
            Self::PayloadSchemaMismatch { kind, detail } => {
                write!(f, "payload does not match schema for semantic event kind '{kind}': {detail}")
            }
            Self::PayloadMustBeObject { kind } => {
                write!(f, "semantic event payload for kind '{kind}' must be a JSON object")
            }
            Self::MissingRequiredField { kind, field } => {
                write!(f, "required field '{field}' missing for semantic event kind '{kind}'")
            }
            Self::UnsupportedFieldType {
                kind,
                field,
                detail,
            } => write!(
                f,
                "unsupported field type for '{kind}.{field}' in semantic event schema: {detail}"
            ),
            Self::MissingContextProvision { kind, field } => write!(
                f,
                "semantic event field '{kind}.{field}' is required but no matching adapter context key exists"
            ),
            Self::ContextTypeMismatch {
                kind,
                field,
                expected,
                got,
            } => write!(
                f,
                "semantic event field '{kind}.{field}' type mismatch: expected context type '{expected}', got '{got}'"
            ),
        }
    }
}

impl std::error::Error for EventBindingError {}

pub fn bind_semantic_event(
    provides: &AdapterProvides,
    event_id: EventId,
    kind: ExternalEventKind,
    at: EventTime,
    semantic_kind: &str,
    payload: serde_json::Value,
) -> Result<ExternalEvent, EventBindingError> {
    let schema = provides.event_schemas.get(semantic_kind).ok_or_else(|| {
        EventBindingError::UnknownSemanticKind {
            kind: semantic_kind.to_string(),
        }
    })?;

    let validator = draft202012::new(schema).map_err(|err| EventBindingError::InvalidSchema {
        kind: semantic_kind.to_string(),
        detail: err.to_string(),
    })?;

    if let Err(err) = validator.validate(&payload) {
        return Err(EventBindingError::PayloadSchemaMismatch {
            kind: semantic_kind.to_string(),
            detail: err.to_string(),
        });
    }

    let payload_object =
        payload
            .as_object()
            .ok_or_else(|| EventBindingError::PayloadMustBeObject {
                kind: semantic_kind.to_string(),
            })?;

    let schema_object =
        schema
            .as_object()
            .ok_or_else(|| EventBindingError::PayloadMustBeObject {
                kind: semantic_kind.to_string(),
            })?;

    let required_fields = schema_required_fields(schema_object);
    let properties = schema_properties(schema_object);

    for field in required_fields {
        let field_schema = properties.and_then(|map| map.get(field)).ok_or_else(|| {
            EventBindingError::UnsupportedFieldType {
                kind: semantic_kind.to_string(),
                field: field.to_string(),
                detail: "required field is not declared in payload_schema.properties".to_string(),
            }
        })?;

        let expected_context_type =
            schema_property_to_context_type(field_schema).map_err(|detail| {
                EventBindingError::UnsupportedFieldType {
                    kind: semantic_kind.to_string(),
                    field: field.to_string(),
                    detail,
                }
            })?;

        let Some(context_key) = provides.context.get(field) else {
            return Err(EventBindingError::MissingContextProvision {
                kind: semantic_kind.to_string(),
                field: field.to_string(),
            });
        };

        if context_key.ty != expected_context_type {
            return Err(EventBindingError::ContextTypeMismatch {
                kind: semantic_kind.to_string(),
                field: field.to_string(),
                expected: expected_context_type.to_string(),
                got: context_key.ty.clone(),
            });
        }

        if payload_object.get(field).is_none() {
            return Err(EventBindingError::MissingRequiredField {
                kind: semantic_kind.to_string(),
                field: field.to_string(),
            });
        }
    }

    let bytes =
        serde_json::to_vec(&payload).map_err(|err| EventBindingError::PayloadSchemaMismatch {
            kind: semantic_kind.to_string(),
            detail: err.to_string(),
        })?;

    Ok(ExternalEvent::with_payload(
        event_id,
        kind,
        at,
        EventPayload { data: bytes },
    ))
}
