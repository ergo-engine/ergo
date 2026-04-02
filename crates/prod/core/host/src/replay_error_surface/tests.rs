//! replay_error_surface tests
//!
//! Purpose:
//! - Lock the host replay-error descriptor table exposed by
//!   `replay_error_surface.rs`.
//!
//! Owns:
//! - Stable expectations for descriptor codes, messages, where/fix guidance,
//!   and detailed payload/effect diagnostics.
//!
//! Does not own:
//! - Replay semantics or host replay orchestration behavior.
//!
//! Safety notes:
//! - These tests intentionally fail on descriptor wording drift because CLI and
//!   other product renderers consume this table directly.

use super::*;
use crate::HostReplaySetupError;

use ergo_adapter::capture::CaptureError;
use ergo_adapter::EventId;
use ergo_adapter::ExternalEventPayloadError;
use ergo_runtime::common::{ActionEffect, EffectWrite, Value};
use ergo_supervisor::replay::{hash_effect, ReplayError};
use ergo_supervisor::CapturedActionEffect;
use std::collections::BTreeSet;

#[derive(Debug)]
struct ExpectedDescriptor<'a> {
    code: &'a str,
    message: String,
    rule_id: Option<&'a str>,
    where_field: Option<String>,
    fix: Option<&'a str>,
    details: Vec<String>,
}

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

fn assert_descriptor(descriptor: HostErrorDescriptor, expected: ExpectedDescriptor<'_>) {
    assert_eq!(descriptor.code, expected.code);
    assert_eq!(descriptor.message, expected.message);
    assert_eq!(descriptor.rule_id.as_deref(), expected.rule_id);
    assert_eq!(descriptor.where_field, expected.where_field);
    assert_eq!(descriptor.fix.as_deref(), expected.fix);
    assert_eq!(descriptor.details, expected.details);
}

