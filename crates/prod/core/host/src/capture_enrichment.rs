//! capture_enrichment
//!
//! Purpose:
//! - Finalize host-owned capture sidecars into supervisor-owned
//!   `CaptureBundle.decisions[*]` records after canonical host execution has
//!   drained effects and observed egress outcomes.
//!
//! Owns:
//! - Mapping applied `ActionEffect`s, durable `CapturedIntentAck`s, and
//!   per-decision interruption strings onto decision records by index.
//! - The decision-indexed sidecar accumulators that `runner.rs` fills during
//!   execution and finalization.
//!
//! Does not own:
//! - `CaptureBundle` / `EpisodeInvocationRecord` schema or serde compatibility;
//!   those live in `ergo-supervisor`.
//! - Replay comparison semantics; replay compares canonical `effects`, while
//!   `intent_acks` and `interruption` remain capture metadata.
//! - Egress provenance, interruption taxonomy, or effect production.
//!
//! Connects to:
//! - `runner.rs`, which records sidecars and calls
//!   `enrich_bundle_with_host_artifacts(...)` while finalizing a hosted run.
//! - `ergo-supervisor`, which owns the persisted capture types and
//!   `hash_effect(...)`.
//!
//! Safety notes:
//! - Enrichment is by decision index, not `event_id`, per the host/supervisor
//!   orchestration contract.
//! - Host-enriched `effects` are the authoritative canonical effect records for
//!   host captures.
//! - If a sidecar slice has no entry for a decision index, enrichment leaves
//!   the existing bundle field untouched.
//! - Sparse `record(...)` calls materialize explicit default gap entries
//!   (`[]`/`None`), so later finalization clears earlier bundle slots back to
//!   defaults rather than treating those gaps as missing.
//! - Repeated `record(...)` calls for the same decision index are
//!   last-write-wins.
//! - The public wrapper types preserve host-specific names while delegating the
//!   sparse/overwrite behavior to one private generic decision-index helper.

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
struct ByDecisionIndex<T> {
    inner: Vec<T>,
}

impl<T: Default> ByDecisionIndex<T> {
    fn record(&mut self, decision_index: usize, value: T) {
        if self.inner.len() <= decision_index {
            self.inner.resize_with(decision_index + 1, T::default);
        }
        self.inner[decision_index] = value;
    }

    fn as_slice(&self) -> &[T] {
        &self.inner
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppliedEffectsByDecision {
    inner: ByDecisionIndex<Vec<ActionEffect>>,
}

impl AppliedEffectsByDecision {
    pub fn record(&mut self, decision_index: usize, effects: Vec<ActionEffect>) {
        self.inner.record(decision_index, effects);
    }

    pub fn effects(&self) -> &[Vec<ActionEffect>] {
        self.inner.as_slice()
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppliedIntentAcksByDecision {
    inner: ByDecisionIndex<Vec<CapturedIntentAck>>,
}

impl AppliedIntentAcksByDecision {
    pub fn record(&mut self, decision_index: usize, intent_acks: Vec<CapturedIntentAck>) {
        self.inner.record(decision_index, intent_acks);
    }

    pub fn intent_acks(&self) -> &[Vec<CapturedIntentAck>] {
        self.inner.as_slice()
    }
}

#[derive(Debug, Clone, Default)]
pub struct StepInterruptionsByDecision {
    inner: ByDecisionIndex<Option<String>>,
}

impl StepInterruptionsByDecision {
    pub fn record(&mut self, decision_index: usize, interruption: String) {
        self.inner.record(decision_index, Some(interruption));
    }

    pub fn interruptions(&self) -> &[Option<String>] {
        self.inner.as_slice()
    }
}

#[cfg(test)]
mod tests;
