use super::*;
use crate::common::ErrorInfo;
use crate::common::{PrimitiveKind, Value, ValueType};
use crate::compute::{
    Cadence, Cardinality, ComputeError, ComputePrimitive, ComputePrimitiveManifest, ErrorSpec,
    ExecutionSpec, InputSpec, OutputSpec, ParameterSpec, PrimitiveState, StateSpec,
};

fn baseline_manifest() -> ComputePrimitiveManifest {
    ComputePrimitiveManifest {
        id: "valid_compute".to_string(),
        version: "0.1.0".to_string(),
        kind: PrimitiveKind::Compute,
        inputs: vec![InputSpec {
            name: "in".to_string(),
            value_type: ValueType::Number,
            required: true,
            cardinality: Cardinality::Single,
        }],
        outputs: vec![OutputSpec {
            name: "out".to_string(),
            value_type: ValueType::Number,
        }],
        parameters: vec![],
        execution: ExecutionSpec {
            deterministic: true,
            cadence: Cadence::Continuous,
            may_error: false,
        },
        errors: ErrorSpec {
            allowed: false,
            types: vec![],
            deterministic: true,
        },
        state: StateSpec {
            allowed: false,
            resettable: false,
            description: None,
        },
        side_effects: false,
    }
}

struct BaselineCompute {
    manifest: ComputePrimitiveManifest,
}

impl BaselineCompute {
    fn new() -> Self {
        Self {
            manifest: baseline_manifest(),
        }
    }
}

impl ComputePrimitive for BaselineCompute {
    fn manifest(&self) -> &ComputePrimitiveManifest {
        &self.manifest
    }

    fn compute(
        &self,
        inputs: &std::collections::HashMap<String, Value>,
        _parameters: &std::collections::HashMap<String, Value>,
        _state: Option<&mut PrimitiveState>,
    ) -> Result<std::collections::HashMap<String, Value>, ComputeError> {
        let v = inputs.get("in").and_then(|v| v.as_number()).unwrap_or(0.0);
        Ok(std::collections::HashMap::from([(
            "out".to_string(),
            Value::Number(v),
        )]))
    }
}

#[test]
fn cmp_1_invalid_id_rejected() {
    let mut manifest = baseline_manifest();
    manifest.id = "Bad-Id".to_string();
    let err = PrimitiveRegistry::validate_manifest(&manifest).unwrap_err();
    assert_eq!(err.rule_id(), "CMP-1");
    assert_eq!(err.path().as_deref(), Some("$.id"));
}

#[test]
fn cmp_2_invalid_version_rejected() {
    let mut manifest = baseline_manifest();
    manifest.version = "not-semver".to_string();
    let err = PrimitiveRegistry::validate_manifest(&manifest).unwrap_err();
    assert_eq!(err.rule_id(), "CMP-2");
    assert_eq!(err.path().as_deref(), Some("$.version"));
}

#[test]
fn cmp_3_kind_compute_accepted() {
    let manifest = baseline_manifest();
    assert!(PrimitiveRegistry::validate_manifest(&manifest).is_ok());
}

#[test]
fn cmp_4_no_inputs_rejected() {
    let mut manifest = baseline_manifest();
    manifest.inputs.clear();
    let err = PrimitiveRegistry::validate_manifest(&manifest).unwrap_err();
    assert_eq!(err.rule_id(), "CMP-4");
    assert_eq!(err.path().as_deref(), Some("$.inputs"));
}

#[test]
fn cmp_5_duplicate_inputs_rejected() {
    let mut manifest = baseline_manifest();
    manifest.inputs.push(InputSpec {
        name: "in".to_string(),
        value_type: ValueType::Number,
        required: true,
        cardinality: Cardinality::Single,
    });
    let err = PrimitiveRegistry::validate_manifest(&manifest).unwrap_err();
    assert_eq!(err.rule_id(), "CMP-5");
    assert_eq!(err.path().as_deref(), Some("$.inputs[1].name"));
}

#[test]
fn cmp_6_no_outputs_rejected() {
    let mut manifest = baseline_manifest();
    manifest.outputs.clear();
    let err = PrimitiveRegistry::validate_manifest(&manifest).unwrap_err();
    assert_eq!(err.rule_id(), "CMP-6");
    assert_eq!(err.path().as_deref(), Some("$.outputs"));
}

#[test]
fn cmp_7_duplicate_outputs_rejected() {
    let mut manifest = baseline_manifest();
    manifest.outputs.push(OutputSpec {
        name: "out".to_string(),
        value_type: ValueType::Number,
    });
    let err = PrimitiveRegistry::validate_manifest(&manifest).unwrap_err();
    assert_eq!(err.rule_id(), "CMP-7");
    assert_eq!(err.path().as_deref(), Some("$.outputs[1].name"));
}

#[test]
fn cmp_8_side_effects_rejected() {
    let mut manifest = baseline_manifest();
    manifest.side_effects = true;
    let err = PrimitiveRegistry::validate_manifest(&manifest).unwrap_err();
    assert_eq!(err.rule_id(), "CMP-8");
    assert_eq!(err.path().as_deref(), Some("$.side_effects"));
}

#[test]
fn cmp_9_state_not_resettable_rejected() {
    let mut manifest = baseline_manifest();
    manifest.state.allowed = true;
    manifest.state.resettable = false;
    let err = PrimitiveRegistry::validate_manifest(&manifest).unwrap_err();
    assert_eq!(err.rule_id(), "CMP-9");
    assert_eq!(err.path().as_deref(), Some("$.state.resettable"));
}

