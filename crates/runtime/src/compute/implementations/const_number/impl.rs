use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputeError, ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::const_number_manifest;

pub struct ConstNumber {
    manifest: ComputePrimitiveManifest,
}

impl ConstNumber {
    pub fn new() -> Self {
        Self {
            manifest: const_number_manifest(),
        }
    }
}

impl Default for ConstNumber {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for ConstNumber {
    fn manifest(&self) -> &ComputePrimitiveManifest {
        &self.manifest
    }

    fn compute(
        &self,
        _inputs: &HashMap<String, Value>,
        parameters: &HashMap<String, Value>,
        _state: Option<&mut PrimitiveState>,
    ) -> Result<HashMap<String, Value>, ComputeError> {
        let value = parameters
            .get("value")
            .and_then(|v| v.as_number())
            .expect("missing required parameter 'value' for const_number");

        Ok(HashMap::from([("value".to_string(), Value::Number(value))]))
    }
}
