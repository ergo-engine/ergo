use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputeError, ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::window_manifest;

pub struct Window {
    manifest: ComputePrimitiveManifest,
}

impl Window {
    pub fn new() -> Self {
        Self {
            manifest: window_manifest(),
        }
    }
}

impl Default for Window {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for Window {
    fn manifest(&self) -> &ComputePrimitiveManifest {
        &self.manifest
    }

    fn compute(
        &self,
        inputs: &HashMap<String, Value>,
        parameters: &HashMap<String, Value>,
        _state: Option<&mut PrimitiveState>,
    ) -> Result<HashMap<String, Value>, ComputeError> {
        let series = inputs
            .get("series")
            .and_then(|v| v.as_series())
            .expect("missing required series input 'series'");
        let size = parameters
            .get("size")
            .and_then(|v| v.as_number())
            .expect("missing required parameter 'size'");

        if !size.is_finite() || size.fract() != 0.0 || size <= 0.0 {
            return Err(ComputeError::InvalidParameter {
                parameter: "size".to_string(),
                reason: "size must be a positive integer".to_string(),
            });
        }

        let size = size as usize;
        let start = series.len().saturating_sub(size);
        let result = series[start..].to_vec();

        Ok(HashMap::from([(
            "result".to_string(),
            Value::Series(result),
        )]))
    }
}
