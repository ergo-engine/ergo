use crate::action::{
    ActionEffects, ActionKind, ActionPrimitiveManifest, ActionValueType, ExecutionSpec, InputSpec,
    OutputSpec, ParameterSpec, ParameterValue, StateSpec,
};

pub fn annotate_action_manifest() -> ActionPrimitiveManifest {
    ActionPrimitiveManifest {
        id: "annotate_action".to_string(),
        version: "0.1.0".to_string(),
        kind: ActionKind::Action,
        inputs: vec![InputSpec {
            name: "event".to_string(),
            value_type: ActionValueType::Event,
            required: true,
            cardinality: crate::action::Cardinality::Single,
        }],
        outputs: vec![OutputSpec {
            name: "outcome".to_string(),
            value_type: ActionValueType::Event,
        }],
        parameters: vec![ParameterSpec {
            name: "note".to_string(),
            value_type: ParameterValue::String(String::new()).value_type(),
            default: Some(ParameterValue::String(String::new())),
            required: false,
            bounds: None,
        }],
        effects: ActionEffects { writes: vec![] },
        execution: ExecutionSpec {
            deterministic: true,
            retryable: false,
        },
        state: StateSpec { allowed: false },
        side_effects: true,
    }
}
