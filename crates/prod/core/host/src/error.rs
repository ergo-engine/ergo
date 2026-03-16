use ergo_adapter::host::{EffectApplyError, HandlerCoverageError};

use crate::egress::{EgressProcessError, EgressValidationError};

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
    EgressDispatchFailure { detail: String },
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
            Self::EgressDispatchFailure { detail } => {
                write!(f, "egress dispatch failure: {detail}")
            }
            Self::EffectsWithoutAdapter => write!(f, "effects emitted in adapter-independent mode"),
        }
    }
}

impl std::error::Error for HostedStepError {}

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
