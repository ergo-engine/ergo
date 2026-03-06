use std::collections::HashMap;

use crate::trigger::{TriggerEvent, TriggerPrimitive, TriggerPrimitiveManifest, TriggerValue};

use super::manifest::emit_if_event_and_true_manifest;

pub struct EmitIfEventAndTrue {
    manifest: TriggerPrimitiveManifest,
}

impl EmitIfEventAndTrue {
    pub fn new() -> Self {
        Self {
            manifest: emit_if_event_and_true_manifest(),
        }
    }
}

impl Default for EmitIfEventAndTrue {
    fn default() -> Self {
        Self::new()
    }
}

impl TriggerPrimitive for EmitIfEventAndTrue {
    fn manifest(&self) -> &TriggerPrimitiveManifest {
        &self.manifest
    }

    fn evaluate(
        &self,
        inputs: &HashMap<String, TriggerValue>,
        _parameters: &HashMap<String, crate::trigger::ParameterValue>,
    ) -> HashMap<String, TriggerValue> {
        let event = inputs
            .get("event")
            .and_then(|v| v.as_event())
            .expect("missing required event input 'event'");
        let condition = inputs
            .get("condition")
            .and_then(|v| v.as_bool())
            .expect("missing required bool input 'condition'");

        let gated_event = if matches!(event, TriggerEvent::Emitted) && condition {
            TriggerEvent::Emitted
        } else {
            TriggerEvent::NotEmitted
        };

        HashMap::from([("event".to_string(), TriggerValue::Event(gated_event))])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect_panic<F: FnOnce() -> R + std::panic::UnwindSafe, R>(f: F) {
        assert!(std::panic::catch_unwind(f).is_err());
    }

    #[test]
    fn emits_when_event_emitted_and_condition_true() {
        let trigger = EmitIfEventAndTrue::new();
        let outputs = trigger.evaluate(
            &HashMap::from([
                (
                    "event".to_string(),
                    TriggerValue::Event(TriggerEvent::Emitted),
                ),
                ("condition".to_string(), TriggerValue::Bool(true)),
            ]),
            &HashMap::new(),
        );
        assert_eq!(
            outputs.get("event"),
            Some(&TriggerValue::Event(TriggerEvent::Emitted))
        );
    }

    #[test]
    fn suppresses_when_event_not_emitted() {
        let trigger = EmitIfEventAndTrue::new();
        let outputs = trigger.evaluate(
            &HashMap::from([
                (
                    "event".to_string(),
                    TriggerValue::Event(TriggerEvent::NotEmitted),
                ),
                ("condition".to_string(), TriggerValue::Bool(true)),
            ]),
            &HashMap::new(),
        );
        assert_eq!(
            outputs.get("event"),
            Some(&TriggerValue::Event(TriggerEvent::NotEmitted))
        );
    }

    #[test]
    fn suppresses_when_condition_false() {
        let trigger = EmitIfEventAndTrue::new();
        let outputs = trigger.evaluate(
            &HashMap::from([
                (
                    "event".to_string(),
                    TriggerValue::Event(TriggerEvent::Emitted),
                ),
                ("condition".to_string(), TriggerValue::Bool(false)),
            ]),
            &HashMap::new(),
        );
        assert_eq!(
            outputs.get("event"),
            Some(&TriggerValue::Event(TriggerEvent::NotEmitted))
        );
    }

    #[test]
    fn missing_event_panics() {
        let trigger = EmitIfEventAndTrue::new();
        expect_panic(|| {
            trigger.evaluate(
                &HashMap::from([("condition".to_string(), TriggerValue::Bool(true))]),
                &HashMap::new(),
            );
        });
    }

    #[test]
    fn missing_condition_panics() {
        let trigger = EmitIfEventAndTrue::new();
        expect_panic(|| {
            trigger.evaluate(
                &HashMap::from([(
                    "event".to_string(),
                    TriggerValue::Event(TriggerEvent::Emitted),
                )]),
                &HashMap::new(),
            );
        });
    }

    #[test]
    fn wrong_event_type_panics() {
        let trigger = EmitIfEventAndTrue::new();
        expect_panic(|| {
            trigger.evaluate(
                &HashMap::from([
                    ("event".to_string(), TriggerValue::Bool(true)),
                    ("condition".to_string(), TriggerValue::Bool(true)),
                ]),
                &HashMap::new(),
            );
        });
    }

    #[test]
    fn wrong_condition_type_panics() {
        let trigger = EmitIfEventAndTrue::new();
        expect_panic(|| {
            trigger.evaluate(
                &HashMap::from([
                    (
                        "event".to_string(),
                        TriggerValue::Event(TriggerEvent::Emitted),
                    ),
                    (
                        "condition".to_string(),
                        TriggerValue::Event(TriggerEvent::Emitted),
                    ),
                ]),
                &HashMap::new(),
            );
        });
    }
}
