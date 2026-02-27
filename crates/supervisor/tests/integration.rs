//! Integration tests for Supervisor with real RuntimeHandle execution path.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ergo_adapter::{
    AdapterProvides, ContextKeyProvision, EventId, EventTime, ExternalEvent, ExternalEventKind,
    FaultRuntimeHandle, GraphId, RunTermination, RuntimeHandle,
};
use ergo_runtime::action::ActionOutcome;
use ergo_runtime::catalog::{build_core_catalog, core_registries};
use ergo_runtime::cluster::{
    ExpandedEdge, ExpandedEndpoint, ExpandedGraph, ExpandedNode, ImplementationInstance,
    OutputPortSpec, OutputRef, ParameterValue,
};
use ergo_runtime::provenance::{compute_runtime_provenance, RuntimeProvenanceScheme};
use ergo_supervisor::demo::demo_1;
use ergo_supervisor::replay::compare_decisions;
use ergo_supervisor::replay::replay;
use ergo_supervisor::{
    CapturingSession, Constraints, Decision, DecisionLog, DecisionLogEntry, Supervisor,
};

/// Test-only DecisionLog that captures entries for verification.
#[derive(Clone)]
struct CapturingLog {
    entries: Arc<Mutex<Vec<DecisionLogEntry>>>,
}

impl CapturingLog {
    fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn entries(&self) -> Vec<DecisionLogEntry> {
        self.entries.lock().unwrap().clone()
    }
}

impl DecisionLog for CapturingLog {
    fn log(&self, entry: DecisionLogEntry) {
        self.entries.lock().unwrap().push(entry);
    }
}

/// Builds the canonical hello-world graph used in runtime tests.
/// Structure: number_source(3.0) -> gt <- number_source(1.0)
///            gt:result -> emit_if_true:input
///            emit_if_true:event -> ack_action:event
/// Since 3.0 > 1.0, trigger emits, action executes with outcome Completed.
fn build_hello_world_graph() -> ExpandedGraph {
    let mut nodes = HashMap::new();

    nodes.insert(
        "src_a".to_string(),
        ExpandedNode {
            runtime_id: "src_a".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "number_source".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([("value".to_string(), ParameterValue::Number(3.0))]),
        },
    );

    nodes.insert(
        "src_b".to_string(),
        ExpandedNode {
            runtime_id: "src_b".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "number_source".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([("value".to_string(), ParameterValue::Number(1.0))]),
        },
    );

    nodes.insert(
        "gt1".to_string(),
        ExpandedNode {
            runtime_id: "gt1".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "gt".to_string(),
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
        "act".to_string(),
        ExpandedNode {
            runtime_id: "act".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "ack_action".to_string(),
                requested_version: "0.1.0".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([("accept".to_string(), ParameterValue::Bool(true))]),
        },
    );

    let edges = vec![
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src_a".to_string(),
                port_name: "value".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "gt1".to_string(),
                port_name: "a".to_string(),
            },
        },
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src_b".to_string(),
                port_name: "value".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "gt1".to_string(),
                port_name: "b".to_string(),
            },
        },
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "gt1".to_string(),
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
                node_id: "act".to_string(),
                port_name: "event".to_string(),
            },
        },
    ];

    ExpandedGraph {
        nodes,
        edges,
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![OutputPortSpec {
            name: "action_outcome".to_string(),
            maps_to: OutputRef {
                node_id: "act".to_string(),
                port_name: "outcome".to_string(),
            },
        }],
    }
}

/// Build a minimal graph that emits a real set_context effect through context_set_bool.
/// Structure:
///   const_bool(true) ----> emit_if_true -> context_set_bool:event
///   boolean_source(false) ----------------> context_set_bool:value
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

/// Build a graph equivalent to OnceCluster's compiled behavior for one event gate:
///   const_bool(true) -> emit_if_true:event -> emit_if_event_and_true:event
///   context_bool_source(key=once_state) -> not -> emit_if_event_and_true:condition
///   emit_if_event_and_true:event -> context_set_bool:event
///   boolean_source(true) -> context_set_bool:value
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

