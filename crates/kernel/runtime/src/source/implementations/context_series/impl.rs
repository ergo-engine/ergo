use std::collections::HashMap;

use crate::common::Value;
use crate::runtime::ExecutionContext;
use crate::source::{ParameterValue, SourcePrimitive, SourcePrimitiveManifest};

use super::manifest::context_series_source_manifest;

const DEFAULT_CONTEXT_KEY: &str = "x";
const KEY_PARAMETER: &str = "key";

pub struct ContextSeriesSource {
    manifest: SourcePrimitiveManifest,
}

impl ContextSeriesSource {
    pub fn new() -> Self {
        Self {
            manifest: context_series_source_manifest(),
        }
    }
}

impl Default for ContextSeriesSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SourcePrimitive for ContextSeriesSource {
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
            .and_then(|v| v.as_series())
            .cloned()
            .unwrap_or_default();

        HashMap::from([("value".to_string(), Value::Series(value))])
    }
}
