//! replay_error_surface
//!
//! Purpose:
//! - Translate replay/setup failures from kernel replay and host orchestration
//!   layers, plus host adapter-required diagnostics, into a shared
//!   host-side descriptor shape for product-facing renderers.
//! - Despite the file name, this is effectively the host error descriptor
//!   factory for this slice of the public surface, not a replay-only mapper.
//!
//! Owns:
//! - The host mapping table from `ReplayError`, `HostedReplayError`,
//!   `HostReplayError`, and adapter-required summaries into `HostErrorDescriptor`.
//! - Host-owned attachment of contextual fields such as the `RUN-CANON-2` rule id
//!   when adapter-required failures are surfaced to clients.
//!
//! Does not own:
//! - Replay semantics, invariant evaluation, or CLI formatting/output.
//! - A public hosted-only translation API; `describe_hosted_replay_error(...)` is
//!   intentionally private and only supports `describe_host_replay_error(...)`.
//!
//! Connects to:
//! - Kernel replay failures from `ergo_supervisor::replay`.
//! - Host replay/setup failures from `crate::replay` and `crate::usecases`.
//! - Product renderers, currently CLI, that consume `HostErrorDescriptor`.
//!
//! Safety notes:
//! - `HostErrorCode` and `HostRuleId` are the local typed authorities for the
//!   public descriptor contract; string renderings should come from those types
//!   rather than ad hoc literals.
//! - `HostErrorDescriptor` keeps construction private and exposes read-only
//!   getters so callers can consume the contract without rebuilding it by
//!   convention.
//! - Effect mismatch details serialize best-effort JSON for operator diagnostics
//!   and fall back to placeholder text instead of panicking on serialization
//!   failure.
//! - Hosted replay `Preflight` and `Compare` currently collapse to the same
//!   descriptor mapping, so callers cannot distinguish those phases through this
//!   surface without a future API split.

use crate::{HostReplayError, HostedReplayError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostErrorCode {
    ReplayUnsupportedCaptureVersion,
    ReplayHashMismatch,
    ReplayInvalidPayload,
    ReplayAdapterProvenanceMismatch,
    ReplayRuntimeProvenanceMismatch,
    ReplayUnexpectedAdapter,
    ReplayAdapterRequired,
    ReplayDuplicateEventId,
    ReplayEffectMismatch,
    ReplayEventRehydrateFailed,
    ReplayHostStepFailed,
    ReplayDecisionMismatch,
    ReplayGraphIdMismatch,
    ReplayExternalEffectKindUnrepresentable,
    ReplayHostSetupFailed,
    AdapterRequiredForGraph,
    ProductionRequiresAdapter,
}

impl HostErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReplayUnsupportedCaptureVersion => "replay.unsupported_capture_version",
            Self::ReplayHashMismatch => "replay.hash_mismatch",
            Self::ReplayInvalidPayload => "replay.invalid_payload",
            Self::ReplayAdapterProvenanceMismatch => "replay.adapter_provenance_mismatch",
            Self::ReplayRuntimeProvenanceMismatch => "replay.runtime_provenance_mismatch",
            Self::ReplayUnexpectedAdapter => "replay.unexpected_adapter",
            Self::ReplayAdapterRequired => "replay.adapter_required",
            Self::ReplayDuplicateEventId => "replay.duplicate_event_id",
            Self::ReplayEffectMismatch => "replay.effect_mismatch",
            Self::ReplayEventRehydrateFailed => "replay.event_rehydrate_failed",
            Self::ReplayHostStepFailed => "replay.host_step_failed",
            Self::ReplayDecisionMismatch => "replay.decision_mismatch",
            Self::ReplayGraphIdMismatch => "replay.graph_id_mismatch",
            Self::ReplayExternalEffectKindUnrepresentable => {
                "replay.external_effect_kind_unrepresentable"
            }
            Self::ReplayHostSetupFailed => "replay.host_setup_failed",
            Self::AdapterRequiredForGraph => "adapter.required_for_graph",
            Self::ProductionRequiresAdapter => "adapter.production_requires_adapter",
        }
    }
}

impl std::fmt::Display for HostErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostRuleId {
    RunCanon2,
}

impl HostRuleId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RunCanon2 => "RUN-CANON-2",
        }
    }
}

impl std::fmt::Display for HostRuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct HostErrorDescriptor {
    code: HostErrorCode,
    message: String,
    rule_id: Option<HostRuleId>,
    where_field: Option<String>,
    fix: Option<String>,
    details: Vec<String>,
}

