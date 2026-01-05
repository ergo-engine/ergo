use crate::common::{PrimitiveKind, ValueType};
use crate::compute::{
    Cadence, ComputePrimitiveManifest, ExecutionSpec, InputSpec, OutputSpec, StateSpec,
};

pub fn select_bool_manifest() -> ComputePrimitiveManifest {
    ComputePrimitiveManifest {
        id: "select_bool".to_string(),
        version: "0.1.0".to_string(),
        kind: PrimitiveKind::Compute,
        inputs: vec![
            InputSpec {
                name: "cond".to_string(),
                value_type: ValueType::Bool,
                required: true,
            },
            InputSpec {
                name: "when_true".to_string(),
                value_type: ValueType::Bool,
                required: true,
            },
            InputSpec {
                name: "when_false".to_string(),
                value_type: ValueType::Bool,
                required: true,
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
        },
        state: StateSpec {
            stateful: false,
            rolling_window: None,
        },
        side_effects: false,
    }
}
