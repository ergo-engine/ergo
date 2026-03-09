use crate::common::{PrimitiveKind, ValueType};
use crate::compute::{
    Cadence, Cardinality, ComputePrimitiveManifest, ErrorSpec, ErrorType, ExecutionSpec, InputSpec,
    OutputSpec, StateSpec,
};

pub fn mean_manifest() -> ComputePrimitiveManifest {
    ComputePrimitiveManifest {
        id: "mean".to_string(),
        version: "0.1.0".to_string(),
        kind: PrimitiveKind::Compute,
        inputs: vec![InputSpec {
            name: "series".to_string(),
            value_type: ValueType::Series,
            required: true,
            cardinality: Cardinality::Single,
        }],
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
            types: vec![ErrorType::NonFiniteResult],
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
