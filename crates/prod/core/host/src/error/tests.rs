//! error::tests
//!
//! Purpose:
//! - Keep contract-focused tests for the host step-error taxonomy out of the
//!   production module while locking the public display and error-chaining
//!   behavior this file exposes.

use super::*;

use crate::egress::{EgressProcessError, EgressValidationError};
use ergo_adapter::{EventBindingError, ExternalEventPayloadError};
use std::error::Error as _;
use std::time::Duration;

use crate::host::{EffectApplyError, HandlerCoverageError};

#[test]
fn egress_dispatch_failure_channel_accessor_and_display_are_stable() {
    let ack_timeout = EgressDispatchFailure::AckTimeout {
        channel: "broker".to_string(),
        intent_id: "intent-123".to_string(),
    };
    assert_eq!(ack_timeout.channel(), "broker");
    assert_eq!(
        ack_timeout.to_string(),
        "ack timeout on channel 'broker' for intent 'intent-123'"
    );
    assert!(ack_timeout.source().is_none());

    let protocol = EgressDispatchFailure::ProtocolViolation {
        channel: "broker".to_string(),
        detail: "unexpected ack frame".to_string(),
    };
    assert_eq!(protocol.channel(), "broker");
    assert_eq!(
        protocol.to_string(),
        "protocol violation on channel 'broker': unexpected ack frame"
    );
    assert!(protocol.source().is_none());

    let io = EgressDispatchFailure::Io {
        channel: "broker".to_string(),
        detail: "broken pipe".to_string(),
    };
    assert_eq!(io.channel(), "broker");
    assert_eq!(
        io.to_string(),
        "I/O failure on channel 'broker': broken pipe"
    );
    assert!(io.source().is_none());
}

#[test]
fn hosted_step_error_display_contracts_are_stable() {
    let cases = [
        (
            HostedStepError::DuplicateEventId {
                event_id: "evt_dup".to_string(),
            },
            "duplicate event_id 'evt_dup' in canonical host runner",
        ),
        (
            HostedStepError::MissingSemanticKind,
            "semantic_kind is required in adapter-bound mode",
        ),
        (
            HostedStepError::MissingPayload,
            "payload is required in adapter-bound mode",
        ),
        (
            HostedStepError::PayloadMustBeObject,
            "payload must be a JSON object",
        ),
        (
            HostedStepError::UnknownSemanticKind {
                kind: "command.place_order".to_string(),
            },
            "unknown semantic event kind 'command.place_order'",
        ),
        (
            HostedStepError::Binding(EventBindingError::PayloadSchemaMismatch {
                kind: "command.place_order".to_string(),
                detail: "binder rejected payload".to_string(),
            }),
            "semantic event binding failed: payload does not match schema for semantic event kind 'command.place_order': binder rejected payload",
        ),
        (
            HostedStepError::EventBuild(HostedEventBuildError::InvalidPayload(
                ExternalEventPayloadError::PayloadMustBeJsonObject {
                    got: "string".to_string(),
                },
            )),
            "event build failed: payload must be a JSON object, got string",
        ),
        (
            HostedStepError::LifecycleViolation {
                detail: "runner already finished".to_string(),
            },
            "host lifecycle violation: runner already finished",
        ),
        (
            HostedStepError::MissingDecisionEntry,
            "missing decision log entry for the completed host step",
        ),
        (
            HostedStepError::EffectApply(EffectApplyError::UnhandledEffectKind {
                kind: "place_order".to_string(),
            }),
            "effect application failed: no registered effect handler for kind 'place_order'",
        ),
        (
            HostedStepError::HandlerCoverage(HandlerCoverageError::MissingHandler {
                effect_kind: "place_order".to_string(),
            }),
            "handler coverage failed: missing effect handler for kind 'place_order'",
        ),
        (
            HostedStepError::EgressValidation(
                HostedEgressValidationError::EgressConfigRequiresAdapterBoundMode,
            ),
            "egress configuration validation failed: egress configuration requires adapter-bound mode",
        ),
        (
            HostedStepError::EgressProcess(EgressProcessError::Startup {
                channel: "broker".to_string(),
                detail: "spawn failed".to_string(),
            }),
            "egress lifecycle failure: egress channel 'broker' startup failed: spawn failed",
        ),
        (
            HostedStepError::EgressDispatchFailure(EgressDispatchFailure::ProtocolViolation {
                channel: "broker".to_string(),
                detail: "unexpected ack frame".to_string(),
            }),
            "egress dispatch failure: protocol violation on channel 'broker': unexpected ack frame",
        ),
        (
            HostedStepError::EffectsWithoutAdapter,
            "effects emitted in adapter-independent mode",
        ),
    ];

    for (err, expected) in cases {
        assert_eq!(err.to_string(), expected);
    }
}

