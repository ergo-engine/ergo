use std::collections::HashMap;

use ergo_adapter::EventId;
use ergo_runtime::action::ActionOutcome;
use ergo_runtime::catalog::{CorePrimitiveCatalog, CoreRegistries};
use ergo_runtime::cluster::{
    ExpandedEdge, ExpandedEndpoint, ExpandedGraph, ExpandedNode, ImplementationInstance,
    OutputPortSpec, OutputRef, ParameterValue,
};
use ergo_runtime::runtime::{
    run, ExecutionContext, ExecutionReport, Registries, RuntimeEvent, RuntimeValue,
};

use crate::EpisodeId;

#[derive(Debug, Clone, PartialEq)]
pub struct Demo1Summary {
    pub sum_left: f64,
    pub sum_total: f64,
    pub action_a_outcome: ActionOutcome,
    pub action_b_outcome: ActionOutcome,
}

pub fn build_demo_1_graph() -> ExpandedGraph {
    let mut nodes = HashMap::new();

    nodes.insert(
        "src_left_a".to_string(),
        ExpandedNode {
            runtime_id: "src_left_a".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "number_source".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([("value".to_string(), ParameterValue::Number(4.0))]),
        },
    );

    nodes.insert(
        "src_left_b".to_string(),
        ExpandedNode {
            runtime_id: "src_left_b".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "number_source".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([("value".to_string(), ParameterValue::Number(2.0))]),
        },
    );

    nodes.insert(
        "src_right_a".to_string(),
        ExpandedNode {
            runtime_id: "src_right_a".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "number_source".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([("value".to_string(), ParameterValue::Number(1.0))]),
        },
    );

    nodes.insert(
        "src_right_b".to_string(),
        ExpandedNode {
            runtime_id: "src_right_b".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "number_source".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([("value".to_string(), ParameterValue::Number(1.0))]),
        },
    );

    nodes.insert(
        "add_left".to_string(),
        ExpandedNode {
            runtime_id: "add_left".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "add".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    nodes.insert(
        "add_right".to_string(),
        ExpandedNode {
            runtime_id: "add_right".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "add".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    nodes.insert(
        "add_total".to_string(),
        ExpandedNode {
            runtime_id: "add_total".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "add".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    nodes.insert(
        "gt_a".to_string(),
        ExpandedNode {
            runtime_id: "gt_a".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "gt".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    nodes.insert(
        "gt_b".to_string(),
        ExpandedNode {
            runtime_id: "gt_b".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "gt".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    nodes.insert(
        "emit_a".to_string(),
        ExpandedNode {
            runtime_id: "emit_a".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "emit_if_true".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    nodes.insert(
        "emit_b".to_string(),
        ExpandedNode {
            runtime_id: "emit_b".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "emit_if_true".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::new(),
        },
    );

    nodes.insert(
        "act_a".to_string(),
        ExpandedNode {
            runtime_id: "act_a".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "ack_action".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([("accept".to_string(), ParameterValue::Bool(true))]),
        },
    );

    nodes.insert(
        "act_b".to_string(),
        ExpandedNode {
            runtime_id: "act_b".to_string(),
            authoring_path: vec![],
            implementation: ImplementationInstance {
                impl_id: "ack_action".to_string(),
                version: "0.1.0".to_string(),
            },
            parameters: HashMap::from([("accept".to_string(), ParameterValue::Bool(true))]),
        },
    );

    let edges = vec![
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src_left_a".to_string(),
                port_name: "value".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "add_left".to_string(),
                port_name: "a".to_string(),
            },
        },
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src_left_b".to_string(),
                port_name: "value".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "add_left".to_string(),
                port_name: "b".to_string(),
            },
        },
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src_right_a".to_string(),
                port_name: "value".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "add_right".to_string(),
                port_name: "a".to_string(),
            },
        },
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "src_right_b".to_string(),
                port_name: "value".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "add_right".to_string(),
                port_name: "b".to_string(),
            },
        },
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "add_left".to_string(),
                port_name: "result".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "add_total".to_string(),
                port_name: "a".to_string(),
            },
        },
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "add_right".to_string(),
                port_name: "result".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "add_total".to_string(),
                port_name: "b".to_string(),
            },
        },
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "add_left".to_string(),
                port_name: "result".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "gt_a".to_string(),
                port_name: "a".to_string(),
            },
        },
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "add_right".to_string(),
                port_name: "result".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "gt_a".to_string(),
                port_name: "b".to_string(),
            },
        },
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "add_right".to_string(),
                port_name: "result".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "gt_b".to_string(),
                port_name: "a".to_string(),
            },
        },
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "add_left".to_string(),
                port_name: "result".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "gt_b".to_string(),
                port_name: "b".to_string(),
            },
        },
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "gt_a".to_string(),
                port_name: "result".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "emit_a".to_string(),
                port_name: "input".to_string(),
            },
        },
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "gt_b".to_string(),
                port_name: "result".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "emit_b".to_string(),
                port_name: "input".to_string(),
            },
        },
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "emit_a".to_string(),
                port_name: "event".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "act_a".to_string(),
                port_name: "event".to_string(),
            },
        },
        ExpandedEdge {
            from: ExpandedEndpoint::NodePort {
                node_id: "emit_b".to_string(),
                port_name: "event".to_string(),
            },
            to: ExpandedEndpoint::NodePort {
                node_id: "act_b".to_string(),
                port_name: "event".to_string(),
            },
        },
    ];

    ExpandedGraph {
        nodes,
        edges,
        boundary_inputs: Vec::new(),
        boundary_outputs: vec![
            OutputPortSpec {
                name: "sum_left".to_string(),
                maps_to: OutputRef {
                    node_id: "add_left".to_string(),
                    port_name: "result".to_string(),
                },
            },
            OutputPortSpec {
                name: "sum_total".to_string(),
                maps_to: OutputRef {
                    node_id: "add_total".to_string(),
                    port_name: "result".to_string(),
                },
            },
            OutputPortSpec {
                name: "action_a_outcome".to_string(),
                maps_to: OutputRef {
                    node_id: "act_a".to_string(),
                    port_name: "outcome".to_string(),
                },
            },
            OutputPortSpec {
                name: "action_b_outcome".to_string(),
                maps_to: OutputRef {
                    node_id: "act_b".to_string(),
                    port_name: "outcome".to_string(),
                },
            },
        ],
    }
}

