use ergo_adapter::composition::{
    validate_action_adapter_composition, validate_capture_format,
    validate_source_adapter_composition, CompositionError, ContextRequirement, SourceRequires,
};
use ergo_adapter::provides::{AdapterProvides, ContextKeyProvision};
use ergo_runtime::action::{
    ActionEffects, ActionWriteSpec, IntentFieldSpec, IntentMirrorWriteSpec, IntentSpec,
};
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
        effect_schemas: HashMap::new(),
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
        effect_schemas: HashMap::new(),
        event_schemas: HashMap::new(),
        capture_format_version: "1".to_string(),
        adapter_fingerprint: "adapter:test@1.0.0;sha256:test".to_string(),
    }
}

fn make_adapter_provides_with_effect_schemas(
    keys: Vec<(&str, &str, bool)>,
    effects: Vec<&str>,
    effect_schemas: Vec<(&str, serde_json::Value)>,
) -> AdapterProvides {
    let mut provides = make_adapter_provides_with_effects(keys, effects);
    provides.effect_schemas = effect_schemas
        .into_iter()
        .map(|(name, schema)| (name.to_string(), schema))
        .collect();
    provides
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

#[test]
fn comp_17_missing_intent_effect_rejected() {
    let adapter = make_adapter_provides_with_effects(vec![], vec![]);
    let effects = ActionEffects {
        writes: vec![],
        intents: vec![IntentSpec {
            name: "place_order".to_string(),
            fields: vec![],
            mirror_writes: vec![],
        }],
    };

    let err = validate_action_adapter_composition(&effects, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-17", Some("$.effects.intents[0].name"));
}

#[test]
fn comp_14_mirror_writes_require_set_context_acceptance() {
    let adapter = make_adapter_provides_with_effects(
        vec![("order_symbol", "String", true)],
        vec!["place_order"],
    );
    let effects = ActionEffects {
        writes: vec![],
        intents: vec![IntentSpec {
            name: "place_order".to_string(),
            fields: vec![IntentFieldSpec {
                name: "symbol".to_string(),
                value_type: ValueType::String,
                from_input: Some("symbol".to_string()),
                from_param: None,
            }],
            mirror_writes: vec![IntentMirrorWriteSpec {
                name: "order_symbol".to_string(),
                value_type: ValueType::String,
                from_field: "symbol".to_string(),
            }],
        }],
    };

    let err = validate_action_adapter_composition(&effects, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-14", Some("$.effects.writes"));
}

#[test]
fn comp_18_missing_payload_schema_rejected() {
    let adapter = make_adapter_provides_with_effects(vec![], vec!["place_order"]);
    let effects = ActionEffects {
        writes: vec![],
        intents: vec![IntentSpec {
            name: "place_order".to_string(),
            fields: vec![IntentFieldSpec {
                name: "symbol".to_string(),
                value_type: ValueType::String,
                from_input: Some("symbol".to_string()),
                from_param: None,
            }],
            mirror_writes: vec![],
        }],
    };

    let err = validate_action_adapter_composition(&effects, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-18", Some("$.effects.intents[0].fields"));
}

#[test]
fn comp_18_and_comp_19_doc_anchors_point_to_action_manifest() {
    let missing = CompositionError::MissingIntentPayloadSchema {
        kind: "place_order".to_string(),
        index: 0,
    };
    assert_eq!(
        missing.doc_anchor(),
        "STABLE/PRIMITIVE_MANIFESTS/action.md#COMP-18"
    );

    let incompatible = CompositionError::IntentPayloadSchemaIncompatible {
        kind: "place_order".to_string(),
        index: 0,
        detail: "mismatch".to_string(),
    };
    assert_eq!(
        incompatible.doc_anchor(),
        "STABLE/PRIMITIVE_MANIFESTS/action.md#COMP-19"
    );
}

#[test]
fn comp_intent_schema_compatibility_valid_pair_passes() {
    let adapter = make_adapter_provides_with_effect_schemas(
        vec![],
        vec!["place_order"],
        vec![(
            "place_order",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string"},
                    "qty": {"type": "number"}
                },
                "required": ["symbol", "qty"],
                "additionalProperties": false
            }),
        )],
    );
    let effects = ActionEffects {
        writes: vec![],
        intents: vec![IntentSpec {
            name: "place_order".to_string(),
            fields: vec![
                IntentFieldSpec {
                    name: "symbol".to_string(),
                    value_type: ValueType::String,
                    from_input: Some("symbol".to_string()),
                    from_param: None,
                },
                IntentFieldSpec {
                    name: "qty".to_string(),
                    value_type: ValueType::Number,
                    from_input: Some("qty".to_string()),
                    from_param: None,
                },
            ],
            mirror_writes: vec![],
        }],
    };

    assert!(validate_action_adapter_composition(&effects, &adapter, &no_params()).is_ok());
}

#[test]
fn comp_19_required_field_mismatch_rejected() {
    let adapter = make_adapter_provides_with_effect_schemas(
        vec![],
        vec!["place_order"],
        vec![(
            "place_order",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string"},
                    "qty": {"type": "number"}
                },
                "required": ["symbol", "qty"],
                "additionalProperties": false
            }),
        )],
    );
    let effects = ActionEffects {
        writes: vec![],
        intents: vec![IntentSpec {
            name: "place_order".to_string(),
            fields: vec![IntentFieldSpec {
                name: "symbol".to_string(),
                value_type: ValueType::String,
                from_input: Some("symbol".to_string()),
                from_param: None,
            }],
            mirror_writes: vec![],
        }],
    };

    let err = validate_action_adapter_composition(&effects, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-19", Some("$.effects.intents[0].fields"));
}

#[test]
fn comp_19_field_type_mismatch_rejected() {
    let adapter = make_adapter_provides_with_effect_schemas(
        vec![],
        vec!["place_order"],
        vec![(
            "place_order",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "number"}
                },
                "required": ["symbol"],
                "additionalProperties": false
            }),
        )],
    );
    let effects = ActionEffects {
        writes: vec![],
        intents: vec![IntentSpec {
            name: "place_order".to_string(),
            fields: vec![IntentFieldSpec {
                name: "symbol".to_string(),
                value_type: ValueType::String,
                from_input: Some("symbol".to_string()),
                from_param: None,
            }],
            mirror_writes: vec![],
        }],
    };

    let err = validate_action_adapter_composition(&effects, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-19", Some("$.effects.intents[0].fields"));
}

#[test]
fn comp_19_unsupported_schema_keyword_rejected_fail_closed() {
    let adapter = make_adapter_provides_with_effect_schemas(
        vec![],
        vec!["place_order"],
        vec![(
            "place_order",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string"}
                },
                "required": ["symbol"],
                "oneOf": [{"required": ["symbol"]}]
            }),
        )],
    );
    let effects = ActionEffects {
        writes: vec![],
        intents: vec![IntentSpec {
            name: "place_order".to_string(),
            fields: vec![IntentFieldSpec {
                name: "symbol".to_string(),
                value_type: ValueType::String,
                from_input: Some("symbol".to_string()),
                from_param: None,
            }],
            mirror_writes: vec![],
        }],
    };

    let err = validate_action_adapter_composition(&effects, &adapter, &no_params()).unwrap_err();
    assert_comp(&err, "COMP-19", Some("$.effects.intents[0].fields"));
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
