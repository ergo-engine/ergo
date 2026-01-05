use crate::common::{PrimitiveKind, ValueType};
use crate::compute::{
    Cadence, ComputePrimitiveManifest, ExecutionSpec, InputSpec, OutputSpec, StateSpec,
};

pub fn max_manifest() -> ComputePrimitiveManifest {
    ComputePrimitiveManifest {
        id: "max".to_string(),
        version: "0.1.0".to_string(),
        kind: PrimitiveKind::Compute,
        inputs: vec![
            InputSpec {
                name: "a".to_string(),
                value_type: ValueType::Number,
                required: true,
            },
            InputSpec {
                name: "b".to_string(),
                value_type: ValueType::Number,
                required: true,
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
        },
        state: StateSpec {
            stateful: false,
            rolling_window: None,
        },
        side_effects: false,
    }
}
