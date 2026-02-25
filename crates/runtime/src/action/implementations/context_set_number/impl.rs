use std::collections::HashMap;

use crate::action::{
    ActionOutcome, ActionPrimitive, ActionPrimitiveManifest, ActionValue, ParameterValue,
};

use super::manifest::context_set_number_manifest;

pub struct ContextSetNumberAction {
    manifest: ActionPrimitiveManifest,
}

impl ContextSetNumberAction {
    pub fn new() -> Self {
        Self {
            manifest: context_set_number_manifest(),
        }
    }
}

impl Default for ContextSetNumberAction {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionPrimitive for ContextSetNumberAction {
    fn manifest(&self) -> &ActionPrimitiveManifest {
        &self.manifest
    }

    fn execute(
        &self,
        inputs: &HashMap<String, ActionValue>,
        _parameters: &HashMap<String, ParameterValue>,
    ) -> HashMap<String, ActionValue> {
        let _event = inputs
            .get("event")
            .and_then(|v| v.as_event())
            .expect("missing required event input 'event'");

        let _value = inputs
            .get("value")
            .and_then(|v| match v {
                ActionValue::Number(n) => Some(*n),
                _ => None,
            })
            .expect("missing required number input 'value'");

        HashMap::from([(
            "outcome".to_string(),
            ActionValue::Event(ActionOutcome::Attempted),
        )])
    }
}
