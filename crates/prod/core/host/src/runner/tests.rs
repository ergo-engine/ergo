//! runner tests
//!
//! Purpose:
//! - Lock the local runner contracts owned by `runner.rs`, including wire DTO shape, recoverable
//!   error classification, and hosted-runner finalization state behavior.
//!
//! Does not own:
//! - Broader canonical run/replay/usecase coverage, which lives in the surrounding host tests and
//!   downstream CLI/SDK suites.

use super::*;
use ergo_adapter::host::{EffectApplyError, HandlerCoverageError};
use ergo_adapter::{compile_event_binder, ContextKeyProvision, RuntimeHandle};
use ergo_runtime::catalog::{build_core_catalog, core_registries};
use ergo_runtime::cluster::{
    ExpandedEdge, ExpandedEndpoint, ExpandedGraph, ExpandedNode, ImplementationInstance,
    OutputPortSpec, OutputRef, ParameterValue,
};
use ergo_supervisor::Constraints;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use crate::egress::{EgressChannelConfig, EgressConfig, EgressRoute};

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
        boundary_inputs: vec![],
        boundary_outputs: vec![OutputPortSpec {
            name: "outcome".to_string(),
            maps_to: OutputRef {
                node_id: "ctx_set".to_string(),
                port_name: "outcome".to_string(),
            },
        }],
    }
}