#[test]
fn hosted_step_error_from_egress_validation_error_stringifies_currently() {
    let step_error = HostedStepError::from(EgressValidationError::RoutedKindNotAcceptedByAdapter {
        intent_kind: "place_order".to_string(),
    });

    match step_error {
        HostedStepError::EgressValidation(HostedEgressValidationError::Validation(detail)) => {
            assert_eq!(
                detail.to_string(),
                "egress route kind 'place_order' is not accepted by adapter (accepts.effects)"
            )
        }
        other => panic!("unexpected step error variant: {other:?}"),
    }
}

#[test]
fn hosted_step_error_from_egress_process_error_preserves_type() {
    let step_error = HostedStepError::from(EgressProcessError::Timeout {
        channel: "broker".to_string(),
        intent_id: "intent-123".to_string(),
        timeout: Duration::from_secs(5),
    });

    match step_error {
        HostedStepError::EgressProcess(detail) => assert_eq!(
            detail.to_string(),
            "egress channel 'broker' timed out waiting for durable-accept ack for intent 'intent-123' after 5000ms"
        ),
        other => panic!("unexpected step error variant: {other:?}"),
    }
}

#[test]
fn hosted_step_error_sources_chain_for_typed_wrappers_only() {
    let effect_apply = HostedStepError::from(EffectApplyError::UnhandledEffectKind {
        kind: "place_order".to_string(),
    });
    assert_eq!(
        effect_apply
            .source()
            .expect("effect apply wrapper should expose its source")
            .to_string(),
        "no registered effect handler for kind 'place_order'"
    );

    let coverage = HostedStepError::from(HandlerCoverageError::ConflictingCoverage {
        effect_kind: "set_context".to_string(),
    });
    assert_eq!(
        coverage
            .source()
            .expect("coverage wrapper should expose its source")
            .to_string(),
        "ambiguous coverage for kind 'set_context': claimed by both handler and egress"
    );

    let binding = HostedStepError::Binding(EventBindingError::PayloadSchemaMismatch {
        kind: "command.place_order".to_string(),
        detail: "binder rejected payload".to_string(),
    });
    assert_eq!(
        binding
            .source()
            .expect("binding wrapper should expose its source")
            .to_string(),
        "payload does not match schema for semantic event kind 'command.place_order': binder rejected payload"
    );

    let event_build = HostedStepError::EventBuild(HostedEventBuildError::InvalidPayload(
        ExternalEventPayloadError::PayloadMustBeJsonObject {
            got: "string".to_string(),
        },
    ));
    assert_eq!(
        event_build
            .source()
            .expect("event build wrapper should expose its source")
            .to_string(),
        "payload must be a JSON object, got string"
    );

    let egress_validation =
        HostedStepError::from(EgressValidationError::RoutedKindNotAcceptedByAdapter {
            intent_kind: "place_order".to_string(),
        });
    assert_eq!(
        egress_validation
            .source()
            .expect("egress validation wrapper should expose its source")
            .to_string(),
        "egress route kind 'place_order' is not accepted by adapter (accepts.effects)"
    );

    let egress_process = HostedStepError::from(EgressProcessError::Startup {
        channel: "broker".to_string(),
        detail: "spawn failed".to_string(),
    });
    assert_eq!(
        egress_process
            .source()
            .expect("egress process wrapper should expose its source")
            .to_string(),
        "egress channel 'broker' startup failed: spawn failed"
    );

    let dispatch = HostedStepError::EgressDispatchFailure(EgressDispatchFailure::Io {
        channel: "broker".to_string(),
        detail: "broken pipe".to_string(),
    });
    let dispatch_source = dispatch
        .source()
        .expect("egress dispatch wrapper should expose its source");
    assert_eq!(
        dispatch_source.to_string(),
        "I/O failure on channel 'broker': broken pipe"
    );
    assert!(dispatch_source.source().is_none());

    let source_free = [
        HostedStepError::DuplicateEventId {
            event_id: "evt_dup".to_string(),
        },
        HostedStepError::MissingSemanticKind,
        HostedStepError::MissingPayload,
        HostedStepError::PayloadMustBeObject,
        HostedStepError::UnknownSemanticKind {
            kind: "command.place_order".to_string(),
        },
        HostedStepError::LifecycleViolation {
            detail: "runner already finished".to_string(),
        },
        HostedStepError::MissingDecisionEntry,
        HostedStepError::EffectsWithoutAdapter,
    ];

    for err in source_free {
        assert!(
            err.source().is_none(),
            "unexpected source for variant: {err:?}"
        );
    }
}