#[test]
fn descriptor_contract_table_is_stable() {
    let required_summary = crate::AdapterDependencySummary {
        requires_adapter: true,
        required_context_nodes: vec!["src.required".to_string()],
        write_nodes: vec!["action.write".to_string()],
    };
    let cases = vec![
        (
            ExpectedDescriptor {
                code: "replay.unsupported_capture_version",
                message: "unsupported capture version 'v9'".to_string(),
                rule_id: None,
                where_field: Some("capture_version".to_string()),
                fix: Some("regenerate capture with a supported runtime version"),
                details: Vec::new(),
            },
            describe_replay_error(&ReplayError::UnsupportedVersion {
                capture_version: "v9".to_string(),
            }),
        ),
        (
            ExpectedDescriptor {
                code: "replay.hash_mismatch",
                message: "payload hash mismatch for event 'evt-hash'".to_string(),
                rule_id: None,
                where_field: Some("event 'evt-hash'".to_string()),
                fix: Some("re-run canonical capture to produce an uncorrupted bundle"),
                details: Vec::new(),
            },
            describe_replay_error(&ReplayError::HashMismatch {
                event_id: EventId::new("evt-hash"),
            }),
        ),
        (
            ExpectedDescriptor {
                code: "replay.invalid_payload",
                message: "invalid payload for event 'evt-payload'".to_string(),
                rule_id: None,
                where_field: Some("event 'evt-payload'".to_string()),
                fix: Some(
                    "re-capture with object payloads or repair the capture bundle payload bytes",
                ),
                details: vec!["payload bytes are not valid JSON: bad json".to_string()],
            },
            describe_replay_error(&ReplayError::InvalidPayload {
                event_id: EventId::new("evt-payload"),
                source: ExternalEventPayloadError::InvalidJson {
                    detail: "bad json".to_string(),
                },
            }),
        ),
        (
            ExpectedDescriptor {
                code: "replay.adapter_provenance_mismatch",
                message: "adapter provenance mismatch".to_string(),
                rule_id: None,
                where_field: Some("capture provenance vs replay adapter".to_string()),
                fix: Some("replay with the adapter used to produce the capture"),
                details: vec![
                    "expected: 'adapter:a'".to_string(),
                    "got: 'adapter:b'".to_string(),
                ],
            },
            describe_replay_error(&ReplayError::AdapterProvenanceMismatch {
                expected: "adapter:a".to_string(),
                got: "adapter:b".to_string(),
            }),
        ),
        (
            ExpectedDescriptor {
                code: "replay.runtime_provenance_mismatch",
                message: "runtime provenance mismatch".to_string(),
                rule_id: None,
                where_field: Some("capture provenance vs replay runtime surface".to_string()),
                fix: Some(
                    "replay against the graph/runtime used to produce the capture or recapture",
                ),
                details: vec!["expected: 'rpv1:a'".to_string(), "got: 'rpv1:b'".to_string()],
            },
            describe_replay_error(&ReplayError::RuntimeProvenanceMismatch {
                expected: "rpv1:a".to_string(),
                got: "rpv1:b".to_string(),
            }),
        ),
        (
            ExpectedDescriptor {
                code: "replay.unexpected_adapter",
                message: "bundle provenance is 'none'; adapter must not be provided".to_string(),
                rule_id: None,
                where_field: Some("replay option '--adapter'".to_string()),
                fix: Some("remove --adapter and replay without adapter"),
                details: Vec::new(),
            },
            describe_replay_error(&ReplayError::UnexpectedAdapterProvidedForNoAdapterCapture),
        ),
        (
            ExpectedDescriptor {
                code: "replay.adapter_required",
                message: "bundle is adapter-provenanced; adapter is required".to_string(),
                rule_id: None,
                where_field: Some("replay option '--adapter'".to_string()),
                fix: Some("provide --adapter <adapter.yaml> that matches capture provenance"),
                details: Vec::new(),
            },
            describe_replay_error(&ReplayError::AdapterRequiredForProvenancedCapture),
        ),
        (
            ExpectedDescriptor {
                code: "replay.duplicate_event_id",
                message: "duplicate event_id 'evt-dup' in strict replay capture input"
                    .to_string(),
                rule_id: None,
                where_field: Some("capture event 'evt-dup'".to_string()),
                fix: Some(
                    "regenerate capture with unique event ids or repair the capture artifact",
                ),
                details: Vec::new(),
            },
            describe_replay_error(&ReplayError::DuplicateEventId {
                event_id: EventId::new("evt-dup"),
            }),
        ),
        (
            ExpectedDescriptor {
                code: "replay.effect_mismatch",
                message: "effect mismatch at index 2 for event 'evt-effect': content mismatch"
                    .to_string(),
                rule_id: None,
                where_field: Some("event 'evt-effect' effect[2]".to_string()),
                fix: Some("inspect action effect drift and regenerate capture if needed"),
                details: Vec::new(),
            },
            describe_replay_error(&ReplayError::EffectMismatch {
                event_id: EventId::new("evt-effect"),
                effect_index: 2,
                expected: None,
                actual: None,
                detail: "content mismatch".to_string(),
            }),
        ),
        (
            ExpectedDescriptor {
                code: "replay.event_rehydrate_failed",
                message: "event 'evt-rh' failed rehydration during replay".to_string(),
                rule_id: None,
                where_field: Some("event 'evt-rh'".to_string()),
                fix: Some("inspect capture payload/hash integrity and recapture if needed"),
                details: vec![String::from(
                    "payload hash mismatch (expected 'expected-hash', actual 'actual-hash')",
                )],
            },
            describe_host_replay_error(&HostReplayError::Hosted(
                HostedReplayError::EventRehydrate {
                    event_id: "evt-rh".to_string(),
                    source: CaptureError::PayloadHashMismatch {
                        expected: "expected-hash".to_string(),
                        actual: "actual-hash".to_string(),
                    },
                },
            )),
        ),
        (
            ExpectedDescriptor {
                code: "replay.host_step_failed",
                message: "host replay step failed".to_string(),
                rule_id: None,
                where_field: Some("ergo-host replay lifecycle".to_string()),
                fix: Some("inspect host lifecycle/effect handler failures and retry"),
                details: vec!["host lifecycle violation: runner already finalized".to_string()],
            },
            describe_host_replay_error(&HostReplayError::Hosted(HostedReplayError::Step(
                crate::HostedStepError::LifecycleViolation {
                    detail: "runner already finalized".to_string(),
                },
            ))),
        ),
        (
            ExpectedDescriptor {
                code: "replay.decision_mismatch",
                message: "replay decisions do not match capture decisions".to_string(),
                rule_id: None,
                where_field: Some("decision stream comparison".to_string()),
                fix: Some("inspect runtime/adapter drift and regenerate capture if needed"),
                details: Vec::new(),
            },
            describe_host_replay_error(&HostReplayError::Hosted(
                HostedReplayError::DecisionMismatch,
            )),
        ),
        (
            ExpectedDescriptor {
                code: "replay.graph_id_mismatch",
                message: "graph_id mismatch".to_string(),
                rule_id: None,
                where_field: Some("capture graph_id 'graph.b' vs replay graph 'graph.a'".to_string()),
                fix: Some("replay with --graph matching the original capture graph"),
                details: vec!["expected: 'graph.a'".to_string(), "got: 'graph.b'".to_string()],
            },
            describe_host_replay_error(&HostReplayError::GraphIdMismatch {
                expected: "graph.a".to_string(),
                got: "graph.b".to_string(),
            }),
        ),
        (
            ExpectedDescriptor {
                code: "replay.external_effect_kind_unrepresentable",
                message:
                    "capture contains external effect kinds not representable by replay graph ownership"
                        .to_string(),
                rule_id: None,
                where_field: Some("replay ownership preflight".to_string()),
                fix: Some("replay with the matching graph/adapter pair used during capture"),
                details: vec!["missing kinds: place_order".to_string()],
            },
            describe_host_replay_error(&HostReplayError::ExternalKindsNotRepresentable {
                missing: vec!["place_order".to_string()],
            }),
        ),
        (
            ExpectedDescriptor {
                code: "replay.host_setup_failed",
                message: "host replay setup failed".to_string(),
                rule_id: None,
                where_field: Some("ergo-host replay setup".to_string()),
                fix: Some("verify capture/graph/adapter paths and retry"),
                details: vec!["replay does not accept live egress configuration".to_string()],
            },
            describe_host_replay_error(&HostReplayError::Setup(
                HostReplaySetupError::LiveEgressConfigurationNotAllowed,
            )),
        ),
        (
            ExpectedDescriptor {
                code: "adapter.required_for_graph",
                message: "graph requires adapter capabilities but no --adapter was provided"
                    .to_string(),
                rule_id: Some("RUN-CANON-2"),
                where_field: Some("node 'src.required'".to_string()),
                fix: Some("provide --adapter <adapter.yaml> for canonical run"),
                details: vec![
                    "required source context at node(s): src.required".to_string(),
                    "action writes at node(s): action.write".to_string(),
                ],
            },
            describe_adapter_required(&required_summary),
        ),
    ];

    let expected_codes = BTreeSet::from([
        "adapter.required_for_graph".to_string(),
        "replay.adapter_provenance_mismatch".to_string(),
        "replay.adapter_required".to_string(),
        "replay.decision_mismatch".to_string(),
        "replay.duplicate_event_id".to_string(),
        "replay.effect_mismatch".to_string(),
        "replay.event_rehydrate_failed".to_string(),
        "replay.external_effect_kind_unrepresentable".to_string(),
        "replay.graph_id_mismatch".to_string(),
        "replay.hash_mismatch".to_string(),
        "replay.host_setup_failed".to_string(),
        "replay.host_step_failed".to_string(),
        "replay.invalid_payload".to_string(),
        "replay.runtime_provenance_mismatch".to_string(),
        "replay.unexpected_adapter".to_string(),
        "replay.unsupported_capture_version".to_string(),
    ]);
    let actual_codes = cases
        .iter()
        .map(|(_, descriptor)| descriptor.code.clone())
        .collect::<BTreeSet<_>>();

    assert_eq!(cases.len(), 16);
    assert_eq!(actual_codes, expected_codes);

    for (expected, descriptor) in cases {
        assert_descriptor(descriptor, expected);
    }
}

