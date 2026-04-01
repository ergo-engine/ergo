//! replay tests
//!
//! Purpose:
//! - Lock the supervisor replay comparison/hash behavior in isolation.
//!
//! Owns:
//! - Scenario-heavy replay/effect-hash tests that would otherwise crowd the
//!   production replay module.
//!
//! Does not own:
//! - Production replay taxonomy or comparison implementation logic.
//!
//! Safety notes:
//! - These tests preserve the legacy empty-intents hash contract and effect
//!   comparison behavior.

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
        intent_acks: vec![],
        interruption: None,
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
    assert_eq!(h1.len(), 64);
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