impl HostErrorDescriptor {
    fn new(code: HostErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            rule_id: None,
            where_field: None,
            fix: None,
            details: Vec::new(),
        }
    }

    pub fn code_id(&self) -> HostErrorCode {
        self.code
    }

    pub fn code(&self) -> &str {
        self.code.as_str()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn rule_id_id(&self) -> Option<HostRuleId> {
        self.rule_id
    }

    pub fn rule_id(&self) -> Option<&str> {
        self.rule_id.map(HostRuleId::as_str)
    }

    pub fn where_field(&self) -> Option<&str> {
        self.where_field.as_deref()
    }

    pub fn fix(&self) -> Option<&str> {
        self.fix.as_deref()
    }

    pub fn details(&self) -> &[String] {
        &self.details
    }

    fn with_rule_id(mut self, rule_id: HostRuleId) -> Self {
        self.rule_id = Some(rule_id);
        self
    }

    fn with_where(mut self, where_field: impl Into<String>) -> Self {
        self.where_field = Some(where_field.into());
        self
    }

    fn with_fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = Some(fix.into());
        self
    }

    fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.details.push(detail.into());
        self
    }
}

pub fn describe_replay_error(err: &ergo_supervisor::replay::ReplayError) -> HostErrorDescriptor {
    match err {
        ergo_supervisor::replay::ReplayError::UnsupportedVersion { capture_version } => {
            HostErrorDescriptor::new(
                HostErrorCode::ReplayUnsupportedCaptureVersion,
                format!("unsupported capture version '{capture_version}'"),
            )
            .with_where("capture_version")
            .with_fix("regenerate capture with a supported runtime version")
        }
        ergo_supervisor::replay::ReplayError::HashMismatch { event_id } => {
            HostErrorDescriptor::new(
                HostErrorCode::ReplayHashMismatch,
                format!("payload hash mismatch for event '{}'", event_id.as_str()),
            )
            .with_where(format!("event '{}'", event_id.as_str()))
            .with_fix("re-run canonical capture to produce an uncorrupted bundle")
        }
        ergo_supervisor::replay::ReplayError::InvalidPayload { event_id, source } => {
            HostErrorDescriptor::new(
                HostErrorCode::ReplayInvalidPayload,
                format!("invalid payload for event '{}'", event_id.as_str()),
            )
            .with_where(format!("event '{}'", event_id.as_str()))
            .with_fix("re-capture with object payloads or repair the capture bundle payload bytes")
            .with_detail(source.to_string())
        }
        ergo_supervisor::replay::ReplayError::AdapterProvenanceMismatch { expected, got } => {
            HostErrorDescriptor::new(
                HostErrorCode::ReplayAdapterProvenanceMismatch,
                "adapter provenance mismatch",
            )
            .with_where("capture provenance vs replay adapter")
            .with_fix("replay with the adapter used to produce the capture")
            .with_detail(format!("expected: '{expected}'"))
            .with_detail(format!("got: '{got}'"))
        }
        ergo_supervisor::replay::ReplayError::RuntimeProvenanceMismatch { expected, got } => {
            HostErrorDescriptor::new(
                HostErrorCode::ReplayRuntimeProvenanceMismatch,
                "runtime provenance mismatch",
            )
            .with_where("capture provenance vs replay runtime surface")
            .with_fix("replay against the graph/runtime used to produce the capture or recapture")
            .with_detail(format!("expected: '{expected}'"))
            .with_detail(format!("got: '{got}'"))
        }
        ergo_supervisor::replay::ReplayError::UnexpectedAdapterProvidedForNoAdapterCapture => {
            HostErrorDescriptor::new(
                HostErrorCode::ReplayUnexpectedAdapter,
                "bundle provenance is 'none'; adapter must not be provided",
            )
            .with_where("replay option '--adapter'")
            .with_fix("remove --adapter and replay without adapter")
        }
        ergo_supervisor::replay::ReplayError::AdapterRequiredForProvenancedCapture => {
            HostErrorDescriptor::new(
                HostErrorCode::ReplayAdapterRequired,
                "bundle is adapter-provenanced; adapter is required",
            )
            .with_where("replay option '--adapter'")
            .with_fix("provide --adapter <adapter.yaml> that matches capture provenance")
        }
        ergo_supervisor::replay::ReplayError::DuplicateEventId { event_id } => {
            HostErrorDescriptor::new(
                HostErrorCode::ReplayDuplicateEventId,
                format!(
                    "duplicate event_id '{}' in strict replay capture input",
                    event_id.as_str()
                ),
            )
            .with_where(format!("capture event '{}'", event_id.as_str()))
            .with_fix("regenerate capture with unique event ids or repair the capture artifact")
        }
        ergo_supervisor::replay::ReplayError::EffectMismatch {
            event_id,
            effect_index,
            expected,
            actual,
            detail,
        } => {
            let mut info = HostErrorDescriptor::new(
                HostErrorCode::ReplayEffectMismatch,
                format!(
                    "effect mismatch at index {} for event '{}': {}",
                    effect_index,
                    event_id.as_str(),
                    detail,
                ),
            )
            .with_where(format!(
                "event '{}' effect[{}]",
                event_id.as_str(),
                effect_index
            ))
            .with_fix("inspect action effect drift and regenerate capture if needed");

            if let Some(exp) = expected {
                info = info.with_detail(format!(
                    "expected: {}",
                    serde_json::to_string(&exp.effect)
                        .unwrap_or_else(|_| "<unserializable>".to_string())
                ));
            }
            if let Some(act) = actual {
                info = info.with_detail(format!(
                    "actual: {}",
                    serde_json::to_string(&act.effect)
                        .unwrap_or_else(|_| "<unserializable>".to_string())
                ));
            }

            info
        }
    }
}

