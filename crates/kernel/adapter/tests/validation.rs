use ergo_adapter::{
    validate_adapter, AcceptsSpec, AdapterManifest, CaptureSpec, ContextKeySpec, EffectSpec,
    EventKindSpec,
};
use ergo_runtime::common::ErrorInfo;
use serde_json::json;

fn baseline_manifest() -> AdapterManifest {
    AdapterManifest {
        kind: "adapter".to_string(),
        id: "good_id".to_string(),
        version: "1.0.0".to_string(),
        runtime_compatibility: "0.1.0".to_string(), // matches validate.rs placeholder RUNTIME_VERSION
        context_keys: vec![ContextKeySpec {
            name: "foo".to_string(),
            ty: "String".to_string(),
            required: false,
            writable: Some(false),
            description: None,
        }],
        event_kinds: vec![EventKindSpec {
            name: "tick".to_string(),
            payload_schema: json!({"type":"object","additionalProperties": false}),
        }],
        accepts: Some(AcceptsSpec {
            effects: vec![EffectSpec {
                name: "set_context".to_string(),
                payload_schema: json!({"type":"object","additionalProperties": false}),
            }],
        }),
        capture: CaptureSpec {
            format_version: "1".to_string(),
            fields: vec![
                "event.tick".to_string(),
                "meta.adapter_id".to_string(),
                "meta.adapter_version".to_string(),
                "meta.timestamp".to_string(),
            ],
        },
    }
}

fn assert_rule(manifest: &AdapterManifest, rule: &str, path: Option<&str>) {
    let err = validate_adapter(manifest).expect_err("expected validation error");
    assert_eq!(err.rule_id(), rule);
    assert_eq!(err.path().as_deref(), path);
}

#[test]
fn adp_1_invalid_id_rejected() {
    let mut m = baseline_manifest();
    m.id = "Bad-Id".to_string(); // violates ^[a-z][a-z0-9_]*$
    assert_rule(&m, "ADP-1", Some("$.id"));
}

#[test]
fn adp_2_invalid_version_rejected() {
    let mut m = baseline_manifest();
    m.version = "nope".to_string();
    assert_rule(&m, "ADP-2", Some("$.version"));
}

#[test]
fn adp_3_incompatible_runtime_rejected() {
    let mut m = baseline_manifest();
    m.runtime_compatibility = "9.9.9".to_string(); // higher than placeholder 0.1.0
    assert_rule(&m, "ADP-3", Some("$.runtime_compatibility"));
}

#[test]
fn adp_3_invalid_runtime_compatibility_rejected() {
    let mut m = baseline_manifest();
    m.runtime_compatibility = "nope".to_string();

    let err = validate_adapter(&m).expect_err("expected validation error");
    assert_eq!(err.rule_id(), "ADP-3");
    assert_eq!(err.path().as_deref(), Some("$.runtime_compatibility"));
    let summary = err.summary();
    assert!(summary.contains("runtime_compatibility"));
    assert!(summary.contains("semver"));
}

#[test]
fn adp_4_empty_adapter_rejected() {
    let mut m = baseline_manifest();
    m.context_keys.clear();
    m.event_kinds.clear();
    assert_rule(&m, "ADP-4", None);
}

#[test]
fn adp_5_duplicate_context_key_rejected() {
    let mut m = baseline_manifest();
    m.context_keys.push(m.context_keys[0].clone()); // duplicate "foo"
    assert_rule(&m, "ADP-5", Some("$.context_keys[1].name"));
}

#[test]
fn adp_6_invalid_context_type_rejected() {
    let mut m = baseline_manifest();
    m.context_keys[0].ty = "Nuber".to_string();
    assert_rule(&m, "ADP-6", Some("$.context_keys[0].type"));
}

#[test]
fn adp_7_duplicate_event_kind_rejected() {
    let mut m = baseline_manifest();
    m.event_kinds.push(m.event_kinds[0].clone()); // duplicate "tick"
    assert_rule(&m, "ADP-7", Some("$.event_kinds[1].name"));
}

#[test]
fn adp_8_invalid_schema_rejected() {
    let mut m = baseline_manifest();
    m.event_kinds[0].payload_schema = json!({"type": 123}); // invalid per JSON Schema
    assert_rule(&m, "ADP-8", Some("$.event_kinds[0].payload_schema"));
}

#[test]
fn adp_8_non_object_schema_rejected() {
    let mut m = baseline_manifest();
    m.event_kinds[0].payload_schema = json!(7);
    assert_rule(&m, "ADP-8", Some("$.event_kinds[0].payload_schema"));
}

#[test]
fn adp_8_one_of_rejected() {
    let mut m = baseline_manifest();
    m.event_kinds[0].payload_schema = json!({
        "oneOf": [{"type": "string"}, {"type": "number"}]
    });
    assert_rule(&m, "ADP-8", Some("$.event_kinds[0].payload_schema"));
}

#[test]
fn adp_8_any_of_rejected() {
    let mut m = baseline_manifest();
    m.event_kinds[0].payload_schema = json!({
        "anyOf": [{"type": "string"}, {"type": "number"}]
    });
    assert_rule(&m, "ADP-8", Some("$.event_kinds[0].payload_schema"));
}

