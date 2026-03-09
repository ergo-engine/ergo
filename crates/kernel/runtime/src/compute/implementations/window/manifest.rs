use crate::common::{PrimitiveKind, Value, ValueType};
use crate::compute::{
    Cadence, Cardinality, ComputePrimitiveManifest, ErrorSpec, ErrorType, ExecutionSpec, InputSpec,
    OutputSpec, ParameterSpec, StateSpec,
};

pub fn window_manifest() -> ComputePrimitiveManifest {
    ComputePrimitiveManifest {
        id: "window".to_string(),
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
            value_type: ValueType::Series,
        }],
        parameters: vec![ParameterSpec {
            name: "size".to_string(),
            value_type: ValueType::Number,
            default: Some(Value::Number(5.0)),
            required: false,
            bounds: Some("integer > 0".to_string()),
        }],
        execution: ExecutionSpec {
            deterministic: true,
            cadence: Cadence::Continuous,
            may_error: true,
        },
        errors: ErrorSpec {
            allowed: true,
            types: vec![ErrorType::InvalidParameter],
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
