use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputeError, ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::abs_manifest;

pub struct Abs {
    manifest: ComputePrimitiveManifest,
}

impl Abs {
    pub fn new() -> Self {
        Self {
            manifest: abs_manifest(),
        }
    }
}

impl Default for Abs {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for Abs {
    fn manifest(&self) -> &ComputePrimitiveManifest {
        &self.manifest
    }

    fn compute(
        &self,
        inputs: &HashMap<String, Value>,
        _parameters: &HashMap<String, Value>,
        _state: Option<&mut PrimitiveState>,
    ) -> Result<HashMap<String, Value>, ComputeError> {
        let value = inputs
            .get("value")
            .and_then(|v| v.as_number())
            .expect("missing required numeric input 'value'");

        Ok(HashMap::from([(
            "result".to_string(),
            Value::Number(value.abs()),
        )]))
    }
}
