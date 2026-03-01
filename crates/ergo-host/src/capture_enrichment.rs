use std::collections::BTreeMap;

use ergo_adapter::EventId;
use ergo_runtime::common::ActionEffect;
use ergo_supervisor::replay::hash_effect;
use ergo_supervisor::{CaptureBundle, CapturedActionEffect};

pub fn enrich_bundle_with_effects(
    bundle: &mut CaptureBundle,
    effects_by_event: &BTreeMap<String, Vec<ActionEffect>>,
) {
    for record in &mut bundle.decisions {
        let event_id = record.event_id.as_str();
        let Some(effects) = effects_by_event.get(event_id) else {
            continue;
        };

        record.effects = effects
            .iter()
            .map(|effect| CapturedActionEffect {
                effect_hash: hash_effect(effect),
                effect: effect.clone(),
            })
            .collect();
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppliedEffectsByEvent {
    inner: BTreeMap<String, Vec<ActionEffect>>,
}

impl AppliedEffectsByEvent {
    pub fn record(&mut self, event_id: &EventId, effects: Vec<ActionEffect>) {
        self.inner.insert(event_id.as_str().to_string(), effects);
    }

    pub fn map(&self) -> &BTreeMap<String, Vec<ActionEffect>> {
        &self.inner
    }
}
