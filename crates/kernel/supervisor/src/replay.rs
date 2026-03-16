use std::collections::HashSet;
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
                record.effects = captured;
                record
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReplayError {
    UnsupportedVersion {
        capture_version: String,
    },
    HashMismatch {
        event_id: EventId,
    },
    InvalidPayload {
        event_id: EventId,
        detail: String,
    },
    AdapterProvenanceMismatch {
        expected: String,
        got: String,
    },
    RuntimeProvenanceMismatch {
        expected: String,
        got: String,
    },
    UnexpectedAdapterProvidedForNoAdapterCapture,
    AdapterRequiredForProvenancedCapture,
    DuplicateEventId {
        event_id: EventId,
    },
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
    validate_bundle_strict(bundle, expectations)?;
    replay_inner(bundle, runtime)
}

pub fn validate_bundle_strict(
    bundle: &CaptureBundle,
    expectations: StrictReplayExpectations<'_>,
) -> Result<(), ReplayError> {
    validate_bundle(bundle)?;
    validate_unique_event_ids(bundle)?;
    validate_replay_provenance(bundle, expectations)?;
    Ok(())
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

fn validate_unique_event_ids(bundle: &CaptureBundle) -> Result<(), ReplayError> {
    let mut seen = HashSet::new();
    for record in &bundle.events {
        let id = record.event_id.as_str().to_string();
        if !seen.insert(id.clone()) {
            return Err(ReplayError::DuplicateEventId {
                event_id: EventId::new(id),
            });
        }
    }
    Ok(())
}

/// Compare replayed decisions against captured decisions with strict effect semantics.
///
/// Returns `Ok(true)` if all decisions match (including effects),
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

        let captured_effects = &cap.effects;
        let replayed_effects = &rep.effects;

        if captured_effects.len() != replayed_effects.len() {
            return Err(ReplayError::EffectMismatch {
                event_id: cap.event_id.clone(),
                effect_index: captured_effects.len().min(replayed_effects.len()),
                expected: captured_effects.get(replayed_effects.len()).cloned(),
                actual: replayed_effects.get(captured_effects.len()).cloned(),
                detail: format!(
                    "expected {} effects, got {}",
                    captured_effects.len(),
                    replayed_effects.len()
                ),
            });
        }

        for (idx, (cap_eff, rep_eff)) in captured_effects
            .iter()
            .zip(replayed_effects.iter())
            .enumerate()
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

    Ok(true)
}

