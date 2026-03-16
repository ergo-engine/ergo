use ergo_adapter::composition::{
    validate_action_adapter_composition, validate_capture_format,
    validate_source_adapter_composition, ContextRequirement, SourceRequires,
};
use ergo_adapter::provides::{AdapterProvides, ContextKeyProvision};
use ergo_runtime::action::{ActionEffects, ActionWriteSpec};
use ergo_runtime::cluster::ParameterValue;
use ergo_runtime::common::{ErrorInfo, ValueType};
use std::collections::{HashMap, HashSet};

fn make_adapter_provides(keys: Vec<(&str, &str)>) -> AdapterProvides {
    let context = keys
        .into_iter()
        .map(|(name, ty)| {
            (
                name.to_string(),
                ContextKeyProvision {
                    ty: ty.to_string(),
                    required: true,
                    writable: false,
                },
            )
        })
        .collect::<HashMap<_, _>>();

    AdapterProvides {
        context,
        events: HashSet::new(),
        effects: HashSet::new(),
        event_schemas: HashMap::new(),
        capture_format_version: "1".to_string(),
        adapter_fingerprint: "adapter:test@1.0.0;sha256:test".to_string(),
    }
}

fn make_adapter_provides_with_effects(
    keys: Vec<(&str, &str, bool)>,
    effects: Vec<&str>,
) -> AdapterProvides {
    let context = keys
        .into_iter()
        .map(|(name, ty, writable)| {
            (
                name.to_string(),
                ContextKeyProvision {
                    ty: ty.to_string(),
                    required: true,
                    writable,
                },
            )
        })
        .collect::<HashMap<_, _>>();

    let effects = effects.into_iter().map(|e| e.to_string()).collect();

    AdapterProvides {
        context,
        events: HashSet::new(),
        effects,
        event_schemas: HashMap::new(),
        capture_format_version: "1".to_string(),
        adapter_fingerprint: "adapter:test@1.0.0;sha256:test".to_string(),
    }
}

fn make_source_requires(keys: Vec<(&str, ValueType, bool)>) -> SourceRequires {
    SourceRequires {
        context: keys
            .into_iter()
            .map(|(name, ty, required)| ContextRequirement {
                name: name.to_string(),
                ty,
                required,
            })
            .collect(),
    }
}

fn no_params() -> HashMap<String, ParameterValue> {
    HashMap::new()
}

fn assert_comp(err: &dyn ErrorInfo, rule: &str, path: Option<&str>) {
    assert_eq!(err.rule_id(), rule);
    assert_eq!(err.path().as_deref(), path);
}

