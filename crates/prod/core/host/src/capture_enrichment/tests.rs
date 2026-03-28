//! capture_enrichment::tests
//!
//! Purpose:
//! - Keep capture-enrichment contract tests out of the production module while
//!   locking the decision-indexed overwrite, trailing-preservation, and
//!   sparse-gap defaulting behavior that hosted-runner finalization depends on.

use super::*;

use ergo_adapter::{EventId, GraphId, RunTermination};
use ergo_runtime::common::{EffectWrite, Value};
use ergo_supervisor::replay::hash_effect;
use ergo_supervisor::{
    CaptureBundle, CapturedActionEffect, Constraints, Decision, EpisodeId, EpisodeInvocationRecord,
    NO_ADAPTER_PROVENANCE,
};

fn sample_effect(key: &str, value: f64) -> ActionEffect {
    ActionEffect {
        kind: "set_context".to_string(),
        writes: vec![EffectWrite {
            key: key.to_string(),
            value: Value::Number(value),
        }],
        intents: vec![],
    }
}

fn captured_effect(key: &str, value: f64) -> CapturedActionEffect {
    let effect = sample_effect(key, value);
    CapturedActionEffect {
        effect_hash: hash_effect(&effect),
        effect,
    }
}

fn sample_ack(intent_id: &str, channel: &str) -> CapturedIntentAck {
    CapturedIntentAck {
        intent_id: intent_id.to_string(),
        channel: channel.to_string(),
        status: "accepted".to_string(),
        acceptance: "durable".to_string(),
        egress_ref: Some(format!("ref-{intent_id}")),
    }
}

fn decision_record(event_id: &str) -> EpisodeInvocationRecord {
    EpisodeInvocationRecord {
        event_id: EventId::new(event_id),
        decision: Decision::Invoke,
        schedule_at: None,
        episode_id: EpisodeId::new(0),
        deadline: None,
        termination: Some(RunTermination::Completed),
        retry_count: 0,
        effects: vec![],
        intent_acks: vec![],
        interruption: None,
    }
}

fn bundle(decisions: Vec<EpisodeInvocationRecord>) -> CaptureBundle {
    CaptureBundle {
        capture_version: "v3".to_string(),
        graph_id: GraphId::new("capture_enrichment_test"),
        config: Constraints::default(),
        events: vec![],
        decisions,
        adapter_provenance: NO_ADAPTER_PROVENANCE.to_string(),
        runtime_provenance: "runtime:test".to_string(),
        egress_provenance: None,
    }
}

#[test]
fn enrichment_writes_effects_acks_and_interruptions_by_decision_index() {
    let mut first = decision_record("evt-shared");
    first.effects = vec![captured_effect("supervisor-first", 0.0)];
    first.intent_acks = vec![sample_ack("old-0", "old-channel-0")];
    first.interruption = Some("old interruption 0".to_string());

    let mut second = decision_record("evt-shared");
    second.effects = vec![captured_effect("supervisor-second", 0.0)];
    second.intent_acks = vec![sample_ack("old-1", "old-channel-1")];
    second.interruption = Some("old interruption 1".to_string());

    let mut bundle = bundle(vec![first, second]);
    let first_effect = sample_effect("first", 1.0);
    let second_effect = sample_effect("second", 2.0);
    let first_ack = sample_ack("intent-0", "email");
    let second_ack = sample_ack("intent-1", "sms");
    let first_interruption = "egress dispatch failed: ack timeout".to_string();
    let second_interruption = "host stop requested".to_string();

    enrich_bundle_with_host_artifacts(
        &mut bundle,
        &[vec![first_effect.clone()], vec![second_effect.clone()]],
        &[vec![first_ack.clone()], vec![second_ack.clone()]],
        &[
            Some(first_interruption.clone()),
            Some(second_interruption.clone()),
        ],
    );

    assert_eq!(
        bundle.decisions[0].effects,
        vec![CapturedActionEffect {
            effect_hash: hash_effect(&first_effect),
            effect: first_effect,
        }]
    );
    assert_eq!(
        bundle.decisions[1].effects,
        vec![CapturedActionEffect {
            effect_hash: hash_effect(&second_effect),
            effect: second_effect,
        }]
    );
    assert_eq!(bundle.decisions[0].intent_acks, vec![first_ack]);
    assert_eq!(bundle.decisions[1].intent_acks, vec![second_ack]);
    assert_eq!(bundle.decisions[0].interruption, Some(first_interruption));
    assert_eq!(bundle.decisions[1].interruption, Some(second_interruption));
}