/// Hash an ActionEffect using the same serialization path as capture.
pub fn hash_effect(effect: &ActionEffect) -> String {
    let effect_bytes = serde_json::to_vec(effect).expect("ActionEffect must be serializable");
    let mut hasher = Sha256::new();
    hasher.update(&effect_bytes);
    hex::encode(hasher.finalize())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Decision, EpisodeId};
    use ergo_runtime::common::{EffectWrite, Value};
    use serde::Serialize;

    #[derive(Debug, Clone, PartialEq, Serialize)]
    struct LegacyActionEffect {
        kind: String,
        writes: Vec<EffectWrite>,
    }

    fn make_record(event_id: &str, effects: Vec<CapturedActionEffect>) -> EpisodeInvocationRecord {
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
            intents: vec![],
        };
        CapturedActionEffect {
            effect_hash: hash_effect(&effect),
            effect,
        }
    }

    fn sample_set_context_effect() -> ActionEffect {
        ActionEffect {
            kind: "set_context".to_string(),
            writes: vec![EffectWrite {
                key: "x".to_string(),
                value: Value::Number(1.0),
            }],
            intents: vec![],
        }
    }

    fn sample_legacy_effect() -> LegacyActionEffect {
        LegacyActionEffect {
            kind: "set_context".to_string(),
            writes: vec![EffectWrite {
                key: "x".to_string(),
                value: Value::Number(1.0),
            }],
        }
    }

    #[test]
    fn compare_decisions_matching_empty_effect_vectors_succeeds() {
        let captured = vec![make_record("e1", vec![])];
        let replayed = vec![make_record("e1", vec![])];
        assert!(compare_decisions(&captured, &replayed).unwrap());
    }

    #[test]
    fn compare_decisions_matching_effects_succeeds() {
        let eff = make_captured_effect("price", 42.0);
        let captured = vec![make_record("e1", vec![eff.clone()])];
        let replayed = vec![make_record("e1", vec![eff])];
        assert!(compare_decisions(&captured, &replayed).unwrap());
    }

    #[test]
    fn compare_decisions_corrupted_key_returns_effect_mismatch() {
        let cap_eff = make_captured_effect("price", 42.0);
        let rep_eff = make_captured_effect("corrupted_key", 42.0);
        // Hash will differ because the key differs
        let captured = vec![make_record("e1", vec![cap_eff])];
        let replayed = vec![make_record("e1", vec![rep_eff])];
        let err = compare_decisions(&captured, &replayed).unwrap_err();
        assert!(matches!(err, ReplayError::EffectMismatch { .. }));
    }

    #[test]
    fn compare_decisions_corrupted_value_returns_effect_mismatch() {
        let cap_eff = make_captured_effect("price", 42.0);
        let rep_eff = make_captured_effect("price", 99.0);
        let captured = vec![make_record("e1", vec![cap_eff])];
        let replayed = vec![make_record("e1", vec![rep_eff])];
        let err = compare_decisions(&captured, &replayed).unwrap_err();
        assert!(matches!(err, ReplayError::EffectMismatch { .. }));
    }

    #[test]
    fn compare_decisions_missing_effect_entry_returns_mismatch() {
        let eff = make_captured_effect("price", 42.0);
        let captured = vec![make_record("e1", vec![eff])];
        let replayed = vec![make_record("e1", vec![])];
        let err = compare_decisions(&captured, &replayed).unwrap_err();
        assert!(matches!(err, ReplayError::EffectMismatch { .. }));
    }

    #[test]
    fn compare_decisions_hash_tampered_returns_mismatch() {
        let mut cap_eff = make_captured_effect("price", 42.0);
        let rep_eff = make_captured_effect("price", 42.0);
        // Tamper with captured hash but leave effect content identical
        cap_eff.effect_hash =
            "0000000000000000000000000000000000000000000000000000000000000000".to_string();
        let captured = vec![make_record("e1", vec![cap_eff])];
        let replayed = vec![make_record("e1", vec![rep_eff])];
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
            intents: vec![],
        };
        let h1 = hash_effect(&effect);
        let h2 = hash_effect(&effect);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn hash_effect_matches_capture_serialization_path() {
        use sha2::{Digest, Sha256};
        let effect = sample_set_context_effect();
        let bytes = serde_json::to_vec(&effect).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let expected = hex::encode(hasher.finalize());
        assert_eq!(hash_effect(&effect), expected);
    }

    #[test]
    fn hash_effect_empty_intents_matches_legacy_two_field_bytes_and_golden() {
        use sha2::{Digest, Sha256};

        let effect = sample_set_context_effect();
        let legacy = sample_legacy_effect();

        let new_bytes = serde_json::to_vec(&effect).expect("new effect serialization must succeed");
        let new_json = std::str::from_utf8(&new_bytes).expect("serialized effect must be UTF-8");
        assert!(
            !new_json.contains("\"intents\""),
            "empty intents must be omitted for backward-compatible hashes"
        );

        let legacy_bytes =
            serde_json::to_vec(&legacy).expect("legacy effect serialization must succeed");
        assert_eq!(
            new_bytes, legacy_bytes,
            "empty intents serialization must match legacy two-field bytes"
        );

        let legacy_hash = hex::encode(Sha256::digest(&legacy_bytes));
        assert_eq!(hash_effect(&effect), legacy_hash);

        // Golden hash for {"kind":"set_context","writes":[{"key":"x","value":{"Number":1.0}}]}
        let expected = "55d64445c2438c7e5e64bd81f1a8e0d4ab3922df4a9f20f3344590178095fdb9";
        assert_eq!(legacy_hash, expected);
    }

    #[test]
    fn legacy_json_without_intents_roundtrip_preserves_hash_and_omits_intents() {
        use sha2::{Digest, Sha256};

        let legacy_json = r#"{"kind":"set_context","writes":[{"key":"x","value":{"Number":1.0}}]}"#;
        let effect: ActionEffect =
            serde_json::from_str(legacy_json).expect("legacy effect JSON must deserialize");
        assert!(
            effect.intents.is_empty(),
            "missing intents must default to empty vec"
        );

        let reserialized = serde_json::to_vec(&effect).expect("reserialization must succeed");
        let reserialized_json =
            std::str::from_utf8(&reserialized).expect("serialized effect must be UTF-8");
        assert!(
            !reserialized_json.contains("\"intents\""),
            "reserialized JSON must omit empty intents"
        );

        let input_hash = hex::encode(Sha256::digest(legacy_json.as_bytes()));
        let output_hash = hex::encode(Sha256::digest(&reserialized));
        assert_eq!(
            input_hash, output_hash,
            "legacy JSON hash must be stable across deserialize/reserialize"
        );
        assert_eq!(hash_effect(&effect), input_hash);
    }
}
