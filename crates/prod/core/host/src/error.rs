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
//! - Host-owned typed wrappers for event-build and egress-validation failures
//!   that add host boundary meaning without flattening lower-level sources.
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
//! - `HostedStepError` preserves typed adapter, event-build, and egress
//!   failures directly; only host-authored lifecycle/precondition diagnostics
//!   remain string-detailed.
//! - `runner.rs` still constructs host-owned `LifecycleViolation` and
//!   `HostedEgressValidationError` variants directly for runner precondition
//!   failures where there is no lower-level source error to preserve.
//! - Variant names and field shapes are live public API because `ergo-host` and
//!   `sdk-rust` both re-export them and downstream code pattern-matches them.

use ergo_adapter::host::{EffectApplyError, HandlerCoverageError};
use ergo_adapter::{EventBindingError, ExternalEventPayloadError};

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
pub enum HostedEventBuildError {
    SerializePayload(serde_json::Error),
    InvalidPayload(ExternalEventPayloadError),
}

impl std::fmt::Display for HostedEventBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SerializePayload(err) => write!(f, "serialize event payload: {err}"),
            Self::InvalidPayload(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for HostedEventBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SerializePayload(err) => Some(err),
            Self::InvalidPayload(err) => Some(err),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostedEgressValidationError {
    ReplayOwnershipWithLiveEgress,
    ReplayOwnedKindConflictsWithHandler { kind: String },
    EgressConfigRequiresAdapterBoundMode,
    ReplayOwnershipRequiresAdapterBoundMode,
    EgressProvenanceRequiresConfig,
    MissingEgressProvenance,
    Validation(EgressValidationError),
}

impl std::fmt::Display for HostedEgressValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReplayOwnershipWithLiveEgress => write!(
                f,
                "replay ownership cannot be supplied when live egress configuration is present"
            ),
            Self::ReplayOwnedKindConflictsWithHandler { kind } => write!(
                f,
                "replay-owned effect kind '{}' conflicts with handler-owned kind",
                kind
            ),
            Self::EgressConfigRequiresAdapterBoundMode => {
                write!(f, "egress configuration requires adapter-bound mode")
            }
            Self::ReplayOwnershipRequiresAdapterBoundMode => {
                write!(f, "replay ownership requires adapter-bound mode")
            }
            Self::EgressProvenanceRequiresConfig => {
                write!(f, "egress provenance requires egress configuration")
            }
            Self::MissingEgressProvenance => write!(
                f,
                "egress provenance is required when egress configuration is present"
            ),
            Self::Validation(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for HostedEgressValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(err) => Some(err),
            Self::ReplayOwnershipWithLiveEgress
            | Self::ReplayOwnedKindConflictsWithHandler { .. }
            | Self::EgressConfigRequiresAdapterBoundMode
            | Self::ReplayOwnershipRequiresAdapterBoundMode
            | Self::EgressProvenanceRequiresConfig
            | Self::MissingEgressProvenance => None,
        }
    }
}

#[derive(Debug)]
pub enum HostedStepError {
    DuplicateEventId { event_id: String },
    MissingSemanticKind,
    MissingPayload,
    PayloadMustBeObject,
    UnknownSemanticKind { kind: String },
    Binding(EventBindingError),
    EventBuild(HostedEventBuildError),
    LifecycleViolation { detail: String },
    MissingDecisionEntry,
    EffectApply(EffectApplyError),
    HandlerCoverage(HandlerCoverageError),
    EgressValidation(HostedEgressValidationError),
    EgressProcess(EgressProcessError),
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
            Self::Binding(err) => write!(f, "semantic event binding failed: {err}"),
            Self::EventBuild(err) => write!(f, "event build failed: {err}"),
            Self::LifecycleViolation { detail } => write!(f, "host lifecycle violation: {detail}"),
            Self::MissingDecisionEntry => {
                write!(f, "missing decision log entry for the completed host step")
            }
            Self::EffectApply(err) => write!(f, "effect application failed: {err}"),
            Self::HandlerCoverage(err) => write!(f, "handler coverage failed: {err}"),
            Self::EgressValidation(err) => {
                write!(f, "egress configuration validation failed: {err}")
            }
            Self::EgressProcess(err) => write!(f, "egress lifecycle failure: {err}"),
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
            Self::Binding(err) => Some(err),
            Self::EventBuild(err) => Some(err),
            Self::EffectApply(err) => Some(err),
            Self::HandlerCoverage(err) => Some(err),
            Self::EgressValidation(err) => Some(err),
            Self::EgressProcess(err) => Some(err),
            Self::EgressDispatchFailure(err) => Some(err),
            Self::DuplicateEventId { .. }
            | Self::MissingSemanticKind
            | Self::MissingPayload
            | Self::PayloadMustBeObject
            | Self::UnknownSemanticKind { .. }
            | Self::LifecycleViolation { .. }
            | Self::MissingDecisionEntry
            | Self::EffectsWithoutAdapter => None,
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
        Self::EgressValidation(HostedEgressValidationError::Validation(value))
    }
}

impl From<EgressProcessError> for HostedStepError {
    fn from(value: EgressProcessError) -> Self {
        Self::EgressProcess(value)
    }
}

#[cfg(test)]
mod tests;
