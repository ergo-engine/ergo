use crate::common::{PrimitiveKind, ValueType};
use crate::compute::{
    Cadence, Cardinality, ComputePrimitiveManifest, ErrorSpec, ExecutionSpec, InputSpec,
    OutputSpec, StateSpec,
};

pub fn neq_manifest() -> ComputePrimitiveManifest {
    ComputePrimitiveManifest {
        id: "neq".to_string(),
        version: "0.1.0".to_string(),
        kind: PrimitiveKind::Compute,
        inputs: vec![
            InputSpec {
                name: "a".to_string(),
                value_type: ValueType::Number,
                required: true,
                cardinality: Cardinality::Single,
            },
            InputSpec {
                name: "b".to_string(),
                value_type: ValueType::Number,
                required: true,
                cardinality: Cardinality::Single,
            },
        ],
        outputs: vec![OutputSpec {
            name: "result".to_string(),
            value_type: ValueType::Bool,
        }],
        parameters: vec![],
        execution: ExecutionSpec {
            deterministic: true,
            cadence: Cadence::Continuous,
            may_error: false,
        },
        errors: ErrorSpec {
            allowed: false,
            types: vec![],
            deterministic: true,
        },
        state: StateSpec {
            allowed: false,
            resettable: false,
            description: None,
        },
        side_effects: false,
    }
}
