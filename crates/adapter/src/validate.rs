use std::collections::HashSet;

use jsonschema::draft202012;
use regex::Regex;
use semver::Version;
use serde_json::Value;

use crate::errors::InvalidAdapter;
use crate::manifest::AdapterManifest;

/// Placeholder constant for runtime version comparison (ADP-3).
/// TODO: Replace with actual runtime version from ergo-runtime crate.
const RUNTIME_VERSION: &str = "0.1.0";

pub fn validate_adapter(manifest: &AdapterManifest) -> Result<(), InvalidAdapter> {
    check_adp_1(manifest)?;
    check_adp_2(manifest)?;
    check_adp_3(manifest)?;
    check_adp_4(manifest)?;
    check_adp_5(manifest)?;
    check_adp_6(manifest)?;
    check_adp_7(manifest)?;
    check_adp_8(manifest)?;
    check_adp_9(manifest)?;
    check_adp_10(manifest)?;
    check_adp_11(manifest)?;
    check_adp_12(manifest)?;
    check_adp_13(manifest)?;
    check_adp_14(manifest)?;
    check_adp_17(manifest)?;
    // ADP-15, ADP-16: TODO - Deferred until REP-SCOPE expansion
    Ok(())
}

fn validate_schema(schema: &Value) -> Result<(), String> {
    if !schema.is_object() {
        return Err("Schema must be a JSON object".to_string());
    }

    validate_schema_bans(schema)?;
    draft202012::new(schema).map_err(|e| e.to_string())?;

    Ok(())
}

fn validate_schema_bans(value: &Value) -> Result<(), String> {
    match value {
        Value::Object(map) => {
            if map.contains_key("oneOf") {
                return Err("Schema contains banned keyword: oneOf".to_string());
            }
            if map.contains_key("anyOf") {
                return Err("Schema contains banned keyword: anyOf".to_string());
            }
            if let Some(reference) = map.get("$ref") {
                match reference {
                    Value::String(reference) => {
                        if !reference.starts_with('#') {
                            return Err(format!("External $ref is forbidden: {}", reference));
                        }
                    }
                    _ => {
                        return Err("Schema $ref must be a string".to_string());
                    }
                }
            }
            if requires_additional_properties_false(map) {
                match map.get("additionalProperties") {
                    Some(Value::Bool(false)) => {}
                    Some(Value::Bool(true)) => {
                        return Err("Schema additionalProperties must be false".to_string());
                    }
                    Some(_) => {
                        return Err("Schema additionalProperties must be false".to_string());
                    }
                    None => {
                        return Err("Schema missing additionalProperties: false".to_string());
                    }
                }
            }
            for value in map.values() {
                validate_schema_bans(value)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                validate_schema_bans(value)?;
            }
        }
        _ => {}
    }

    Ok(())
}

fn requires_additional_properties_false(map: &serde_json::Map<String, Value>) -> bool {
    if map.contains_key("properties") {
        return true;
    }

    match map.get("type") {
        Some(Value::String(ty)) => ty == "object",
        Some(Value::Array(types)) => types.iter().any(|ty| match ty {
            Value::String(value) => value == "object",
            _ => false,
        }),
        _ => false,
    }
}

/// ADP-1: ID format valid
/// Enforce id matches regex: ^[a-z][a-z0-9_]*$
fn check_adp_1(m: &AdapterManifest) -> Result<(), InvalidAdapter> {
    let re = Regex::new(r"^[a-z][a-z0-9_]*$").expect("valid regex");
    if !re.is_match(&m.id) {
        return Err(InvalidAdapter::InvalidId { id: m.id.clone() });
    }
    Ok(())
}

/// ADP-2: Version valid semver
fn check_adp_2(m: &AdapterManifest) -> Result<(), InvalidAdapter> {
    if Version::parse(&m.version).is_err() {
        return Err(InvalidAdapter::InvalidVersion {
            version: m.version.clone(),
        });
    }
    Ok(())
}

/// ADP-3: Runtime compatibility satisfied
fn check_adp_3(m: &AdapterManifest) -> Result<(), InvalidAdapter> {
    let required = Version::parse(&m.runtime_compatibility).map_err(|_| {
        InvalidAdapter::InvalidRuntimeCompatibility {
            version: m.runtime_compatibility.clone(),
        }
    })?;

    let actual = Version::parse(RUNTIME_VERSION).expect("valid constant");

    if actual < required {
        return Err(InvalidAdapter::IncompatibleRuntime {
            required: m.runtime_compatibility.clone(),
            actual: RUNTIME_VERSION.to_string(),
        });
    }
    Ok(())
}

/// ADP-4: Provides something
/// Reject if context_keys.is_empty() AND event_kinds.is_empty()
fn check_adp_4(m: &AdapterManifest) -> Result<(), InvalidAdapter> {
    if m.context_keys.is_empty() && m.event_kinds.is_empty() {
        return Err(InvalidAdapter::ProvidesNothing);
    }
    Ok(())
}

/// ADP-5: Context key names unique
fn check_adp_5(m: &AdapterManifest) -> Result<(), InvalidAdapter> {
    let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (index, key) in m.context_keys.iter().enumerate() {
        if let Some(&first_index) = seen.get(key.name.as_str()) {
            return Err(InvalidAdapter::DuplicateContextKey {
                name: key.name.clone(),
                first_index,
                second_index: index,
            });
        }
        seen.insert(&key.name, index);
    }
    Ok(())
}

