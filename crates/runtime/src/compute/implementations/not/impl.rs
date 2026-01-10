use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputeError, ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::not_manifest;

pub struct Not {
    manifest: ComputePrimitiveManifest,
}

impl Not {
    pub fn new() -> Self {
        Self {
            manifest: not_manifest(),
        }
    }
}

impl Default for Not {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for Not {
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
            .and_then(|v| v.as_bool())
            .expect("missing required bool input 'value'");

        Ok(HashMap::from([("result".to_string(), Value::Bool(!value))]))
    }
}