fn describe_hosted_replay_error(err: &HostedReplayError) -> HostErrorDescriptor {
    match err {
        HostedReplayError::Preflight(replay_err) | HostedReplayError::Compare(replay_err) => {
            describe_replay_error(replay_err)
        }
        HostedReplayError::EventRehydrate { event_id, source } => HostErrorDescriptor::new(
            HostErrorCode::ReplayEventRehydrateFailed,
            format!("event '{}' failed rehydration during replay", event_id),
        )
        .with_where(format!("event '{}'", event_id))
        .with_fix("inspect capture payload/hash integrity and recapture if needed")
        .with_detail(source.to_string()),
        HostedReplayError::Step(step_err) => HostErrorDescriptor::new(
            HostErrorCode::ReplayHostStepFailed,
            "host replay step failed",
        )
        .with_where("ergo-host replay lifecycle")
        .with_fix("inspect host lifecycle/effect handler failures and retry")
        .with_detail(step_err.to_string()),
        HostedReplayError::DecisionMismatch => HostErrorDescriptor::new(
            HostErrorCode::ReplayDecisionMismatch,
            "replay decisions do not match capture decisions",
        )
        .with_where("decision stream comparison")
        .with_fix("inspect runtime/adapter drift and regenerate capture if needed"),
    }
}

pub fn describe_host_replay_error(err: &HostReplayError) -> HostErrorDescriptor {
    match err {
        HostReplayError::Hosted(hosted) => describe_hosted_replay_error(hosted),
        HostReplayError::GraphIdMismatch { expected, got } => {
            HostErrorDescriptor::new(HostErrorCode::ReplayGraphIdMismatch, "graph_id mismatch")
                .with_where(format!(
                    "capture graph_id '{}' vs replay graph '{}'",
                    got, expected
                ))
                .with_fix("replay with --graph matching the original capture graph")
                .with_detail(format!("expected: '{}'", expected))
                .with_detail(format!("got: '{}'", got))
        }
        HostReplayError::ExternalKindsNotRepresentable { missing } => HostErrorDescriptor::new(
            HostErrorCode::ReplayExternalEffectKindUnrepresentable,
            "capture contains external effect kinds not representable by replay graph ownership",
        )
        .with_where("replay ownership preflight")
        .with_fix("replay with the matching graph/adapter pair used during capture")
        .with_detail(format!("missing kinds: {}", missing.join(", "))),
        HostReplayError::Setup(detail) => HostErrorDescriptor::new(
            HostErrorCode::ReplayHostSetupFailed,
            "host replay setup failed",
        )
        .with_where("ergo-host replay setup")
        .with_fix("verify capture/graph/adapter paths and retry")
        .with_detail(detail.to_string()),
    }
}

pub fn describe_adapter_required(summary: &crate::AdapterDependencySummary) -> HostErrorDescriptor {
    let where_field = if let Some(node) = summary
        .required_context_nodes
        .first()
        .or_else(|| summary.write_nodes.first())
    {
        format!("node '{}'", node)
    } else {
        "graph dependency scan".to_string()
    };

    let mut info = HostErrorDescriptor::new(
        HostErrorCode::AdapterRequiredForGraph,
        "graph requires adapter capabilities but no --adapter was provided",
    )
    .with_rule_id(HostRuleId::RunCanon2)
    .with_where(where_field)
    .with_fix("provide --adapter <adapter.yaml> for canonical run");

    if !summary.required_context_nodes.is_empty() {
        info = info.with_detail(format!(
            "required source context at node(s): {}",
            summary.required_context_nodes.join(", ")
        ));
    }

    if !summary.write_nodes.is_empty() {
        info = info.with_detail(format!(
            "action writes at node(s): {}",
            summary.write_nodes.join(", ")
        ));
    }

    info
}

pub fn describe_production_requires_adapter() -> HostErrorDescriptor {
    HostErrorDescriptor::new(
        HostErrorCode::ProductionRequiresAdapter,
        "production session requires an adapter contract but no --adapter was provided",
    )
    .with_rule_id(HostRuleId::RunCanon2)
    .with_where("session intent")
    .with_fix(
        "provide --adapter <adapter.yaml> for production execution, \
         or use a fixture driver/fixture-items ingress for adapter-exempt testing",
    )
}

#[cfg(test)]
mod tests;
