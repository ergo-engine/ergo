use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputeError, ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::append_manifest;

pub struct Append {
    manifest: ComputePrimitiveManifest,
}

impl Append {
    pub fn new() -> Self {
        Self {
            manifest: append_manifest(),
        }
    }
}

impl Default for Append {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for Append {
    fn manifest(&self) -> &ComputePrimitiveManifest {
        &self.manifest
    }

    fn compute(
        &self,
        inputs: &HashMap<String, Value>,
        _parameters: &HashMap<String, Value>,
        _state: Option<&mut PrimitiveState>,
    ) -> Result<HashMap<String, Value>, ComputeError> {
        let mut series = inputs
            .get("series")
            .and_then(|v| v.as_series())
            .cloned()
            .expect("missing required series input 'series'");
        let value = inputs
            .get("value")
            .and_then(|v| v.as_number())
            .expect("missing required numeric input 'value'");

        series.push(value);

        Ok(HashMap::from([(
            "result".to_string(),
            Value::Series(series),
        )]))
    }
}
