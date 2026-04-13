//! process_driver unit tests
//!
//! Purpose:
//! - Lock the private process-driver protocol contract owned by
//!   `process_driver.rs`.
//!
//! Owns:
//! - Exact JSON wire-shape tests for the v0 process-ingress frames and the
//!   host-local protocol-version constant.
//!
//! Does not own:
//! - Broader canonical run behavior, stop semantics, or capture-shape coverage
//!   already exercised by `usecases/tests/process_driver.rs`.

use super::*;
use crate::PROCESS_DRIVER_PROTOCOL_VERSION;
use ergo_adapter::{EventTime, ExternalEventKind};

#[test]
fn process_driver_protocol_version_constant_is_locked() {
    assert_eq!(PROCESS_DRIVER_PROTOCOL_VERSION, "ergo-driver.v0");
}

#[test]
fn process_driver_hello_wire_shape_is_stable() {
    let message = ProcessDriverMessage::Hello {
        protocol: PROCESS_DRIVER_PROTOCOL_VERSION.to_string(),
    };

    assert_eq!(
        serde_json::to_string(&message).expect("hello must serialize"),
        r#"{"type":"hello","protocol":"ergo-driver.v0"}"#
    );
}

#[test]
fn process_driver_event_wire_shape_is_stable() {
    let message = ProcessDriverMessage::Event {
        event: HostedEvent {
            event_id: "evt-1".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: Some("price_bar".to_string()),
            payload: Some(serde_json::json!({"close": 101.25})),
        },
    };

    assert_eq!(
        serde_json::to_string(&message).expect("event frame must serialize"),
        r#"{"type":"event","event":{"event_id":"evt-1","kind":"Command","at":{"secs":0,"nanos":0},"semantic_kind":"price_bar","payload":{"close":101.25}}}"#
    );
}

#[test]
fn process_driver_end_wire_shape_is_stable() {
    assert_eq!(
        serde_json::to_string(&ProcessDriverMessage::End).expect("end frame must serialize"),
        r#"{"type":"end"}"#
    );
}

#[test]
fn process_driver_progress_tracks_first_committed_step_explicitly() {
    let mut progress = ProcessDriverProgress::new_v0();

    assert_eq!(
        progress.commit_phase(),
        ProcessDriverCommitPhase::BeforeFirstCommittedStep
    );

    progress.record_interrupted_event();
    assert_eq!(
        progress.commit_phase(),
        ProcessDriverCommitPhase::BeforeFirstCommittedStep
    );

    progress.record_committed_event();
    assert_eq!(
        progress.commit_phase(),
        ProcessDriverCommitPhase::AfterFirstCommittedStep
    );
}

#[test]
fn process_driver_v0_episode_ledger_materializes_one_episode_only_when_events_exist() {
    assert!(ProcessDriverEpisodeLedger::new_v0()
        .into_episode_event_counts()
        .is_empty());

    let mut ledger = ProcessDriverEpisodeLedger::new_v0();
    ledger.record_event();
    ledger.record_event();

    assert_eq!(
        ledger.into_episode_event_counts(),
        vec![("E1".to_string(), 2)]
    );
}
