use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::divide_manifest;

pub struct Divide {
    manifest: ComputePrimitiveManifest,
}

impl Divide {
    pub fn new() -> Self {
        Self {
            manifest: divide_manifest(),
        }
    }
}

impl Default for Divide {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for Divide {
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

        // TODO(B.2): IEEE 754 divide-by-zero produces inf/-inf/NaN, not an error.
        // Risks: NaN propagates silently; NaN comparisons are non-deterministic
        // for replay; downstream branching on NaN may diverge across runs.
        // Decision deferred to v1: either "structured error on divisor==0"
        // vs "allow IEEE + canonicalization rules". See issue #7.
        HashMap::from([("result".to_string(), Value::Number(a / b))])
    }
}