fn build_number_source_graph() -> ExpandedGraph {
    let nodes = HashMap::from([(
        "src".to_string(),
        ExpandedNode {
            runtime_id: "src".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "number_source".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([("value".to_string(), ParameterValue::Number(1.0))]),
        },
    )]);

    ExpandedGraph {
        nodes,
        edges: vec![],
        boundary_inputs: vec![],
        boundary_outputs: vec![OutputPortSpec {
            name: "value".to_string(),
            maps_to: OutputRef {
                node_id: "src".to_string(),
                port_name: "value".to_string(),
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
        boundary_inputs: vec![],
        boundary_outputs: vec![OutputPortSpec {
            name: "outcome".to_string(),
            maps_to: OutputRef {
                node_id: "ctx_set".to_string(),
                port_name: "outcome".to_string(),
            },
        }],
    }
}

fn build_merge_precedence_graph() -> ExpandedGraph {
    let mut nodes = HashMap::new();

    nodes.insert(
        "armed_src".to_string(),
        ExpandedNode {
            runtime_id: "armed_src".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "context_bool_source".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([(
                "key".to_string(),
                ParameterValue::String("armed".to_string()),
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
        "set_armed".to_string(),
        ExpandedNode {
            runtime_id: "set_armed".to_string(),
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
                node_id: "armed_src".to_string(),
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
                node_id: "set_armed".to_string(),
                port_name: "event".to_string(),
            },
        },
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "set_value".to_string(),
                port_name: "value".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "set_armed".to_string(),
                port_name: "value".to_string(),
            },
        },
    ];

    ExpandedGraph {
        nodes,
        edges,
        boundary_inputs: vec![],
        boundary_outputs: vec![OutputPortSpec {
            name: "outcome".to_string(),
            maps_to: OutputRef {
                node_id: "set_armed".to_string(),
                port_name: "outcome".to_string(),
            },
        }],
    }
}

// Allow non-Send/Sync in Arc: CoreRegistries and CorePrimitiveCatalog contain non-Send/Sync types.
#[allow(clippy::arc_with_non_send_sync)]
fn runtime_for_graph(graph: ExpandedGraph, provides: AdapterProvides) -> RuntimeHandle {
    RuntimeHandle::new(
        Arc::new(graph),
        Arc::new(build_core_catalog()),
        Arc::new(core_registries().expect("core registries must initialize for host tests")),
        provides,
    )
}

fn adapter_provides_with_effects(extra_effects: &[&str]) -> AdapterProvides {
    let mut context = HashMap::new();
    context.insert(
        "armed".to_string(),
        ContextKeyProvision {
            ty: "Bool".to_string(),
            required: false,
            writable: true,
        },
    );
    context.insert(
        "price".to_string(),
        ContextKeyProvision {
            ty: "Number".to_string(),
            required: false,
            writable: false,
        },
    );

    let mut effects = HashSet::from([HOST_INTERNAL_SET_CONTEXT_KIND.to_string()]);
    for effect in extra_effects {
        effects.insert((*effect).to_string());
    }

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "price": { "type": "number" },
            "armed": { "type": "boolean" }
        },
        "additionalProperties": false
    });
    let mut event_schemas = HashMap::new();
    event_schemas.insert("price_bar".to_string(), schema);

    AdapterProvides {
        context,
        events: HashSet::from(["price_bar".to_string()]),
        effects,
        effect_schemas: HashMap::new(),
        event_schemas,
        capture_format_version: "1".to_string(),
        adapter_fingerprint: "adapter:test@1.0.0;sha256:test".to_string(),
    }
}

fn adapter_config(provides: AdapterProvides) -> HostedAdapterConfig {
    let binder = compile_event_binder(&provides).expect("event binder should compile");
    HostedAdapterConfig {
        provides,
        binder,
        adapter_provenance: "adapter:test@1.0.0;sha256:test".to_string(),
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
        effects: HashSet::from([HOST_INTERNAL_SET_CONTEXT_KIND.to_string()]),
        effect_schemas: HashMap::new(),
        event_schemas,
        capture_format_version: "1".to_string(),
        adapter_fingerprint: "adapter:test@1.0.0;sha256:test".to_string(),
    }
}

#[test]
fn hosted_event_wire_shape_is_stable() {
    let event = HostedEvent {
        event_id: "evt-1".to_string(),
        kind: ExternalEventKind::Command,
        at: EventTime::default(),
        semantic_kind: Some("price_bar".to_string()),
        payload: Some(serde_json::json!({"close": 101.25})),
    };

    let raw = serde_json::to_string(&event).expect("hosted event must serialize");
    assert_eq!(
        raw,
        r#"{"event_id":"evt-1","kind":"Command","at":{"secs":0,"nanos":0},"semantic_kind":"price_bar","payload":{"close":101.25}}"#
    );

    let round_trip: HostedEvent =
        serde_json::from_str(&raw).expect("hosted event wire shape must deserialize");
    assert_eq!(round_trip.event_id, "evt-1");
    assert_eq!(round_trip.kind, ExternalEventKind::Command);
    assert_eq!(round_trip.at, EventTime::default());
    assert_eq!(round_trip.semantic_kind.as_deref(), Some("price_bar"));
    assert_eq!(
        round_trip.payload,
        Some(serde_json::json!({"close": 101.25}))
    );
}

#[test]
fn hosted_step_error_recoverability_contract_is_locked() {
    let cases = vec![
        (
            HostedStepError::DuplicateEventId {
                event_id: "dup".to_string(),
            },
            true,
        ),
        (HostedStepError::MissingSemanticKind, true),
        (HostedStepError::MissingPayload, true),
        (HostedStepError::PayloadMustBeObject, true),
        (
            HostedStepError::UnknownSemanticKind {
                kind: "price_bar".to_string(),
            },
            true,
        ),
        (
            HostedStepError::BindingError("binding failed".to_string()),
            true,
        ),
        (
            HostedStepError::EventBuildError("bad payload".to_string()),
            true,
        ),
        (
            HostedStepError::LifecycleViolation {
                detail: "bad state".to_string(),
            },
            false,
        ),
        (HostedStepError::MissingDecisionEntry, false),
        (
            HostedStepError::EffectApply(EffectApplyError::UnhandledEffectKind {
                kind: "send_notification".to_string(),
            }),
            false,
        ),
        (
            HostedStepError::HandlerCoverage(HandlerCoverageError::MissingHandler {
                effect_kind: "send_notification".to_string(),
            }),
            false,
        ),
        (
            HostedStepError::EgressValidation("invalid config".to_string()),
            false,
        ),
        (
            HostedStepError::EgressLifecycle("child exited".to_string()),
            false,
        ),
        (
            HostedStepError::EgressDispatchFailure(EgressDispatchFailure::AckTimeout {
                channel: "broker".to_string(),
                intent_id: "intent-1".to_string(),
            }),
            false,
        ),
        (HostedStepError::EffectsWithoutAdapter, false),
    ];

    for (err, expected) in cases {
        assert_eq!(
            is_recoverable_step_error(&err),
            expected,
            "recoverability drifted for error variant: {err:?}"
        );
    }
}

#[test]
fn fresh_runner_cannot_finalize_before_first_committed_step() {
    let runtime = runtime_for_graph(build_number_source_graph(), AdapterProvides::default());
    let runner = HostedRunner::new(
        GraphId::new("g"),
        Constraints::default(),
        runtime,
        "runtime:test".to_string(),
        None,
        None,
        None,
        None,
    )
    .expect("hosted runner should initialize");

    let err = runner
        .ensure_capture_finalizable()
        .expect_err("fresh runner must reject finalization");
    assert!(matches!(err, HostedStepError::LifecycleViolation { .. }));
}

#[test]
fn successful_step_makes_runner_finalizable() {
    let runtime = runtime_for_graph(build_number_source_graph(), AdapterProvides::default());
    let mut runner = HostedRunner::new(
        GraphId::new("g"),
        Constraints::default(),
        runtime,
        "runtime:test".to_string(),
        None,
        None,
        None,
        None,
    )
    .expect("hosted runner should initialize");

    runner
        .step(HostedEvent {
            event_id: "evt-1".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: None,
            payload: Some(serde_json::json!({"foo": "bar"})),
        })
        .expect("first step should execute");

    runner
        .ensure_capture_finalizable()
        .expect("successful step should make capture finalizable");
}

#[test]
fn egress_dispatch_failure_transitions_runner_to_finalize_only() {
    let runtime = runtime_for_graph(build_number_source_graph(), AdapterProvides::default());
    let mut runner = HostedRunner::new(
        GraphId::new("g"),
        Constraints::default(),
        runtime,
        "runtime:test".to_string(),
        None,
        None,
        None,
        None,
    )
    .expect("hosted runner should initialize");

    runner
        .step(HostedEvent {
            event_id: "evt-1".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: None,
            payload: Some(serde_json::json!({"foo": "bar"})),
        })
        .expect("first step should execute");

    runner.record_step_error(&HostedStepError::EgressDispatchFailure(
        EgressDispatchFailure::AckTimeout {
            channel: "broker".to_string(),
            intent_id: "intent-1".to_string(),
        },
    ));

    runner
        .ensure_capture_finalizable()
        .expect("dispatch failure should still allow finalization");

    let err = runner
        .step(HostedEvent {
            event_id: "evt-2".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: None,
            payload: Some(serde_json::json!({"foo": "baz"})),
        })
        .expect_err("finalize-only runner must reject additional steps");

    match err {
        HostedStepError::LifecycleViolation { detail } => {
            assert!(
                detail.contains("must be finalized after egress dispatch failure"),
                "unexpected detail: {detail}"
            );
        }
        other => panic!("expected finalize-only lifecycle violation, got {other:?}"),
    }
}

#[test]
fn adapter_bound_step_applies_effects_and_enriches_capture() {
    let provides = adapter_provides_with_effects(&[]);
    let runtime = runtime_for_graph(build_context_set_bool_graph(), provides.clone());
    let adapter = adapter_config(provides);

    let mut runner = HostedRunner::new(
        GraphId::new("g"),
        Constraints::default(),
        runtime,
        "runtime:test".to_string(),
        Some(adapter),
        None,
        None,
        None,
    )
    .expect("hosted runner should initialize");

    let outcome = runner
        .step(HostedEvent {
            event_id: "e1".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: Some("price_bar".to_string()),
            payload: Some(serde_json::json!({"price": 101.5})),
        })
        .expect("adapter-bound step should execute");

    assert_eq!(outcome.decision, Decision::Invoke);
    assert_eq!(outcome.termination, Some(RunTermination::Completed));
    assert_eq!(outcome.retry_count, 0);
    assert_eq!(outcome.effects.len(), 1);
    assert_eq!(outcome.effects[0].kind, "set_context");
    assert_eq!(outcome.applied_writes.len(), 1);
    assert_eq!(outcome.applied_writes[0].key, "armed");
    assert_eq!(
        runner.context_snapshot().get("armed"),
        Some(&serde_json::json!(false))
    );

    let bundle = runner.into_capture_bundle();
    let effects = &bundle.decisions[0].effects;
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].effect.kind, "set_context");
}

