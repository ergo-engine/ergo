//! manifest_usecases::common
//!
//! Purpose:
//! - Hold the shared file-surface parsing helpers reused across manifest
//!   families inside the host manifest ingress module.
//!
//! Owns:
//! - Value-type alias parsing shared by multiple manifest families.
//! - Integer-default parsing used by parameter default normalization.
//!
//! Does not own:
//! - Family-specific rule mapping or host-facing error projection.
//! - Runtime registry validation or adapter composition semantics.
//!
//! Connects to:
//! - `source.rs`, `compute.rs`, `trigger.rs`, and `action.rs`, which apply
//!   these helpers while lowering raw file manifests into typed runtime forms.
//!
//! Safety notes:
//! - These helpers preserve the existing file-surface aliases such as
//!   `boolean` -> `Bool`.
//! - `parse_int_value` intentionally accepts integral JSON numbers like `3.0`
//!   to preserve the current host manifest contract.

use ergo_runtime::common::ValueType;

pub(super) fn parse_value_type(input: &str) -> Option<ValueType> {
    match input.to_ascii_lowercase().as_str() {
        "number" => Some(ValueType::Number),
        "bool" | "boolean" => Some(ValueType::Bool),
        "string" => Some(ValueType::String),
        "series" => Some(ValueType::Series),
        _ => None,
    }
}

pub(super) fn parse_int_value(value: &serde_json::Value) -> Result<i64, String> {
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
