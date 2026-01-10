use std::collections::HashMap;

use crate::common::{PrimitiveKind, ValidationError};
use crate::compute::{ComputePrimitive, ComputePrimitiveManifest};

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
        if manifest.kind != PrimitiveKind::Compute {
            return Err(ValidationError::WrongKind {
                expected: PrimitiveKind::Compute,
                got: manifest.kind.clone(),
            });
        }

        if manifest.side_effects {
            return Err(ValidationError::SideEffectsNotAllowed);
        }

        if !manifest.execution.deterministic {
            return Err(ValidationError::NonDeterministicExecution);
        }

        // X.7: Compute primitives must declare at least one input.
        if manifest.inputs.is_empty() {
            return Err(ValidationError::NoInputsDeclared {
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

    pub fn get(&self, id: &str) -> Option<&Box<dyn ComputePrimitive>> {
        self.primitives.get(id)
    }
}

impl Default for PrimitiveRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{PrimitiveKind, Value, ValueType};
    use crate::compute::{
        Cadence, ComputeError, ComputePrimitive, ComputePrimitiveManifest, ExecutionSpec,
        InputSpec, OutputSpec, PrimitiveState, StateSpec,
    };

    struct ZeroInputCompute {
        manifest: ComputePrimitiveManifest,
    }

    impl ZeroInputCompute {
        fn new() -> Self {
            Self {
                manifest: ComputePrimitiveManifest {
                    id: "zero_input".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Compute,
                    inputs: Vec::new(),
                    outputs: vec![OutputSpec {
                        name: "out".to_string(),
                        value_type: ValueType::Number,
                    }],
                    parameters: Vec::new(),
                    execution: ExecutionSpec {
                        deterministic: true,
                        cadence: Cadence::Continuous,
                    },
                    state: StateSpec {
                        stateful: false,
                        rolling_window: None,
                    },
                    side_effects: false,
                },
            }
        }
    }

    impl ComputePrimitive for ZeroInputCompute {
        fn manifest(&self) -> &ComputePrimitiveManifest {
            &self.manifest
        }

        fn compute(
            &self,
            _inputs: &std::collections::HashMap<String, Value>,
            _parameters: &std::collections::HashMap<String, Value>,
            _state: Option<&mut PrimitiveState>,
        ) -> Result<std::collections::HashMap<String, Value>, ComputeError> {
            Ok(std::collections::HashMap::from([(
                "out".to_string(),
                Value::Number(0.0),
            )]))
        }
    }

    struct SingleInputCompute {
        manifest: ComputePrimitiveManifest,
    }

    impl SingleInputCompute {
        fn new() -> Self {
            Self {
                manifest: ComputePrimitiveManifest {
                    id: "single_input".to_string(),
                    version: "0.1.0".to_string(),
                    kind: PrimitiveKind::Compute,
                    inputs: vec![InputSpec {
                        name: "in".to_string(),
                        value_type: ValueType::Number,
                        required: true,
                    }],
                    outputs: vec![OutputSpec {
                        name: "out".to_string(),
                        value_type: ValueType::Number,
                    }],
                    parameters: Vec::new(),
                    execution: ExecutionSpec {
                        deterministic: true,
                        cadence: Cadence::Continuous,
                    },
                    state: StateSpec {
                        stateful: false,
                        rolling_window: None,
                    },
                    side_effects: false,
                },
            }
        }
    }

    impl ComputePrimitive for SingleInputCompute {
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
    fn compute_with_zero_inputs_rejected() {
        let mut registry = PrimitiveRegistry::new();
        let err = registry
            .register(Box::new(ZeroInputCompute::new()))
            .unwrap_err();

        assert!(matches!(
            err,
            ValidationError::NoInputsDeclared { primitive } if primitive == "zero_input"
        ));
    }

    #[test]
    fn compute_with_inputs_registers() {
        let mut registry = PrimitiveRegistry::new();
        let result = registry.register(Box::new(SingleInputCompute::new()));
        assert!(result.is_ok());
    }
}
