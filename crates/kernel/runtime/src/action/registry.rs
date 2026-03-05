use std::collections::HashMap;

use semver::Version;

use super::{
    ActionKind, ActionPrimitive, ActionPrimitiveManifest, ActionValidationError, ActionValueType,
    OutputSpec, ParameterType,
};
use crate::common::ValueType;

pub struct ActionRegistry {
    primitives: HashMap<String, Box<dyn ActionPrimitive>>,
}

impl ActionRegistry {
    pub fn new() -> Self {
        Self {
            primitives: HashMap::new(),
        }
    }

    pub fn validate_manifest(
        manifest: &ActionPrimitiveManifest,
    ) -> Result<(), ActionValidationError> {
        if !is_valid_id(&manifest.id) {
            return Err(ActionValidationError::InvalidId {
                id: manifest.id.clone(),
            });
        }

        if Version::parse(&manifest.version).is_err() {
            return Err(ActionValidationError::InvalidVersion {
                version: manifest.version.clone(),
            });
        }

        if manifest.kind != ActionKind::Action {
            return Err(ActionValidationError::WrongKind {
                expected: ActionKind::Action,
                got: manifest.kind.clone(),
            });
        }

        let mut seen_inputs: HashMap<&str, usize> = HashMap::new();
        for (index, input) in manifest.inputs.iter().enumerate() {
            if let Some(&first_index) = seen_inputs.get(input.name.as_str()) {
                return Err(ActionValidationError::DuplicateInput {
                    name: input.name.clone(),
                    first_index,
                    second_index: index,
                });
            }
            seen_inputs.insert(&input.name, index);
        }

        for parameter in &manifest.parameters {
            if let Some(default) = &parameter.default {
                let got = default.value_type();
                if got != parameter.value_type {
                    return Err(ActionValidationError::InvalidParameterType {
                        parameter: parameter.name.clone(),
                        expected: parameter.value_type.clone(),
                        got,
                    });
                }
            }
        }

        if !manifest.side_effects {
            return Err(ActionValidationError::SideEffectsRequired);
        }

        if manifest.execution.retryable {
            return Err(ActionValidationError::RetryNotAllowed);
        }

        if !manifest.execution.deterministic {
            return Err(ActionValidationError::NonDeterministicExecution);
        }

        if manifest.state.allowed {
            return Err(ActionValidationError::StateNotAllowed);
        }

        if !manifest
            .inputs
            .iter()
            .any(|input| input.value_type == ActionValueType::Event)
        {
            return Err(ActionValidationError::EventInputRequired);
        }

        let mut seen_writes: HashMap<&str, usize> = HashMap::new();
        for (index, write) in manifest.effects.writes.iter().enumerate() {
            if let Some(&first_index) = seen_writes.get(write.name.as_str()) {
                return Err(ActionValidationError::DuplicateWriteName {
                    name: write.name.clone(),
                    first_index,
                    second_index: index,
                });
            }
            seen_writes.insert(&write.name, index);

            if write.value_type == ValueType::Series {
                return Err(ActionValidationError::InvalidWriteType {
                    name: write.name.clone(),
                    got: write.value_type.clone(),
                });
            }

            // ACT-20/ACT-21: Validate $key references in write specs.
            if let Some(param_name) = write.name.strip_prefix('$') {
                let found = manifest.parameters.iter().find(|p| p.name == param_name);
                match found {
                    None => {
                        return Err(ActionValidationError::UnboundWriteKeyReference {
                            name: write.name.clone(),
                            referenced_param: param_name.to_string(),
                        });
                    }
                    Some(p) if p.value_type != ParameterType::String => {
                        return Err(ActionValidationError::WriteKeyReferenceNotString {
                            name: write.name.clone(),
                            referenced_param: param_name.to_string(),
                        });
                    }
                    _ => {}
                }
            }

            // ACT-22: from_input must reference a declared input.
            // ACT-23: Referenced input must be scalar (not Event) and type-compatible.
            {
                let input = manifest.inputs.iter().find(|i| i.name == write.from_input);
                match input {
                    None => {
                        return Err(ActionValidationError::WriteFromInputNotFound {
                            write_name: write.name.clone(),
                            from_input: write.from_input.clone(),
                        });
                    }
                    Some(inp) => {
                        let compatible = matches!(
                            (&write.value_type, &inp.value_type),
                            (ValueType::Number, super::ActionValueType::Number)
                                | (ValueType::Bool, super::ActionValueType::Bool)
                                | (ValueType::String, super::ActionValueType::String)
                        );
                        if !compatible {
                            return Err(ActionValidationError::WriteFromInputTypeMismatch {
                                write_name: write.name.clone(),
                                from_input: write.from_input.clone(),
                                expected: write.value_type.clone(),
                                found: inp.value_type.clone(),
                            });
                        }
                    }
                }
            }
        }

        Self::validate_outputs(&manifest.outputs)?;

        Ok(())
    }

