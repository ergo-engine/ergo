use std::collections::HashMap;

use crate::action::{
    implementations::{
        context_set_bool_manifest, context_set_number_manifest, context_set_series_manifest,
        context_set_string_manifest,
    },
    AckAction, ActionEffects, ActionKind, ActionOutcome, ActionPrimitive, ActionPrimitiveManifest,
    ActionRegistry, ActionValidationError, ActionValue, ActionValueType, AnnotateAction,
    Cardinality, ContextSetBoolAction, ContextSetNumberAction, ContextSetSeriesAction,
    ContextSetStringAction, ExecutionSpec, InputSpec, IntentFieldSpec, IntentMirrorWriteSpec,
    IntentSpec, OutputSpec, ParameterSpec, ParameterType, ParameterValue, StateSpec,
};
use crate::common::ValueType;

fn expect_panic<F: FnOnce() -> R + std::panic::UnwindSafe, R>(f: F) {
    assert!(std::panic::catch_unwind(f).is_err());
}

#[test]
fn ack_action_respects_accept_parameter() {
    let action = AckAction::new();
    let accepted = action.execute(
        &HashMap::from([(
            "event".to_string(),
            ActionValue::Event(ActionOutcome::Attempted),
        )]),
        &HashMap::from([("accept".to_string(), ParameterValue::Bool(true))]),
    );
    assert_eq!(
        accepted.get("outcome"),
        Some(&ActionValue::Event(ActionOutcome::Completed))
    );

    let rejected = action.execute(
        &HashMap::from([(
            "event".to_string(),
            ActionValue::Event(ActionOutcome::Attempted),
        )]),
        &HashMap::from([("accept".to_string(), ParameterValue::Bool(false))]),
    );
    assert_eq!(
        rejected.get("outcome"),
        Some(&ActionValue::Event(ActionOutcome::Rejected))
    );
}

#[test]
fn annotate_action_emits_attempted() {
    let action = AnnotateAction::new();
    let outputs = action.execute(
        &HashMap::from([(
            "event".to_string(),
            ActionValue::Event(ActionOutcome::Attempted),
        )]),
        &HashMap::from([(
            "note".to_string(),
            ParameterValue::String("hello".to_string()),
        )]),
    );
    assert_eq!(
        outputs.get("outcome"),
        Some(&ActionValue::Event(ActionOutcome::Attempted))
    );
}

#[test]
fn actions_require_event_input() {
    let action = AckAction::new();
    expect_panic(|| {
        action.execute(&HashMap::new(), &HashMap::new());
    });
}

#[test]
fn context_set_number_emits_attempted_outcome() {
    let action = ContextSetNumberAction::new();
    let outputs = action.execute(
        &HashMap::from([
            (
                "event".to_string(),
                ActionValue::Event(ActionOutcome::Attempted),
            ),
            ("value".to_string(), ActionValue::Number(42.0)),
        ]),
        &HashMap::from([(
            "key".to_string(),
            ParameterValue::String("fast_ema".to_string()),
        )]),
    );
    assert_eq!(
        outputs.get("outcome"),
        Some(&ActionValue::Event(ActionOutcome::Attempted))
    );
}

#[test]
fn context_set_bool_emits_attempted_outcome() {
    let action = ContextSetBoolAction::new();
    let outputs = action.execute(
        &HashMap::from([
            (
                "event".to_string(),
                ActionValue::Event(ActionOutcome::Attempted),
            ),
            ("value".to_string(), ActionValue::Bool(true)),
        ]),
        &HashMap::from([(
            "key".to_string(),
            ParameterValue::String("armed".to_string()),
        )]),
    );
    assert_eq!(
        outputs.get("outcome"),
        Some(&ActionValue::Event(ActionOutcome::Attempted))
    );
}

#[test]
fn context_set_string_emits_attempted_outcome() {
    let action = ContextSetStringAction::new();
    let outputs = action.execute(
        &HashMap::from([
            (
                "event".to_string(),
                ActionValue::Event(ActionOutcome::Attempted),
            ),
            (
                "value".to_string(),
                ActionValue::String("hello".to_string()),
            ),
        ]),
        &HashMap::from([(
            "key".to_string(),
            ParameterValue::String("note".to_string()),
        )]),
    );
    assert_eq!(
        outputs.get("outcome"),
        Some(&ActionValue::Event(ActionOutcome::Attempted))
    );
}

