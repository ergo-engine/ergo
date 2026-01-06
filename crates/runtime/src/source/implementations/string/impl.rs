use std::collections::HashMap;

use crate::common::Value;
use crate::source::{ParameterValue, SourcePrimitive, SourcePrimitiveManifest};

use super::manifest::string_source_manifest;

pub struct StringSource {
    manifest: SourcePrimitiveManifest,
}

impl StringSource {
    pub fn new() -> Self {
        Self {
            manifest: string_source_manifest(),
        }
    }
}

impl Default for StringSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SourcePrimitive for StringSource {
    fn manifest(&self) -> &SourcePrimitiveManifest {
        &self.manifest
    }

    fn produce(&self, parameters: &HashMap<String, ParameterValue>) -> HashMap<String, Value> {
        let value = parameters
            .get("value")
            .and_then(|v| match v {
                ParameterValue::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default();

        HashMap::from([("value".to_string(), Value::String(value))])
    }
}
