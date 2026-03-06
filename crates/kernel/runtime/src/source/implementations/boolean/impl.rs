use std::collections::HashMap;

use crate::common::Value;
use crate::runtime::ExecutionContext;
use crate::source::{ParameterValue, SourcePrimitive, SourcePrimitiveManifest};

use super::manifest::boolean_source_manifest;

pub struct BooleanSource {
    manifest: SourcePrimitiveManifest,
}

impl BooleanSource {
    pub fn new() -> Self {
        Self {
            manifest: boolean_source_manifest(),
        }
    }
}

impl Default for BooleanSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SourcePrimitive for BooleanSource {
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
                ParameterValue::Bool(b) => Some(*b),
                _ => None,
            })
            .expect("missing required parameter 'value' for boolean_source");

        HashMap::from([("value".to_string(), Value::Bool(value))])
    }
}