#[test]
fn cmp_10_non_deterministic_errors_rejected() {
    let mut manifest = baseline_manifest();
    manifest.errors.allowed = true;
    manifest.errors.deterministic = false;
    let err = PrimitiveRegistry::validate_manifest(&manifest).unwrap_err();
    assert_eq!(err.rule_id(), "CMP-10");
    assert_eq!(err.path().as_deref(), Some("$.errors.deterministic"));
}

#[test]
fn cmp_13_invalid_input_type_rejected() {
    let mut manifest = baseline_manifest();
    manifest.inputs[0].value_type = ValueType::String;
    let err = PrimitiveRegistry::validate_manifest(&manifest).unwrap_err();
    assert_eq!(err.rule_id(), "CMP-13");
    assert_eq!(err.path().as_deref(), Some("$.inputs[].type"));
}

#[test]
fn cmp_14_invalid_input_cardinality_rejected() {
    let mut manifest = baseline_manifest();
    manifest.inputs[0].cardinality = Cardinality::Multiple;
    let err = PrimitiveRegistry::validate_manifest(&manifest).unwrap_err();
    assert_eq!(err.rule_id(), "CMP-14");
    assert_eq!(err.path().as_deref(), Some("$.inputs[].cardinality"));
}

#[test]
fn cmp_20_output_types_valid() {
    let mut manifest = baseline_manifest();
    manifest.outputs = vec![
        OutputSpec {
            name: "num".to_string(),
            value_type: ValueType::Number,
        },
        OutputSpec {
            name: "series".to_string(),
            value_type: ValueType::Series,
        },
        OutputSpec {
            name: "flag".to_string(),
            value_type: ValueType::Bool,
        },
        OutputSpec {
            name: "label".to_string(),
            value_type: ValueType::String,
        },
    ];

    assert!(PrimitiveRegistry::validate_manifest(&manifest).is_ok());
}

#[test]
fn cmp_16_invalid_cadence_rejected() {
    let mut manifest = baseline_manifest();
    manifest.execution.cadence = Cadence::Event;
    let err = PrimitiveRegistry::validate_manifest(&manifest).unwrap_err();
    assert_eq!(err.rule_id(), "CMP-16");
    assert_eq!(err.path().as_deref(), Some("$.execution.cadence"));
}

#[test]
fn cmp_15_invalid_parameter_type_rejected() {
    let mut manifest = baseline_manifest();
    manifest.parameters.push(ParameterSpec {
        name: "param".to_string(),
        value_type: ValueType::String,
        default: None,
        required: true,
        bounds: None,
    });
    let err = PrimitiveRegistry::validate_manifest(&manifest).unwrap_err();
    assert_eq!(err.rule_id(), "CMP-15");
    assert_eq!(err.path().as_deref(), Some("$.parameters[].type"));
}

#[test]
fn cmp_19_invalid_parameter_type_default_rejected() {
    let mut manifest = baseline_manifest();
    manifest.parameters.push(ParameterSpec {
        name: "param".to_string(),
        value_type: ValueType::Number,
        default: Some(Value::Bool(true)),
        required: false,
        bounds: None,
    });

    let err = PrimitiveRegistry::validate_manifest(&manifest).unwrap_err();
    assert_eq!(err.rule_id(), "CMP-19");
    assert_eq!(err.path().as_deref(), Some("$.parameters[].default"));
    assert!(matches!(
        err,
        ValidationError::InvalidParameterType {
            parameter,
            expected: ValueType::Number,
            got: ValueType::Bool
        } if parameter == "param"
    ));
}

#[test]
fn cmp_19_matching_parameter_default_accepted() {
    let mut manifest = baseline_manifest();
    manifest.parameters.push(ParameterSpec {
        name: "param".to_string(),
        value_type: ValueType::Number,
        default: Some(Value::Number(42.0)),
        required: false,
        bounds: None,
    });

    assert!(PrimitiveRegistry::validate_manifest(&manifest).is_ok());
}

#[test]
fn cmp_17_non_deterministic_execution_rejected() {
    let mut manifest = baseline_manifest();
    manifest.execution.deterministic = false;
    let err = PrimitiveRegistry::validate_manifest(&manifest).unwrap_err();
    assert_eq!(err.rule_id(), "CMP-17");
    assert_eq!(err.path().as_deref(), Some("$.execution.deterministic"));
}

#[test]
fn compute_with_inputs_registers() {
    let mut registry = PrimitiveRegistry::new();
    let result = registry.register(Box::new(BaselineCompute::new()));
    assert!(result.is_ok());
}

#[test]
fn cmp_18_duplicate_id_rejected() {
    let mut registry = PrimitiveRegistry::new();

    registry.register(Box::new(BaselineCompute::new())).unwrap();

    let err = registry
        .register(Box::new(BaselineCompute::new()))
        .unwrap_err();

    assert!(matches!(
        err,
        ValidationError::DuplicateId(ref id) if id == "valid_compute"
    ));
    assert_eq!(err.rule_id(), "CMP-18");
    assert_eq!(err.path().as_deref(), Some("$.id"));
    assert_eq!(
        err.fix().as_deref(),
        Some("Choose a unique ID not already registered")
    );
}