#[test]
fn comp_1_missing_context_key_rejected() {
    let adapter = make_adapter_provides(vec![("price", "Number")]);
    let source = make_source_requires(vec![("volume", ValueType::Number, true)]);

    let err = validate_source_adapter_composition(&source, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-1", Some("$.requires.context[0].name"));
}

#[test]
fn comp_2_context_type_mismatch_rejected() {
    let adapter = make_adapter_provides(vec![("price", "Number")]);
    let source = make_source_requires(vec![("price", ValueType::String, true)]);

    let err = validate_source_adapter_composition(&source, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-2", Some("$.requires.context[0].type"));
}

#[test]
fn comp_3_unsupported_capture_format_rejected() {
    let err = validate_capture_format("2").unwrap_err();
    assert_comp(&err, "COMP-3", Some("$.capture.format_version"));
}

/// SRC-10: Required context keys exist in adapter.
/// Alias for COMP-1 — same predicate, same enforcement, source-contract traceability.
#[test]
fn src_10_missing_context_key_rejected() {
    let adapter = make_adapter_provides(vec![("price", "Number")]);
    let source = make_source_requires(vec![("volume", ValueType::Number, true)]);

    let err = validate_source_adapter_composition(&source, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-1", Some("$.requires.context[0].name"));
}

/// SRC-11: Required context types match adapter.
/// Alias for COMP-2 — same predicate, same enforcement, source-contract traceability.
#[test]
fn src_11_context_type_mismatch_rejected() {
    let adapter = make_adapter_provides(vec![("price", "Number")]);
    let source = make_source_requires(vec![("price", ValueType::String, true)]);

    let err = validate_source_adapter_composition(&source, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-2", Some("$.requires.context[0].type"));
}

#[test]
fn comp_11_write_target_not_provided_rejected() {
    let adapter = make_adapter_provides_with_effects(vec![], vec!["set_context"]);
    let effects = ActionEffects {
        writes: vec![ActionWriteSpec {
            name: "price".to_string(),
            value_type: ValueType::Number,
            from_input: String::new(),
        }],
        intents: vec![],
    };

    let err = validate_action_adapter_composition(&effects, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-11", Some("$.effects.writes[0].name"));
}

#[test]
fn comp_12_write_target_not_writable_rejected() {
    let adapter =
        make_adapter_provides_with_effects(vec![("price", "Number", false)], vec!["set_context"]);
    let effects = ActionEffects {
        writes: vec![ActionWriteSpec {
            name: "price".to_string(),
            value_type: ValueType::Number,
            from_input: String::new(),
        }],
        intents: vec![],
    };

    let err = validate_action_adapter_composition(&effects, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-12", Some("$.effects.writes[0].name"));
}

#[test]
fn comp_13_write_type_mismatch_rejected() {
    let adapter =
        make_adapter_provides_with_effects(vec![("price", "Number", true)], vec!["set_context"]);
    let effects = ActionEffects {
        writes: vec![ActionWriteSpec {
            name: "price".to_string(),
            value_type: ValueType::Bool,
            from_input: String::new(),
        }],
        intents: vec![],
    };

    let err = validate_action_adapter_composition(&effects, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-13", Some("$.effects.writes[0].type"));
}

#[test]
fn comp_14_missing_set_context_rejected() {
    let adapter = make_adapter_provides_with_effects(vec![("price", "Number", true)], vec![]);
    let effects = ActionEffects {
        writes: vec![ActionWriteSpec {
            name: "price".to_string(),
            value_type: ValueType::Number,
            from_input: String::new(),
        }],
        intents: vec![],
    };

    let err = validate_action_adapter_composition(&effects, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-14", Some("$.effects.writes"));
}

// --- $key resolution tests for source composition ---

#[test]
fn comp_source_dollar_key_resolves_to_parameter_value() {
    let adapter = make_adapter_provides(vec![("sample_key", "Number")]);
    let source = make_source_requires(vec![("$key", ValueType::Number, true)]);
    let params = HashMap::from([(
        "key".to_string(),
        ParameterValue::String("sample_key".to_string()),
    )]);

    assert!(validate_source_adapter_composition(&source, &adapter, &params).is_ok());
}

#[test]
fn comp_source_dollar_key_missing_adapter_provision_rejected() {
    let adapter = make_adapter_provides(vec![("price", "Number")]);
    let source = make_source_requires(vec![("$key", ValueType::Number, true)]);
    let params = HashMap::from([(
        "key".to_string(),
        ParameterValue::String("sample_key".to_string()),
    )]);

    let err = validate_source_adapter_composition(&source, &adapter, &params).unwrap_err();
    assert_comp(&err, "COMP-1", Some("$.requires.context[0].name"));
}

#[test]
fn comp_source_dollar_key_type_mismatch_rejected() {
    let adapter = make_adapter_provides(vec![("sample_key", "String")]);
    let source = make_source_requires(vec![("$key", ValueType::Number, true)]);
    let params = HashMap::from([(
        "key".to_string(),
        ParameterValue::String("sample_key".to_string()),
    )]);

    let err = validate_source_adapter_composition(&source, &adapter, &params).unwrap_err();
    assert_comp(&err, "COMP-2", Some("$.requires.context[0].type"));
}

#[test]
fn comp_source_dollar_key_missing_parameter_rejected() {
    let adapter = make_adapter_provides(vec![("sample_key", "Number")]);
    let source = make_source_requires(vec![("$key", ValueType::Number, true)]);

    let err = validate_source_adapter_composition(&source, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-16", Some("$.requires.context[0].name"));
}

/// Optional $key with missing parameter still fails resolution (COMP-16).
/// Guards the optional early-return regression: resolution must run before
/// the `required: false` skip.
#[test]
fn comp_source_optional_dollar_key_missing_parameter_rejected() {
    let adapter = make_adapter_provides(vec![("sample_key", "Number")]);
    let source = make_source_requires(vec![("$key", ValueType::Number, false)]);

    let err = validate_source_adapter_composition(&source, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-16", Some("$.requires.context[0].name"));
}

/// Optional $key with non-String parameter still fails resolution (COMP-16).
/// Guards the same optional early-return regression.
#[test]
fn comp_source_optional_dollar_key_non_string_param_rejected() {
    let adapter = make_adapter_provides(vec![("sample_key", "Number")]);
    let source = make_source_requires(vec![("$key", ValueType::Number, false)]);
    let params = HashMap::from([("key".to_string(), ParameterValue::Number(42.0))]);

    let err = validate_source_adapter_composition(&source, &adapter, &params).unwrap_err();
    assert_comp(&err, "COMP-16", Some("$.requires.context[0].name"));
}

#[test]
fn comp_source_optional_context_missing_key_allowed() {
    let adapter = make_adapter_provides(vec![("price", "Number")]);
    let source = make_source_requires(vec![("fast_ema_prev", ValueType::Number, false)]);

    assert!(validate_source_adapter_composition(&source, &adapter, &no_params()).is_ok());
}

#[test]
fn comp_source_optional_context_type_mismatch_rejected() {
    let adapter = make_adapter_provides(vec![("fast_ema_prev", "String")]);
    let source = make_source_requires(vec![("fast_ema_prev", ValueType::Number, false)]);

    let err = validate_source_adapter_composition(&source, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-2", Some("$.requires.context[0].type"));
}

#[test]
fn comp_source_optional_context_unknown_adapter_type_rejected() {
    let adapter = make_adapter_provides(vec![("fast_ema_prev", "Object")]);
    let source = make_source_requires(vec![("fast_ema_prev", ValueType::Number, false)]);

    let err = validate_source_adapter_composition(&source, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-2", Some("$.requires.context[0].type"));
}

#[test]
fn comp_source_optional_dollar_key_type_mismatch_rejected() {
    let adapter = make_adapter_provides(vec![("fast_ema_prev", "String")]);
    let source = make_source_requires(vec![("$key", ValueType::Number, false)]);
    let params = HashMap::from([(
        "key".to_string(),
        ParameterValue::String("fast_ema_prev".to_string()),
    )]);

    let err = validate_source_adapter_composition(&source, &adapter, &params).unwrap_err();
    assert_comp(&err, "COMP-2", Some("$.requires.context[0].type"));
}

// --- $key resolution tests for action composition ---

#[test]
fn comp_action_dollar_key_resolves_to_parameter_value() {
    let adapter = make_adapter_provides_with_effects(
        vec![("sample_key", "Number", true)],
        vec!["set_context"],
    );
    let effects = ActionEffects {
        writes: vec![ActionWriteSpec {
            name: "$key".to_string(),
            value_type: ValueType::Number,
            from_input: String::new(),
        }],
        intents: vec![],
    };
    let params = HashMap::from([(
        "key".to_string(),
        ParameterValue::String("sample_key".to_string()),
    )]);

    assert!(validate_action_adapter_composition(&effects, &adapter, &params).is_ok());
}

#[test]
fn comp_action_dollar_key_missing_provision_rejected() {
    let adapter =
        make_adapter_provides_with_effects(vec![("price", "Number", true)], vec!["set_context"]);
    let effects = ActionEffects {
        writes: vec![ActionWriteSpec {
            name: "$key".to_string(),
            value_type: ValueType::Number,
            from_input: String::new(),
        }],
        intents: vec![],
    };
    let params = HashMap::from([(
        "key".to_string(),
        ParameterValue::String("sample_key".to_string()),
    )]);

    let err = validate_action_adapter_composition(&effects, &adapter, &params).unwrap_err();
    assert_comp(&err, "COMP-11", Some("$.effects.writes[0].name"));
}

#[test]
fn comp_action_dollar_key_missing_parameter_rejected() {
    let adapter = make_adapter_provides_with_effects(
        vec![("sample_key", "Number", true)],
        vec!["set_context"],
    );
    let effects = ActionEffects {
        writes: vec![ActionWriteSpec {
            name: "$key".to_string(),
            value_type: ValueType::Number,
            from_input: String::new(),
        }],
        intents: vec![],
    };

    let err = validate_action_adapter_composition(&effects, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-16", Some("$.effects.writes[0].name"));
}

/// Action write $key with non-String parameter fails resolution (COMP-16).
/// Covers the WrongParameterType resolver branch for the action composition path.
#[test]
fn comp_action_dollar_key_non_string_param_rejected() {
    let adapter = make_adapter_provides_with_effects(
        vec![("sample_key", "Number", true)],
        vec!["set_context"],
    );
    let effects = ActionEffects {
        writes: vec![ActionWriteSpec {
            name: "$key".to_string(),
            value_type: ValueType::Number,
            from_input: String::new(),
        }],
        intents: vec![],
    };
    let params = HashMap::from([("key".to_string(), ParameterValue::Number(42.0))]);

    let err = validate_action_adapter_composition(&effects, &adapter, &params).unwrap_err();
    assert_comp(&err, "COMP-16", Some("$.effects.writes[0].name"));
}