fn number_output(report: &ExecutionReport, name: &str) -> f64 {
    match report.outputs.get(name) {
        Some(RuntimeValue::Number(value)) => *value,
        other => panic!("expected numeric output '{}', got {:?}", name, other),
    }
}

fn action_output(report: &ExecutionReport, name: &str) -> ActionOutcome {
    match report.outputs.get(name) {
        Some(RuntimeValue::Event(RuntimeEvent::Action(outcome))) => outcome.clone(),
        other => panic!("expected action output '{}', got {:?}", name, other),
    }
}

pub fn summarize_report(report: &ExecutionReport) -> Demo1Summary {
    Demo1Summary {
        sum_left: number_output(report, "sum_left"),
        sum_total: number_output(report, "sum_total"),
        action_a_outcome: action_output(report, "action_a_outcome"),
        action_b_outcome: action_output(report, "action_b_outcome"),
    }
}

pub fn compute_summary(
    graph: &ExpandedGraph,
    catalog: &CorePrimitiveCatalog,
    registries: &CoreRegistries,
) -> Demo1Summary {
    let runtime_registries = Registries {
        sources: &registries.sources,
        computes: &registries.computes,
        triggers: &registries.triggers,
        actions: &registries.actions,
    };
    let report = run(graph, catalog, &runtime_registries, &ExecutionContext)
        .expect("demo graph should execute");
    summarize_report(&report)
}

pub fn format_episode_summary(
    episode_id: EpisodeId,
    event_id: &EventId,
    summary: &Demo1Summary,
) -> String {
    let trigger_a = if summary.action_a_outcome == ActionOutcome::Skipped {
        "not_emitted"
    } else {
        "emitted"
    };
    let trigger_b = if summary.action_b_outcome == ActionOutcome::Skipped {
        "not_emitted"
    } else {
        "emitted"
    };
    let action_a_status = if summary.action_a_outcome == ActionOutcome::Skipped {
        "skipped"
    } else {
        "executed"
    };
    let action_b_status = if summary.action_b_outcome == ActionOutcome::Skipped {
        "skipped"
    } else {
        "executed"
    };

    format!(
        "episode {} ({}): TriggerA={} TriggerB={} ActionA={} ActionB={} sum_left={} sum_total={}",
        episode_id.as_u64(),
        event_id.as_str(),
        trigger_a,
        trigger_b,
        action_a_status,
        action_b_status,
        summary.sum_left,
        summary.sum_total
    )
}

pub fn format_replay_identity(matches: bool) -> String {
    let marker = if matches { "\u{2705}" } else { "\u{274c}" };
    format!("replay identity: {}", marker)
}