#[test]
fn context_set_series_emits_attempted_outcome() {
    let action = ContextSetSeriesAction::new();
    let outputs = action.execute(
        &HashMap::from([
            (
                "event".to_string(),
                ActionValue::Event(ActionOutcome::Attempted),
            ),
            (
                "value".to_string(),
                ActionValue::Series(vec![1.0, 2.0, 3.0]),
            ),
        ]),
        &HashMap::from([(
            "key".to_string(),
            ParameterValue::String("samples".to_string()),
        )]),
    );
    assert_eq!(
        outputs.get("outcome"),
        Some(&ActionValue::Event(ActionOutcome::Attempted))
    );
}

#[test]
fn context_set_actions_require_event_input() {
    expect_panic(|| {
        ContextSetNumberAction::new().execute(
            &HashMap::from([("value".to_string(), ActionValue::Number(1.0))]),
            &HashMap::new(),
        );
    });
    expect_panic(|| {
        ContextSetBoolAction::new().execute(
            &HashMap::from([("value".to_string(), ActionValue::Bool(true))]),
            &HashMap::new(),
        );
    });
    expect_panic(|| {
        ContextSetStringAction::new().execute(
            &HashMap::from([("value".to_string(), ActionValue::String("x".to_string()))]),
            &HashMap::new(),
        );
    });
    expect_panic(|| {
        ContextSetSeriesAction::new().execute(
            &HashMap::from([("value".to_string(), ActionValue::Series(vec![1.0]))]),
            &HashMap::new(),
        );
    });
}

#[test]
fn context_set_actions_require_typed_value_input() {
    expect_panic(|| {
        ContextSetNumberAction::new().execute(
            &HashMap::from([(
                "event".to_string(),
                ActionValue::Event(ActionOutcome::Attempted),
            )]),
            &HashMap::new(),
        );
    });
    expect_panic(|| {
        ContextSetBoolAction::new().execute(
            &HashMap::from([(
                "event".to_string(),
                ActionValue::Event(ActionOutcome::Attempted),
            )]),
            &HashMap::new(),
        );
    });
    expect_panic(|| {
        ContextSetStringAction::new().execute(
            &HashMap::from([(
                "event".to_string(),
                ActionValue::Event(ActionOutcome::Attempted),
            )]),
            &HashMap::new(),
        );
    });
    expect_panic(|| {
        ContextSetSeriesAction::new().execute(
            &HashMap::from([(
                "event".to_string(),
                ActionValue::Event(ActionOutcome::Attempted),
            )]),
            &HashMap::new(),
        );
    });
}

#[test]
fn context_set_manifests_validate_and_are_stateless() {
    let number = context_set_number_manifest();
    let boolean = context_set_bool_manifest();
    let series = context_set_series_manifest();
    let string = context_set_string_manifest();

    assert!(ActionRegistry::validate_manifest(&number).is_ok());
    assert!(ActionRegistry::validate_manifest(&boolean).is_ok());
    assert!(ActionRegistry::validate_manifest(&series).is_ok());
    assert!(ActionRegistry::validate_manifest(&string).is_ok());

    assert!(!number.state.allowed);
    assert!(!boolean.state.allowed);
    assert!(!series.state.allowed);
    assert!(!string.state.allowed);

    assert_eq!(number.effects.writes[0].name, "$key");
    assert_eq!(boolean.effects.writes[0].name, "$key");
    assert_eq!(series.effects.writes[0].name, "$key");
    assert_eq!(string.effects.writes[0].name, "$key");

    assert_eq!(number.effects.writes[0].from_input, "value");
    assert_eq!(boolean.effects.writes[0].from_input, "value");
    assert_eq!(series.effects.writes[0].from_input, "value");
    assert_eq!(string.effects.writes[0].from_input, "value");
}

