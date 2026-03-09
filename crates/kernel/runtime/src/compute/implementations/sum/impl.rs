use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputeError, ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::sum_manifest;

pub struct Sum {
    manifest: ComputePrimitiveManifest,
}

impl Sum {
    pub fn new() -> Self {
        Self {
            manifest: sum_manifest(),
        }
    }
}

impl Default for Sum {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for Sum {
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

        let sum: f64 = series.iter().copied().sum();
        if !sum.is_finite() {
            return Err(ComputeError::NonFiniteResult);
        }

        Ok(HashMap::from([("result".to_string(), Value::Number(sum))]))
    }
}
