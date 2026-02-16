use crate::common::{PrimitiveKind, ValueType};
use crate::compute::{
    Cadence, Cardinality, ComputePrimitiveManifest, ErrorSpec, ExecutionSpec, InputSpec,
    OutputSpec, StateSpec,
};

// Output is numeric; both branches must be numeric to avoid implicit coercion.
pub fn select_manifest() -> ComputePrimitiveManifest {
    ComputePrimitiveManifest {
        id: "select".to_string(),
        version: "0.1.0".to_string(),
        kind: PrimitiveKind::Compute,
        inputs: vec![
            InputSpec {
                name: "cond".to_string(),
                value_type: ValueType::Bool,
                required: true,
                cardinality: Cardinality::Single,
            },
            InputSpec {
                name: "when_true".to_string(),
                value_type: ValueType::Number,
                required: true,
                cardinality: Cardinality::Single,
            },
            InputSpec {
                name: "when_false".to_string(),
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
