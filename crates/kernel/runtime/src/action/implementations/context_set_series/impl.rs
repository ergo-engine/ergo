use std::collections::HashMap;

use crate::action::{
    ActionOutcome, ActionPrimitive, ActionPrimitiveManifest, ActionValue, ParameterValue,
};

use super::manifest::context_set_series_manifest;

pub struct ContextSetSeriesAction {
    manifest: ActionPrimitiveManifest,
}

impl ContextSetSeriesAction {
    pub fn new() -> Self {
        Self {
            manifest: context_set_series_manifest(),
        }
    }
}

impl Default for ContextSetSeriesAction {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionPrimitive for ContextSetSeriesAction {
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
            .and_then(|v| v.as_series())
            .expect("missing required series input 'value'");

        HashMap::from([(
            "outcome".to_string(),
            ActionValue::Event(ActionOutcome::Attempted),
        )])
    }
}