#[test]
fn non_invoke_decision_has_no_effects_or_applied_writes() {
    let provides = adapter_provides_with_effects(&[]);
    let runtime = runtime_for_graph(build_context_set_bool_graph(), provides.clone());
    let adapter = adapter_config(provides);

    let mut runner = HostedRunner::new(
        GraphId::new("g"),
        Constraints {
            max_in_flight: Some(0),
            ..Constraints::default()
        },
        runtime,
        "runtime:test".to_string(),
        Some(adapter),
        None,
        None,
        None,
    )
    .expect("hosted runner should initialize");

    let outcome = runner
        .step(HostedEvent {
            event_id: "e_defer".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: Some("price_bar".to_string()),
            payload: Some(serde_json::json!({"price": 10.0})),
        })
        .expect("defer decision should still produce a step outcome");

    assert_eq!(outcome.decision, Decision::Defer);
    assert!(outcome.termination.is_none());
    assert!(outcome.effects.is_empty());
    assert!(outcome.applied_writes.is_empty());
    assert!(runner.context_snapshot().is_empty());
}

#[test]
fn adapter_independent_mode_executes_without_adapter_config() {
    let runtime = runtime_for_graph(build_number_source_graph(), AdapterProvides::default());
    let mut runner = HostedRunner::new(
        GraphId::new("g"),
        Constraints::default(),
        runtime,
        "runtime:test".to_string(),
        None,
        None,
        None,
        None,
    )
    .expect("hosted runner should initialize in adapter-independent mode");

    let outcome = runner
        .step(HostedEvent {
            event_id: "e1".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: None,
            payload: Some(serde_json::json!({"foo": "bar"})),
        })
        .expect("adapter-independent step should execute");

    assert_eq!(outcome.decision, Decision::Invoke);
    assert_eq!(outcome.termination, Some(RunTermination::Completed));
    assert!(outcome.effects.is_empty());
    assert!(outcome.applied_writes.is_empty());
}

