use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::max_manifest;

pub struct Max {
    manifest: ComputePrimitiveManifest,
}

impl Max {
    pub fn new() -> Self {
        Self {
            manifest: max_manifest(),
        }
    }
}

impl Default for Max {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for Max {
    fn manifest(&self) -> &ComputePrimitiveManifest {
        &self.manifest
    }

    fn compute(
        &self,
        inputs: &HashMap<String, Value>,
        _parameters: &HashMap<String, Value>,
        _state: Option<&mut PrimitiveState>,
    ) -> HashMap<String, Value> {
        let a = inputs
            .get("a")
            .and_then(|v| v.as_number())
            .expect("missing required numeric input 'a'");
        let b = inputs
            .get("b")
            .and_then(|v| v.as_number())
            .expect("missing required numeric input 'b'");

        HashMap::from([("result".to_string(), Value::Number(a.max(b)))])
    }
}
