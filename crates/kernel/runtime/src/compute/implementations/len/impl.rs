use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputeError, ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::len_manifest;

pub struct Len {
    manifest: ComputePrimitiveManifest,
}

impl Len {
    pub fn new() -> Self {
        Self {
            manifest: len_manifest(),
        }
    }
}

impl Default for Len {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for Len {
    fn manifest(&self) -> &ComputePrimitiveManifest {
        &self.manifest
    }

    fn compute(
        &self,
        inputs: &HashMap<String, Value>,
        _parameters: &HashMap<String, Value>,
        _state: Option<&mut PrimitiveState>,
    ) -> Result<HashMap<String, Value>, ComputeError> {
        let series = inputs
            .get("series")
            .and_then(|v| v.as_series())
            .expect("missing required series input 'series'");

        Ok(HashMap::from([(
            "result".to_string(),
            Value::Number(series.len() as f64),
        )]))
    }
}