#[test]
fn replay_step_runs_shared_lifecycle_and_effect_application() {
    let provides = adapter_provides_with_effects(&[]);
    let runtime = runtime_for_graph(build_context_set_bool_graph(), provides.clone());
    let adapter = adapter_config(provides);

    let mut runner = HostedRunner::new(
        GraphId::new("g"),
        Constraints::default(),
        runtime,
        "runtime:test".to_string(),
        Some(adapter),
        None,
        None,
        None,
    )
    .expect("hosted runner should initialize");

    let event = ergo_adapter::ExternalEvent::with_payload(
        EventId::new("e1"),
        ExternalEventKind::Command,
        EventTime::default(),
        ergo_adapter::EventPayload {
            data: br#"{"price":101.5}"#.to_vec(),
        },
    )
    .expect("payload should produce external event");

    let outcome = runner
        .replay_step(event)
        .expect("replay_step should execute");

    assert_eq!(outcome.decision, Decision::Invoke);
    assert_eq!(outcome.termination, Some(RunTermination::Completed));
    assert_eq!(outcome.retry_count, 0);
    assert_eq!(outcome.effects.len(), 1);
    assert_eq!(outcome.effects[0].kind, "set_context");
    assert_eq!(
        runner.context_snapshot().get("armed"),
        Some(&serde_json::json!(false))
    );
}

#[test]
fn replay_step_threads_replay_mode_into_execute_step() {
    let runtime = runtime_for_graph(build_number_source_graph(), AdapterProvides::default());
    let mut runner = HostedRunner::new(
        GraphId::new("g"),
        Constraints::default(),
        runtime,
        "runtime:test".to_string(),
        None,
        None,
        None,
        None,
    )
    .expect("hosted runner should initialize");

    let event = ExternalEvent::mechanical(EventId::new("replay_mode"), ExternalEventKind::Command);
    runner
        .replay_step(event)
        .expect("replay_step should execute");

    assert_eq!(runner.last_step_mode(), Some(StepMode::Replay));
}

