use std::collections::HashMap;

use semver::Version;

use crate::common::{is_valid_id, PrimitiveKind, ValidationError, ValueType};
use crate::compute::{Cardinality, ComputePrimitive, ComputePrimitiveManifest};

pub struct PrimitiveRegistry {
    primitives: HashMap<String, Box<dyn ComputePrimitive>>,
}

impl PrimitiveRegistry {
    pub fn new() -> Self {
        Self {
            primitives: HashMap::new(),
        }
    }

    pub fn validate_manifest(manifest: &ComputePrimitiveManifest) -> Result<(), ValidationError> {
        if !is_valid_id(&manifest.id) {
            return Err(ValidationError::InvalidId {
                id: manifest.id.clone(),
            });
        }

        if Version::parse(&manifest.version).is_err() {
            return Err(ValidationError::InvalidVersion {
                version: manifest.version.clone(),
            });
        }

        if manifest.kind != PrimitiveKind::Compute {
            return Err(ValidationError::WrongKind {
                expected: PrimitiveKind::Compute,
                got: manifest.kind.clone(),
            });
        }

        if manifest.execution.cadence != crate::compute::Cadence::Continuous {
            return Err(ValidationError::InvalidCadence {
                primitive: manifest.id.clone(),
            });
        }

        if manifest.side_effects {
            return Err(ValidationError::SideEffectsNotAllowed);
        }

        if !manifest.execution.deterministic {
            return Err(ValidationError::NonDeterministicExecution);
        }

        if manifest.errors.allowed && !manifest.errors.deterministic {
            return Err(ValidationError::NonDeterministicErrors {
                primitive: manifest.id.clone(),
            });
        }

        // X.7: Compute primitives must declare at least one input.
        if manifest.inputs.is_empty() {
            return Err(ValidationError::NoInputsDeclared {
                primitive: manifest.id.clone(),
            });
        }

        if manifest.outputs.is_empty() {
            return Err(ValidationError::NoOutputsDeclared {
                primitive: manifest.id.clone(),
            });
        }

        let mut seen_inputs: HashMap<&str, usize> = HashMap::new();
        for (index, input) in manifest.inputs.iter().enumerate() {
            if let Some(&first_index) = seen_inputs.get(input.name.as_str()) {
                return Err(ValidationError::DuplicateInput {
                    name: input.name.clone(),
                    first_index,
                    second_index: index,
                });
            }
            seen_inputs.insert(&input.name, index);

            if input.value_type == ValueType::String {
                return Err(ValidationError::InvalidInputType {
                    input: input.name.clone(),
                    expected: ValueType::Number,
                    got: input.value_type.clone(),
                });
            }

            if input.cardinality != Cardinality::Single {
                return Err(ValidationError::InvalidInputCardinality {
                    primitive: manifest.id.clone(),
                    input: input.name.clone(),
                    got: format!("{:?}", input.cardinality),
                });
            }
        }

        let mut seen_outputs: HashMap<&str, usize> = HashMap::new();
        for (index, output) in manifest.outputs.iter().enumerate() {
            if let Some(&first_index) = seen_outputs.get(output.name.as_str()) {
                return Err(ValidationError::DuplicateOutput {
                    name: output.name.clone(),
                    first_index,
                    second_index: index,
                });
            }
            seen_outputs.insert(&output.name, index);

            match output.value_type {
                ValueType::Number | ValueType::Series | ValueType::Bool | ValueType::String => {}
            }
        }

        for parameter in &manifest.parameters {
            if parameter.value_type == ValueType::Series
                || parameter.value_type == ValueType::String
            {
                return Err(ValidationError::UnsupportedParameterType {
                    primitive: manifest.id.clone(),
                    version: manifest.version.clone(),
                    parameter: parameter.name.clone(),
                    got: parameter.value_type.clone(),
                });
            }

            if let Some(default) = &parameter.default {
                let got = default.value_type();
                if got != parameter.value_type {
                    return Err(ValidationError::InvalidParameterType {
                        parameter: parameter.name.clone(),
                        expected: parameter.value_type.clone(),
                        got,
                    });
                }
            }
        }

        if manifest.state.allowed && !manifest.state.resettable {
            return Err(ValidationError::StateNotResettable {
                primitive: manifest.id.clone(),
            });
        }

        Ok(())
    }

    pub fn register(
        &mut self,
        primitive: Box<dyn ComputePrimitive>,
    ) -> Result<(), ValidationError> {
        let manifest = primitive.manifest();

        Self::validate_manifest(manifest)?;

        if self.primitives.contains_key(&manifest.id) {
            return Err(ValidationError::DuplicateId(manifest.id.clone()));
        }

        self.primitives.insert(manifest.id.clone(), primitive);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&dyn ComputePrimitive> {
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

impl Default for PrimitiveRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
