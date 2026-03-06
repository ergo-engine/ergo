use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputeError, ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::lte_manifest;

pub struct Lte {
    manifest: ComputePrimitiveManifest,
}

impl Lte {
    pub fn new() -> Self {
        Self {
            manifest: lte_manifest(),
        }
    }
}

impl Default for Lte {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for Lte {
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

        Ok(HashMap::from([("result".to_string(), Value::Bool(a <= b))]))
    }
}
