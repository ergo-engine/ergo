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
