pub mod doc_anchor;
pub mod effect;
pub mod error_info;
pub mod errors;
pub mod intent_id;
pub mod manifest;
pub mod value;

pub use doc_anchor::doc_anchor_for_rule;
pub use effect::{ActionEffect, EffectWrite, IntentField, IntentRecord};
pub use error_info::{ErrorInfo, Phase, RuleViolation};
pub use errors::ValidationError;
pub use intent_id::derive_intent_id;
pub use manifest::{resolve_manifest_name, ManifestNameError};
pub use value::{PrimitiveKind, Value, ValueType};

/// Validate that an identifier follows the naming convention:
/// starts with lowercase ASCII letter, followed by lowercase ASCII
/// letters, digits, or underscores.
///
/// Used across all four registry crates (source, compute, trigger, action)
/// for primitive ID and version validation.
pub fn is_valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}
