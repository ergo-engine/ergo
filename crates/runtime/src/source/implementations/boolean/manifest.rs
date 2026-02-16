use crate::common::ValueType;
use crate::source::{
    Cadence, ExecutionSpec, OutputSpec, ParameterSpec, ParameterValue, SourceKind,
    SourcePrimitiveManifest, SourceRequires, StateSpec,
};

pub fn boolean_source_manifest() -> SourcePrimitiveManifest {
    SourcePrimitiveManifest {
        id: "boolean_source".to_string(),
        version: "0.1.0".to_string(),
        kind: SourceKind::Source,
        inputs: vec![],
        outputs: vec![OutputSpec {
            name: "value".to_string(),
            value_type: ValueType::Bool,
        }],
        parameters: vec![ParameterSpec {
            name: "value".to_string(),
            value_type: ParameterValue::Bool(false).value_type(),
            default: Some(ParameterValue::Bool(false)),
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