    fn validate_outputs(outputs: &[OutputSpec]) -> Result<(), ActionValidationError> {
        if outputs.len() != 1 {
            return Err(ActionValidationError::UndeclaredOutput {
                primitive: "action".to_string(),
                output: "expected exactly one outcome event".to_string(),
            });
        }

        let index = 0;
        let output = &outputs[index];
        if output.name != "outcome" {
            return Err(ActionValidationError::OutputNotOutcome {
                name: output.name.clone(),
                index,
            });
        }

        if output.value_type != ActionValueType::Event {
            return Err(ActionValidationError::InvalidOutputType {
                output: output.name.clone(),
                expected: ActionValueType::Event,
                got: output.value_type.clone(),
            });
        }

        Ok(())
    }

    pub fn register(
        &mut self,
        primitive: Box<dyn ActionPrimitive>,
    ) -> Result<(), ActionValidationError> {
        let manifest = primitive.manifest();

        Self::validate_manifest(manifest)?;

        if self.primitives.contains_key(&manifest.id) {
            return Err(ActionValidationError::DuplicateId(manifest.id.clone()));
        }

        self.primitives.insert(manifest.id.clone(), primitive);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&dyn ActionPrimitive> {
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

impl Default for ActionRegistry {
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
    use crate::action::{
        ActionEffects, ActionKind, ActionPrimitiveManifest, ActionValueType, ExecutionSpec,
        InputSpec, OutputSpec, ParameterSpec, ParameterType, ParameterValue, StateSpec,
    };
    use crate::common::ErrorInfo;
    use crate::common::ValueType;

    fn baseline_manifest() -> ActionPrimitiveManifest {
        ActionPrimitiveManifest {
            id: "test_action".to_string(),
            version: "0.1.0".to_string(),
            kind: ActionKind::Action,
            inputs: vec![
                InputSpec {
                    name: "event".to_string(),
                    value_type: ActionValueType::Event,
                    required: true,
                    cardinality: crate::action::Cardinality::Single,
                },
                InputSpec {
                    name: "value".to_string(),
                    value_type: ActionValueType::Number,
                    required: true,
                    cardinality: crate::action::Cardinality::Single,
                },
            ],
            outputs: vec![OutputSpec {
                name: "outcome".to_string(),
                value_type: ActionValueType::Event,
            }],
            parameters: vec![ParameterSpec {
                name: "accept".to_string(),
                value_type: ParameterValue::Bool(true).value_type(),
                default: Some(ParameterValue::Bool(true)),
                required: false,
                bounds: None,
            }],
            effects: ActionEffects { writes: vec![] },
            execution: ExecutionSpec {
                deterministic: true,
                retryable: false,
            },
            state: StateSpec { allowed: false },
            side_effects: true,
        }
    }

    #[test]
    fn act_1_invalid_id_rejected() {
        let mut manifest = baseline_manifest();
        manifest.id = "Bad-Id".to_string();
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(err, ActionValidationError::InvalidId { .. }));
        assert_eq!(err.rule_id(), "ACT-1");
        assert_eq!(err.path().as_deref(), Some("$.id"));
    }

    #[test]
    fn act_2_invalid_version_rejected() {
        let mut manifest = baseline_manifest();
        manifest.version = "not-semver".to_string();
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(err, ActionValidationError::InvalidVersion { .. }));
        assert_eq!(err.rule_id(), "ACT-2");
        assert_eq!(err.path().as_deref(), Some("$.version"));
    }

    #[test]
    fn act_3_kind_action_accepted() {
        let manifest = baseline_manifest();
        assert!(ActionRegistry::validate_manifest(&manifest).is_ok());
    }

    #[test]
    fn act_4_no_event_input_rejected() {
        let mut manifest = baseline_manifest();
        manifest.inputs[0].value_type = ActionValueType::Number;
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(err, ActionValidationError::EventInputRequired));
        assert_eq!(err.rule_id(), "ACT-4");
        assert_eq!(err.path().as_deref(), Some("$.inputs"));
    }

