use std::collections::HashMap;

use crate::common::ErrorInfo;
use crate::common::Value;
use crate::common::ValueType;
use crate::runtime::ExecutionContext;
use crate::source::{
    BooleanSource, Cadence, ContextNumberSource, ExecutionSpec, InputSpec, NumberSource,
    OutputSpec, ParameterSpec, ParameterType, SourceKind, SourcePrimitive, SourcePrimitiveManifest,
    SourceRegistry, SourceRequires, SourceValidationError, StateSpec, StringSource,
};

fn expect_panic<F: FnOnce() -> R + std::panic::UnwindSafe, R>(f: F) {
    assert!(std::panic::catch_unwind(f).is_err());
}

#[test]
fn number_source_requires_parameter() {
    let source = NumberSource::new();
    let ctx = ExecutionContext::default();
    let outputs = source.produce(
        &HashMap::from([(
            "value".to_string(),
            crate::source::ParameterValue::Number(3.5),
        )]),
        &ctx,
    );
    assert_eq!(outputs.get("value"), Some(&Value::Number(3.5)));

    expect_panic(|| {
        source.produce(&HashMap::new(), &ctx);
    });
}

#[test]
fn boolean_source_requires_parameter() {
    let source = BooleanSource::new();
    let ctx = ExecutionContext::default();
    let outputs = source.produce(
        &HashMap::from([(
            "value".to_string(),
            crate::source::ParameterValue::Bool(true),
        )]),
        &ctx,
    );
    assert_eq!(outputs.get("value"), Some(&Value::Bool(true)));

    expect_panic(|| {
        source.produce(&HashMap::new(), &ctx);
    });
}

#[test]
fn string_source_emits_configured_value() {
    let source = StringSource::new();
    let ctx = ExecutionContext::default();
    let outputs = source.produce(
        &HashMap::from([(
            "value".to_string(),
            crate::source::ParameterValue::String("hello".to_string()),
        )]),
        &ctx,
    );
    assert_eq!(
        outputs.get("value"),
        Some(&Value::String("hello".to_string()))
    );
}

#[test]
fn string_source_defaults_to_empty_string() {
    let source = StringSource::new();
    let ctx = ExecutionContext::default();
    let outputs = source.produce(&HashMap::new(), &ctx);
    assert_eq!(outputs.get("value"), Some(&Value::String(String::new())));
}

#[test]
fn context_number_source_reads_context_value() {
    let source = ContextNumberSource::new();
    let ctx = ExecutionContext::from_values(HashMap::from([("x".to_string(), Value::Number(9.5))]));

    let outputs = source.produce(&HashMap::new(), &ctx);
    assert_eq!(outputs.get("value"), Some(&Value::Number(9.5)));
}

#[test]
fn context_number_source_missing_key_returns_default() {
    let source = ContextNumberSource::new();
    // Context has no "x" key
    let ctx = ExecutionContext::from_values(HashMap::from([(
        "other_key".to_string(),
        Value::Number(99.0),
    )]));

    let outputs = source.produce(&HashMap::new(), &ctx);
    assert_eq!(outputs.get("value"), Some(&Value::Number(0.0)));
}

#[test]
fn context_number_source_wrong_type_returns_default() {
    let source = ContextNumberSource::new();
    // Context has "x" key but wrong type (String instead of Number)
    let ctx = ExecutionContext::from_values(HashMap::from([(
        "x".to_string(),
        Value::String("not a number".to_string()),
    )]));

    let outputs = source.produce(&HashMap::new(), &ctx);
    assert_eq!(outputs.get("value"), Some(&Value::Number(0.0)));
}

fn valid_manifest() -> SourcePrimitiveManifest {
    SourcePrimitiveManifest {
        id: "valid_source".to_string(),
        version: "1.0.0".to_string(),
        kind: SourceKind::Source,
        inputs: vec![],
        outputs: vec![OutputSpec {
            name: "out".to_string(),
            value_type: ValueType::Number,
        }],
        parameters: vec![],
        requires: SourceRequires { context: vec![] },
        execution: ExecutionSpec {
            deterministic: true,
            cadence: Cadence::Continuous,
        },
        state: StateSpec { allowed: false },
        side_effects: false,
    }
}

#[test]
fn src_1_invalid_id_rejected() {
    let mut manifest = valid_manifest();
    manifest.id = "Bad-Id".to_string();

    let err = SourceRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(err, SourceValidationError::InvalidId { .. }));
    assert_eq!(err.rule_id(), "SRC-1");
    assert_eq!(err.path().as_deref(), Some("$.id"));
}

#[test]
fn src_2_invalid_version_rejected() {
    let mut manifest = valid_manifest();
    manifest.version = "not-semver".to_string();

    let err = SourceRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(err, SourceValidationError::InvalidVersion { .. }));
    assert_eq!(err.rule_id(), "SRC-2");
    assert_eq!(err.path().as_deref(), Some("$.version"));
}

#[test]
fn src_3_kind_source_accepted() {
    let manifest = valid_manifest();
    assert!(SourceRegistry::validate_manifest(&manifest).is_ok());
}

#[test]
fn src_4_source_has_inputs_rejected() {
    let mut manifest = valid_manifest();
    manifest.inputs = vec![InputSpec {
        name: "in".to_string(),
        value_type: ValueType::Number,
        required: true,
    }];

    let err = SourceRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(err, SourceValidationError::InputsNotAllowed));
    assert_eq!(err.rule_id(), "SRC-4");
    assert_eq!(err.path().as_deref(), Some("$.inputs"));
}

