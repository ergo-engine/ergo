use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_temp_file(name: &str, contents: &str) -> std::path::PathBuf {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "ergo-phase7-cli-test-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join(name);
    fs::write(&path, contents).expect("write temp manifest");
    path
}

fn validate_json_error(manifest_name: &str, manifest: &str) -> serde_json::Value {
    let path = write_temp_file(manifest_name, manifest);
    let err = ergo_cli::validate::validate_command(&[
        "--format".to_string(),
        "json".to_string(),
        path.to_string_lossy().to_string(),
    ])
    .expect_err("expected failure");
    serde_json::from_str(&err).expect("error output should be json in --format json mode")
}

#[test]
fn validate_compute_success_text() {
    let manifest = r#"
kind: compute
id: my_compute
version: 0.1.0

inputs:
  - name: a
    type: number
    required: true
    cardinality: single

outputs:
  - name: out
    type: number

parameters: []

execution:
  cadence: continuous
  deterministic: true
  may_error: true

errors:
  allowed: false
  types: []
  deterministic: true

state:
  allowed: false
  resettable: true

side_effects: false
"#;

    let path = write_temp_file("compute.yaml", manifest);
    let out = ergo_cli::validate::validate_command(&[path.to_string_lossy().to_string()])
        .expect("expected success");
    assert!(out.contains("Manifest valid"), "out: {out}");
    assert!(out.contains("Kind: compute"), "out: {out}");
}

#[test]
fn validate_compute_failure_json_includes_rule_id() {
    let manifest = r#"
kind: compute
id: bad_compute
version: 0.1.0

inputs: []   # invalid: must have at least one input

outputs:
  - name: out
    type: number

parameters: []

execution:
  cadence: continuous
  deterministic: true
  may_error: true

errors:
  allowed: false
  types: []
  deterministic: true

state:
  allowed: false
  resettable: true

side_effects: false
"#;

    let path = write_temp_file("bad-compute.yaml", manifest);
    let err = ergo_cli::validate::validate_command(&[
        "--format".to_string(),
        "json".to_string(),
        path.to_string_lossy().to_string(),
    ])
    .expect_err("expected failure");
    let parsed: serde_json::Value =
        serde_json::from_str(&err).expect("error output should be json in --format json mode");
    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["errors"][0]["rule_id"], "CMP-4");
}

#[test]
fn check_compose_source_adapter_failure_text_shows_comp_rule() {
    let adapter = r#"
kind: adapter
id: test_adapter
version: 0.1.0
runtime_compatibility: 0.1.0

context_keys:
  - name: y
    type: Number
    required: false
    writable: false

event_kinds: []

accepts:
  effects: []

capture:
  format_version: "1"
  fields: ["meta.adapter_id"]
"#;

    let source = r#"
kind: source
id: needs_x
version: 0.1.0

inputs: []
outputs:
  - name: out
    type: number

parameters: []

requires:
  context:
    - name: x
      type: Number
      required: true

execution:
  cadence: continuous
  deterministic: true

state:
  allowed: false

side_effects: false
"#;

    let adapter_path = write_temp_file("adapter.yaml", adapter);
    let source_path = write_temp_file("source.yaml", source);
    let err = ergo_cli::validate::check_compose_command(&[
        adapter_path.to_string_lossy().to_string(),
        source_path.to_string_lossy().to_string(),
    ])
    .expect_err("expected failure");

    assert!(err.contains("Composition invalid"), "err: {err}");
    assert!(err.contains("COMP-1"), "err: {err}");
}

#[test]
fn validate_adapter_success_text() {
    let adapter = r#"
kind: adapter
id: test_adapter
version: 0.1.0
runtime_compatibility: 0.1.0

context_keys:
  - name: price
    type: Number
    required: false
    writable: false

event_kinds: []

capture:
  format_version: "1"
  fields: ["meta.adapter_id"]
"#;

    let path = write_temp_file("adapter-valid.yaml", adapter);
    let out = ergo_cli::validate::validate_command(&[path.to_string_lossy().to_string()])
        .expect("expected success");
    assert!(out.contains("Kind: adapter"), "out: {out}");
}

#[test]
fn validate_trigger_success_text() {
    let trigger = r#"
kind: trigger
id: test_trigger
version: 0.1.0

inputs:
  - name: gate
    type: event
    required: true
    cardinality: single

outputs:
  - name: emitted
    type: event

parameters: []

execution:
  cadence: continuous
  deterministic: true

state:
  allowed: false

side_effects: false
"#;

    let path = write_temp_file("trigger-valid.yaml", trigger);
    let out = ergo_cli::validate::validate_command(&[path.to_string_lossy().to_string()])
        .expect("expected success");
    assert!(out.contains("Kind: trigger"), "out: {out}");
}