#[test]
fn effect_mismatch_descriptor_includes_serialized_expected_and_actual_effects() {
    let descriptor = describe_replay_error(&ReplayError::EffectMismatch {
        event_id: EventId::new("evt-1"),
        effect_index: 0,
        expected: Some(captured_effect("price", 42.0)),
        actual: Some(captured_effect("volume", 99.0)),
        detail: "content mismatch".to_string(),
    });

    assert_eq!(descriptor.code, "replay.effect_mismatch");
    assert_eq!(
        descriptor.where_field.as_deref(),
        Some("event 'evt-1' effect[0]")
    );
    assert_eq!(
        descriptor.fix.as_deref(),
        Some("inspect action effect drift and regenerate capture if needed")
    );
    assert!(descriptor
        .details
        .iter()
        .any(|detail| detail.contains("expected:") && detail.contains("\"price\"")));
    assert!(descriptor
        .details
        .iter()
        .any(|detail| detail.contains("actual:") && detail.contains("\"volume\"")));
}

#[test]
fn hosted_replay_helper_maps_step_failures_to_host_descriptor() {
    let descriptor = describe_hosted_replay_error(&HostedReplayError::Step(
        crate::HostedStepError::LifecycleViolation {
            detail: "runner already finalized".to_string(),
        },
    ));

    assert_eq!(descriptor.code, "replay.host_step_failed");
    assert_eq!(
        descriptor.where_field.as_deref(),
        Some("ergo-host replay lifecycle")
    );
    assert_eq!(
        descriptor.fix.as_deref(),
        Some("inspect host lifecycle/effect handler failures and retry")
    );
    assert_eq!(
        descriptor.details,
        vec!["host lifecycle violation: runner already finalized".to_string()]
    );
}