#[test]
fn src_5_no_outputs_rejected() {
    let mut manifest = valid_manifest();
    manifest.outputs = vec![];

    let err = SourceRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(err, SourceValidationError::OutputsRequired));
    assert_eq!(err.rule_id(), "SRC-5");
    assert_eq!(err.path().as_deref(), Some("$.outputs"));
}

#[test]
fn src_6_duplicate_output_rejected() {
    let mut manifest = valid_manifest();
    manifest.outputs = vec![
        OutputSpec {
            name: "dup".to_string(),
            value_type: ValueType::Number,
        },
        OutputSpec {
            name: "dup".to_string(),
            value_type: ValueType::Number,
        },
    ];

    let err = SourceRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(err, SourceValidationError::DuplicateOutput { .. }));
    assert_eq!(err.rule_id(), "SRC-6");
    assert_eq!(err.path().as_deref(), Some("$.outputs[1].name"));
}

#[test]
fn src_7_output_types_valid() {
    let mut manifest = valid_manifest();
    manifest.outputs = vec![
        OutputSpec {
            name: "s".to_string(),
            value_type: ValueType::String,
        },
        OutputSpec {
            name: "b".to_string(),
            value_type: ValueType::Bool,
        },
    ];

    assert!(SourceRegistry::validate_manifest(&manifest).is_ok());
}

#[test]
fn src_8_source_has_state_rejected() {
    let mut manifest = valid_manifest();
    manifest.state = StateSpec { allowed: true };

    let err = SourceRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(err, SourceValidationError::StateNotAllowed));
    assert_eq!(err.rule_id(), "SRC-8");
    assert_eq!(err.path().as_deref(), Some("$.state.allowed"));
}

#[test]
fn src_9_source_has_side_effects_rejected() {
    let mut manifest = valid_manifest();
    manifest.side_effects = true;

    let err = SourceRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(err, SourceValidationError::SideEffectsNotAllowed));
    assert_eq!(err.rule_id(), "SRC-9");
    assert_eq!(err.path().as_deref(), Some("$.side_effects"));
}

#[test]
fn src_12_non_deterministic_execution_rejected() {
    let mut manifest = valid_manifest();
    manifest.execution.deterministic = false;

    let err = SourceRegistry::validate_manifest(&manifest).unwrap_err();
    assert!(matches!(
        err,
        SourceValidationError::NonDeterministicExecution
    ));
    assert_eq!(err.rule_id(), "SRC-12");
    assert_eq!(err.path().as_deref(), Some("$.execution.deterministic"));
}

#[test]
fn src_15_invalid_parameter_type_default_rejected() {
    let mut manifest = valid_manifest();
    manifest.parameters.push(ParameterSpec {
        name: "threshold".to_string(),
        value_type: ParameterType::Number,
        default: Some(crate::source::ParameterValue::Bool(true)),
        bounds: None,
    });

    let err = SourceRegistry::validate_manifest(&manifest).unwrap_err();
    assert_eq!(err.rule_id(), "SRC-15");
    assert_eq!(err.path().as_deref(), Some("$.parameters[].default"));
    assert!(matches!(
        err,
        SourceValidationError::InvalidParameterType {
            parameter,
            expected: ParameterType::Number,
            got: ParameterType::Bool
        } if parameter == "threshold"
    ));
}

#[test]
fn src_15_matching_parameter_default_accepted() {
    let mut manifest = valid_manifest();
    manifest.parameters.push(ParameterSpec {
        name: "threshold".to_string(),
        value_type: ParameterType::Number,
        default: Some(crate::source::ParameterValue::Number(1.5)),
        bounds: None,
    });

    assert!(SourceRegistry::validate_manifest(&manifest).is_ok());
}

// SRC-13 test: InvalidCadence is currently untestable because the Cadence enum
// only has Continuous. The enforcement code exists at registry.rs:77-78 and will
// be exercised when Cadence variants are expanded (v1 work). See source.md §4.3.

struct TestSource {
    manifest: SourcePrimitiveManifest,
}

impl SourcePrimitive for TestSource {
    fn manifest(&self) -> &SourcePrimitiveManifest {
        &self.manifest
    }

    fn produce(
        &self,
        _parameters: &HashMap<String, crate::source::ParameterValue>,
        _ctx: &ExecutionContext,
    ) -> HashMap<String, Value> {
        HashMap::from([("out".to_string(), Value::Number(0.0))])
    }
}

#[test]
fn src_14_duplicate_id_rejected() {
    let mut registry = SourceRegistry::new();

    registry
        .register(Box::new(TestSource {
            manifest: valid_manifest(),
        }))
        .unwrap();

    let err = registry
        .register(Box::new(TestSource {
            manifest: valid_manifest(),
        }))
        .unwrap_err();

    assert!(matches!(
        err,
        SourceValidationError::DuplicateId(ref id) if id == "valid_source"
    ));
    assert_eq!(err.rule_id(), "SRC-14");
    assert_eq!(err.path().as_deref(), Some("$.id"));
    assert_eq!(
        err.fix().as_deref(),
        Some("Choose a unique ID not already registered")
    );
}
