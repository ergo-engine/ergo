use std::collections::HashMap;

use crate::common::Value;
use crate::runtime::ExecutionContext;
use crate::source::{ParameterValue, SourcePrimitive, SourcePrimitiveManifest};

use super::manifest::number_source_manifest;

pub struct NumberSource {
    manifest: SourcePrimitiveManifest,
}

impl NumberSource {
    pub fn new() -> Self {
        Self {
            manifest: number_source_manifest(),
        }
    }
}

impl Default for NumberSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SourcePrimitive for NumberSource {
    fn manifest(&self) -> &SourcePrimitiveManifest {
        &self.manifest
    }

    fn produce(
        &self,
        parameters: &HashMap<String, ParameterValue>,
        _ctx: &ExecutionContext,
    ) -> HashMap<String, Value> {
        let value = parameters
            .get("value")
            .and_then(|v| match v {
                ParameterValue::Number(n) => Some(*n),
                ParameterValue::Int(i) => Some(*i as f64),
                _ => None,
            })
            .expect("missing required parameter 'value' for number_source");

        HashMap::from([("value".to_string(), Value::Number(value))])
    }
}