#[test]
fn validate_action_success_text() {
    let action = r#"
kind: action
id: test_action
version: 0.1.0

inputs:
  - name: gate
    type: event
    required: true
    cardinality: single

outputs:
  - name: outcome
    type: event

parameters: []

effects:
  writes: []

execution:
  deterministic: true
  retryable: false

state:
  allowed: false

side_effects: true
"#;

    let path = write_temp_file("action-valid.yaml", action);
    let out = ergo_cli::validate::validate_command(&[path.to_string_lossy().to_string()])
        .expect("expected success");
    assert!(out.contains("Kind: action"), "out: {out}");
}

#[test]
fn check_compose_action_adapter_failure_json_includes_rule_id() {
    let adapter = r#"
kind: adapter
id: test_adapter
version: 0.1.0
runtime_compatibility: 0.1.0

context_keys:
  - name: x
    type: Number
    required: false
    writable: false

event_kinds: []

accepts:
  effects: []

capture:
  format_version: "1"
  fields: ["meta.adapter_id"]
"#;

    let action = r#"
kind: action
id: write_x
version: 0.1.0

inputs:
  - name: gate
    type: event
    required: true
    cardinality: single

outputs:
  - name: outcome
    type: event

parameters: []

effects:
  writes:
    - name: x
      type: Number
      from_input: gate

execution:
  deterministic: true
  retryable: false

state:
  allowed: false

side_effects: true
"#;

    let adapter_path = write_temp_file("adapter-action.yaml", adapter);
    let action_path = write_temp_file("action-write.yaml", action);
    let err = ergo_cli::validate::check_compose_command(&[
        "--format".to_string(),
        "json".to_string(),
        adapter_path.to_string_lossy().to_string(),
        action_path.to_string_lossy().to_string(),
    ])
    .expect_err("expected failure");

    let parsed: serde_json::Value =
        serde_json::from_str(&err).expect("error output should be json in --format json mode");
    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["errors"][0]["rule_id"], "COMP-12");
}

#[test]
fn check_compose_source_dollar_key_uses_manifest_default() {
    let adapter = r#"
kind: adapter
id: test_adapter
version: 0.1.0
runtime_compatibility: 0.1.0

context_keys:
  - name: price
    type: Number
    required: false
    writable: false

event_kinds: []

accepts:
  effects: []

capture:
  format_version: "1"
  fields: ["meta.adapter_id"]
"#;

    let source = r#"
kind: source
id: needs_price_by_param
version: 0.1.0

inputs: []
outputs:
  - name: out
    type: number

parameters:
  - name: key
    type: string
    default: price

requires:
  context:
    - name: "$key"
      type: Number
      required: true

execution:
  cadence: continuous
  deterministic: true

state:
  allowed: false

side_effects: false
"#;

    let adapter_path = write_temp_file("adapter-dollar-source-success.yaml", adapter);
    let source_path = write_temp_file("source-dollar-default-success.yaml", source);
    let out = ergo_cli::validate::check_compose_command(&[
        adapter_path.to_string_lossy().to_string(),
        source_path.to_string_lossy().to_string(),
    ])
    .expect("expected compose success");

    assert!(out.contains("Composition valid"), "out: {out}");
}

#[test]
fn check_compose_action_dollar_key_uses_manifest_default() {
    let adapter = r#"
kind: adapter
id: test_adapter
version: 0.1.0
runtime_compatibility: 0.1.0

context_keys:
  - name: target_price
    type: Number
    required: false
    writable: true

event_kinds: []

accepts:
  effects:
    - name: set_context
      payload_schema: {}

capture:
  format_version: "1"
  fields: ["meta.adapter_id"]
"#;

    let action = r#"
kind: action
id: write_target_price
version: 0.1.0

inputs:
  - name: gate
    type: event
    required: true
    cardinality: single
  - name: value
    type: number
    required: true
    cardinality: single

outputs:
  - name: outcome
    type: event

parameters:
  - name: key
    type: string
    default: target_price
    required: false

effects:
  writes:
    - name: "$key"
      type: Number
      from_input: value

execution:
  deterministic: true
  retryable: false

state:
  allowed: false

side_effects: true
"#;

    let adapter_path = write_temp_file("adapter-dollar-action-success.yaml", adapter);
    let action_path = write_temp_file("action-dollar-default-success.yaml", action);
    let out = ergo_cli::validate::check_compose_command(&[
        adapter_path.to_string_lossy().to_string(),
        action_path.to_string_lossy().to_string(),
    ])
    .expect("expected compose success");

    assert!(out.contains("Composition valid"), "out: {out}");
}

