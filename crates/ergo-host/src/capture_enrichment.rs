use ergo_runtime::common::ActionEffect;
use ergo_supervisor::replay::hash_effect;
use ergo_supervisor::{CaptureBundle, CapturedActionEffect};

pub fn enrich_bundle_with_effects(
    bundle: &mut CaptureBundle,
    effects_by_decision: &[Vec<ActionEffect>],
) {
    for (index, record) in bundle.decisions.iter_mut().enumerate() {
        let Some(effects) = effects_by_decision.get(index) else {
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
pub struct AppliedEffectsByDecision {
    inner: Vec<Vec<ActionEffect>>,
}

impl AppliedEffectsByDecision {
    pub fn record(&mut self, decision_index: usize, effects: Vec<ActionEffect>) {
        if self.inner.len() <= decision_index {
            self.inner.resize_with(decision_index + 1, Vec::new);
        }
        self.inner[decision_index] = effects;
    }

    pub fn effects(&self) -> &[Vec<ActionEffect>] {
        &self.inner
    }
}
