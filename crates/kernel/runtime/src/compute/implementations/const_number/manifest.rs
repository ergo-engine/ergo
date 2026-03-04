use crate::common::{PrimitiveKind, ValueType};
use crate::compute::{
    Cadence, Cardinality, ComputePrimitiveManifest, ErrorSpec, ExecutionSpec, InputSpec,
    OutputSpec, ParameterSpec, StateSpec,
};

pub fn const_number_manifest() -> ComputePrimitiveManifest {
    ComputePrimitiveManifest {
        id: "const_number".to_string(),
        version: "0.1.0".to_string(),
        kind: PrimitiveKind::Compute,
        inputs: vec![InputSpec {
            name: "unit".to_string(),
            value_type: ValueType::Number,
            required: false,
            cardinality: Cardinality::Single,
        }],
        outputs: vec![OutputSpec {
            name: "value".to_string(),
            value_type: ValueType::Number,
        }],
        parameters: vec![ParameterSpec {
            name: "value".to_string(),
            value_type: ValueType::Number,
            default: None,
            required: true,
            bounds: None,
        }],
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
