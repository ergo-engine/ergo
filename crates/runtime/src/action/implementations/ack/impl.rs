use std::collections::HashMap;

use crate::action::{
    ActionOutcome, ActionPrimitive, ActionPrimitiveManifest, ActionValue, ParameterValue,
};

use super::manifest::ack_action_manifest;

pub struct AckAction {
    manifest: ActionPrimitiveManifest,
}

impl AckAction {
    pub fn new() -> Self {
        Self {
            manifest: ack_action_manifest(),
        }
    }
}

impl Default for AckAction {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionPrimitive for AckAction {
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

        let accept = parameters
            .get("accept")
            .and_then(|v| match v {
                ParameterValue::Bool(b) => Some(*b),
                _ => None,
            })
            .unwrap_or(true);

        let outcome = if accept {
            ActionOutcome::Completed
        } else {
            ActionOutcome::Rejected
        };

        HashMap::from([("outcome".to_string(), ActionValue::Event(outcome))])
    }
}