#[test]
fn replay_mode_does_not_start_egress_channels() {
    let provides = adapter_provides_with_effects(&["place_order"]);
    let runtime = runtime_for_graph(build_number_source_graph(), provides.clone());
    let adapter = adapter_config(provides);
    let egress_config = EgressConfig::builder(Duration::from_millis(50))
        .channel(
            "broker",
            EgressChannelConfig::process(vec!["/definitely/missing-egress-binary".to_string()])
                .expect("channel config should be valid"),
        )
        .expect("channel should insert")
        .route(
            "place_order",
            EgressRoute::new("broker", None).expect("route should be valid"),
        )
        .expect("route should insert")
        .build()
        .expect("egress config should build");

    let mut runner = HostedRunner::new(
        GraphId::new("g"),
        Constraints::default(),
        runtime,
        "runtime:test".to_string(),
        Some(adapter),
        Some(egress_config),
        Some("epv1:sha256:test".to_string()),
        None,
    )
    .expect("runner initialization should validate egress config");

    runner
        .replay_step(ExternalEvent::mechanical(
            EventId::new("replay_skip_egress"),
            ExternalEventKind::Command,
        ))
        .expect("replay mode must not spawn egress");

    let live_err = runner
        .step(HostedEvent {
            event_id: "live_startup".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: Some("price_bar".to_string()),
            payload: Some(serde_json::json!({"price": 1.0})),
        })
        .expect_err("live mode should attempt egress startup and fail");
    assert!(
        matches!(live_err, HostedStepError::EgressLifecycle(_)),
        "expected egress lifecycle error, got {live_err:?}"
    );
}

#[test]
fn runner_init_rejects_egress_and_replay_ownership_together() {
    let provides = adapter_provides_with_effects(&["place_order"]);
    let runtime = runtime_for_graph(build_number_source_graph(), provides.clone());
    let adapter = adapter_config(provides);
    let egress_config = EgressConfig::builder(Duration::from_millis(50))
        .channel(
            "broker",
            EgressChannelConfig::process(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "exit 0".to_string(),
            ])
            .expect("channel config should be valid"),
        )
        .expect("channel should insert")
        .route(
            "place_order",
            EgressRoute::new("broker", None).expect("route should be valid"),
        )
        .expect("route should insert")
        .build()
        .expect("egress config should build");

    let err = match HostedRunner::new(
        GraphId::new("g"),
        Constraints::default(),
        runtime,
        "runtime:test".to_string(),
        Some(adapter),
        Some(egress_config),
        Some("epv1:sha256:test".to_string()),
        Some(HashSet::from(["place_order".to_string()])),
    ) {
        Ok(_) => panic!("runner initialization must reject mixed live/replay ownership config"),
        Err(err) => err,
    };

    assert!(
        matches!(err, HostedStepError::EgressValidation(_)),
        "expected egress validation failure, got {err:?}"
    );
}

#[test]
fn runner_init_rejects_replay_ownership_overlap_with_handler_kind() {
    let provides = adapter_provides_with_effects(&["set_context"]);
    let runtime = runtime_for_graph(build_number_source_graph(), provides.clone());
    let adapter = adapter_config(provides);

    let err = match HostedRunner::new(
        GraphId::new("g"),
        Constraints::default(),
        runtime,
        "runtime:test".to_string(),
        Some(adapter),
        None,
        None,
        Some(HashSet::from(["set_context".to_string()])),
    ) {
        Ok(_) => panic!("replay ownership overlapping handler ownership must fail"),
        Err(err) => err,
    };

    assert!(
        matches!(err, HostedStepError::EgressValidation(_)),
        "expected egress validation failure, got {err:?}"
    );
}

