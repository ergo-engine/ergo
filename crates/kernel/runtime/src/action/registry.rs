use std::collections::HashMap;

use semver::Version;

use super::{
    ActionKind, ActionPrimitive, ActionPrimitiveManifest, ActionValidationError, ActionValueType,
    OutputSpec, ParameterType,
};
use crate::common::{is_valid_id, ValueType};

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

            if !matches!(
                write.value_type,
                ValueType::Number | ValueType::Series | ValueType::Bool | ValueType::String
            ) {
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
                    Some(inp)
                        if !value_type_matches_action_input(&write.value_type, &inp.value_type) =>
                    {
                        return Err(ActionValidationError::WriteFromInputTypeMismatch {
                            write_name: write.name.clone(),
                            from_input: write.from_input.clone(),
                            expected: write.value_type.clone(),
                            found: inp.value_type.clone(),
                        });
                    }
                    Some(_) => {}
                }
            }
        }

        let mut seen_intents: HashMap<&str, usize> = HashMap::new();
        for (intent_index, intent) in manifest.effects.intents.iter().enumerate() {
            if let Some(&first_index) = seen_intents.get(intent.name.as_str()) {
                return Err(ActionValidationError::DuplicateIntentName {
                    name: intent.name.clone(),
                    first_index,
                    second_index: intent_index,
                });
            }
            seen_intents.insert(&intent.name, intent_index);

            let mut seen_fields: HashMap<&str, usize> = HashMap::new();
            let mut field_types: HashMap<&str, ValueType> = HashMap::new();

            for (field_index, field) in intent.fields.iter().enumerate() {
                if let Some(&first_index) = seen_fields.get(field.name.as_str()) {
                    return Err(ActionValidationError::DuplicateIntentFieldName {
                        intent_name: intent.name.clone(),
                        field_name: field.name.clone(),
                        first_index,
                        second_index: field_index,
                    });
                }
                seen_fields.insert(&field.name, field_index);
                field_types.insert(&field.name, field.value_type.clone());

                match (field.from_input.as_ref(), field.from_param.as_ref()) {
                    (Some(_), Some(_)) => {
                        return Err(ActionValidationError::IntentFieldMultipleSources {
                            intent_name: intent.name.clone(),
                            field_name: field.name.clone(),
                        });
                    }
                    (None, None) => {
                        return Err(ActionValidationError::IntentFieldMissingSource {
                            intent_name: intent.name.clone(),
                            field_name: field.name.clone(),
                        });
                    }
                    (Some(from_input), None) => {
                        let input = manifest.inputs.iter().find(|i| i.name == *from_input);
                        match input {
                            None => {
                                return Err(ActionValidationError::IntentFieldFromInputNotFound {
                                    intent_name: intent.name.clone(),
                                    field_name: field.name.clone(),
                                    from_input: from_input.clone(),
                                });
                            }
                            Some(inp)
                                if !value_type_matches_action_input(
                                    &field.value_type,
                                    &inp.value_type,
                                ) =>
                            {
                                return Err(
                                    ActionValidationError::IntentFieldFromInputTypeMismatch {
                                        intent_name: intent.name.clone(),
                                        field_name: field.name.clone(),
                                        from_input: from_input.clone(),
                                        expected: field.value_type.clone(),
                                        found: inp.value_type.clone(),
                                    },
                                );
                            }
                            Some(_) => {}
                        }
                    }
                    (None, Some(from_param)) => {
                        let parameter = manifest.parameters.iter().find(|p| p.name == *from_param);
                        match parameter {
                            None => {
                                return Err(ActionValidationError::IntentFieldFromParamNotFound {
                                    intent_name: intent.name.clone(),
                                    field_name: field.name.clone(),
                                    from_param: from_param.clone(),
                                });
                            }
                            Some(param)
                                if !value_type_matches_parameter(
                                    &field.value_type,
                                    &param.value_type,
                                ) =>
                            {
                                return Err(
                                    ActionValidationError::IntentFieldFromParamTypeMismatch {
                                        intent_name: intent.name.clone(),
                                        field_name: field.name.clone(),
                                        from_param: from_param.clone(),
                                        expected: field.value_type.clone(),
                                        found: param.value_type.clone(),
                                    },
                                );
                            }
                            Some(_) => {}
                        }
                    }
                }
            }

            for mirror in &intent.mirror_writes {
                let Some(field_type) = field_types.get(mirror.from_field.as_str()) else {
                    return Err(ActionValidationError::MirrorWriteFromFieldNotFound {
                        intent_name: intent.name.clone(),
                        write_name: mirror.name.clone(),
                        from_field: mirror.from_field.clone(),
                    });
                };

                if mirror.value_type != *field_type {
                    return Err(ActionValidationError::MirrorWriteTypeMismatch {
                        intent_name: intent.name.clone(),
                        write_name: mirror.name.clone(),
                        from_field: mirror.from_field.clone(),
                        expected: mirror.value_type.clone(),
                        found: field_type.clone(),
                    });
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

fn value_type_matches_action_input(expected: &ValueType, found: &ActionValueType) -> bool {
    matches!(
        (expected, found),
        (ValueType::Number, ActionValueType::Number)
            | (ValueType::Series, ActionValueType::Series)
            | (ValueType::Bool, ActionValueType::Bool)
            | (ValueType::String, ActionValueType::String)
    )
}

fn value_type_matches_parameter(expected: &ValueType, found: &ParameterType) -> bool {
    matches!(
        (expected, found),
        (ValueType::Number, ParameterType::Number)
            | (ValueType::Bool, ParameterType::Bool)
            | (ValueType::String, ParameterType::String)
    )
}

#[cfg(test)]
mod tests;
