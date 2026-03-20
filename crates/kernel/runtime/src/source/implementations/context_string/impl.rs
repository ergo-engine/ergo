use std::collections::HashMap;

use crate::common::Value;
use crate::runtime::ExecutionContext;
use crate::source::{ParameterValue, SourcePrimitive, SourcePrimitiveManifest};

use super::manifest::context_string_source_manifest;

const DEFAULT_CONTEXT_KEY: &str = "x";
const KEY_PARAMETER: &str = "key";

pub struct ContextStringSource {
    manifest: SourcePrimitiveManifest,
}

impl ContextStringSource {
    pub fn new() -> Self {
        Self {
            manifest: context_string_source_manifest(),
        }
    }
}

impl Default for ContextStringSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SourcePrimitive for ContextStringSource {
    fn manifest(&self) -> &SourcePrimitiveManifest {
        &self.manifest
    }

    fn produce(
        &self,
        parameters: &HashMap<String, ParameterValue>,
        ctx: &ExecutionContext,
    ) -> HashMap<String, Value> {
        let context_key = parameters
            .get(KEY_PARAMETER)
            .and_then(|value| match value {
                ParameterValue::String(string) => Some(string.as_str()),
                _ => None,
            })
            .unwrap_or(DEFAULT_CONTEXT_KEY);

        let value = ctx
            .value(context_key)
            .and_then(|value| value.as_string())
            .map(str::to_string)
            .unwrap_or_default();

        HashMap::from([("value".to_string(), Value::String(value))])
    }
}
