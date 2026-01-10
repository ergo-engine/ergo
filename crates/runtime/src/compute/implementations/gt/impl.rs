use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputeError, ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::gt_manifest;

pub struct Gt {
    manifest: ComputePrimitiveManifest,
}

impl Gt {
    pub fn new() -> Self {
        Self {
            manifest: gt_manifest(),
        }
    }
}

impl Default for Gt {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for Gt {
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

        Ok(HashMap::from([("result".to_string(), Value::Bool(a > b))]))
    }
}
