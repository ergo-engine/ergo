use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputeError, ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::mean_manifest;

pub struct Mean {
    manifest: ComputePrimitiveManifest,
}

impl Mean {
    pub fn new() -> Self {
        Self {
            manifest: mean_manifest(),
        }
    }
}

impl Default for Mean {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for Mean {
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

        let result = if series.is_empty() {
            0.0
        } else {
            let sum: f64 = series.iter().copied().sum();
            let mean = sum / (series.len() as f64);
            if !mean.is_finite() {
                return Err(ComputeError::NonFiniteResult);
            }
            mean
        };

        Ok(HashMap::from([(
            "result".to_string(),
            Value::Number(result),
        )]))
    }
}
