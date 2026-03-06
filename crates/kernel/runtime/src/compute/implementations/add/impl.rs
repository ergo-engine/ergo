use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputeError, ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::add_manifest;

pub struct Add {
    pub manifest: ComputePrimitiveManifest,
}

impl Add {
    pub fn new() -> Self {
        Self {
            manifest: add_manifest(),
        }
    }
}

impl Default for Add {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for Add {
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
            .and_then(|v| v.as_number())
            .expect("missing required numeric input 'a'");
        let b = inputs
            .get("b")
            .and_then(|v| v.as_number())
            .expect("missing required numeric input 'b'");

        let mut outputs = HashMap::new();
        outputs.insert("result".to_string(), Value::Number(a + b));
        Ok(outputs)
    }
}
