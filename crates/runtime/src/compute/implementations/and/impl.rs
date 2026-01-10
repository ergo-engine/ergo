use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputeError, ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::and_manifest;

pub struct And {
    manifest: ComputePrimitiveManifest,
}

impl And {
    pub fn new() -> Self {
        Self {
            manifest: and_manifest(),
        }
    }
}

impl Default for And {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for And {
    fn manifest(&self) -> &ComputePrimitiveManifest {
        &self.manifest
    }

    fn compute(
        &self,
        inputs: &HashMap<String, Value>,
        _parameters: &HashMap<String, Value>,
        _state: Option<&mut PrimitiveState>,
    ) -> Result<HashMap<String, Value>, ComputeError> {
        let a = inputs
            .get("a")
            .and_then(|v| v.as_bool())
            .expect("missing required bool input 'a'");
        let b = inputs
            .get("b")
            .and_then(|v| v.as_bool())
            .expect("missing required bool input 'b'");

        Ok(HashMap::from([("result".to_string(), Value::Bool(a && b))]))
    }
}
