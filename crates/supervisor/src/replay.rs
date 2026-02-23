use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};

use ergo_adapter::capture::{CaptureError, ExternalEventRecord};
use ergo_adapter::{EventId, RuntimeInvoker};
use ergo_runtime::common::ActionEffect;

use crate::{
    CaptureBundle, CapturedActionEffect, DecisionLog, DecisionLogEntry, EpisodeInvocationRecord,
    Supervisor, NO_ADAPTER_PROVENANCE,
};

#[derive(Clone, Default)]
pub struct MemoryDecisionLog {
    entries: Arc<Mutex<Vec<DecisionLogEntry>>>,
}

impl DecisionLog for MemoryDecisionLog {
    fn log(&self, entry: DecisionLogEntry) {
        let mut guard = self.entries.lock().expect("decision log poisoned");
        guard.push(entry);
    }
}

impl MemoryDecisionLog {
    pub fn records(&self) -> Vec<EpisodeInvocationRecord> {
        let guard = self.entries.lock().expect("decision log poisoned");
        guard
            .iter()
            .map(|entry| {
                let mut record = EpisodeInvocationRecord::from(entry);
                // Hash effects for comparison with captured data.
                let captured: Vec<CapturedActionEffect> = entry
                    .effects
                    .iter()
                    .map(|effect| CapturedActionEffect {
                        effect_hash: hash_effect(effect),
                        effect: effect.clone(),
                    })
                    .collect();
                record.effects = Some(captured);
                record
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReplayError {
    UnsupportedVersion { capture_version: String },
    HashMismatch { event_id: EventId },
    InvalidPayload { event_id: EventId, detail: String },
    AdapterProvenanceMismatch { expected: String, got: String },
    RuntimeProvenanceMismatch { expected: String, got: String },
    UnexpectedAdapterProvidedForNoAdapterCapture,
    AdapterRequiredForProvenancedCapture,
    EffectMismatch {
        event_id: EventId,
        effect_index: usize,
        expected: Option<CapturedActionEffect>,
        actual: Option<CapturedActionEffect>,
        detail: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct StrictReplayExpectations<'a> {
    pub expected_adapter_provenance: &'a str,
    pub expected_runtime_provenance: &'a str,
}

pub fn validate_bundle(bundle: &CaptureBundle) -> Result<(), ReplayError> {
    if bundle.capture_version != crate::CAPTURE_FORMAT_VERSION {
        return Err(ReplayError::UnsupportedVersion {
            capture_version: bundle.capture_version.clone(),
        });
    }

    for record in &bundle.events {
        if !record.validate_hash() {
            return Err(ReplayError::HashMismatch {
                event_id: record.event_id.clone(),
            });
        }
    }

    Ok(())
}

pub fn replay_checked<R: RuntimeInvoker + Clone>(
    bundle: &CaptureBundle,
    runtime: R,
) -> Result<Vec<EpisodeInvocationRecord>, ReplayError> {
    validate_bundle(bundle)?;
    replay_inner(bundle, runtime)
}

pub fn replay_checked_strict<R: RuntimeInvoker + Clone>(
    bundle: &CaptureBundle,
    runtime: R,
    expectations: StrictReplayExpectations<'_>,
) -> Result<Vec<EpisodeInvocationRecord>, ReplayError> {
    validate_bundle(bundle)?;
    validate_replay_provenance(bundle, expectations)?;
    replay_inner(bundle, runtime)
}

pub fn replay<R: RuntimeInvoker + Clone>(
    bundle: &CaptureBundle,
    runtime: R,
) -> Vec<EpisodeInvocationRecord> {
    replay_checked(bundle, runtime).expect("capture bundle validation failed")
}

fn replay_inner<R: RuntimeInvoker + Clone>(
    bundle: &CaptureBundle,
    runtime: R,
) -> Result<Vec<EpisodeInvocationRecord>, ReplayError> {
    let decision_log = MemoryDecisionLog::default();
    let mut supervisor = Supervisor::with_runtime(
        bundle.graph_id.clone(),
        bundle.config.clone(),
        decision_log.clone(),
        runtime,
    );

    for record in &bundle.events {
        supervisor.on_event(rehydrate_event(record)?);
    }

    Ok(decision_log.records())
}

fn validate_replay_provenance(
    bundle: &CaptureBundle,
    expectations: StrictReplayExpectations<'_>,
) -> Result<(), ReplayError> {
    let provenance = bundle.adapter_provenance.as_str();
    if provenance == NO_ADAPTER_PROVENANCE {
        if expectations.expected_adapter_provenance != NO_ADAPTER_PROVENANCE {
            return Err(ReplayError::UnexpectedAdapterProvidedForNoAdapterCapture);
        }
    } else if expectations.expected_adapter_provenance == NO_ADAPTER_PROVENANCE {
        return Err(ReplayError::AdapterRequiredForProvenancedCapture);
    } else if expectations.expected_adapter_provenance != provenance {
        return Err(ReplayError::AdapterProvenanceMismatch {
            expected: provenance.to_string(),
            got: expectations.expected_adapter_provenance.to_string(),
        });
    }

    if expectations.expected_runtime_provenance != bundle.runtime_provenance {
        return Err(ReplayError::RuntimeProvenanceMismatch {
            expected: expectations.expected_runtime_provenance.to_string(),
            got: bundle.runtime_provenance.clone(),
        });
    }

    Ok(())
}

/// Compare replayed decisions against captured decisions with backward-compatible effect semantics.
///
/// - If captured effects is `None` (legacy bundle): skip effect comparison for that record.
/// - If captured effects is `Some`: verify effect count, structured content, and hash.
///
/// Returns `Ok(true)` if all decisions match (including effects where applicable),
/// `Ok(false)` for decision-stream mismatch (non-effect fields), or `Err` for effect mismatch.
pub fn compare_decisions(
    captured: &[EpisodeInvocationRecord],
    replayed: &[EpisodeInvocationRecord],
) -> Result<bool, ReplayError> {
    if captured.len() != replayed.len() {
        return Ok(false);
    }

    for (cap, rep) in captured.iter().zip(replayed.iter()) {
        // Compare non-effect fields
        if cap.event_id != rep.event_id
            || cap.decision != rep.decision
            || cap.schedule_at != rep.schedule_at
            || cap.episode_id != rep.episode_id
            || cap.deadline != rep.deadline
            || cap.termination != rep.termination
            || cap.retry_count != rep.retry_count
        {
            return Ok(false);
        }

        // Effect comparison: only if captured has effect data (backward compat with legacy bundles).
        if let Some(captured_effects) = &cap.effects {
            // MemoryDecisionLog::records() hashes replayed effects into CapturedActionEffect,
            // so replayed records always have effects = Some(...) for effect-aware captures.
            let replayed_effects = match &rep.effects {
                Some(effs) => effs,
                None => {
                    if !captured_effects.is_empty() {
                        return Err(ReplayError::EffectMismatch {
                            event_id: cap.event_id.clone(),
                            effect_index: 0,
                            expected: captured_effects.first().cloned(),
                            actual: None,
                            detail: format!(
                                "expected {} effects, replayed record has no effect data",
                                captured_effects.len()
                            ),
                        });
                    }
                    continue;
                }
            };

            if captured_effects.len() != replayed_effects.len() {
                return Err(ReplayError::EffectMismatch {
                    event_id: cap.event_id.clone(),
                    effect_index: captured_effects.len().min(replayed_effects.len()),
                    expected: captured_effects
                        .get(replayed_effects.len())
                        .cloned(),
                    actual: replayed_effects
                        .get(captured_effects.len())
                        .cloned(),
                    detail: format!(
                        "expected {} effects, got {}",
                        captured_effects.len(),
                        replayed_effects.len()
                    ),
                });
            }

            for (idx, (cap_eff, rep_eff)) in
                captured_effects.iter().zip(replayed_effects.iter()).enumerate()
            {
                if cap_eff.effect != rep_eff.effect || cap_eff.effect_hash != rep_eff.effect_hash {
                    return Err(ReplayError::EffectMismatch {
                        event_id: cap.event_id.clone(),
                        effect_index: idx,
                        expected: Some(cap_eff.clone()),
                        actual: Some(rep_eff.clone()),
                        detail: format!(
                            "effect mismatch at index {}: expected hash '{}', got '{}'",
                            idx, cap_eff.effect_hash, rep_eff.effect_hash
                        ),
                    });
                }
            }
        }
    }

    Ok(true)
}

/// Hash an ActionEffect using the same serialization path as capture.
pub fn hash_effect(effect: &ActionEffect) -> String {
    let effect_bytes = serde_json::to_vec(effect).expect("ActionEffect must be serializable");
    let mut hasher = Sha256::new();
    hasher.update(&effect_bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Decision, EpisodeId};
    use ergo_runtime::common::{EffectWrite, Value};

    fn make_record(
        event_id: &str,
        effects: Option<Vec<CapturedActionEffect>>,
    ) -> EpisodeInvocationRecord {
        EpisodeInvocationRecord {
            event_id: EventId::new(event_id),
            decision: Decision::Invoke,
            schedule_at: None,
            episode_id: EpisodeId::new(0),
            deadline: None,
            termination: Some(ergo_adapter::RunTermination::Completed),
            retry_count: 0,
            effects,
        }
    }

    fn make_captured_effect(key: &str, value: f64) -> CapturedActionEffect {
        let effect = ActionEffect {
            kind: "set_context".to_string(),
            writes: vec![EffectWrite {
                key: key.to_string(),
                value: Value::Number(value),
            }],
        };
        CapturedActionEffect {
            effect_hash: hash_effect(&effect),
            effect,
        }
    }

    #[test]
    fn compare_decisions_legacy_effects_none_skips_comparison() {
        let captured = vec![make_record("e1", None)];
        let replayed = vec![make_record("e1", Some(vec![]))];
        assert_eq!(compare_decisions(&captured, &replayed).unwrap(), true);
    }

    #[test]
    fn compare_decisions_matching_effects_succeeds() {
        let eff = make_captured_effect("price", 42.0);
        let captured = vec![make_record("e1", Some(vec![eff.clone()]))];
        let replayed = vec![make_record("e1", Some(vec![eff]))];
        assert_eq!(compare_decisions(&captured, &replayed).unwrap(), true);
    }

    #[test]
    fn compare_decisions_corrupted_key_returns_effect_mismatch() {
        let cap_eff = make_captured_effect("price", 42.0);
        let rep_eff = make_captured_effect("corrupted_key", 42.0);
        // Hash will differ because the key differs
        let captured = vec![make_record("e1", Some(vec![cap_eff]))];
        let replayed = vec![make_record("e1", Some(vec![rep_eff]))];
        let err = compare_decisions(&captured, &replayed).unwrap_err();
        assert!(matches!(err, ReplayError::EffectMismatch { .. }));
    }

    #[test]
    fn compare_decisions_corrupted_value_returns_effect_mismatch() {
        let cap_eff = make_captured_effect("price", 42.0);
        let rep_eff = make_captured_effect("price", 99.0);
        let captured = vec![make_record("e1", Some(vec![cap_eff]))];
        let replayed = vec![make_record("e1", Some(vec![rep_eff]))];
        let err = compare_decisions(&captured, &replayed).unwrap_err();
        assert!(matches!(err, ReplayError::EffectMismatch { .. }));
    }

    #[test]
    fn compare_decisions_missing_effect_entry_returns_mismatch() {
        let eff = make_captured_effect("price", 42.0);
        let captured = vec![make_record("e1", Some(vec![eff]))];
        let replayed = vec![make_record("e1", Some(vec![]))];
        let err = compare_decisions(&captured, &replayed).unwrap_err();
        assert!(matches!(err, ReplayError::EffectMismatch { .. }));
    }

    #[test]
    fn compare_decisions_hash_tampered_returns_mismatch() {
        let mut cap_eff = make_captured_effect("price", 42.0);
        let rep_eff = make_captured_effect("price", 42.0);
        // Tamper with captured hash but leave effect content identical
        cap_eff.effect_hash = "0000000000000000000000000000000000000000000000000000000000000000".to_string();
        let captured = vec![make_record("e1", Some(vec![cap_eff]))];
        let replayed = vec![make_record("e1", Some(vec![rep_eff]))];
        let err = compare_decisions(&captured, &replayed).unwrap_err();
        assert!(matches!(err, ReplayError::EffectMismatch { .. }));
    }

    #[test]
    fn hash_effect_is_deterministic() {
        let effect = ActionEffect {
            kind: "set_context".to_string(),
            writes: vec![EffectWrite {
                key: "price".to_string(),
                value: Value::Number(42.0),
            }],
        };
        let h1 = hash_effect(&effect);
        let h2 = hash_effect(&effect);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn hash_effect_matches_capture_serialization_path() {
        use sha2::{Digest, Sha256};
        let effect = ActionEffect {
            kind: "set_context".to_string(),
            writes: vec![EffectWrite {
                key: "x".to_string(),
                value: Value::Number(1.0),
            }],
        };
        let bytes = serde_json::to_vec(&effect).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let expected = hex::encode(hasher.finalize());
        assert_eq!(hash_effect(&effect), expected);
    }
}

fn rehydrate_event(
    record: &ExternalEventRecord,
) -> Result<ergo_adapter::ExternalEvent, ReplayError> {
    record.rehydrate_checked().map_err(|err| match err {
        CaptureError::PayloadHashMismatch { .. } => ReplayError::HashMismatch {
            event_id: record.event_id.clone(),
        },
        CaptureError::InvalidPayload { detail } => ReplayError::InvalidPayload {
            event_id: record.event_id.clone(),
            detail,
        },
    })
}
