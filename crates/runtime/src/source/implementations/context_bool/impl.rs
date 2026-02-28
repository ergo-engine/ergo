use std::collections::HashMap;

use crate::common::Value;
use crate::runtime::ExecutionContext;
use crate::source::{ParameterValue, SourcePrimitive, SourcePrimitiveManifest};

use super::manifest::context_bool_source_manifest;

const DEFAULT_CONTEXT_KEY: &str = "x";
const KEY_PARAMETER: &str = "key";

pub struct ContextBoolSource {
    manifest: SourcePrimitiveManifest,
}

impl ContextBoolSource {
    pub fn new() -> Self {
        Self {
            manifest: context_bool_source_manifest(),
        }
    }
}

impl Default for ContextBoolSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SourcePrimitive for ContextBoolSource {
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
            .and_then(|v| match v {
                ParameterValue::String(s) => Some(s.as_str()),
                _ => None,
            })
            .unwrap_or(DEFAULT_CONTEXT_KEY);

        let value = ctx
            .value(context_key)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        HashMap::from([("value".to_string(), Value::Bool(value))])
    }
}