#[test]
fn merged_payload_incoming_overrides_store() {
    let provides = adapter_provides_with_effects(&[]);
    let runtime = runtime_for_graph(build_merge_precedence_graph(), provides.clone());
    let adapter = adapter_config(provides);

    let mut runner = HostedRunner::new(
        GraphId::new("g"),
        Constraints::default(),
        runtime,
        "runtime:test".to_string(),
        Some(adapter),
        None,
        None,
        None,
    )
    .expect("hosted runner should initialize");

    let first = runner
        .step(HostedEvent {
            event_id: "e1".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: Some("price_bar".to_string()),
            payload: Some(serde_json::json!({"armed": false, "price": 10.0})),
        })
        .expect("first step should execute");
    assert_eq!(first.effects.len(), 1);
    assert_eq!(
        runner.context_snapshot().get("armed"),
        Some(&serde_json::json!(true))
    );

    let second = runner
        .step(HostedEvent {
            event_id: "e2".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: Some("price_bar".to_string()),
            payload: Some(serde_json::json!({"price": 11.0})),
        })
        .expect("second step should execute");
    assert!(
        second.effects.is_empty(),
        "store-sourced armed=true should suppress emit on second step"
    );

    let third = runner
        .step(HostedEvent {
            event_id: "e3".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: Some("price_bar".to_string()),
            payload: Some(serde_json::json!({"armed": false, "price": 12.0})),
        })
        .expect("third step should execute");
    assert_eq!(
        third.effects.len(),
        1,
        "incoming armed=false must override stored armed=true"
    );
}

#[test]
fn lifecycle_guard_rejects_step_when_pending_effects_exist() {
    let provides = adapter_provides_with_effects(&[]);
    let runtime = runtime_for_graph(build_context_set_bool_graph(), provides.clone());
    let adapter = adapter_config(provides);

    let mut runner = HostedRunner::new(
        GraphId::new("g"),
        Constraints::default(),
        runtime,
        "runtime:test".to_string(),
        Some(adapter),
        None,
        None,
        None,
    )
    .expect("hosted runner should initialize");

    let bypass = ergo_adapter::ExternalEvent::with_payload(
        EventId::new("bypass"),
        ExternalEventKind::Command,
        EventTime::default(),
        ergo_adapter::EventPayload {
            data: br#"{"price":101.5}"#.to_vec(),
        },
    )
    .expect("bypass event should construct");
    runner.session.on_event(bypass);

    let err = runner
        .step(HostedEvent {
            event_id: "e1".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: Some("price_bar".to_string()),
            payload: Some(serde_json::json!({"price": 11.0})),
        })
        .expect_err("pending buffer should trigger lifecycle guard");

    match err {
        HostedStepError::LifecycleViolation { detail } => {
            assert!(
                detail.contains("pending effect buffer must be drained"),
                "unexpected detail: {detail}"
            );
        }
        other => panic!("expected lifecycle violation, got {:?}", other),
    }
}

#[test]
fn handler_coverage_ignores_non_emittable_accepted_effect() {
    let provides = adapter_provides_with_effects(&["send_notification"]);

    let runtime_ok = runtime_for_graph(build_context_set_bool_graph(), provides.clone());
    let adapter_ok = adapter_config(provides.clone());
    let ok = HostedRunner::new(
        GraphId::new("g"),
        Constraints::default(),
        runtime_ok,
        "runtime:test".to_string(),
        Some(adapter_ok),
        None,
        None,
        None,
    );
    assert!(ok.is_ok());
}

#[test]
fn decision_order_preserves_effects_across_steps() {
    let provides = adapter_provides_for_number_effect();
    let runtime = runtime_for_graph(
        build_context_set_number_from_price_graph(),
        provides.clone(),
    );
    let adapter = adapter_config(provides);

    let mut runner = HostedRunner::new(
        GraphId::new("g"),
        Constraints::default(),
        runtime,
        "runtime:test".to_string(),
        Some(adapter),
        None,
        None,
        None,
    )
    .expect("hosted runner should initialize");

    runner
        .step(HostedEvent {
            event_id: "evt_1".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: Some("price_bar".to_string()),
            payload: Some(serde_json::json!({"price": 100.0})),
        })
        .expect("first duplicate-id step should execute");

    runner
        .step(HostedEvent {
            event_id: "evt_2".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: Some("price_bar".to_string()),
            payload: Some(serde_json::json!({"price": 200.0})),
        })
        .expect("second duplicate-id step should execute");

    let bundle = runner.into_capture_bundle();
    assert_eq!(bundle.decisions.len(), 2);

    let first_writes = &bundle.decisions[0].effects[0].effect.writes;
    let second_writes = &bundle.decisions[1].effects[0].effect.writes;

    assert_eq!(first_writes[0].key, "ema");
    assert_eq!(second_writes[0].key, "ema");
    assert_eq!(
        first_writes[0].value,
        ergo_runtime::common::Value::Number(100.0)
    );
    assert_eq!(
        second_writes[0].value,
        ergo_runtime::common::Value::Number(200.0)
    );
}

