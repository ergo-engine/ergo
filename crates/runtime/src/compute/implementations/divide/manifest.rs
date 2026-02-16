use crate::common::{PrimitiveKind, ValueType};
use crate::compute::{
    Cadence, Cardinality, ComputePrimitiveManifest, ErrorSpec, ErrorType, ExecutionSpec, InputSpec,
    OutputSpec, StateSpec,
};

pub fn divide_manifest() -> ComputePrimitiveManifest {
    ComputePrimitiveManifest {
        id: "divide".to_string(),
        version: "0.2.0".to_string(),
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
            value_type: ValueType::Number,
        }],
        parameters: vec![],
        execution: ExecutionSpec {
            deterministic: true,
            cadence: Cadence::Continuous,
            may_error: true,
        },
        errors: ErrorSpec {
            allowed: true,
            types: vec![ErrorType::DivisionByZero, ErrorType::NonFiniteResult],
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
