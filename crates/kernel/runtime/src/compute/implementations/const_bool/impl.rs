use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputeError, ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::const_bool_manifest;

pub struct ConstBool {
    manifest: ComputePrimitiveManifest,
}

impl ConstBool {
    pub fn new() -> Self {
        Self {
            manifest: const_bool_manifest(),
        }
    }
}

impl Default for ConstBool {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for ConstBool {
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
            .and_then(|v| v.as_bool())
            .expect("missing required parameter 'value' for const_bool");

        Ok(HashMap::from([("value".to_string(), Value::Bool(value))]))
    }
}