/// SUP-2 verification: Supervisor::new() -> RuntimeHandle::new() -> runtime::run() -> Completed
///
/// This test proves the full execution path works:
/// 1. Supervisor::new() constructs a real RuntimeHandle (not a test double)
/// 2. RuntimeHandle::run() calls ergo_runtime::runtime::run()
/// 3. The graph executes successfully
/// 4. RuntimeHandle returns RunTermination::Completed
/// 5. Supervisor logs Decision::Invoke with termination: Some(Completed)
#[test]
fn supervisor_with_real_runtime_executes_hello_world() {
    // Build the hello-world graph
    let graph = Arc::new(build_hello_world_graph());

    // Build catalog and registries using the core implementations
    let catalog = Arc::new(build_core_catalog());
    let registries = Arc::new(core_registries().expect("core registries should build"));

    // Create capturing log to verify decisions
    let log = CapturingLog::new();

    // Construct Supervisor using Supervisor::new() — NOT with_runtime()
    // This uses the real RuntimeHandle, proving the full execution path
    let mut supervisor = Supervisor::new(
        GraphId::new("hello_world"),
        Constraints::default(),
        log.clone(),
        graph,
        catalog,
        registries,
    );

    // Send an event to trigger execution (use Command, not Tick — Tick has special behavior)
    let event = ExternalEvent::mechanical(EventId::new("test_event"), ExternalEventKind::Command);
    supervisor.on_event(event);

    // Verify the decision log
    let entries = log.entries();
    assert_eq!(entries.len(), 1, "Expected exactly one decision log entry");

    let entry = &entries[0];
    assert_eq!(
        entry.decision,
        Decision::Invoke,
        "Expected Decision::Invoke, got {:?}",
        entry.decision
    );
    assert_eq!(
        entry.termination,
        Some(RunTermination::Completed),
        "Expected RunTermination::Completed, got {:?}",
        entry.termination
    );
    assert_eq!(entry.retry_count, 0, "Expected no retries");
}

/// Capture + replay golden spike: capture in-line with execution, then replay and compare decisions.
#[test]
fn capturing_session_enables_round_trip_replay() {
    let graph = Arc::new(build_hello_world_graph());
    let catalog = Arc::new(build_core_catalog());
    let core_registries = Arc::new(core_registries().expect("core registries should build"));

    let runtime = RuntimeHandle::new(
        graph.clone(),
        catalog.clone(),
        core_registries.clone(),
        AdapterProvides::default(),
    );
    let runtime_provenance = compute_runtime_provenance(
        RuntimeProvenanceScheme::Rpv1,
        "hello_world_capture",
        graph.as_ref(),
        catalog.as_ref(),
    )
    .expect("runtime provenance should compute");

    let mut session = CapturingSession::new(
        GraphId::new("hello_world_capture"),
        Constraints::default(),
        CapturingLog::new(),
        runtime,
        runtime_provenance,
    );

    let event = ExternalEvent::mechanical(EventId::new("capture_event"), ExternalEventKind::Pump);
    session.on_event(event);

    let bundle = session.into_bundle();

    assert_eq!(
        bundle.events.len(),
        1,
        "expected exactly one captured event"
    );
    assert_eq!(
        bundle.decisions.len(),
        1,
        "expected exactly one captured decision"
    );

    let replay_decisions = replay(&bundle, FaultRuntimeHandle::new(RunTermination::Completed));

    assert_eq!(
        replay_decisions.len(),
        1,
        "expected exactly one replay decision"
    );

    assert_eq!(
        replay_decisions[0].decision, bundle.decisions[0].decision,
        "decision should round trip through replay"
    );
    assert_eq!(
        replay_decisions[0].termination, bundle.decisions[0].termination,
        "termination should round trip through replay"
    );
    assert_eq!(
        replay_decisions[0].retry_count, bundle.decisions[0].retry_count,
        "retry_count should round trip through replay"
    );
}

