//! usecases facade contract tests
//!
//! Purpose:
//! - Lock the public type and error contracts defined in `usecases.rs`.
//!
//! Owns:
//! - Display, code, and error-chain expectations for the facade-level host types consumed by CLI
//!   and SDK.

use super::*;
use std::io;

#[test]
fn host_run_error_display_contract_is_locked() {
    let summary = AdapterDependencySummary {
        requires_adapter: true,
        required_context_nodes: vec!["src_ctx".to_string()],
        write_nodes: vec!["act_write".to_string()],
    };

    let cases = vec![
        (
            HostRunError::AdapterRequired(summary),
            "graph requires adapter capabilities but no adapter was provided (required context nodes: [src_ctx], write nodes: [act_write])",
        ),
        (
            HostRunError::ProductionRequiresAdapter,
            "production session requires an adapter contract but no adapter was provided",
        ),
        (
            HostRunError::Setup(HostSetupError::DependencyScan(
                HostDependencyScanError::MissingActionPrimitive {
                    primitive_id: "bad_action".to_string(),
                },
            )),
            "action 'bad_action' missing in core registry",
        ),
        (
            HostRunError::Driver(HostDriverError::Start(HostDriverStartError::new(
                "spawn failed",
            ))),
            "spawn failed",
        ),
        (
            HostRunError::Driver(HostDriverError::Protocol(
                HostDriverProtocolError::new("bad frame"),
            )),
            "bad frame",
        ),
        (
            HostRunError::Driver(HostDriverError::Io(HostDriverIoError::new(
                "pipe closed",
            ))),
            "pipe closed",
        ),
        (
            HostRunError::Step(HostedStepError::MissingPayload),
            "payload is required in adapter-bound mode",
        ),
        (
            HostRunError::CaptureWrite(CaptureWriteError::CreateOutputDirectory {
                path: PathBuf::from("capture"),
                source: io::Error::other("write capture"),
            }),
            "create capture output directory: write capture",
        ),
    ];

    for (err, expected) in cases {
        assert_eq!(err.to_string(), expected);
    }
}

#[test]
fn host_replay_error_display_contract_is_locked() {
    let cases = vec![
        (
            HostReplayError::Hosted(HostedReplayError::Step(HostedStepError::MissingPayload)),
            "host replay step failed: payload is required in adapter-bound mode",
        ),
        (
            HostReplayError::GraphIdMismatch {
                expected: "expected_graph".to_string(),
                got: "got_graph".to_string(),
            },
            "graph_id mismatch (expected 'expected_graph', got 'got_graph')",
        ),
        (
            HostReplayError::ExternalKindsNotRepresentable {
                missing: vec!["place_order".to_string(), "send_email".to_string()],
            },
            "capture includes external effect kinds not representable by replay graph ownership surface: [place_order, send_email]",
        ),
        (
            HostReplayError::Setup(HostReplaySetupError::LiveEgressConfigurationNotAllowed),
            "replay does not accept live egress configuration",
        ),
    ];

    for (err, expected) in cases {
        assert_eq!(err.to_string(), expected);
    }
}

#[test]
fn host_replay_error_source_contract_is_locked() {
    let hosted = HostReplayError::Hosted(HostedReplayError::Step(HostedStepError::MissingPayload));
    let hosted_source = std::error::Error::source(&hosted).expect("hosted replay error must chain");
    assert_eq!(
        hosted_source.to_string(),
        "host replay step failed: payload is required in adapter-bound mode"
    );
    let nested_source =
        std::error::Error::source(hosted_source).expect("HostedReplayError::Step must chain");
    assert_eq!(
        nested_source.to_string(),
        "payload is required in adapter-bound mode"
    );

    let graph_id = HostReplayError::GraphIdMismatch {
        expected: "a".to_string(),
        got: "b".to_string(),
    };
    assert!(std::error::Error::source(&graph_id).is_none());

    let external = HostReplayError::ExternalKindsNotRepresentable {
        missing: vec!["place_order".to_string()],
    };
    assert!(std::error::Error::source(&external).is_none());

    let setup = HostReplayError::Setup(HostReplaySetupError::LiveEgressConfigurationNotAllowed);
    let setup_source = std::error::Error::source(&setup).expect("setup should chain");
    assert_eq!(
        setup_source.to_string(),
        "replay does not accept live egress configuration"
    );
    assert!(std::error::Error::source(setup_source).is_none());
}

#[test]
fn hosted_replay_error_converts_into_host_replay_error() {
    let hosted = HostedReplayError::DecisionMismatch;
    let wrapped: HostReplayError = hosted.into();

    match wrapped {
        HostReplayError::Hosted(inner) => {
            assert_eq!(
                inner.to_string(),
                "replay decisions do not match captured decisions"
            );
        }
        other => panic!("expected hosted replay error wrapper, got {other:?}"),
    }
}

#[test]
fn interruption_reason_code_and_display_contract_is_locked() {
    let cases = vec![
        (InterruptionReason::HostStopRequested, "host_stop_requested"),
        (InterruptionReason::DriverTerminated, "driver_terminated"),
        (InterruptionReason::ProtocolViolation, "protocol_violation"),
        (InterruptionReason::DriverIo, "driver_io"),
        (
            InterruptionReason::EgressAckTimeout {
                channel: "broker".to_string(),
                intent_id: "intent-1".to_string(),
            },
            "egress_ack_timeout",
        ),
        (
            InterruptionReason::EgressProtocolViolation {
                channel: "broker".to_string(),
            },
            "egress_protocol_violation",
        ),
        (
            InterruptionReason::EgressIo {
                channel: "broker".to_string(),
            },
            "egress_io",
        ),
    ];

    for (reason, expected) in cases {
        assert_eq!(reason.code(), expected);
        assert_eq!(reason.to_string(), expected);
    }
}

#[test]
fn interruption_from_egress_dispatch_failure_contract_is_locked() {
    let ack_timeout =
        interruption_from_egress_dispatch_failure(EgressDispatchFailure::AckTimeout {
            channel: "broker".to_string(),
            intent_id: "intent-1".to_string(),
        });
    assert_eq!(
        ack_timeout,
        InterruptionReason::EgressAckTimeout {
            channel: "broker".to_string(),
            intent_id: "intent-1".to_string(),
        }
    );

    let protocol =
        interruption_from_egress_dispatch_failure(EgressDispatchFailure::ProtocolViolation {
            channel: "broker".to_string(),
            detail: "unexpected frame".to_string(),
        });
    assert_eq!(
        protocol,
        InterruptionReason::EgressProtocolViolation {
            channel: "broker".to_string(),
        }
    );

    let io = interruption_from_egress_dispatch_failure(EgressDispatchFailure::Io {
        channel: "broker".to_string(),
        detail: "pipe closed".to_string(),
    });
    assert_eq!(
        io,
        InterruptionReason::EgressIo {
            channel: "broker".to_string(),
        }
    );
}
