use std::collections::HashMap;

use semver::Version;

use super::{Cadence, SourceKind, SourcePrimitive, SourcePrimitiveManifest, SourceValidationError};
use crate::common::ValueType;

pub struct SourceRegistry {
    primitives: HashMap<String, Box<dyn SourcePrimitive>>,
}

impl SourceRegistry {
    pub fn new() -> Self {
        Self {
            primitives: HashMap::new(),
        }
    }

    pub fn validate_manifest(
        manifest: &SourcePrimitiveManifest,
    ) -> Result<(), SourceValidationError> {
        if !is_valid_id(&manifest.id) {
            return Err(SourceValidationError::InvalidId {
                id: manifest.id.clone(),
            });
        }

        if Version::parse(&manifest.version).is_err() {
            return Err(SourceValidationError::InvalidVersion {
                version: manifest.version.clone(),
            });
        }

        if manifest.kind != SourceKind::Source {
            return Err(SourceValidationError::WrongKind {
                expected: SourceKind::Source,
                got: manifest.kind.clone(),
            });
        }

        if !manifest.inputs.is_empty() {
            return Err(SourceValidationError::InputsNotAllowed);
        }

        if manifest.outputs.is_empty() {
            return Err(SourceValidationError::OutputsRequired);
        }

        let mut seen: HashMap<&str, usize> = HashMap::new();
        for (index, output) in manifest.outputs.iter().enumerate() {
            if let Some(&first_index) = seen.get(output.name.as_str()) {
                return Err(SourceValidationError::DuplicateOutput {
                    name: output.name.clone(),
                    first_index,
                    second_index: index,
                });
            }
            seen.insert(&output.name, index);
        }

        for output in &manifest.outputs {
            match output.value_type {
                ValueType::Number | ValueType::Series | ValueType::Bool | ValueType::String => {}
            }
        }

        for parameter in &manifest.parameters {
            if let Some(default) = &parameter.default {
                let got = default.value_type();
                if got != parameter.value_type {
                    return Err(SourceValidationError::InvalidParameterType {
                        parameter: parameter.name.clone(),
                        expected: parameter.value_type.clone(),
                        got,
                    });
                }
            }
        }

        if manifest.side_effects {
            return Err(SourceValidationError::SideEffectsNotAllowed);
        }

        if !manifest.execution.deterministic {
            return Err(SourceValidationError::NonDeterministicExecution);
        }

        if manifest.execution.cadence != Cadence::Continuous {
            return Err(SourceValidationError::InvalidCadence);
        }

        if manifest.state.allowed {
            return Err(SourceValidationError::StateNotAllowed);
        }

        Ok(())
    }

    pub fn register(
        &mut self,
        primitive: Box<dyn SourcePrimitive>,
    ) -> Result<(), SourceValidationError> {
        let manifest = primitive.manifest();

        Self::validate_manifest(manifest)?;

        if self.primitives.contains_key(&manifest.id) {
            return Err(SourceValidationError::DuplicateId(manifest.id.clone()));
        }

        self.primitives.insert(manifest.id.clone(), primitive);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&Box<dyn SourcePrimitive>> {
        self.primitives.get(id)
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

impl Default for SourceRegistry {
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