#[test]
fn check_compose_dollar_key_without_default_still_fails_comp16() {
    let adapter = r#"
kind: adapter
id: test_adapter
version: 0.1.0
runtime_compatibility: 0.1.0

context_keys:
  - name: price
    type: Number
    required: false
    writable: false

event_kinds: []

accepts:
  effects: []

capture:
  format_version: "1"
  fields: ["meta.adapter_id"]
"#;

    let source = r#"
kind: source
id: missing_default_key
version: 0.1.0

inputs: []
outputs:
  - name: out
    type: number

parameters:
  - name: key
    type: string

requires:
  context:
    - name: "$key"
      type: Number
      required: true

execution:
  cadence: continuous
  deterministic: true

state:
  allowed: false

side_effects: false
"#;

    let adapter_path = write_temp_file("adapter-dollar-source-missing-default.yaml", adapter);
    let source_path = write_temp_file("source-dollar-missing-default.yaml", source);
    let err = ergo_cli::validate::check_compose_command(&[
        adapter_path.to_string_lossy().to_string(),
        source_path.to_string_lossy().to_string(),
    ])
    .expect_err("expected compose failure");

    assert!(err.contains("COMP-16"), "err: {err}");
}

#[test]
fn check_compose_source_dollar_key_non_string_default_still_fails_comp16() {
    let adapter = r#"
kind: adapter
id: test_adapter
version: 0.1.0
runtime_compatibility: 0.1.0

context_keys:
  - name: price
    type: Number
    required: false
    writable: false

event_kinds: []

accepts:
  effects: []

capture:
  format_version: "1"
  fields: ["meta.adapter_id"]
"#;

    let source = r#"
kind: source
id: wrong_type_default_key
version: 0.1.0

inputs: []
outputs:
  - name: out
    type: number

parameters:
  - name: key
    type: number
    default: 1.5

requires:
  context:
    - name: "$key"
      type: Number
      required: true

execution:
  cadence: continuous
  deterministic: true

state:
  allowed: false

side_effects: false
"#;

    let adapter_path = write_temp_file("adapter-dollar-source-wrong-type-default.yaml", adapter);
    let source_path = write_temp_file("source-dollar-wrong-type-default.yaml", source);
    let err = ergo_cli::validate::check_compose_command(&[
        adapter_path.to_string_lossy().to_string(),
        source_path.to_string_lossy().to_string(),
    ])
    .expect_err("expected compose failure");

    assert!(err.contains("COMP-16"), "err: {err}");
}

/// Action write missing from_input must fail YAML parse (serde missing field).
#[test]
fn validate_action_write_missing_from_input_rejected() {
    let manifest = r#"
kind: action
id: missing_from_input
version: 0.1.0

inputs:
  - name: gate
    type: event
    required: true
    cardinality: single

outputs:
  - name: outcome
    type: event

parameters: []

effects:
  writes:
    - name: x
      type: Number

execution:
  deterministic: true
  retryable: false

state:
  allowed: false

side_effects: true
"#;

    let path = write_temp_file("action-no-from-input.yaml", manifest);
    let result = ergo_cli::validate::validate_command(&[path.to_string_lossy().to_string()]);
    assert!(
        result.is_err(),
        "action write without from_input must fail validation"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("from_input"),
        "error should mention missing from_input field: {err}"
    );
}

#[test]
fn validate_compute_parse_default_type_mismatch_maps_cmp_19() {
    let manifest = r#"
kind: compute
id: bad_default_compute
version: 0.1.0

inputs:
  - name: a
    type: number
    required: true
    cardinality: single

outputs:
  - name: out
    type: number

parameters:
  - name: threshold
    type: number
    default: true
    required: false

execution:
  cadence: continuous
  deterministic: true
  may_error: false

errors:
  allowed: false
  types: []
  deterministic: true

state:
  allowed: false
  resettable: true

side_effects: false
"#;

    let parsed = validate_json_error("compute-bad-default.yaml", manifest);
    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["errors"][0]["rule_id"], "CMP-19");
    assert_eq!(parsed["errors"][0]["path"], "$.parameters[].default");
}

#[test]
fn validate_compute_parse_default_type_match_accepted() {
    let manifest = r#"
kind: compute
id: good_default_compute
version: 0.1.0

inputs:
  - name: a
    type: number
    required: true
    cardinality: single

outputs:
  - name: out
    type: number

parameters:
  - name: threshold
    type: number
    default: 42.5
    required: false

execution:
  cadence: continuous
  deterministic: true
  may_error: false

errors:
  allowed: false
  types: []
  deterministic: true

state:
  allowed: false
  resettable: true

side_effects: false
"#;

    let path = write_temp_file("compute-good-default.yaml", manifest);
    let out = ergo_cli::validate::validate_command(&[path.to_string_lossy().to_string()])
        .expect("expected success");
    assert!(out.contains("Manifest valid"), "out: {out}");
    assert!(out.contains("Kind: compute"), "out: {out}");
}

