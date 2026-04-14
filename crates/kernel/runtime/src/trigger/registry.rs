use std::collections::HashMap;

use semver::Version;

use super::{
    OutputSpec, TriggerKind, TriggerPrimitive, TriggerPrimitiveManifest, TriggerValidationError,
    TriggerValueType,
};
use crate::common::is_valid_id;

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

#[cfg(test)]
mod tests;