fn baseline_intent_manifest(intents: Vec<IntentSpec>) -> ActionPrimitiveManifest {
    ActionPrimitiveManifest {
        id: "intent_test_action".to_string(),
        version: "0.1.0".to_string(),
        kind: ActionKind::Action,
        inputs: vec![
            InputSpec {
                name: "event".to_string(),
                value_type: ActionValueType::Event,
                required: true,
                cardinality: Cardinality::Single,
            },
            InputSpec {
                name: "symbol".to_string(),
                value_type: ActionValueType::String,
                required: true,
                cardinality: Cardinality::Single,
            },
            InputSpec {
                name: "qty".to_string(),
                value_type: ActionValueType::Number,
                required: true,
                cardinality: Cardinality::Single,
            },
        ],
        outputs: vec![OutputSpec {
            name: "outcome".to_string(),
            value_type: ActionValueType::Event,
        }],
        parameters: vec![ParameterSpec {
            name: "side".to_string(),
            value_type: ParameterType::String,
            default: Some(ParameterValue::String("buy".to_string())),
            required: false,
            bounds: None,
        }],
        effects: ActionEffects {
            writes: vec![],
            intents,
        },
        execution: ExecutionSpec {
            deterministic: true,
            retryable: false,
        },
        state: StateSpec { allowed: false },
        side_effects: true,
    }
}

fn valid_input_intent() -> IntentSpec {
    IntentSpec {
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
    }
}

#[test]
fn intent_validation_valid_from_input_passes() {
    let manifest = baseline_intent_manifest(vec![valid_input_intent()]);
    assert!(ActionRegistry::validate_manifest(&manifest).is_ok());
}

#[test]
fn intent_validation_valid_from_param_passes() {
    let mut intent = valid_input_intent();
    intent.fields[0].from_input = None;
    intent.fields[0].from_param = Some("side".to_string());

    let manifest = baseline_intent_manifest(vec![intent]);
    assert!(ActionRegistry::validate_manifest(&manifest).is_ok());
}

#[test]
fn intent_validation_valid_mirror_write_reference_passes() {
    let mut intent = valid_input_intent();
    intent.mirror_writes = vec![IntentMirrorWriteSpec {
        name: "last_symbol".to_string(),
        value_type: ValueType::String,
        from_field: "symbol".to_string(),
    }];

    let manifest = baseline_intent_manifest(vec![intent]);
    assert!(ActionRegistry::validate_manifest(&manifest).is_ok());
}

#[test]
fn intent_validation_from_field_missing_rejected() {
    let mut intent = valid_input_intent();
    intent.mirror_writes = vec![IntentMirrorWriteSpec {
        name: "last_symbol".to_string(),
        value_type: ValueType::String,
        from_field: "missing".to_string(),
    }];

    let manifest = baseline_intent_manifest(vec![intent]);
    let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(
        err,
        ActionValidationError::MirrorWriteFromFieldNotFound { .. }
    ));
}

#[test]
fn intent_validation_from_field_type_mismatch_rejected() {
    let mut intent = valid_input_intent();
    intent.mirror_writes = vec![IntentMirrorWriteSpec {
        name: "last_symbol".to_string(),
        value_type: ValueType::Number,
        from_field: "symbol".to_string(),
    }];

    let manifest = baseline_intent_manifest(vec![intent]);
    let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(
        err,
        ActionValidationError::MirrorWriteTypeMismatch { .. }
    ));
}

#[test]
fn intent_validation_both_sources_set_rejected() {
    let mut intent = valid_input_intent();
    intent.fields[0].from_param = Some("side".to_string());

    let manifest = baseline_intent_manifest(vec![intent]);
    let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(
        err,
        ActionValidationError::IntentFieldMultipleSources { .. }
    ));
}

#[test]
fn intent_validation_neither_source_set_rejected() {
    let mut intent = valid_input_intent();
    intent.fields[0].from_input = None;

    let manifest = baseline_intent_manifest(vec![intent]);
    let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(
        err,
        ActionValidationError::IntentFieldMissingSource { .. }
    ));
}

#[test]
fn intent_validation_duplicate_field_names_rejected() {
    let mut intent = valid_input_intent();
    intent.fields.push(IntentFieldSpec {
        name: "symbol".to_string(),
        value_type: ValueType::String,
        from_input: Some("symbol".to_string()),
        from_param: None,
    });

    let manifest = baseline_intent_manifest(vec![intent]);
    let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(
        err,
        ActionValidationError::DuplicateIntentFieldName { .. }
    ));
}

#[test]
fn intent_validation_duplicate_intent_names_rejected() {
    let manifest = baseline_intent_manifest(vec![valid_input_intent(), valid_input_intent()]);
    let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(
        err,
        ActionValidationError::DuplicateIntentName { .. }
    ));
}