#[test]
fn adp_9_no_capture_format_rejected() {
    let mut m = baseline_manifest();
    m.capture.format_version = "".to_string(); // empty string per roadmap
    assert_rule(&m, "ADP-9", Some("$.capture.format_version"));
}

#[test]
fn adp_10_invalid_capture_field_rejected() {
    let mut m = baseline_manifest();
    m.capture.fields.push("meta.not_allowed".to_string());
    assert_rule(&m, "ADP-10", Some("$.capture.fields[4]"));
}

#[test]
fn adp_11_missing_writable_flag_rejected() {
    let mut m = baseline_manifest();
    m.context_keys[0].writable = None;
    assert_rule(&m, "ADP-11", Some("$.context_keys[0].writable"));
}

#[test]
fn adp_12_duplicate_effect_name_rejected() {
    let mut m = baseline_manifest();
    let eff = m.accepts.as_ref().unwrap().effects[0].clone(); // "set_context"
    m.accepts.as_mut().unwrap().effects.push(eff); // duplicate
    assert_rule(&m, "ADP-12", Some("$.accepts.effects[1].name"));
}

#[test]
fn adp_13_invalid_effect_schema_rejected() {
    let mut m = baseline_manifest();
    m.accepts.as_mut().unwrap().effects[0].payload_schema =
        json!({"$ref": "https://example.com/schema.json"});
    assert_rule(&m, "ADP-13", Some("$.accepts.effects[0].payload_schema"));
}

#[test]
fn adp_13_missing_additional_properties_rejected() {
    let mut m = baseline_manifest();
    m.accepts.as_mut().unwrap().effects[0].payload_schema = json!({
        "type": "object",
        "properties": {
            "x": {"type": "string"}
        }
    });
    assert_rule(&m, "ADP-13", Some("$.accepts.effects[0].payload_schema"));
}

#[test]
fn adp_13_additional_properties_true_rejected() {
    let mut m = baseline_manifest();
    m.accepts.as_mut().unwrap().effects[0].payload_schema = json!({
        "type": "object",
        "properties": {
            "x": {"type": "string"}
        },
        "additionalProperties": true
    });
    assert_rule(&m, "ADP-13", Some("$.accepts.effects[0].payload_schema"));
}

#[test]
fn adp_14_writable_without_set_context_rejected() {
    let mut m = baseline_manifest();
    m.context_keys[0].writable = Some(true);

    // remove set_context
    m.accepts = Some(AcceptsSpec { effects: vec![] });

    assert_rule(&m, "ADP-14", Some("$.accepts.effects"));
}

#[test]
fn adp_17_writable_key_required_rejected() {
    let mut m = baseline_manifest();
    m.context_keys[0].writable = Some(true);
    m.context_keys[0].required = true;
    assert_rule(&m, "ADP-17", Some("$.context_keys[0]"));
}

#[test]
fn adp_18_required_event_field_not_provided_rejected() {
    let mut m = baseline_manifest();
    m.event_kinds[0].payload_schema = json!({
        "type": "object",
        "properties": {
            "price": {"type": "number"}
        },
        "required": ["price"],
        "additionalProperties": false
    });
    assert_rule(
        &m,
        "ADP-18",
        Some("$.event_kinds[0].payload_schema.properties.price"),
    );
}

#[test]
fn adp_18_no_required_fields_vacuously_passes() {
    let mut m = baseline_manifest();
    m.context_keys.clear();
    m.context_keys.push(ContextKeySpec {
        name: "foo".to_string(),
        ty: "String".to_string(),
        required: false,
        writable: Some(false),
        description: None,
    });
    m.event_kinds[0].payload_schema = json!({
        "type": "object",
        "properties": {
            "price": {"type": "number"}
        },
        "additionalProperties": false
    });

    validate_adapter(&m).expect("ADP-18 should only enforce declared required fields");
}

#[test]
fn adp_18_required_event_field_type_mismatch_rejected() {
    let mut m = baseline_manifest();
    m.context_keys[0].name = "price".to_string();
    m.context_keys[0].ty = "String".to_string();
    m.event_kinds[0].payload_schema = json!({
        "type": "object",
        "properties": {
            "price": {"type": "number"}
        },
        "required": ["price"],
        "additionalProperties": false
    });
    assert_rule(
        &m,
        "ADP-18",
        Some("$.event_kinds[0].payload_schema.properties.price"),
    );
}

#[test]
fn adp_19_event_payload_schema_must_be_object() {
    let mut m = baseline_manifest();
    m.event_kinds[0].payload_schema = json!({"type":"string"});
    assert_rule(&m, "ADP-19", Some("$.event_kinds[0].payload_schema"));
}

#[test]
fn adp_19_unsupported_event_field_type_rejected() {
    let mut m = baseline_manifest();
    m.event_kinds[0].payload_schema = json!({
        "type": "object",
        "properties": {
            "nested": {
                "type": "object",
                "additionalProperties": false
            }
        },
        "additionalProperties": false
    });
    assert_rule(
        &m,
        "ADP-19",
        Some("$.event_kinds[0].payload_schema.properties.nested"),
    );
}
