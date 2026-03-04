use crate::action::{
    ActionEffects, ActionKind, ActionPrimitiveManifest, ActionValueType, ActionWriteSpec,
    ExecutionSpec, InputSpec, OutputSpec, ParameterSpec, ParameterValue, StateSpec,
};
use crate::common::ValueType;

pub fn context_set_bool_manifest() -> ActionPrimitiveManifest {
    ActionPrimitiveManifest {
        id: "context_set_bool".to_string(),
        version: "0.1.0".to_string(),
        kind: ActionKind::Action,
        inputs: vec![
            InputSpec {
                name: "event".to_string(),
                value_type: ActionValueType::Event,
                required: true,
                cardinality: crate::action::Cardinality::Single,
            },
            InputSpec {
                name: "value".to_string(),
                value_type: ActionValueType::Bool,
                required: true,
                cardinality: crate::action::Cardinality::Single,
            },
        ],
        outputs: vec![OutputSpec {
            name: "outcome".to_string(),
            value_type: ActionValueType::Event,
        }],
        parameters: vec![ParameterSpec {
            name: "key".to_string(),
            value_type: ParameterValue::String(String::new()).value_type(),
            default: None,
            required: true,
            bounds: None,
        }],
        effects: ActionEffects {
            writes: vec![ActionWriteSpec {
                name: "$key".to_string(),
                value_type: ValueType::Bool,
                from_input: "value".to_string(),
            }],
        },
        execution: ExecutionSpec {
            deterministic: true,
            retryable: false,
        },
        state: StateSpec { allowed: false },
        side_effects: true,
    }
}
