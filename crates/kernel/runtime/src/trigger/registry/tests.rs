use super::*;
use crate::common::ErrorInfo;
use crate::trigger::{
    Cadence, ExecutionSpec, InputSpec, ParameterSpec, ParameterType, ParameterValue, StateSpec,
};

fn make_valid_manifest() -> TriggerPrimitiveManifest {
    TriggerPrimitiveManifest {
        id: "test_trigger".to_string(),
        version: "0.1.0".to_string(),
        kind: TriggerKind::Trigger,
        inputs: vec![InputSpec {
            name: "input".to_string(),
            value_type: TriggerValueType::Bool,
            required: true,
            cardinality: super::super::Cardinality::Single,
        }],
        outputs: vec![OutputSpec {
            name: "event".to_string(),
            value_type: TriggerValueType::Event,
        }],
        parameters: vec![],
        execution: ExecutionSpec {
            deterministic: true,
            cadence: Cadence::Continuous,
        },
        state: StateSpec {
            allowed: false,
            description: None,
        },
        side_effects: false,
    }
}

#[test]
fn trg_1_invalid_id_rejected() {
    let mut manifest = make_valid_manifest();
    manifest.id = "Bad-Id".to_string();
    let err = TriggerRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(err, TriggerValidationError::InvalidId { .. }));
    assert_eq!(err.rule_id(), "TRG-1");
    assert_eq!(err.path().as_deref(), Some("$.id"));
}

#[test]
fn trg_2_invalid_version_rejected() {
    let mut manifest = make_valid_manifest();
    manifest.version = "not-semver".to_string();
    let err = TriggerRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(err, TriggerValidationError::InvalidVersion { .. }));
    assert_eq!(err.rule_id(), "TRG-2");
    assert_eq!(err.path().as_deref(), Some("$.version"));
}

#[test]
fn trg_3_kind_trigger_accepted() {
    let manifest = make_valid_manifest();
    assert!(TriggerRegistry::validate_manifest(&manifest).is_ok());
}

#[test]
fn trg_4_no_inputs_rejected() {
    let mut manifest = make_valid_manifest();
    manifest.inputs.clear();
    let err = TriggerRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(
        err,
        TriggerValidationError::NoInputsDeclared { .. }
    ));
    assert_eq!(err.rule_id(), "TRG-4");
    assert_eq!(err.path().as_deref(), Some("$.inputs"));
}

#[test]
fn trg_5_duplicate_input_rejected() {
    let mut manifest = make_valid_manifest();
    manifest.inputs.push(InputSpec {
        name: "input".to_string(),
        value_type: TriggerValueType::Bool,
        required: true,
        cardinality: super::super::Cardinality::Single,
    });
    let err = TriggerRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(err, TriggerValidationError::DuplicateInput { .. }));
    assert_eq!(err.rule_id(), "TRG-5");
    assert_eq!(err.path().as_deref(), Some("$.inputs[1].name"));
}

#[test]
fn trg_6_input_types_valid() {
    let mut manifest = make_valid_manifest();
    manifest.inputs[0].value_type = TriggerValueType::Event;
    assert!(TriggerRegistry::validate_manifest(&manifest).is_ok());
}

#[test]
fn trg_7_wrong_output_count_rejected() {
    let mut manifest = make_valid_manifest();
    manifest.outputs.push(OutputSpec {
        name: "extra".to_string(),
        value_type: TriggerValueType::Event,
    });
    let err = TriggerRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(
        err,
        TriggerValidationError::TriggerWrongOutputCount { got } if got == 2
    ));
    assert_eq!(err.rule_id(), "TRG-7");
    assert_eq!(err.path().as_deref(), Some("$.outputs"));
}

#[test]
fn trg_8_output_not_event_rejected() {
    let mut manifest = make_valid_manifest();
    manifest.outputs[0].value_type = TriggerValueType::Bool;
    let err = TriggerRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(
        err,
        TriggerValidationError::InvalidOutputType { .. }
    ));
    assert_eq!(err.rule_id(), "TRG-8");
    assert_eq!(err.path().as_deref(), Some("$.outputs[0].type"));
}