/// ADP-6: Context key types valid
/// ty string must be one of: "Number" | "Bool" | "String" | "Series"
fn check_adp_6(m: &AdapterManifest) -> Result<(), InvalidAdapter> {
    const VALID_TYPES: &[&str] = &["Number", "Bool", "String", "Series"];
    for (index, key) in m.context_keys.iter().enumerate() {
        if !VALID_TYPES.contains(&key.ty.as_str()) {
            return Err(InvalidAdapter::InvalidContextKeyType {
                name: key.name.clone(),
                got: key.ty.clone(),
                index,
            });
        }
    }
    Ok(())
}

/// ADP-7: Event kind names unique
fn check_adp_7(m: &AdapterManifest) -> Result<(), InvalidAdapter> {
    let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (index, event) in m.event_kinds.iter().enumerate() {
        if let Some(&_first_index) = seen.get(event.name.as_str()) {
            return Err(InvalidAdapter::DuplicateEventKind {
                name: event.name.clone(),
                index,
            });
        }
        seen.insert(&event.name, index);
    }
    Ok(())
}

/// ADP-8: Event schemas valid JSON Schema
/// Validates Draft 2020-12 plus Phase 1 schema bans.
fn check_adp_8(m: &AdapterManifest) -> Result<(), InvalidAdapter> {
    for (index, event) in m.event_kinds.iter().enumerate() {
        if let Err(e) = validate_schema(&event.payload_schema) {
            return Err(InvalidAdapter::InvalidPayloadSchema {
                event: event.name.clone(),
                error: e.to_string(),
                index,
            });
        }
    }
    Ok(())
}

/// ADP-9: Capture format version present
/// Reject if capture.format_version is empty string
fn check_adp_9(m: &AdapterManifest) -> Result<(), InvalidAdapter> {
    if m.capture.format_version.is_empty() {
        return Err(InvalidAdapter::NoCaptureFormat);
    }
    Ok(())
}

/// ADP-10: Capture fields referentially valid
/// CaptureFieldSet(adapter) = event.<event_kind_name> for each declared event kind
///                          + meta.adapter_id, meta.adapter_version, meta.timestamp
fn check_adp_10(m: &AdapterManifest) -> Result<(), InvalidAdapter> {
    let mut valid_fields: HashSet<String> = HashSet::new();

    // Add event.<kind> for each declared event kind
    for event in &m.event_kinds {
        valid_fields.insert(format!("event.{}", event.name));
    }

    // Add meta fields
    valid_fields.insert("meta.adapter_id".to_string());
    valid_fields.insert("meta.adapter_version".to_string());
    valid_fields.insert("meta.timestamp".to_string());

    // Check each capture field
    for (index, field) in m.capture.fields.iter().enumerate() {
        if !valid_fields.contains(field) {
            return Err(InvalidAdapter::InvalidCaptureField {
                field: field.clone(),
                index,
            });
        }
    }
    Ok(())
}

/// ADP-11: Writable flag must be present
/// Reject if any context key has writable: None
fn check_adp_11(m: &AdapterManifest) -> Result<(), InvalidAdapter> {
    for (index, key) in m.context_keys.iter().enumerate() {
        if key.writable.is_none() {
            return Err(InvalidAdapter::MissingWritableFlag {
                key: key.name.clone(),
                index,
            });
        }
    }
    Ok(())
}

/// ADP-12: Effect names unique
fn check_adp_12(m: &AdapterManifest) -> Result<(), InvalidAdapter> {
    if let Some(accepts) = &m.accepts {
        let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for (index, effect) in accepts.effects.iter().enumerate() {
            if let Some(&_first_index) = seen.get(effect.name.as_str()) {
                return Err(InvalidAdapter::DuplicateEffectName {
                    name: effect.name.clone(),
                    index,
                });
            }
            seen.insert(&effect.name, index);
        }
    }
    Ok(())
}

/// ADP-13: Effect schemas valid
/// Validates Draft 2020-12 plus Phase 1 schema bans.
fn check_adp_13(m: &AdapterManifest) -> Result<(), InvalidAdapter> {
    if let Some(accepts) = &m.accepts {
        for (index, effect) in accepts.effects.iter().enumerate() {
            if let Err(e) = validate_schema(&effect.payload_schema) {
                return Err(InvalidAdapter::InvalidEffectSchema {
                    effect: effect.name.clone(),
                    error: e.to_string(),
                    index,
                });
            }
        }
    }
    Ok(())
}

/// ADP-14: Writable implies set_context accepted
/// If any context key has writable == Some(true), require accepts contains "set_context"
fn check_adp_14(m: &AdapterManifest) -> Result<(), InvalidAdapter> {
    let writable_keys: Vec<String> = m
        .context_keys
        .iter()
        .filter(|k| k.writable == Some(true))
        .map(|k| k.name.clone())
        .collect();

    if !writable_keys.is_empty() {
        let has_set_context = m
            .accepts
            .as_ref()
            .map(|a| a.effects.iter().any(|e| e.name == "set_context"))
            .unwrap_or(false);

        if !has_set_context {
            return Err(InvalidAdapter::WritableWithoutSetContext {
                keys: writable_keys,
            });
        }
    }
    Ok(())
}

/// ADP-17: Writable keys cannot be required
/// Reject if any context key has writable == Some(true) AND required == true
fn check_adp_17(m: &AdapterManifest) -> Result<(), InvalidAdapter> {
    for (index, key) in m.context_keys.iter().enumerate() {
        if key.writable == Some(true) && key.required {
            return Err(InvalidAdapter::WritableKeyRequired {
                key: key.name.clone(),
                index,
            });
        }
    }
    Ok(())
}
