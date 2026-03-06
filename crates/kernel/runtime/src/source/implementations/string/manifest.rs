use crate::common::ValueType;
use crate::source::{
    Cadence, ExecutionSpec, OutputSpec, ParameterSpec, ParameterValue, SourceKind,
    SourcePrimitiveManifest, SourceRequires, StateSpec,
};

pub fn string_source_manifest() -> SourcePrimitiveManifest {
    SourcePrimitiveManifest {
        id: "string_source".to_string(),
        version: "0.1.0".to_string(),
        kind: SourceKind::Source,
        inputs: vec![],
        outputs: vec![OutputSpec {
            name: "value".to_string(),
            value_type: ValueType::String,
        }],
        parameters: vec![ParameterSpec {
            name: "value".to_string(),
            value_type: ParameterValue::String(String::new()).value_type(),
            default: Some(ParameterValue::String(String::new())),
            bounds: None,
        }],
        requires: SourceRequires { context: vec![] },
        execution: ExecutionSpec {
            deterministic: true,
            cadence: Cadence::Continuous,
        },
        state: StateSpec { allowed: false },
        side_effects: false,
    }
}
