use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputeError, ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::select_bool_manifest;

pub struct SelectBool {
    manifest: ComputePrimitiveManifest,
}

impl SelectBool {
    pub fn new() -> Self {
        Self {
            manifest: select_bool_manifest(),
        }
    }
}

impl Default for SelectBool {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for SelectBool {
    fn manifest(&self) -> &ComputePrimitiveManifest {
        &self.manifest
    }

    fn compute(
        &self,
        inputs: &HashMap<String, Value>,
        _parameters: &HashMap<String, Value>,
        _state: Option<&mut PrimitiveState>,
    ) -> Result<HashMap<String, Value>, ComputeError> {
        let cond = inputs
            .get("cond")
            .and_then(|v| v.as_bool())
            .expect("missing required bool input 'cond'");
        let when_true = inputs
            .get("when_true")
            .and_then(|v| v.as_bool())
            .expect("missing required bool input 'when_true'");
        let when_false = inputs
            .get("when_false")
            .and_then(|v| v.as_bool())
            .expect("missing required bool input 'when_false'");

        let result = if cond { when_true } else { when_false };

        Ok(HashMap::from([("result".to_string(), Value::Bool(result))]))
    }
}