#[test]
fn enrichment_leaves_short_sidecars_untouched_and_ignores_extra_entries() {
    let mut first = decision_record("evt-0");
    first.effects = vec![captured_effect("existing-0", 0.0)];
    first.intent_acks = vec![sample_ack("existing-ack-0", "existing-channel-0")];
    first.interruption = Some("existing interruption 0".to_string());

    let mut second = decision_record("evt-1");
    second.effects = vec![captured_effect("existing-1", 1.0)];
    second.intent_acks = vec![sample_ack("existing-ack-1", "existing-channel-1")];
    second.interruption = Some("existing interruption 1".to_string());

    let mut bundle = bundle(vec![first, second]);
    let replacement_effect = sample_effect("replacement", 5.0);
    let first_ack = sample_ack("ack-0", "email");
    let second_ack = sample_ack("ack-1", "sms");

    enrich_bundle_with_host_artifacts(
        &mut bundle,
        &[vec![replacement_effect.clone()]],
        &[
            vec![first_ack.clone()],
            vec![second_ack.clone()],
            vec![sample_ack("ignored", "ignored")],
        ],
        &[],
    );

    assert_eq!(
        bundle.decisions[0].effects,
        vec![CapturedActionEffect {
            effect_hash: hash_effect(&replacement_effect),
            effect: replacement_effect,
        }]
    );
    assert_eq!(
        bundle.decisions[1].effects,
        vec![captured_effect("existing-1", 1.0)]
    );
    assert_eq!(bundle.decisions[0].intent_acks, vec![first_ack]);
    assert_eq!(bundle.decisions[1].intent_acks, vec![second_ack]);
    assert_eq!(
        bundle.decisions[0].interruption,
        Some("existing interruption 0".to_string())
    );
    assert_eq!(
        bundle.decisions[1].interruption,
        Some("existing interruption 1".to_string())
    );
}

#[test]
fn sparse_recorders_clear_gap_slots_back_to_defaults_during_finalization() {
    let mut effects_by_decision = AppliedEffectsByDecision::default();
    let mut intent_acks_by_decision = AppliedIntentAcksByDecision::default();
    let mut interruptions_by_decision = StepInterruptionsByDecision::default();

    let terminal_effect = sample_effect("terminal", 9.0);
    let terminal_ack = sample_ack("terminal-ack", "terminal-channel");
    let terminal_interruption = "terminal interruption".to_string();

    effects_by_decision.record(2, vec![terminal_effect.clone()]);
    intent_acks_by_decision.record(2, vec![terminal_ack.clone()]);
    interruptions_by_decision.record(2, terminal_interruption.clone());

    let mut first = decision_record("evt-0");
    first.effects = vec![captured_effect("existing-0", 0.0)];
    first.intent_acks = vec![sample_ack("existing-ack-0", "existing-channel-0")];
    first.interruption = Some("existing interruption 0".to_string());

    let mut second = decision_record("evt-1");
    second.effects = vec![captured_effect("existing-1", 1.0)];
    second.intent_acks = vec![sample_ack("existing-ack-1", "existing-channel-1")];
    second.interruption = Some("existing interruption 1".to_string());

    let mut third = decision_record("evt-2");
    third.effects = vec![captured_effect("existing-2", 2.0)];
    third.intent_acks = vec![sample_ack("existing-ack-2", "existing-channel-2")];
    third.interruption = Some("existing interruption 2".to_string());

    let mut bundle = bundle(vec![first, second, third]);
    enrich_bundle_with_host_artifacts(
        &mut bundle,
        effects_by_decision.effects(),
        intent_acks_by_decision.intent_acks(),
        interruptions_by_decision.interruptions(),
    );

    assert!(bundle.decisions[0].effects.is_empty());
    assert!(bundle.decisions[1].effects.is_empty());
    assert!(bundle.decisions[0].intent_acks.is_empty());
    assert!(bundle.decisions[1].intent_acks.is_empty());
    assert_eq!(bundle.decisions[0].interruption, None);
    assert_eq!(bundle.decisions[1].interruption, None);
    assert_eq!(
        bundle.decisions[2].effects,
        vec![CapturedActionEffect {
            effect_hash: hash_effect(&terminal_effect),
            effect: terminal_effect,
        }]
    );
    assert_eq!(bundle.decisions[2].intent_acks, vec![terminal_ack]);
    assert_eq!(
        bundle.decisions[2].interruption,
        Some(terminal_interruption)
    );
}

#[test]
fn applied_effects_record_is_sparse_and_last_write_wins() {
    let mut recorded = AppliedEffectsByDecision::default();
    let first = vec![sample_effect("first", 1.0)];
    let second = vec![sample_effect("second", 2.0)];

    recorded.record(2, first);
    assert!(recorded.effects()[0].is_empty());
    assert!(recorded.effects()[1].is_empty());

    recorded.record(2, second.clone());
    assert_eq!(recorded.effects()[2], second);
}

#[test]
fn applied_intent_acks_record_is_sparse_and_last_write_wins() {
    let mut recorded = AppliedIntentAcksByDecision::default();
    let first = vec![sample_ack("first", "email")];
    let second = vec![sample_ack("second", "sms")];

    recorded.record(2, first);
    assert!(recorded.intent_acks()[0].is_empty());
    assert!(recorded.intent_acks()[1].is_empty());

    recorded.record(2, second.clone());
    assert_eq!(recorded.intent_acks()[2], second);
}

#[test]
fn step_interruptions_record_is_sparse_and_last_write_wins() {
    let mut recorded = StepInterruptionsByDecision::default();

    recorded.record(2, "first".to_string());
    assert_eq!(recorded.interruptions()[0], None);
    assert_eq!(recorded.interruptions()[1], None);

    recorded.record(2, "second".to_string());
    assert_eq!(recorded.interruptions()[2], Some("second".to_string()));
}
