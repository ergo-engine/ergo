use crate::{HostReplayError, HostedReplayError};

#[derive(Debug, Clone)]
pub struct HostErrorDescriptor {
    pub code: String,
    pub message: String,
    pub rule_id: Option<String>,
    pub where_field: Option<String>,
    pub fix: Option<String>,
    pub details: Vec<String>,
}

impl HostErrorDescriptor {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            rule_id: None,
            where_field: None,
            fix: None,
            details: Vec::new(),
        }
    }

    fn with_rule_id(mut self, rule_id: impl Into<String>) -> Self {
        self.rule_id = Some(rule_id.into());
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
                "replay.unsupported_capture_version",
                format!("unsupported capture version '{capture_version}'"),
            )
            .with_where("capture_version")
            .with_fix("regenerate capture with a supported runtime version")
        }
        ergo_supervisor::replay::ReplayError::HashMismatch { event_id } => {
            HostErrorDescriptor::new(
                "replay.hash_mismatch",
                format!("payload hash mismatch for event '{}'", event_id.as_str()),
            )
            .with_where(format!("event '{}'", event_id.as_str()))
            .with_fix("re-run canonical capture to produce an uncorrupted bundle")
        }
        ergo_supervisor::replay::ReplayError::InvalidPayload { event_id, detail } => {
            HostErrorDescriptor::new(
                "replay.invalid_payload",
                format!("invalid payload for event '{}'", event_id.as_str()),
            )
            .with_where(format!("event '{}'", event_id.as_str()))
            .with_fix("re-capture with object payloads or repair the capture bundle payload bytes")
            .with_detail(detail.clone())
        }
        ergo_supervisor::replay::ReplayError::AdapterProvenanceMismatch { expected, got } => {
            HostErrorDescriptor::new(
                "replay.adapter_provenance_mismatch",
                "adapter provenance mismatch",
            )
            .with_where("capture provenance vs replay adapter")
            .with_fix("replay with the adapter used to produce the capture")
            .with_detail(format!("expected: '{expected}'"))
            .with_detail(format!("got: '{got}'"))
        }
        ergo_supervisor::replay::ReplayError::RuntimeProvenanceMismatch { expected, got } => {
            HostErrorDescriptor::new(
                "replay.runtime_provenance_mismatch",
                "runtime provenance mismatch",
            )
            .with_where("capture provenance vs replay runtime surface")
            .with_fix("replay against the graph/runtime used to produce the capture or recapture")
            .with_detail(format!("expected: '{expected}'"))
            .with_detail(format!("got: '{got}'"))
        }
        ergo_supervisor::replay::ReplayError::UnexpectedAdapterProvidedForNoAdapterCapture => {
            HostErrorDescriptor::new(
                "replay.unexpected_adapter",
                "bundle provenance is 'none'; adapter must not be provided",
            )
            .with_where("replay option '--adapter'")
            .with_fix("remove --adapter and replay without adapter")
        }
        ergo_supervisor::replay::ReplayError::AdapterRequiredForProvenancedCapture => {
            HostErrorDescriptor::new(
                "replay.adapter_required",
                "bundle is adapter-provenanced; adapter is required",
            )
            .with_where("replay option '--adapter'")
            .with_fix("provide --adapter <adapter.yaml> that matches capture provenance")
        }
        ergo_supervisor::replay::ReplayError::DuplicateEventId { event_id } => {
            HostErrorDescriptor::new(
                "replay.duplicate_event_id",
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
                "replay.effect_mismatch",
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
        HostedReplayError::EventRehydrate { event_id, detail } => HostErrorDescriptor::new(
            "replay.event_rehydrate_failed",
            format!("event '{}' failed rehydration during replay", event_id),
        )
        .with_where(format!("event '{}'", event_id))
        .with_fix("inspect capture payload/hash integrity and recapture if needed")
        .with_detail(detail.clone()),
        HostedReplayError::Step(step_err) => {
            HostErrorDescriptor::new("replay.host_step_failed", "host replay step failed")
                .with_where("ergo-host replay lifecycle")
                .with_fix("inspect host lifecycle/effect handler failures and retry")
                .with_detail(step_err.to_string())
        }
        HostedReplayError::DecisionMismatch => HostErrorDescriptor::new(
            "replay.decision_mismatch",
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
            HostErrorDescriptor::new("replay.graph_id_mismatch", "graph_id mismatch")
                .with_where(format!(
                    "capture graph_id '{}' vs replay graph '{}'",
                    got, expected
                ))
                .with_fix("replay with --graph matching the original capture graph")
                .with_detail(format!("expected: '{}'", expected))
                .with_detail(format!("got: '{}'", got))
        }
        HostReplayError::ExternalKindsNotRepresentable { missing } => {
            HostErrorDescriptor::new(
                "replay.external_effect_kind_unrepresentable",
                "capture contains external effect kinds not representable by replay graph ownership",
            )
            .with_where("replay ownership preflight")
            .with_fix("replay with the matching graph/adapter pair used during capture")
            .with_detail(format!("missing kinds: {}", missing.join(", ")))
        }
        HostReplayError::Setup(detail) => {
            HostErrorDescriptor::new("replay.host_setup_failed", "host replay setup failed")
                .with_where("ergo-host replay setup")
                .with_fix("verify capture/graph/adapter paths and retry")
                .with_detail(detail.clone())
        }
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
        "adapter.required_for_graph",
        "graph requires adapter capabilities but no --adapter was provided",
    )
    .with_rule_id("RUN-CANON-2")
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
