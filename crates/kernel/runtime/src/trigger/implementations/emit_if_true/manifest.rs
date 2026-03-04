use crate::trigger::{
    Cadence, Cardinality, ExecutionSpec, InputSpec, OutputSpec, StateSpec, TriggerKind,
    TriggerPrimitiveManifest, TriggerValueType,
};

pub fn emit_if_true_manifest() -> TriggerPrimitiveManifest {
    TriggerPrimitiveManifest {
        id: "emit_if_true".to_string(),
        version: "0.1.0".to_string(),
        kind: TriggerKind::Trigger,
        inputs: vec![InputSpec {
            name: "input".to_string(),
            value_type: TriggerValueType::Bool,
            required: true,
            cardinality: Cardinality::Single,
        }],
        outputs: vec![OutputSpec {
            name: "event".to_string(),
            value_type: TriggerValueType::Event,
        }],
        parameters: vec![],
        execution: ExecutionSpec {
            deterministic: true,
            cadence: Cadence::Continuous,
        },
        state: StateSpec {
            allowed: false,
            description: None,
        },
        side_effects: false,
    }
}
