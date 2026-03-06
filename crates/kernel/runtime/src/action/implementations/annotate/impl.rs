use std::collections::HashMap;

use crate::action::{
    ActionOutcome, ActionPrimitive, ActionPrimitiveManifest, ActionValue, ParameterValue,
};

use super::manifest::annotate_action_manifest;

pub struct AnnotateAction {
    manifest: ActionPrimitiveManifest,
}

impl AnnotateAction {
    pub fn new() -> Self {
        Self {
            manifest: annotate_action_manifest(),
        }
    }
}

impl Default for AnnotateAction {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionPrimitive for AnnotateAction {
    fn manifest(&self) -> &ActionPrimitiveManifest {
        &self.manifest
    }

    fn execute(
        &self,
        inputs: &HashMap<String, ActionValue>,
        parameters: &HashMap<String, ParameterValue>,
    ) -> HashMap<String, ActionValue> {
        let _event = inputs
            .get("event")
            .and_then(|v| v.as_event())
            .expect("missing required event input 'event'");

        let _note = parameters
            .get("note")
            .and_then(|v| match v {
                ParameterValue::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default();

        HashMap::from([(
            "outcome".to_string(),
            ActionValue::Event(ActionOutcome::Attempted),
        )])
    }
}
