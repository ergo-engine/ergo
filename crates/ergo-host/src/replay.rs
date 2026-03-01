use ergo_supervisor::replay::{
    compare_decisions, validate_bundle_strict, ReplayError, StrictReplayExpectations,
};
use ergo_supervisor::{CaptureBundle, Decision};

use crate::{HostedRunner, HostedStepError};

#[derive(Debug)]
pub enum HostedReplayError {
    Preflight(ReplayError),
    EventRehydrate { event_id: String, detail: String },
    Step(HostedStepError),
    Compare(ReplayError),
    DecisionMismatch,
}

impl std::fmt::Display for HostedReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preflight(err) => write!(f, "strict replay preflight failed: {err:?}"),
            Self::EventRehydrate { event_id, detail } => write!(
                f,
                "failed to rehydrate event '{}' during host replay: {}",
                event_id, detail
            ),
            Self::Step(err) => write!(f, "host replay step failed: {err}"),
            Self::Compare(err) => write!(f, "replay decision comparison failed: {err:?}"),
            Self::DecisionMismatch => {
                write!(f, "replay decisions do not match captured decisions")
            }
        }
    }
}

impl std::error::Error for HostedReplayError {}

pub fn replay_bundle_strict(
    bundle: &CaptureBundle,
    mut runner: HostedRunner,
    expectations: StrictReplayExpectations<'_>,
) -> Result<CaptureBundle, HostedReplayError> {
    validate_bundle_strict(bundle, expectations).map_err(HostedReplayError::Preflight)?;

    for record in &bundle.events {
        let event =
            record
                .rehydrate_checked()
                .map_err(|err| HostedReplayError::EventRehydrate {
                    event_id: record.event_id.as_str().to_string(),
                    detail: match err {
                        ergo_adapter::capture::CaptureError::PayloadHashMismatch {
                            expected,
                            actual,
                        } => format!(
                            "payload hash mismatch (expected '{}', actual '{}')",
                            expected, actual
                        ),
                        ergo_adapter::capture::CaptureError::InvalidPayload { detail } => detail,
                    },
                })?;
        runner.replay_step(event).map_err(HostedReplayError::Step)?;
    }

    let replayed_bundle = runner.into_capture_bundle();
    let matches = compare_decisions(&bundle.decisions, &replayed_bundle.decisions)
        .map_err(HostedReplayError::Compare)?;
    if !matches {
        return Err(HostedReplayError::DecisionMismatch);
    }

    Ok(replayed_bundle)
}

