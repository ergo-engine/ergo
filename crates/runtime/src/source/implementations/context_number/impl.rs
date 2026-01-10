use std::collections::HashMap;

use crate::common::Value;
use crate::runtime::ExecutionContext;
use crate::source::{ParameterValue, SourcePrimitive, SourcePrimitiveManifest};

use super::manifest::context_number_source_manifest;

const CONTEXT_KEY: &str = "x";

pub struct ContextNumberSource {
    manifest: SourcePrimitiveManifest,
}

impl ContextNumberSource {
    pub fn new() -> Self {
        Self {
            manifest: context_number_source_manifest(),
        }
    }
}

impl Default for ContextNumberSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SourcePrimitive for ContextNumberSource {
    fn manifest(&self) -> &SourcePrimitiveManifest {
        &self.manifest
    }

    fn produce(
        &self,
        _parameters: &HashMap<String, ParameterValue>,
        ctx: &ExecutionContext,
    ) -> HashMap<String, Value> {
        let value = ctx
            .value(CONTEXT_KEY)
            .and_then(|v| v.as_number())
            .unwrap_or(0.0);

        HashMap::from([("value".to_string(), Value::Number(value))])
    }
}
