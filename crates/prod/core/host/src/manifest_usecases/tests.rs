//! manifest_usecases tests
//!
//! Purpose:
//! - Lock the host-owned file-manifest ingress contracts in
//!   `manifest_usecases.rs`.
//!
//! Owns:
//! - Lower-level text/value entrypoint coverage and file-surface
//!   normalization checks that CLI path-based tests do not exercise directly.
//!
//! Does not own:
//! - End-to-end CLI rendering coverage or typed runtime registry coverage.
//!
//! Safety notes:
//! - These tests lock host-specific aliasing/defaulting and the current host
//!   projection of unsupported compose targets.

use super::*;
use ergo_runtime::common::{doc_anchor_for_rule, Value, ValueType};

#[test]
fn validate_manifest_text_accepts_source_yaml() -> Result<(), String> {
    let summary = validate_manifest_text(
        "memory/source.yaml",
        r#"
kind: source
id: mem_source
version: "0.1.0"
inputs: []
outputs:
  - name: value
    type: number
parameters: []
requires:
  context: []
execution:
  deterministic: true
  cadence: continuous
state:
  allowed: false
side_effects: false
"#,
    )
    .map_err(|err| format!("{err:?}"))?;

    assert_eq!(summary.kind, "source");
    assert_eq!(summary.id, "mem_source");
    assert_eq!(summary.version, "0.1.0");
    Ok(())
}

#[test]
fn validate_manifest_value_reports_source_label_on_parse_error() {
    let err = validate_manifest_value(
        "memory/bad.yaml",
        serde_json::json!({
            "id": "broken",
            "version": "0.1.0"
        }),
    )
    .expect_err("missing kind should fail")
    .into_rule_violation();

    assert_eq!(err.rule_id, "INTERNAL");
    assert!(err.summary.contains("memory/bad.yaml"));
    assert_eq!(err.path, None);
    assert_eq!(err.fix, None);
}

#[test]
fn check_compose_text_accepts_in_memory_manifests() -> Result<(), String> {
    check_compose_text(
        "memory/adapter.yaml",
        r#"
kind: adapter
id: demo_adapter
version: "0.1.0"
runtime_compatibility: "0.1.0"
context_keys:
  - name: auth_token
    type: String
    required: true
    writable: false
event_kinds: []
capture:
  format_version: "1"
  fields: []
"#,
        "memory/source.yaml",
        r#"
kind: source
id: demo_source
version: "0.1.0"
inputs: []
outputs:
  - name: value
    type: number
parameters: []
requires:
  context:
    - name: auth_token
      type: string
      required: true
execution:
  deterministic: true
  cadence: continuous
state:
  allowed: false
side_effects: false
"#,
    )
    .map_err(|err| format!("{err:?}"))
}

#[test]
fn check_compose_values_rejects_unsupported_target_kind_with_current_projection() {
    let err = check_compose_values(
        "memory/adapter.json",
        serde_json::json!({
            "kind": "adapter",
            "id": "demo_adapter",
            "version": "0.1.0",
            "runtime_compatibility": "0.1.0",
            "context_keys": [{
                "name": "auth_token",
                "type": "String",
                "required": true,
                "writable": false,
                "description": null
            }],
            "event_kinds": [],
            "capture": {"format_version": "1", "fields": []}
        }),
        "memory/compute.json",
        serde_json::json!({
            "kind": "compute",
            "id": "other",
            "version": "0.1.0",
            "inputs": [],
            "outputs": [{"name":"value","type":"number"}],
            "parameters": [],
            "execution": {"deterministic": true, "cadence": "continuous", "may_error": false},
            "errors": {"allowed": false, "types": [], "deterministic": true},
            "state": {"allowed": false, "resettable": false, "description": null},
            "side_effects": false
        }),
    )
    .expect_err("compute target should be rejected")
    .into_rule_violation();

    assert_eq!(err.rule_id, "COMP-1");
    assert_eq!(err.phase, "composition");
    assert_eq!(err.doc_anchor, doc_anchor_for_rule("COMP-1"));
    assert_eq!(
        err.summary,
        "unsupported manifest kind for composition: 'compute'"
    );
    assert_eq!(err.path.as_deref(), Some("$.kind"));
    assert_eq!(
        err.fix.as_deref(),
        Some("Use a source or action manifest as the composition target")
    );
}

#[test]
fn action_file_surface_defaults_effects_to_empty_writes_and_no_intents() {
    let parsed = parse_manifest_text(
        "memory/action.yaml",
        r#"
kind: action
id: demo_action
version: "0.1.0"
inputs:
  - name: gate
    type: event
    required: true
    cardinality: single
outputs:
  - name: outcome
    type: event
parameters: []
execution:
  deterministic: true
  retryable: false
state:
  allowed: false
side_effects: true
"#,
    )
    .expect("action manifest should parse");

    match parsed {
        ParsedManifest::Action { manifest, .. } => {
            assert!(manifest.effects.writes.is_empty());
            assert!(manifest.effects.intents.is_empty());
        }
        _ => panic!("expected action manifest"),
    }
}

#[test]
fn parse_value_type_accepts_boolean_alias_as_bool() {
    assert_eq!(common::parse_value_type("boolean"), Some(ValueType::Bool));
}

#[test]
fn compute_manifest_int_alias_normalizes_default_to_number_value() {
    let parsed = parse_manifest_text(
        "memory/compute.yaml",
        r#"
kind: compute
id: demo_compute
version: "0.1.0"
inputs:
  - name: in
    type: number
    required: true
outputs:
  - name: out
    type: number
parameters:
  - name: threshold
    type: int
    default: 3
    required: false
execution:
  deterministic: true
  cadence: continuous
  may_error: false
errors:
  allowed: false
  types: []
  deterministic: true
state:
  allowed: false
  resettable: true
side_effects: false
"#,
    )
    .expect("compute manifest should parse");

    match parsed {
        ParsedManifest::Compute { manifest, .. } => {
            assert_eq!(manifest.parameters.len(), 1);
            let parameter = &manifest.parameters[0];
            assert_eq!(parameter.value_type, ValueType::Number);
            assert_eq!(parameter.default, Some(Value::Number(3.0)));
        }
        _ => panic!("expected compute manifest"),
    }
}