#[test]
fn host_replay_error_delegates_hosted_variants_through_private_helper() {
    let descriptor = describe_host_replay_error(&HostReplayError::Hosted(
        HostedReplayError::DecisionMismatch,
    ));

    assert_eq!(descriptor.code, "replay.decision_mismatch");
    assert_eq!(
        descriptor.where_field.as_deref(),
        Some("decision stream comparison")
    );
    assert_eq!(
        descriptor.fix.as_deref(),
        Some("inspect runtime/adapter drift and regenerate capture if needed")
    );
}

#[test]
fn hosted_preflight_and_compare_currently_collapse_to_the_same_descriptor() {
    let preflight = describe_host_replay_error(&HostReplayError::Hosted(
        HostedReplayError::Preflight(ReplayError::HashMismatch {
            event_id: EventId::new("evt-phase"),
        }),
    ));
    let compare = describe_host_replay_error(&HostReplayError::Hosted(HostedReplayError::Compare(
        ReplayError::HashMismatch {
            event_id: EventId::new("evt-phase"),
        },
    )));

    assert_eq!(preflight.code, compare.code);
    assert_eq!(preflight.message, compare.message);
    assert_eq!(preflight.where_field, compare.where_field);
    assert_eq!(preflight.fix, compare.fix);
    assert_eq!(preflight.details, compare.details);
}

#[test]
fn adapter_required_descriptor_carries_run_canon_2_rule_and_node_details() {
    let descriptor = describe_adapter_required(&crate::AdapterDependencySummary {
        requires_adapter: true,
        required_context_nodes: vec!["src.required".to_string()],
        write_nodes: vec!["action.write".to_string()],
    });

    assert_eq!(descriptor.code, "adapter.required_for_graph");
    assert_eq!(descriptor.rule_id.as_deref(), Some("RUN-CANON-2"));
    assert_eq!(
        descriptor.where_field.as_deref(),
        Some("node 'src.required'")
    );
    assert!(descriptor.details.iter().any(
        |detail| detail.contains("required source context") && detail.contains("src.required")
    ));
    assert!(descriptor
        .details
        .iter()
        .any(|detail| detail.contains("action writes") && detail.contains("action.write")));
}

#[test]
fn adapter_required_descriptor_falls_back_to_dependency_scan_when_summary_is_empty() {
    let descriptor = describe_adapter_required(&crate::AdapterDependencySummary::default());

    assert_eq!(descriptor.rule_id.as_deref(), Some("RUN-CANON-2"));
    assert_eq!(
        descriptor.where_field.as_deref(),
        Some("graph dependency scan")
    );
    assert!(descriptor.details.is_empty());
}
