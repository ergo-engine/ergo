//! common.rs — Shared utilities for the adapter crate
//!
//! Purpose:
//! - Houses small, reusable helpers that are needed across multiple
//!   modules within the adapter crate.
//!
//! Owns:
//! - JSON value type naming convention
//!
//! Does not own:
//! - Any adapter semantics, validation, or binding logic

/// Return the canonical JSON Schema type name for a `serde_json::Value`.
pub(crate) fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}
