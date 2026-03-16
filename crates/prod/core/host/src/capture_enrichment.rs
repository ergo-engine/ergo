use ergo_runtime::common::ActionEffect;
use ergo_supervisor::replay::hash_effect;
use ergo_supervisor::{CaptureBundle, CapturedActionEffect, CapturedIntentAck};

pub fn enrich_bundle_with_host_artifacts(
    bundle: &mut CaptureBundle,
    effects_by_decision: &[Vec<ActionEffect>],
    intent_acks_by_decision: &[Vec<CapturedIntentAck>],
    interruptions_by_decision: &[Option<String>],
) {
    for (index, record) in bundle.decisions.iter_mut().enumerate() {
        if let Some(effects) = effects_by_decision.get(index) {
            record.effects = effects
                .iter()
                .map(|effect| CapturedActionEffect {
                    effect_hash: hash_effect(effect),
                    effect: effect.clone(),
                })
                .collect();
        }

        if let Some(intent_acks) = intent_acks_by_decision.get(index) {
            record.intent_acks = intent_acks.clone();
        }

        if let Some(interruption) = interruptions_by_decision.get(index) {
            record.interruption = interruption.clone();
        }
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

#[derive(Debug, Clone, Default)]
pub struct AppliedIntentAcksByDecision {
    inner: Vec<Vec<CapturedIntentAck>>,
}

impl AppliedIntentAcksByDecision {
    pub fn record(&mut self, decision_index: usize, intent_acks: Vec<CapturedIntentAck>) {
        if self.inner.len() <= decision_index {
            self.inner.resize_with(decision_index + 1, Vec::new);
        }
        self.inner[decision_index] = intent_acks;
    }

    pub fn intent_acks(&self) -> &[Vec<CapturedIntentAck>] {
        &self.inner
    }
}

#[derive(Debug, Clone, Default)]
pub struct StepInterruptionsByDecision {
    inner: Vec<Option<String>>,
}

impl StepInterruptionsByDecision {
    pub fn record(&mut self, decision_index: usize, interruption: String) {
        if self.inner.len() <= decision_index {
            self.inner.resize_with(decision_index + 1, || None);
        }
        self.inner[decision_index] = Some(interruption);
    }

    pub fn interruptions(&self) -> &[Option<String>] {
        &self.inner
    }
}