    #[test]
    fn act_5_duplicate_input_rejected() {
        let mut manifest = baseline_manifest();
        manifest.inputs.push(InputSpec {
            name: "event".to_string(),
            value_type: ActionValueType::Event,
            required: true,
            cardinality: crate::action::Cardinality::Single,
        });
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(err, ActionValidationError::DuplicateInput { .. }));
        assert_eq!(err.rule_id(), "ACT-5");
        assert_eq!(err.path().as_deref(), Some("$.inputs[2].name"));
    }

    #[test]
    fn act_6_input_types_valid() {
        let mut manifest = baseline_manifest();
        manifest.inputs.push(InputSpec {
            name: "flag".to_string(),
            value_type: ActionValueType::Bool,
            required: false,
            cardinality: crate::action::Cardinality::Single,
        });
        assert!(ActionRegistry::validate_manifest(&manifest).is_ok());
    }

    #[test]
    fn act_7_wrong_output_count_rejected() {
        let mut manifest = baseline_manifest();
        manifest.outputs.push(OutputSpec {
            name: "extra".to_string(),
            value_type: ActionValueType::Event,
        });
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(
            err,
            ActionValidationError::UndeclaredOutput { .. }
        ));
        assert_eq!(err.rule_id(), "ACT-7");
        assert_eq!(err.path().as_deref(), Some("$.outputs"));
    }

    #[test]
    fn act_8_output_not_outcome_rejected() {
        let mut manifest = baseline_manifest();
        manifest.outputs[0].name = "not_outcome".to_string();
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(
            err,
            ActionValidationError::OutputNotOutcome { .. }
        ));
        assert_eq!(err.rule_id(), "ACT-8");
        assert_eq!(err.path().as_deref(), Some("$.outputs[0].name"));
    }

    #[test]
    fn act_9_output_not_event_rejected() {
        let mut manifest = baseline_manifest();
        manifest.outputs[0].value_type = ActionValueType::Bool;
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(
            err,
            ActionValidationError::InvalidOutputType { .. }
        ));
        assert_eq!(err.rule_id(), "ACT-9");
        assert_eq!(err.path().as_deref(), Some("$.outputs[0].type"));
    }

    #[test]
    fn act_10_action_has_state_rejected() {
        let mut manifest = baseline_manifest();
        manifest.state.allowed = true;
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(err, ActionValidationError::StateNotAllowed));
        assert_eq!(err.rule_id(), "ACT-10");
        assert_eq!(err.path().as_deref(), Some("$.state.allowed"));
    }

    #[test]
    fn act_11_action_no_side_effects_rejected() {
        let mut manifest = baseline_manifest();
        manifest.side_effects = false;
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(err, ActionValidationError::SideEffectsRequired));
        assert_eq!(err.rule_id(), "ACT-11");
        assert_eq!(err.path().as_deref(), Some("$.side_effects"));
    }

    #[test]
    fn act_14_duplicate_write_name_rejected() {
        let mut manifest = baseline_manifest();
        manifest.effects.writes = vec![
            crate::action::ActionWriteSpec {
                name: "price".to_string(),
                value_type: ValueType::Number,
                from_input: "value".to_string(),
            },
            crate::action::ActionWriteSpec {
                name: "price".to_string(),
                value_type: ValueType::Number,
                from_input: "value".to_string(),
            },
        ];
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(
            err,
            ActionValidationError::DuplicateWriteName { .. }
        ));
        assert_eq!(err.rule_id(), "ACT-14");
        assert_eq!(err.path().as_deref(), Some("$.effects.writes[1].name"));
    }

    #[test]
    fn act_15_invalid_write_type_rejected() {
        let mut manifest = baseline_manifest();
        manifest.effects.writes = vec![crate::action::ActionWriteSpec {
            name: "series".to_string(),
            value_type: ValueType::Series,
            from_input: String::new(),
        }];
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(
            err,
            ActionValidationError::InvalidWriteType { .. }
        ));
        assert_eq!(err.rule_id(), "ACT-15");
        assert_eq!(err.path().as_deref(), Some("$.effects.writes[].type"));
    }

    #[test]
    fn act_16_retryable_not_allowed_rejected() {
        let mut manifest = baseline_manifest();
        manifest.execution.retryable = true;
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(err, ActionValidationError::RetryNotAllowed));
        assert_eq!(err.rule_id(), "ACT-16");
        assert_eq!(err.path().as_deref(), Some("$.execution.retryable"));
    }

    #[test]
    fn act_17_non_deterministic_execution_rejected() {
        let mut manifest = baseline_manifest();
        manifest.execution.deterministic = false;
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(
            err,
            ActionValidationError::NonDeterministicExecution
        ));
        assert_eq!(err.rule_id(), "ACT-17");
        assert_eq!(err.path().as_deref(), Some("$.execution.deterministic"));
    }

    #[test]
    fn act_19_invalid_parameter_type_default_rejected() {
        let mut manifest = baseline_manifest();
        manifest.parameters.push(ParameterSpec {
            name: "slippage".to_string(),
            value_type: ParameterType::Number,
            default: Some(ParameterValue::Bool(true)),
            required: false,
            bounds: None,
        });

        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert_eq!(err.rule_id(), "ACT-19");
        assert_eq!(err.path().as_deref(), Some("$.parameters[].default"));
        assert!(matches!(
            err,
            ActionValidationError::InvalidParameterType {
                parameter,
                expected: ParameterType::Number,
                got: ParameterType::Bool
            } if parameter == "slippage"
        ));
    }

    #[test]
    fn act_19_matching_parameter_default_accepted() {
        let mut manifest = baseline_manifest();
        manifest.parameters.push(ParameterSpec {
            name: "slippage".to_string(),
            value_type: ParameterType::Number,
            default: Some(ParameterValue::Number(0.1)),
            required: false,
            bounds: None,
        });

        assert!(ActionRegistry::validate_manifest(&manifest).is_ok());
    }

    struct TestAction {
        manifest: ActionPrimitiveManifest,
    }

    impl ActionPrimitive for TestAction {
        fn manifest(&self) -> &ActionPrimitiveManifest {
            &self.manifest
        }

        fn execute(
            &self,
            _inputs: &HashMap<String, crate::action::ActionValue>,
            _parameters: &HashMap<String, ParameterValue>,
        ) -> HashMap<String, crate::action::ActionValue> {
            HashMap::new()
        }
    }

    #[test]
    fn act_18_duplicate_id_rejected() {
        let mut registry = ActionRegistry::new();

        registry
            .register(Box::new(TestAction {
                manifest: baseline_manifest(),
            }))
            .unwrap();

        let err = registry
            .register(Box::new(TestAction {
                manifest: baseline_manifest(),
            }))
            .unwrap_err();

        assert!(matches!(
            err,
            ActionValidationError::DuplicateId(ref id) if id == "test_action"
        ));
        assert_eq!(err.rule_id(), "ACT-18");
        assert_eq!(err.path().as_deref(), Some("$.id"));
        assert_eq!(
            err.fix().as_deref(),
            Some("Choose a unique ID not already registered")
        );
    }

    #[test]
    fn act_20_dollar_key_write_referencing_nonexistent_param_rejected() {
        let mut manifest = baseline_manifest();
        manifest.effects.writes = vec![crate::action::ActionWriteSpec {
            name: "$key".to_string(),
            value_type: ValueType::Number,
            from_input: String::new(),
        }];
        // No parameter named "key" — should fail ACT-20.
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(
            err,
            ActionValidationError::UnboundWriteKeyReference { .. }
        ));
        assert_eq!(err.rule_id(), "ACT-20");
        assert_eq!(err.path().as_deref(), Some("$.effects.writes[].name"));
    }

    #[test]
    fn act_21_dollar_key_write_referencing_non_string_param_rejected() {
        let mut manifest = baseline_manifest();
        manifest.parameters.push(ParameterSpec {
            name: "key".to_string(),
            value_type: ParameterType::Number,
            default: None,
            required: true,
            bounds: None,
        });
        manifest.effects.writes = vec![crate::action::ActionWriteSpec {
            name: "$key".to_string(),
            value_type: ValueType::Number,
            from_input: String::new(),
        }];
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(
            err,
            ActionValidationError::WriteKeyReferenceNotString { .. }
        ));
        assert_eq!(err.rule_id(), "ACT-21");
        assert_eq!(err.path().as_deref(), Some("$.effects.writes[].name"));
    }

    #[test]
    fn act_20_dollar_key_write_referencing_string_param_accepted() {
        let mut manifest = baseline_manifest();
        manifest.parameters.push(ParameterSpec {
            name: "key".to_string(),
            value_type: ParameterType::String,
            default: Some(ParameterValue::String("price".to_string())),
            required: false,
            bounds: None,
        });
        manifest.effects.writes = vec![crate::action::ActionWriteSpec {
            name: "$key".to_string(),
            value_type: ValueType::Number,
            from_input: "value".to_string(),
        }];
        assert!(ActionRegistry::validate_manifest(&manifest).is_ok());
    }

    #[test]
    fn act_22_from_input_not_found_rejected() {
        let mut manifest = baseline_manifest();
        manifest.effects.writes = vec![crate::action::ActionWriteSpec {
            name: "price".to_string(),
            value_type: ValueType::Number,
            from_input: "nonexistent".to_string(),
        }];
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(
            err,
            ActionValidationError::WriteFromInputNotFound { .. }
        ));
        assert_eq!(err.rule_id(), "ACT-22");
        assert_eq!(err.path().as_deref(), Some("$.effects.writes[].from_input"));
    }

    #[test]
    fn act_23_from_input_event_type_rejected() {
        let mut manifest = baseline_manifest();
        // "event" input is ActionValueType::Event — not compatible with Number write
        manifest.effects.writes = vec![crate::action::ActionWriteSpec {
            name: "price".to_string(),
            value_type: ValueType::Number,
            from_input: "event".to_string(),
        }];
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(
            err,
            ActionValidationError::WriteFromInputTypeMismatch { .. }
        ));
        assert_eq!(err.rule_id(), "ACT-23");
    }

    #[test]
    fn act_23_from_input_scalar_type_mismatch_rejected() {
        let mut manifest = baseline_manifest();
        manifest.inputs.push(crate::action::InputSpec {
            name: "flag".to_string(),
            value_type: crate::action::ActionValueType::Bool,
            required: true,
            cardinality: crate::action::Cardinality::Single,
        });
        manifest.effects.writes = vec![crate::action::ActionWriteSpec {
            name: "price".to_string(),
            value_type: ValueType::Number,
            from_input: "flag".to_string(),
        }];
        let err = ActionRegistry::validate_manifest(&manifest).unwrap_err();
        assert!(matches!(
            err,
            ActionValidationError::WriteFromInputTypeMismatch { .. }
        ));
        assert_eq!(err.rule_id(), "ACT-23");
    }

    #[test]
    fn act_22_valid_from_input_matching_scalar_accepted() {
        let mut manifest = baseline_manifest();
        // baseline already includes a "value" Number input
        manifest.effects.writes = vec![crate::action::ActionWriteSpec {
            name: "price".to_string(),
            value_type: ValueType::Number,
            from_input: "value".to_string(),
        }];
        assert!(ActionRegistry::validate_manifest(&manifest).is_ok());
    }
}