#[test]
fn step_rejects_duplicate_event_id() {
    let runtime = runtime_for_graph(build_number_source_graph(), AdapterProvides::default());
    let mut runner = HostedRunner::new(
        GraphId::new("g"),
        Constraints::default(),
        runtime,
        "runtime:test".to_string(),
        None,
        None,
        None,
        None,
    )
    .expect("hosted runner should initialize");

    runner
        .step(HostedEvent {
            event_id: "dup_evt".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: None,
            payload: Some(serde_json::json!({"foo": "bar"})),
        })
        .expect("first event should execute");

    let err = runner
        .step(HostedEvent {
            event_id: "dup_evt".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: None,
            payload: Some(serde_json::json!({"foo": "baz"})),
        })
        .expect_err("duplicate event id must fail");

    assert!(matches!(
        err,
        HostedStepError::DuplicateEventId { event_id } if event_id == "dup_evt"
    ));
}

#[test]
fn replay_step_rejects_duplicate_event_id() {
    let runtime = runtime_for_graph(build_number_source_graph(), AdapterProvides::default());
    let mut runner = HostedRunner::new(
        GraphId::new("g"),
        Constraints::default(),
        runtime,
        "runtime:test".to_string(),
        None,
        None,
        None,
        None,
    )
    .expect("hosted runner should initialize");

    let event = ExternalEvent::mechanical(EventId::new("dup_evt"), ExternalEventKind::Command);
    runner
        .replay_step(event.clone())
        .expect("first replay event should execute");

    let err = runner
        .replay_step(event)
        .expect_err("duplicate replay event id must fail");
    assert!(matches!(
        err,
        HostedStepError::DuplicateEventId { event_id } if event_id == "dup_evt"
    ));
}

#[test]
fn fatal_step_error_is_sticky_for_future_steps_and_finalization() {
    let provides = adapter_provides_with_effects(&[]);
    let runtime = runtime_for_graph(build_context_set_bool_graph(), provides.clone());
    let adapter = adapter_config(provides);
    let mut runner = HostedRunner::new(
        GraphId::new("g"),
        Constraints::default(),
        runtime,
        "runtime:test".to_string(),
        Some(adapter),
        None,
        None,
        None,
    )
    .expect("hosted runner should initialize");

    let bypass = ergo_adapter::ExternalEvent::with_payload(
        EventId::new("bypass"),
        ExternalEventKind::Command,
        EventTime::default(),
        ergo_adapter::EventPayload {
            data: br#"{"price":101.5}"#.to_vec(),
        },
    )
    .expect("bypass event should construct");
    runner.session.on_event(bypass);

    let first_err = runner
        .step(HostedEvent {
            event_id: "e1".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: Some("price_bar".to_string()),
            payload: Some(serde_json::json!({"price": 101.5})),
        })
        .expect_err("pending-effect lifecycle violation should be fatal");
    assert!(matches!(
        first_err,
        HostedStepError::LifecycleViolation { .. }
    ));

    let second_err = runner
        .step(HostedEvent {
            event_id: "e2".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: Some("price_bar".to_string()),
            payload: Some(serde_json::json!({"price": 102.0})),
        })
        .expect_err("fatal state should block future steps");
    assert!(matches!(
        second_err,
        HostedStepError::LifecycleViolation { .. }
    ));

    let finalize_err = runner
        .ensure_capture_finalizable()
        .expect_err("fatal state should block finalization");
    assert!(matches!(
        finalize_err,
        HostedStepError::LifecycleViolation { .. }
    ));
}