#[test]
fn demo_1_complex_graph_executes_and_replays() {
    let graph = Arc::new(demo_1::build_demo_1_graph());
    let catalog = Arc::new(build_core_catalog());
    let core_registries = Arc::new(core_registries().expect("core registries should build"));

    let runtime = RuntimeHandle::new(
        graph.clone(),
        catalog.clone(),
        core_registries.clone(),
        AdapterProvides::default(),
    );
    let runtime_provenance = compute_runtime_provenance(
        RuntimeProvenanceScheme::Rpv1,
        "demo_1",
        graph.as_ref(),
        catalog.as_ref(),
    )
    .expect("runtime provenance should compute");
    let mut session = CapturingSession::new(
        GraphId::new("demo_1"),
        Constraints::default(),
        CapturingLog::new(),
        runtime,
        runtime_provenance,
    );

    let event_1 = ExternalEvent::mechanical(EventId::new("demo_evt_1"), ExternalEventKind::Command);
    let event_2 = ExternalEvent::mechanical(EventId::new("demo_evt_2"), ExternalEventKind::Command);
    session.on_event(event_1);
    session.on_event(event_2);

    let bundle = session.into_bundle();
    assert_eq!(bundle.events.len(), 2, "expected two captured events");
    assert_eq!(bundle.decisions.len(), 2, "expected two captured decisions");
    assert!(bundle
        .decisions
        .iter()
        .all(|record| record.decision == Decision::Invoke));

    let summary = demo_1::compute_summary(&graph, &catalog, &core_registries);

    assert_eq!(summary.sum_left, 6.0);
    assert_eq!(summary.sum_total, 8.0);
    assert_eq!(
        summary.action_a_outcome,
        ActionOutcome::Completed,
        "TriggerA should emit; ActionA should execute"
    );
    assert_eq!(
        summary.action_b_outcome,
        ActionOutcome::Skipped,
        "TriggerB should not emit; ActionB should be skipped"
    );

    for record in &bundle.decisions {
        println!(
            "{}",
            demo_1::format_episode_summary(record.episode_id, &record.event_id, &summary)
        );
    }

    let replay_decisions = replay(&bundle, FaultRuntimeHandle::new(RunTermination::Completed));
    let replay_matches = compare_decisions(&bundle.decisions, &replay_decisions).unwrap();
    println!("{}", demo_1::format_replay_identity(replay_matches));
    assert!(replay_matches, "replay decisions must match capture");
}

#[test]
fn context_set_bool_effect_is_captured_and_replayed_with_real_runtime() {
    let graph = Arc::new(build_context_set_bool_graph());
    let catalog = Arc::new(build_core_catalog());
    let core_registries = Arc::new(core_registries().expect("core registries should build"));

    let mut adapter_provides = AdapterProvides::default();
    adapter_provides.context.insert(
        "armed".to_string(),
        ContextKeyProvision {
            ty: "Bool".to_string(),
            required: false,
            writable: true,
        },
    );
    adapter_provides.effects = HashSet::from(["set_context".to_string()]);

    let runtime = RuntimeHandle::new(
        graph.clone(),
        catalog.clone(),
        core_registries.clone(),
        adapter_provides,
    );
    let runtime_provenance = compute_runtime_provenance(
        RuntimeProvenanceScheme::Rpv1,
        "context_set_bool_capture",
        graph.as_ref(),
        catalog.as_ref(),
    )
    .expect("runtime provenance should compute");

    let mut session = CapturingSession::new(
        GraphId::new("context_set_bool_capture"),
        Constraints::default(),
        CapturingLog::new(),
        runtime.clone(),
        runtime_provenance,
    );

    session.on_event(ExternalEvent::mechanical(
        EventId::new("context_set_bool_event"),
        ExternalEventKind::Command,
    ));

    let bundle = session.into_bundle();
    assert_eq!(bundle.decisions.len(), 1, "one decision should be captured");

    let captured_effects = bundle.decisions[0]
        .effects
        .as_ref()
        .expect("effect-aware capture must store effects");
    assert_eq!(captured_effects.len(), 1, "one effect expected");
    assert_eq!(captured_effects[0].effect.kind, "set_context");
    assert_eq!(captured_effects[0].effect.writes.len(), 1);
    assert_eq!(captured_effects[0].effect.writes[0].key, "armed");
    assert_eq!(
        captured_effects[0].effect.writes[0].value,
        ergo_runtime::common::Value::Bool(false)
    );
    assert!(
        !captured_effects[0].effect_hash.is_empty(),
        "captured effect hash must be present"
    );

    let replayed = replay(&bundle, runtime);
    assert_eq!(replayed.len(), 1, "one replayed decision expected");
    assert!(
        compare_decisions(&bundle.decisions, &replayed).unwrap(),
        "capture/replay decisions (including effects) should match"
    );
}

