use std::collections::HashMap;

use semver::Version;

use super::{
    OutputSpec, TriggerKind, TriggerPrimitive, TriggerPrimitiveManifest, TriggerValidationError,
    TriggerValueType,
};

pub struct TriggerRegistry {
    primitives: HashMap<String, Box<dyn TriggerPrimitive>>,
}

impl TriggerRegistry {
    pub fn new() -> Self {
        Self {
            primitives: HashMap::new(),
        }
    }

    pub fn validate_manifest(
        manifest: &TriggerPrimitiveManifest,
    ) -> Result<(), TriggerValidationError> {
        if !is_valid_id(&manifest.id) {
            return Err(TriggerValidationError::InvalidId {
                id: manifest.id.clone(),
            });
        }

        if Version::parse(&manifest.version).is_err() {
            return Err(TriggerValidationError::InvalidVersion {
                version: manifest.version.clone(),
            });
        }

        if manifest.kind != TriggerKind::Trigger {
            return Err(TriggerValidationError::WrongKind {
                expected: TriggerKind::Trigger,
                got: manifest.kind.clone(),
            });
        }

        if manifest.inputs.is_empty() {
            return Err(TriggerValidationError::NoInputsDeclared {
                trigger: manifest.id.clone(),
            });
        }

        let mut seen_inputs: HashMap<&str, usize> = HashMap::new();
        for (index, input) in manifest.inputs.iter().enumerate() {
            if let Some(&first_index) = seen_inputs.get(input.name.as_str()) {
                return Err(TriggerValidationError::DuplicateInput {
                    name: input.name.clone(),
                    first_index,
                    second_index: index,
                });
            }
            seen_inputs.insert(&input.name, index);

            match input.value_type {
                TriggerValueType::Number
                | TriggerValueType::Series
                | TriggerValueType::Bool
                | TriggerValueType::Event => {}
            }

            if input.cardinality != super::Cardinality::Single {
                return Err(TriggerValidationError::InvalidInputCardinality {
                    input: input.name.clone(),
                    got: format!("{:?}", input.cardinality),
                });
            }
        }

        for parameter in &manifest.parameters {
            if let Some(default) = &parameter.default {
                let got = default.value_type();
                if got != parameter.value_type {
                    return Err(TriggerValidationError::InvalidParameterType {
                        parameter: parameter.name.clone(),
                        expected: parameter.value_type.clone(),
                        got,
                    });
                }
            }
        }

        if manifest.side_effects {
            return Err(TriggerValidationError::SideEffectsNotAllowed);
        }

        if !manifest.execution.deterministic {
            return Err(TriggerValidationError::NonDeterministicExecution);
        }

        // TRG-STATE-1: Triggers must be stateless.
        // Temporal patterns requiring memory must be implemented as clusters.
        if manifest.state.allowed {
            return Err(TriggerValidationError::StatefulTriggerNotAllowed {
                trigger_id: manifest.id.clone(),
            });
        }

        Self::validate_outputs(&manifest.outputs)?;

        Ok(())
    }

    fn validate_outputs(outputs: &[OutputSpec]) -> Result<(), TriggerValidationError> {
        if outputs.len() != 1 {
            return Err(TriggerValidationError::TriggerWrongOutputCount { got: outputs.len() });
        }

        let output = &outputs[0];
        if output.value_type != TriggerValueType::Event {
            return Err(TriggerValidationError::InvalidOutputType {
                output: output.name.clone(),
                expected: TriggerValueType::Event,
                got: output.value_type.clone(),
            });
        }

        Ok(())
    }

    pub fn register(
        &mut self,
        primitive: Box<dyn TriggerPrimitive>,
    ) -> Result<(), TriggerValidationError> {
        let manifest = primitive.manifest();

        Self::validate_manifest(manifest)?;

        if self.primitives.contains_key(&manifest.id) {
            return Err(TriggerValidationError::DuplicateId(manifest.id.clone()));
        }

        self.primitives.insert(manifest.id.clone(), primitive);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&dyn TriggerPrimitive> {
        self.primitives.get(id).map(|b| b.as_ref())
    }

    pub fn keys(&self) -> Vec<(String, String)> {
        let mut keys: Vec<(String, String)> = self
            .primitives
            .values()
            .map(|primitive| {
                let manifest = primitive.manifest();
                (manifest.id.clone(), manifest.version.clone())
            })
            .collect();
        keys.sort();
        keys
    }
}

impl Default for TriggerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn is_valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[cfg(test)]
mod tests {
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
}
