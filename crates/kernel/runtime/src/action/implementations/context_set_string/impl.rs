use std::collections::HashMap;

use crate::action::{
    ActionOutcome, ActionPrimitive, ActionPrimitiveManifest, ActionValue, ParameterValue,
};

use super::manifest::context_set_string_manifest;

pub struct ContextSetStringAction {
    manifest: ActionPrimitiveManifest,
}

impl ContextSetStringAction {
    pub fn new() -> Self {
        Self {
            manifest: context_set_string_manifest(),
        }
    }
}

impl Default for ContextSetStringAction {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionPrimitive for ContextSetStringAction {
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
                ActionValue::String(s) => Some(s.clone()),
                _ => None,
            })
            .expect("missing required string input 'value'");

        HashMap::from([(
            "outcome".to_string(),
            ActionValue::Event(ActionOutcome::Attempted),
        )])
    }
}