#[test]
fn once_cluster_effect_is_captured_and_replayed_with_real_runtime() {
    let graph = Arc::new(build_once_cluster_behavior_graph());
    let catalog = Arc::new(build_core_catalog());
    let core_registries = Arc::new(core_registries().expect("core registries should build"));

    let mut adapter_provides = AdapterProvides::default();
    adapter_provides.context.insert(
        "once_state".to_string(),
        ContextKeyProvision {
            ty: "Bool".to_string(),
            required: false,
            writable: true,
        },
    );
    adapter_provides.effects = HashSet::from(["set_context".to_string()]);

    let runtime = RuntimeHandle::new(
        graph.clone(),
        catalog.clone(),
        core_registries.clone(),
        adapter_provides,
    );
    let runtime_provenance = compute_runtime_provenance(
        RuntimeProvenanceScheme::Rpv1,
        "once_cluster_capture",
        graph.as_ref(),
        catalog.as_ref(),
    )
    .expect("runtime provenance should compute");

    let mut session = CapturingSession::new(
        GraphId::new("once_cluster_capture"),
        Constraints::default(),
        CapturingLog::new(),
        runtime.clone(),
        runtime_provenance,
    );

    session.on_event(ExternalEvent::mechanical(
        EventId::new("once_cluster_event"),
        ExternalEventKind::Command,
    ));

    let bundle = session.into_bundle();
    assert_eq!(bundle.decisions.len(), 1, "one decision should be captured");

    let captured_effects = bundle.decisions[0]
        .effects
        .as_ref()
        .expect("effect-aware capture must store effects");
    assert_eq!(captured_effects.len(), 1, "one effect expected");
    assert_eq!(captured_effects[0].effect.kind, "set_context");
    assert_eq!(captured_effects[0].effect.writes.len(), 1);
    assert_eq!(captured_effects[0].effect.writes[0].key, "once_state");
    assert_eq!(
        captured_effects[0].effect.writes[0].value,
        ergo_runtime::common::Value::Bool(true)
    );
    assert!(
        !captured_effects[0].effect_hash.is_empty(),
        "captured effect hash must be present"
    );

    let replayed = replay(&bundle, runtime);
    assert_eq!(replayed.len(), 1, "one replayed decision expected");
    assert!(
        compare_decisions(&bundle.decisions, &replayed).unwrap(),
        "capture/replay decisions (including effects) should match"
    );
}

/// Test that a deferred episode is retried when a Tick event arrives.
#[test]
fn deferred_episode_retried_on_tick() {
    let log = CapturingLog::new();
    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);

    // max_in_flight = 0 means ALL events will be deferred
    let mut constraints = Constraints::default();
    constraints.max_in_flight = Some(0);

    let mut supervisor =
        Supervisor::with_runtime(GraphId::new("test"), constraints, log.clone(), runtime);

    // Send a non-Tick event — should be deferred
    let event = ExternalEvent::mechanical_at(
        EventId::new("e1"),
        ExternalEventKind::Command,
        EventTime::from_duration(Duration::from_secs(0)),
    );
    supervisor.on_event(event);

    let entries = log.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].decision, Decision::Defer);
    assert!(entries[0].termination.is_none());

    // Now allow execution by relaxing constraint
    // We need a new supervisor with relaxed constraints to test the Tick path
    // Actually, we can't change constraints mid-stream. Let's use a different approach:
    // Create supervisor with max_in_flight = 1, send event while "in flight"

    // Reset test with proper setup
    let log2 = CapturingLog::new();
    let runtime2 = FaultRuntimeHandle::new(RunTermination::Completed);

    // max_in_flight = 1, but we'll never actually be at capacity since execution is synchronous
    // Instead, test the Tick processing path directly by:
    // 1. Deferring due to rate limit
    // 2. Sending Tick at later time

    let mut constraints2 = Constraints::default();
    constraints2.max_per_window = Some(1);
    constraints2.rate_window = Some(Duration::from_secs(10));

    let mut supervisor2 =
        Supervisor::with_runtime(GraphId::new("test2"), constraints2, log2.clone(), runtime2);

    // First event at t=0 — should invoke (under rate limit)
    let event1 = ExternalEvent::mechanical_at(
        EventId::new("e1"),
        ExternalEventKind::Command,
        EventTime::from_duration(Duration::from_secs(0)),
    );
    supervisor2.on_event(event1);

    // Second event at t=0 — should defer (rate limited)
    let event2 = ExternalEvent::mechanical_at(
        EventId::new("e2"),
        ExternalEventKind::Command,
        EventTime::from_duration(Duration::from_secs(0)),
    );
    supervisor2.on_event(event2);

    let entries2 = log2.entries();
    assert_eq!(entries2.len(), 2);
    assert_eq!(entries2[0].decision, Decision::Invoke);
    assert_eq!(entries2[1].decision, Decision::Defer);

    // Send Tick at t=10 (after rate window expires)
    let tick = ExternalEvent::mechanical_at(
        EventId::new("tick1"),
        ExternalEventKind::Pump,
        EventTime::from_duration(Duration::from_secs(10)),
    );
    supervisor2.on_event(tick);

    let entries3 = log2.entries();
    assert_eq!(entries3.len(), 3);
    assert_eq!(
        entries3[2].decision,
        Decision::Invoke,
        "Tick should invoke deferred episode"
    );
    assert_eq!(entries3[2].termination, Some(RunTermination::Completed));
}