#[test]
fn trg_9_trigger_has_state_rejected() {
    let mut manifest = make_valid_manifest();
    manifest.state.allowed = true;

    let err = TriggerRegistry::validate_manifest(&manifest).unwrap_err();
    match &err {
        TriggerValidationError::StatefulTriggerNotAllowed { trigger_id } => {
            assert_eq!(trigger_id, "test_trigger");
        }
        other => panic!("expected StatefulTriggerNotAllowed, got {:?}", other),
    }
    assert_eq!(err.rule_id(), "TRG-9");
    assert_eq!(err.path().as_deref(), Some("$.state.allowed"));
}

#[test]
fn trg_10_trigger_has_side_effects_rejected() {
    let mut manifest = make_valid_manifest();
    manifest.side_effects = true;
    let err = TriggerRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(err, TriggerValidationError::SideEffectsNotAllowed));
    assert_eq!(err.rule_id(), "TRG-10");
    assert_eq!(err.path().as_deref(), Some("$.side_effects"));
}

#[test]
fn trg_11_non_deterministic_execution_rejected() {
    let mut manifest = make_valid_manifest();
    manifest.execution.deterministic = false;
    let err = TriggerRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(
        err,
        TriggerValidationError::NonDeterministicExecution
    ));
    assert_eq!(err.rule_id(), "TRG-11");
    assert_eq!(err.path().as_deref(), Some("$.execution.deterministic"));
}

#[test]
fn trg_12_invalid_input_cardinality_rejected() {
    let mut manifest = make_valid_manifest();
    manifest.inputs[0].cardinality = super::super::Cardinality::Multiple;
    let err = TriggerRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(
        err,
        TriggerValidationError::InvalidInputCardinality { .. }
    ));
    assert_eq!(err.rule_id(), "TRG-12");
    assert_eq!(err.path().as_deref(), Some("$.inputs[].cardinality"));
}

#[test]
fn trg_14_invalid_parameter_type_default_rejected() {
    let mut manifest = make_valid_manifest();
    manifest.parameters.push(ParameterSpec {
        name: "window".to_string(),
        value_type: ParameterType::Number,
        default: Some(ParameterValue::Bool(true)),
        required: false,
        bounds: None,
    });

    let err = TriggerRegistry::validate_manifest(&manifest).unwrap_err();
    assert_eq!(err.rule_id(), "TRG-14");
    assert_eq!(err.path().as_deref(), Some("$.parameters[].default"));
    assert!(matches!(
        err,
        TriggerValidationError::InvalidParameterType {
            parameter,
            expected: ParameterType::Number,
            got: ParameterType::Bool
        } if parameter == "window"
    ));
}

#[test]
fn trg_14_matching_parameter_default_accepted() {
    let mut manifest = make_valid_manifest();
    manifest.parameters.push(ParameterSpec {
        name: "window".to_string(),
        value_type: ParameterType::Number,
        default: Some(ParameterValue::Number(3.0)),
        required: false,
        bounds: None,
    });

    assert!(TriggerRegistry::validate_manifest(&manifest).is_ok());
}

struct TestTrigger {
    manifest: TriggerPrimitiveManifest,
}

impl TriggerPrimitive for TestTrigger {
    fn manifest(&self) -> &TriggerPrimitiveManifest {
        &self.manifest
    }

    fn evaluate(
        &self,
        _inputs: &HashMap<String, crate::trigger::TriggerValue>,
        _parameters: &HashMap<String, crate::trigger::ParameterValue>,
    ) -> HashMap<String, crate::trigger::TriggerValue> {
        HashMap::new()
    }
}

#[test]
fn trg_13_duplicate_id_rejected() {
    let mut registry = TriggerRegistry::new();

    registry
        .register(Box::new(TestTrigger {
            manifest: make_valid_manifest(),
        }))
        .unwrap();

    let err = registry
        .register(Box::new(TestTrigger {
            manifest: make_valid_manifest(),
        }))
        .unwrap_err();

    assert!(matches!(
        err,
        TriggerValidationError::DuplicateId(ref id) if id == "test_trigger"
    ));
    assert_eq!(err.rule_id(), "TRG-13");
    assert_eq!(err.path().as_deref(), Some("$.id"));
    assert_eq!(
        err.fix().as_deref(),
        Some("Choose a unique ID not already registered")
    );
}