#[test]
fn validate_source_parse_default_type_mismatch_maps_src_15() {
    let manifest = r#"
kind: source
id: bad_default_source
version: 0.1.0

inputs: []
outputs:
  - name: out
    type: number

parameters:
  - name: threshold
    type: number
    default: "not-a-number"

requires:
  context: []

execution:
  cadence: continuous
  deterministic: true

state:
  allowed: false

side_effects: false
"#;

    let parsed = validate_json_error("source-bad-default.yaml", manifest);
    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["errors"][0]["rule_id"], "SRC-15");
    assert_eq!(parsed["errors"][0]["path"], "$.parameters[].default");
}

#[test]
fn validate_source_parse_default_type_match_accepted() {
    let manifest = r#"
kind: source
id: good_default_source
version: 0.1.0

inputs: []
outputs:
  - name: out
    type: number

parameters:
  - name: threshold
    type: number
    default: 21

requires:
  context: []

execution:
  cadence: continuous
  deterministic: true

state:
  allowed: false

side_effects: false
"#;

    let path = write_temp_file("source-good-default.yaml", manifest);
    let out = ergo_cli::validate::validate_command(&[path.to_string_lossy().to_string()])
        .expect("expected success");
    assert!(out.contains("Manifest valid"), "out: {out}");
    assert!(out.contains("Kind: source"), "out: {out}");
}

#[test]
fn validate_trigger_parse_default_type_mismatch_maps_trg_14() {
    let manifest = r#"
kind: trigger
id: bad_default_trigger
version: 0.1.0

inputs:
  - name: gate
    type: event
    required: true
    cardinality: single

outputs:
  - name: emitted
    type: event

parameters:
  - name: threshold
    type: number
    default: "not-a-number"
    required: false

execution:
  cadence: continuous
  deterministic: true

state:
  allowed: false

side_effects: false
"#;

    let parsed = validate_json_error("trigger-bad-default.yaml", manifest);
    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["errors"][0]["rule_id"], "TRG-14");
    assert_eq!(parsed["errors"][0]["path"], "$.parameters[].default");
}

#[test]
fn validate_trigger_parse_default_type_match_accepted() {
    let manifest = r#"
kind: trigger
id: good_default_trigger
version: 0.1.0

inputs:
  - name: gate
    type: event
    required: true
    cardinality: single

outputs:
  - name: emitted
    type: event

parameters:
  - name: threshold
    type: number
    default: 10
    required: false

execution:
  cadence: continuous
  deterministic: true

state:
  allowed: false

side_effects: false
"#;

    let path = write_temp_file("trigger-good-default.yaml", manifest);
    let out = ergo_cli::validate::validate_command(&[path.to_string_lossy().to_string()])
        .expect("expected success");
    assert!(out.contains("Manifest valid"), "out: {out}");
    assert!(out.contains("Kind: trigger"), "out: {out}");
}

#[test]
fn validate_action_parse_default_type_mismatch_maps_act_19() {
    let manifest = r#"
kind: action
id: bad_default_action
version: 0.1.0

inputs:
  - name: gate
    type: event
    required: true
    cardinality: single

outputs:
  - name: outcome
    type: event

parameters:
  - name: threshold
    type: number
    default: "not-a-number"
    required: false

effects:
  writes: []

execution:
  deterministic: true
  retryable: false

state:
  allowed: false

side_effects: true
"#;

    let parsed = validate_json_error("action-bad-default.yaml", manifest);
    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["errors"][0]["rule_id"], "ACT-19");
    assert_eq!(parsed["errors"][0]["path"], "$.parameters[].default");
}

#[test]
fn validate_action_parse_default_type_match_accepted() {
    let manifest = r#"
kind: action
id: good_default_action
version: 0.1.0

inputs:
  - name: gate
    type: event
    required: true
    cardinality: single

outputs:
  - name: outcome
    type: event

parameters:
  - name: threshold
    type: number
    default: 5
    required: false

effects:
  writes: []

execution:
  deterministic: true
  retryable: false

state:
  allowed: false

side_effects: true
"#;

    let path = write_temp_file("action-good-default.yaml", manifest);
    let out = ergo_cli::validate::validate_command(&[path.to_string_lossy().to_string()])
        .expect("expected success");
    assert!(out.contains("Manifest valid"), "out: {out}");
    assert!(out.contains("Kind: action"), "out: {out}");
}

#[test]
fn gen_docs_check_passes() {
    let out = ergo_cli::gen_docs::gen_docs_command(&["--check".to_string()])
        .expect("expected docs to be up-to-date");
    assert!(out.contains("Docs up-to-date"), "out: {out}");
}