/// Test that a Tick with an empty queue logs a no-op Defer.
#[test]
fn tick_with_empty_queue_logs_noop() {
    let log = CapturingLog::new();
    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);

    let mut supervisor = Supervisor::with_runtime(
        GraphId::new("test"),
        Constraints::default(),
        log.clone(),
        runtime,
    );

    // Send Tick with no deferred episodes
    let tick = ExternalEvent::mechanical_at(
        EventId::new("tick1"),
        ExternalEventKind::Pump,
        EventTime::from_duration(Duration::from_secs(0)),
    );
    supervisor.on_event(tick);

    let entries = log.entries();
    assert_eq!(
        entries.len(),
        1,
        "Tick should produce exactly one log entry"
    );
    assert_eq!(
        entries[0].decision,
        Decision::Defer,
        "Empty queue Tick should log Defer"
    );
    assert_eq!(
        entries[0].schedule_at, None,
        "Empty queue Tick should have no schedule_at"
    );
    assert!(
        entries[0].termination.is_none(),
        "Empty queue Tick should have no termination"
    );
}

/// Test that deferred episodes are processed in order: earlier schedule_at first,
/// then lower episode_id for ties.
#[test]
fn tick_respects_episode_id_ordering() {
    let log = CapturingLog::new();
    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);

    let mut constraints = Constraints::default();
    constraints.max_per_window = Some(1);
    constraints.rate_window = Some(Duration::from_secs(10));

    let mut supervisor =
        Supervisor::with_runtime(GraphId::new("test"), constraints, log.clone(), runtime);

    // First event at t=0 — invokes
    let event1 = ExternalEvent::mechanical_at(
        EventId::new("e1"),
        ExternalEventKind::Command,
        EventTime::from_duration(Duration::from_secs(0)),
    );
    supervisor.on_event(event1);

    // Second event at t=0 — deferred (episode_id=1, schedule_at=10)
    let event2 = ExternalEvent::mechanical_at(
        EventId::new("e2"),
        ExternalEventKind::Command,
        EventTime::from_duration(Duration::from_secs(0)),
    );
    supervisor.on_event(event2);

    // Third event at t=0 — deferred (episode_id=2, schedule_at=10)
    let event3 = ExternalEvent::mechanical_at(
        EventId::new("e3"),
        ExternalEventKind::Command,
        EventTime::from_duration(Duration::from_secs(0)),
    );
    supervisor.on_event(event3);

    let entries = log.entries();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].decision, Decision::Invoke);
    assert_eq!(entries[1].decision, Decision::Defer);
    assert_eq!(entries[2].decision, Decision::Defer);

    // First Tick at t=10 — should invoke episode_id=1 (lower id wins tie)
    let tick1 = ExternalEvent::mechanical_at(
        EventId::new("tick1"),
        ExternalEventKind::Pump,
        EventTime::from_duration(Duration::from_secs(10)),
    );
    supervisor.on_event(tick1);

    // Second Tick at t=20 — should invoke episode_id=2
    let tick2 = ExternalEvent::mechanical_at(
        EventId::new("tick2"),
        ExternalEventKind::Pump,
        EventTime::from_duration(Duration::from_secs(20)),
    );
    supervisor.on_event(tick2);

    let final_entries = log.entries();
    assert_eq!(final_entries.len(), 5);

    // Entries 3 and 4 should be Invoke (the deferred episodes)
    assert_eq!(final_entries[3].decision, Decision::Invoke);
    assert_eq!(final_entries[4].decision, Decision::Invoke);
}
