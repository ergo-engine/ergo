//! error
//!
//! Purpose:
//! - Define the host-owned step-boundary error surface used by `HostedRunner`,
//!   replay, and embedded/manual-runner callers.
//!
//! Owns:
//! - The public `HostedStepError` taxonomy for step-time validation, lifecycle,
//!   effect-application, and egress failures.
//! - The public `EgressDispatchFailure` taxonomy for in-run egress dispatch
//!   failures that later map to interruption reasons.
//! - Conversion bridges from lower-level adapter and egress errors into the
//!   host step boundary.
//!
//! Does not own:
//! - Top-level run/replay error shaping such as `HostRunError` or replay
//!   descriptor rendering.
//! - The underlying semantics of `EgressValidationError` or `EgressProcessError`.
//!
//! Connects to:
//! - `runner.rs`, which is the dominant producer of `HostedStepError`.
//! - `usecases.rs`, which maps `EgressDispatchFailure` into `InterruptionReason`.
//! - `sdk-rust`, which re-exports these types and pattern-matches specific
//!   variants as part of its manual-runner state machine.
//!
//! Safety notes:
//! - `EgressDispatchFailure` is fully typed and intentionally mirrors the decided
//!   in-run egress dispatch taxonomy.
//! - `HostedStepError` mixes typed wrappers (`EffectApply`, `HandlerCoverage`,
//!   `EgressDispatchFailure`) with string buckets (`BindingError`,
//!   `EventBuildError`, `EgressValidation`, `EgressLifecycle`).
//! - Those string buckets are not produced only by the `From` impls below:
//!   `runner.rs` also constructs `EgressValidation(String)` directly for runner
//!   precondition failures and constructs `EgressLifecycle(String)` by
//!   destructuring `EgressProcessError` and adding host-owned channel context.
//! - Variant names and field shapes are live public API because `ergo-host` and
//!   `sdk-rust` both re-export them and downstream code pattern-matches them.

use ergo_adapter::host::{EffectApplyError, HandlerCoverageError};

use crate::egress::{EgressProcessError, EgressValidationError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EgressDispatchFailure {
    AckTimeout { channel: String, intent_id: String },
    ProtocolViolation { channel: String, detail: String },
    Io { channel: String, detail: String },
}

impl EgressDispatchFailure {
    pub fn channel(&self) -> &str {
        match self {
            Self::AckTimeout { channel, .. }
            | Self::ProtocolViolation { channel, .. }
            | Self::Io { channel, .. } => channel,
        }
    }
}

impl std::fmt::Display for EgressDispatchFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AckTimeout { channel, intent_id } => write!(
                f,
                "ack timeout on channel '{channel}' for intent '{intent_id}'"
            ),
            Self::ProtocolViolation { channel, detail } => {
                write!(f, "protocol violation on channel '{channel}': {detail}")
            }
            Self::Io { channel, detail } => {
                write!(f, "I/O failure on channel '{channel}': {detail}")
            }
        }
    }
}

impl std::error::Error for EgressDispatchFailure {}

#[derive(Debug)]
pub enum HostedStepError {
    DuplicateEventId { event_id: String },
    MissingSemanticKind,
    MissingPayload,
    PayloadMustBeObject,
    UnknownSemanticKind { kind: String },
    BindingError(String),
    EventBuildError(String),
    LifecycleViolation { detail: String },
    MissingDecisionEntry,
    EffectApply(EffectApplyError),
    HandlerCoverage(HandlerCoverageError),
    EgressValidation(String),
    EgressLifecycle(String),
    EgressDispatchFailure(EgressDispatchFailure),
    EffectsWithoutAdapter,
}

impl std::fmt::Display for HostedStepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateEventId { event_id } => {
                write!(
                    f,
                    "duplicate event_id '{}' in canonical host runner",
                    event_id
                )
            }
            Self::MissingSemanticKind => {
                write!(f, "semantic_kind is required in adapter-bound mode")
            }
            Self::MissingPayload => write!(f, "payload is required in adapter-bound mode"),
            Self::PayloadMustBeObject => write!(f, "payload must be a JSON object"),
            Self::UnknownSemanticKind { kind } => {
                write!(f, "unknown semantic event kind '{kind}'")
            }
            Self::BindingError(detail) => write!(f, "semantic event binding failed: {detail}"),
            Self::EventBuildError(detail) => write!(f, "event build failed: {detail}"),
            Self::LifecycleViolation { detail } => write!(f, "host lifecycle violation: {detail}"),
            Self::MissingDecisionEntry => {
                write!(f, "missing decision log entry for the completed host step")
            }
            Self::EffectApply(err) => write!(f, "effect application failed: {err}"),
            Self::HandlerCoverage(err) => write!(f, "handler coverage failed: {err}"),
            Self::EgressValidation(detail) => {
                write!(f, "egress configuration validation failed: {detail}")
            }
            Self::EgressLifecycle(detail) => write!(f, "egress lifecycle failure: {detail}"),
            Self::EgressDispatchFailure(detail) => {
                write!(f, "egress dispatch failure: {detail}")
            }
            Self::EffectsWithoutAdapter => write!(f, "effects emitted in adapter-independent mode"),
        }
    }
}

impl std::error::Error for HostedStepError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EffectApply(err) => Some(err),
            Self::HandlerCoverage(err) => Some(err),
            Self::EgressDispatchFailure(err) => Some(err),
            _ => None,
        }
    }
}

impl From<EffectApplyError> for HostedStepError {
    fn from(value: EffectApplyError) -> Self {
        Self::EffectApply(value)
    }
}

impl From<HandlerCoverageError> for HostedStepError {
    fn from(value: HandlerCoverageError) -> Self {
        Self::HandlerCoverage(value)
    }
}

impl From<EgressValidationError> for HostedStepError {
    fn from(value: EgressValidationError) -> Self {
        Self::EgressValidation(value.to_string())
    }
}

impl From<EgressProcessError> for HostedStepError {
    fn from(value: EgressProcessError) -> Self {
        Self::EgressLifecycle(value.to_string())
    }
}

#[cfg(test)]
mod tests;