pub fn decision_counts(bundle: &CaptureBundle) -> (usize, usize, usize) {
    let invoke_count = bundle
        .decisions
        .iter()
        .filter(|record| record.decision == Decision::Invoke)
        .count();
    let defer_count = bundle
        .decisions
        .iter()
        .filter(|record| record.decision == Decision::Defer)
        .count();
    let skip_count = bundle
        .decisions
        .iter()
        .filter(|record| record.decision == Decision::Skip)
        .count();

    (invoke_count, defer_count, skip_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HostedAdapterConfig, HostedEvent};
    use ergo_adapter::{
        compile_event_binder, AdapterProvides, ContextKeyProvision, ExternalEventKind,
    };
    use ergo_runtime::catalog::{build_core_catalog, core_registries};
    use ergo_runtime::cluster::{
        ExpandedEdge, ExpandedEndpoint, ExpandedGraph, ExpandedNode, ImplementationInstance,
        OutputPortSpec, OutputRef, ParameterValue,
    };
    use ergo_runtime::common::Value;
    use ergo_supervisor::replay::hash_effect;
    use ergo_supervisor::Constraints;
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    const ADAPTER_PROVENANCE: &str = "adapter:test@1.0.0;sha256:test";
    const RUNTIME_PROVENANCE: &str = "runtime:test";
    const GRAPH_ID: &str = "host_replay_test";

    fn build_context_set_bool_graph() -> ExpandedGraph {
        let mut nodes = HashMap::new();

        nodes.insert(
            "gate".to_string(),
            ExpandedNode {
                runtime_id: "gate".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "const_bool".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([("value".to_string(), ParameterValue::Bool(true))]),
            },
        );

        nodes.insert(
            "payload".to_string(),
            ExpandedNode {
                runtime_id: "payload".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "boolean_source".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([("value".to_string(), ParameterValue::Bool(false))]),
            },
        );

        nodes.insert(
            "emit".to_string(),
            ExpandedNode {
                runtime_id: "emit".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "emit_if_true".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::new(),
            },
        );

        nodes.insert(
            "ctx_set".to_string(),
            ExpandedNode {
                runtime_id: "ctx_set".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "context_set_bool".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([(
                    "key".to_string(),
                    ParameterValue::String("armed".to_string()),
                )]),
            },
        );

        let edges = vec![
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "gate".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "input".to_string(),
                },
            },
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "event".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "ctx_set".to_string(),
                    port_name: "event".to_string(),
                },
            },
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "payload".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "ctx_set".to_string(),
                    port_name: "value".to_string(),
                },
            },
        ];

        ExpandedGraph {
            nodes,
            edges,
            boundary_inputs: Vec::new(),
            boundary_outputs: vec![OutputPortSpec {
                name: "outcome".to_string(),
                maps_to: OutputRef {
                    node_id: "ctx_set".to_string(),
                    port_name: "outcome".to_string(),
                },
            }],
        }
    }

    fn build_once_cluster_behavior_graph() -> ExpandedGraph {
        let mut nodes = HashMap::new();

        nodes.insert(
            "event_signal".to_string(),
            ExpandedNode {
                runtime_id: "event_signal".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "const_bool".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([("value".to_string(), ParameterValue::Bool(true))]),
            },
        );

        nodes.insert(
            "emit_source_event".to_string(),
            ExpandedNode {
                runtime_id: "emit_source_event".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "emit_if_true".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::new(),
            },
        );

        nodes.insert(
            "state_source".to_string(),
            ExpandedNode {
                runtime_id: "state_source".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "context_bool_source".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([(
                    "key".to_string(),
                    ParameterValue::String("once_state".to_string()),
                )]),
            },
        );

        nodes.insert(
            "not_state".to_string(),
            ExpandedNode {
                runtime_id: "not_state".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "not".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::new(),
            },
        );

        nodes.insert(
            "gate_event".to_string(),
            ExpandedNode {
                runtime_id: "gate_event".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "emit_if_event_and_true".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::new(),
            },
        );

        nodes.insert(
            "set_value".to_string(),
            ExpandedNode {
                runtime_id: "set_value".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "boolean_source".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([("value".to_string(), ParameterValue::Bool(true))]),
            },
        );

        nodes.insert(
            "set_state".to_string(),
            ExpandedNode {
                runtime_id: "set_state".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "context_set_bool".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([(
                    "key".to_string(),
                    ParameterValue::String("once_state".to_string()),
                )]),
            },
        );

        let edges = vec![
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "event_signal".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "emit_source_event".to_string(),
                    port_name: "input".to_string(),
                },
            },
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "emit_source_event".to_string(),
                    port_name: "event".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "gate_event".to_string(),
                    port_name: "event".to_string(),
                },
            },
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "state_source".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "not_state".to_string(),
                    port_name: "value".to_string(),
                },
            },
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "not_state".to_string(),
                    port_name: "result".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "gate_event".to_string(),
                    port_name: "condition".to_string(),
                },
            },
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "gate_event".to_string(),
                    port_name: "event".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "set_state".to_string(),
                    port_name: "event".to_string(),
                },
            },
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "set_value".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "set_state".to_string(),
                    port_name: "value".to_string(),
                },
            },
        ];

        ExpandedGraph {
            nodes,
            edges,
            boundary_inputs: Vec::new(),
            boundary_outputs: vec![OutputPortSpec {
                name: "event".to_string(),
                maps_to: OutputRef {
                    node_id: "gate_event".to_string(),
                    port_name: "event".to_string(),
                },
            }],
        }
    }

    fn build_context_set_number_from_price_graph() -> ExpandedGraph {
        let mut nodes = HashMap::new();

        nodes.insert(
            "gate".to_string(),
            ExpandedNode {
                runtime_id: "gate".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "const_bool".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([("value".to_string(), ParameterValue::Bool(true))]),
            },
        );

        nodes.insert(
            "emit".to_string(),
            ExpandedNode {
                runtime_id: "emit".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "emit_if_true".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::new(),
            },
        );

        nodes.insert(
            "price_source".to_string(),
            ExpandedNode {
                runtime_id: "price_source".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "context_number_source".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([(
                    "key".to_string(),
                    ParameterValue::String("price".to_string()),
                )]),
            },
        );

        nodes.insert(
            "ctx_set".to_string(),
            ExpandedNode {
                runtime_id: "ctx_set".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "context_set_number".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([(
                    "key".to_string(),
                    ParameterValue::String("ema".to_string()),
                )]),
            },
        );

        let edges = vec![
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "gate".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "input".to_string(),
                },
            },
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "event".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "ctx_set".to_string(),
                    port_name: "event".to_string(),
                },
            },
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "price_source".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "ctx_set".to_string(),
                    port_name: "value".to_string(),
                },
            },
        ];

        ExpandedGraph {
            nodes,
            edges,
            boundary_inputs: Vec::new(),
            boundary_outputs: vec![OutputPortSpec {
                name: "outcome".to_string(),
                maps_to: OutputRef {
                    node_id: "ctx_set".to_string(),
                    port_name: "outcome".to_string(),
                },
            }],
        }
    }

    fn adapter_provides(context_keys: &[&str]) -> AdapterProvides {
        let mut context = HashMap::new();
        for key in context_keys {
            context.insert(
                (*key).to_string(),
                ContextKeyProvision {
                    ty: "Bool".to_string(),
                    required: false,
                    writable: true,
                },
            );
        }

        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "price": { "type": "number" },
                "armed": { "type": "boolean" },
                "once_state": { "type": "boolean" }
            },
            "additionalProperties": false
        });

        let mut event_schemas = HashMap::new();
        event_schemas.insert("price_bar".to_string(), schema);

        AdapterProvides {
            context,
            events: HashSet::from(["price_bar".to_string()]),
            effects: HashSet::from(["set_context".to_string()]),
            event_schemas,
            capture_format_version: "1".to_string(),
            adapter_fingerprint: ADAPTER_PROVENANCE.to_string(),
        }
    }

    fn adapter_provides_for_number_effect() -> AdapterProvides {
        let context = HashMap::from([
            (
                "price".to_string(),
                ContextKeyProvision {
                    ty: "Number".to_string(),
                    required: false,
                    writable: false,
                },
            ),
            (
                "ema".to_string(),
                ContextKeyProvision {
                    ty: "Number".to_string(),
                    required: false,
                    writable: true,
                },
            ),
        ]);

        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "price": { "type": "number" },
                "ema": { "type": "number" }
            },
            "additionalProperties": false
        });

        let mut event_schemas = HashMap::new();
        event_schemas.insert("price_bar".to_string(), schema);

        AdapterProvides {
            context,
            events: HashSet::from(["price_bar".to_string()]),
            effects: HashSet::from(["set_context".to_string()]),
            event_schemas,
            capture_format_version: "1".to_string(),
            adapter_fingerprint: ADAPTER_PROVENANCE.to_string(),
        }
    }

    fn runner_for_graph(graph: ExpandedGraph, provides: AdapterProvides) -> HostedRunner {
        let runtime = ergo_adapter::RuntimeHandle::new(
            Arc::new(graph),
            Arc::new(build_core_catalog()),
            Arc::new(
                core_registries().expect("core registries must initialize for host replay tests"),
            ),
            provides.clone(),
        );
        let binder = compile_event_binder(&provides).expect("binder must compile");
        let adapter = HostedAdapterConfig {
            provides,
            binder,
            adapter_provenance: ADAPTER_PROVENANCE.to_string(),
        };
        HostedRunner::new(
            ergo_adapter::GraphId::new(GRAPH_ID),
            Constraints::default(),
            runtime,
            RUNTIME_PROVENANCE.to_string(),
            Some(adapter),
        )
        .expect("hosted runner must initialize")
    }

    fn build_effect_bundle() -> CaptureBundle {
        let graph = build_context_set_bool_graph();
        let provides = adapter_provides(&["armed"]);
        let mut runner = runner_for_graph(graph, provides);

        runner
            .step(HostedEvent {
                event_id: "e1".to_string(),
                kind: ExternalEventKind::Command,
                at: ergo_adapter::EventTime::default(),
                semantic_kind: Some("price_bar".to_string()),
                payload: Some(serde_json::json!({"price": 101.5})),
            })
            .expect("host step should execute");

        let bundle = runner.into_capture_bundle();
        assert_eq!(bundle.decisions.len(), 1);
        assert_eq!(bundle.decisions[0].effects.len(), 1);
        bundle
    }

    #[test]
    fn context_set_bool_host_path_replays_with_effect_integrity() {
        let graph = build_context_set_bool_graph();
        let provides = adapter_provides(&["armed"]);

        let mut capture_runner = runner_for_graph(graph.clone(), provides.clone());
        capture_runner
            .step(HostedEvent {
                event_id: "e1".to_string(),
                kind: ExternalEventKind::Command,
                at: ergo_adapter::EventTime::default(),
                semantic_kind: Some("price_bar".to_string()),
                payload: Some(serde_json::json!({"price": 101.5})),
            })
            .expect("capture step should execute");
        let captured = capture_runner.into_capture_bundle();

        assert_eq!(captured.decisions[0].effects.len(), 1);
        assert_eq!(captured.decisions[0].effects[0].effect.kind, "set_context");

        let replay_runner = runner_for_graph(graph, provides);
        let replayed = replay_bundle_strict(
            &captured,
            replay_runner,
            StrictReplayExpectations {
                expected_adapter_provenance: ADAPTER_PROVENANCE,
                expected_runtime_provenance: RUNTIME_PROVENANCE,
            },
        )
        .expect("host replay should match capture");
        assert_eq!(replayed.decisions.len(), captured.decisions.len());
    }

    #[test]
    fn once_cluster_host_path_replays_with_effect_integrity() {
        let graph = build_once_cluster_behavior_graph();
        let provides = adapter_provides(&["once_state"]);

        let mut capture_runner = runner_for_graph(graph.clone(), provides.clone());
        capture_runner
            .step(HostedEvent {
                event_id: "e1".to_string(),
                kind: ExternalEventKind::Command,
                at: ergo_adapter::EventTime::default(),
                semantic_kind: Some("price_bar".to_string()),
                payload: Some(serde_json::json!({"price": 12.0})),
            })
            .expect("capture step should execute");
        let captured = capture_runner.into_capture_bundle();

        assert_eq!(captured.decisions[0].effects.len(), 1);
        assert_eq!(
            captured.decisions[0].effects[0].effect.writes[0].value,
            Value::Bool(true)
        );

        let replay_runner = runner_for_graph(graph, provides);
        replay_bundle_strict(
            &captured,
            replay_runner,
            StrictReplayExpectations {
                expected_adapter_provenance: ADAPTER_PROVENANCE,
                expected_runtime_provenance: RUNTIME_PROVENANCE,
            },
        )
        .expect("host replay should match capture");
    }

    #[test]
    fn tampered_effect_content_returns_mismatch() {
        let graph = build_context_set_bool_graph();
        let provides = adapter_provides(&["armed"]);
        let mut bundle = build_effect_bundle();

        let effect = &mut bundle.decisions[0].effects[0];
        effect.effect.writes[0].key = "corrupted".to_string();
        effect.effect_hash = hash_effect(&effect.effect);

        let replay_runner = runner_for_graph(graph, provides);
        let err = replay_bundle_strict(
            &bundle,
            replay_runner,
            StrictReplayExpectations {
                expected_adapter_provenance: ADAPTER_PROVENANCE,
                expected_runtime_provenance: RUNTIME_PROVENANCE,
            },
        )
        .expect_err("tampered effect content should fail replay");
        assert!(matches!(
            err,
            HostedReplayError::Compare(ReplayError::EffectMismatch { .. })
        ));
    }

    #[test]
    fn tampered_effect_hash_returns_mismatch() {
        let graph = build_context_set_bool_graph();
        let provides = adapter_provides(&["armed"]);
        let mut bundle = build_effect_bundle();

        bundle.decisions[0].effects[0].effect_hash =
            "0000000000000000000000000000000000000000000000000000000000000000".to_string();

        let replay_runner = runner_for_graph(graph, provides);
        let err = replay_bundle_strict(
            &bundle,
            replay_runner,
            StrictReplayExpectations {
                expected_adapter_provenance: ADAPTER_PROVENANCE,
                expected_runtime_provenance: RUNTIME_PROVENANCE,
            },
        )
        .expect_err("tampered effect hash should fail replay");
        assert!(matches!(
            err,
            HostedReplayError::Compare(ReplayError::EffectMismatch { .. })
        ));
    }

    #[test]
    fn missing_effect_entry_returns_mismatch() {
        let graph = build_context_set_bool_graph();
        let provides = adapter_provides(&["armed"]);
        let mut bundle = build_effect_bundle();

        bundle.decisions[0].effects.clear();

        let replay_runner = runner_for_graph(graph, provides);
        let err = replay_bundle_strict(
            &bundle,
            replay_runner,
            StrictReplayExpectations {
                expected_adapter_provenance: ADAPTER_PROVENANCE,
                expected_runtime_provenance: RUNTIME_PROVENANCE,
            },
        )
        .expect_err("missing effect entry should fail replay");
        assert!(matches!(
            err,
            HostedReplayError::Compare(ReplayError::EffectMismatch { .. })
        ));
    }

    #[test]
    fn duplicate_event_id_capture_fails_strict_preflight() {
        let graph = build_context_set_number_from_price_graph();
        let provides = adapter_provides_for_number_effect();
        let mut capture_runner = runner_for_graph(graph.clone(), provides.clone());

        capture_runner
            .step(HostedEvent {
                event_id: "evt_1".to_string(),
                kind: ExternalEventKind::Command,
                at: ergo_adapter::EventTime::default(),
                semantic_kind: Some("price_bar".to_string()),
                payload: Some(serde_json::json!({"price": 100.0})),
            })
            .expect("first duplicate-id step should execute");
        capture_runner
            .step(HostedEvent {
                event_id: "evt_2".to_string(),
                kind: ExternalEventKind::Command,
                at: ergo_adapter::EventTime::default(),
                semantic_kind: Some("price_bar".to_string()),
                payload: Some(serde_json::json!({"price": 200.0})),
            })
            .expect("second capture step should execute");

        let mut captured = capture_runner.into_capture_bundle();
        assert_eq!(captured.decisions.len(), 2);
        // Corrupt bundle identity to duplicate event IDs and ensure strict preflight catches it.
        captured.events[1].event_id = captured.events[0].event_id.clone();

        let replay_runner = runner_for_graph(graph, provides);
        let err = replay_bundle_strict(
            &captured,
            replay_runner,
            StrictReplayExpectations {
                expected_adapter_provenance: ADAPTER_PROVENANCE,
                expected_runtime_provenance: RUNTIME_PROVENANCE,
            },
        )
        .expect_err("duplicate event IDs must fail strict replay preflight");
        assert!(matches!(
            err,
            HostedReplayError::Preflight(ReplayError::DuplicateEventId { .. })
        ));
    }
}
