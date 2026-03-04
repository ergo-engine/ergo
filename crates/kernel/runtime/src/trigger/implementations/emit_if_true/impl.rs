use std::collections::HashMap;

use crate::trigger::{TriggerEvent, TriggerPrimitive, TriggerPrimitiveManifest, TriggerValue};

use super::manifest::emit_if_true_manifest;

pub struct EmitIfTrue {
    manifest: TriggerPrimitiveManifest,
}

impl EmitIfTrue {
    pub fn new() -> Self {
        Self {
            manifest: emit_if_true_manifest(),
        }
    }
}

impl Default for EmitIfTrue {
    fn default() -> Self {
        Self::new()
    }
}

impl TriggerPrimitive for EmitIfTrue {
    fn manifest(&self) -> &TriggerPrimitiveManifest {
        &self.manifest
    }

    fn evaluate(
        &self,
        inputs: &HashMap<String, TriggerValue>,
        _parameters: &HashMap<String, crate::trigger::ParameterValue>,
    ) -> HashMap<String, TriggerValue> {
        let should_emit = inputs
            .get("input")
            .and_then(|v| v.as_bool())
            .expect("missing required bool input 'input'");

        let event = if should_emit {
            TriggerEvent::Emitted
        } else {
            TriggerEvent::NotEmitted
        };

        HashMap::from([("event".to_string(), TriggerValue::Event(event))])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect_panic<F: FnOnce() -> R + std::panic::UnwindSafe, R>(f: F) {
        assert!(std::panic::catch_unwind(f).is_err());
    }

    #[test]
    fn emits_only_on_true() {
        let trigger = EmitIfTrue::new();
        let outputs_true = trigger.evaluate(
            &HashMap::from([("input".to_string(), TriggerValue::Bool(true))]),
            &HashMap::new(),
        );
        assert_eq!(
            outputs_true.get("event"),
            Some(&TriggerValue::Event(TriggerEvent::Emitted))
        );

        let outputs_false = trigger.evaluate(
            &HashMap::from([("input".to_string(), TriggerValue::Bool(false))]),
            &HashMap::new(),
        );
        assert_eq!(
            outputs_false.get("event"),
            Some(&TriggerValue::Event(TriggerEvent::NotEmitted))
        );
    }

    #[test]
    fn missing_input_panics() {
        let trigger = EmitIfTrue::new();
        expect_panic(|| {
            trigger.evaluate(&HashMap::new(), &HashMap::new());
        });
    }
}
