use crate::common::ValueType;
use crate::source::{
    Cadence, ContextRequirement, ExecutionSpec, OutputSpec, SourceKind, SourcePrimitiveManifest,
    SourceRequires, StateSpec,
};

pub fn context_number_source_manifest() -> SourcePrimitiveManifest {
    SourcePrimitiveManifest {
        id: "context_number_source".to_string(),
        version: "0.1.0".to_string(),
        kind: SourceKind::Source,
        inputs: vec![],
        outputs: vec![OutputSpec {
            name: "value".to_string(),
            value_type: ValueType::Number,
        }],
        parameters: vec![],
        requires: SourceRequires {
            context: vec![ContextRequirement {
                name: "x".to_string(),
                ty: ValueType::Number,
                required: false,
            }],
        },
        execution: ExecutionSpec {
            deterministic: true,
            cadence: Cadence::Continuous,
        },
        state: StateSpec { allowed: false },
        side_effects: false,
    }
}
