// ===============================
// RUNTIME EXECUTION (PHASES 5–6)
//
// This module consumes ExpandedGraph.
// - No ExternalInput is permitted.
// - All inputs must originate from Source primitives.
// - ExecutionContext must not supply values.
// - Single unified DAG, single execution pass.
//
// DO NOT introduce alternative input paths.
// ===============================

use std::collections::{BTreeSet, HashMap};

use crate::cluster::{ExpandedEndpoint, ExpandedGraph, PrimitiveCatalog, PrimitiveKind, ValueType};

use super::types::{Endpoint, ValidatedEdge, ValidatedGraph, ValidatedNode, ValidationError};

pub fn validate<C: PrimitiveCatalog>(
    expanded: &ExpandedGraph,
    catalog: &C,
) -> Result<ValidatedGraph, ValidationError> {
    let mut nodes: HashMap<String, ValidatedNode> = HashMap::new();

    for (id, node) in &expanded.nodes {
        let meta = catalog
            .get(&node.implementation.impl_id, &node.implementation.version)
            .ok_or_else(|| ValidationError::MissingPrimitive {
                id: node.implementation.impl_id.clone(),
                version: node.implementation.version.clone(),
            })?;

        nodes.insert(
            id.clone(),
            ValidatedNode {
                runtime_id: id.clone(),
                impl_id: node.implementation.impl_id.clone(),
                version: node.implementation.version.clone(),
                kind: meta.kind.clone(),
                inputs: meta.inputs.clone(),
                outputs: meta.outputs.clone(),
                parameters: node.parameters.clone(),
            },
        );
    }

    let edges: Vec<ValidatedEdge> = expanded
        .edges
        .iter()
        .map(|e| {
            Ok(ValidatedEdge {
                from: map_endpoint(&e.from)?,
                to: map_endpoint(&e.to)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    enforce_edge_nodes_exist(&nodes, &edges)?;
    let topo_order = topological_sort(&nodes, &edges)?;

    enforce_wiring_matrix(&nodes, &edges)?;
    enforce_required_inputs(&nodes, &edges)?;
    enforce_types(&nodes, &edges)?;
    enforce_action_gating(&nodes, &edges)?;
    enforce_boundary_outputs(&nodes, &expanded.boundary_outputs)?;

    Ok(ValidatedGraph {
        nodes,
        edges,
        topo_order,
        boundary_outputs: expanded.boundary_outputs.clone(),
    })
}

fn map_endpoint(ep: &ExpandedEndpoint) -> Result<Endpoint, ValidationError> {
    match ep {
        ExpandedEndpoint::NodePort { node_id, port_name } => Ok(Endpoint::NodePort {
            node_id: node_id.clone(),
            port_name: port_name.clone(),
        }),
        ExpandedEndpoint::ExternalInput { name } => {
            Err(ValidationError::ExternalInputNotAllowed { name: name.clone() })
        }
    }
}

fn topological_sort(
    nodes: &HashMap<String, ValidatedNode>,
    edges: &[ValidatedEdge],
) -> Result<Vec<String>, ValidationError> {
    let mut in_degree: HashMap<String, usize> = nodes.keys().map(|k| (k.clone(), 0)).collect();
    let mut dependents: HashMap<String, Vec<String>> =
        nodes.keys().map(|k| (k.clone(), vec![])).collect();

    for edge in edges {
        let Endpoint::NodePort { node_id: from, .. } = &edge.from;
        let Endpoint::NodePort { node_id: to, .. } = &edge.to;
        *in_degree
            .get_mut(to)
            .ok_or_else(|| ValidationError::UnknownNode(to.clone()))? += 1;
        dependents
            .get_mut(from)
            .ok_or_else(|| ValidationError::UnknownNode(from.clone()))?
            .push(to.clone());
    }

    let mut queue: BTreeSet<String> = in_degree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(id, _)| id.clone())
        .collect();

    let mut sorted = Vec::new();

    while let Some(node_id) = queue.iter().next().cloned() {
        queue.remove(&node_id);
        sorted.push(node_id.clone());

        if let Some(deps) = dependents.get(&node_id) {
            for dep in deps {
                let deg = in_degree
                    .get_mut(dep)
                    .ok_or_else(|| ValidationError::UnknownNode(dep.clone()))?;
                *deg -= 1;
                if *deg == 0 {
                    queue.insert(dep.clone());
                }
            }
        }
    }

    if sorted.len() != nodes.len() {
        return Err(ValidationError::CycleDetected);
    }

    Ok(sorted)
}

fn enforce_edge_nodes_exist(
    nodes: &HashMap<String, ValidatedNode>,
    edges: &[ValidatedEdge],
) -> Result<(), ValidationError> {
    for edge in edges {
        let Endpoint::NodePort { node_id: from, .. } = &edge.from;
        if !nodes.contains_key(from) {
            return Err(ValidationError::UnknownNode(from.clone()));
        }

        let Endpoint::NodePort { node_id: to, .. } = &edge.to;
        if !nodes.contains_key(to) {
            return Err(ValidationError::UnknownNode(to.clone()));
        }
    }
    Ok(())
}

fn enforce_wiring_matrix(
    nodes: &HashMap<String, ValidatedNode>,
    edges: &[ValidatedEdge],
) -> Result<(), ValidationError> {
    for edge in edges {
        let Endpoint::NodePort { node_id: from, .. } = &edge.from;
        let Endpoint::NodePort { node_id: to, .. } = &edge.to;

        let from_kind = &nodes
            .get(from)
            .ok_or_else(|| ValidationError::UnknownNode(from.clone()))?
            .kind;
        let to_kind = &nodes
            .get(to)
            .ok_or_else(|| ValidationError::UnknownNode(to.clone()))?
            .kind;

        if !wiring_allowed(from_kind, to_kind) {
            return Err(ValidationError::InvalidEdgeKind {
                from: from_kind.clone(),
                to: to_kind.clone(),
            });
        }
    }
    Ok(())
}

fn enforce_required_inputs(
    nodes: &HashMap<String, ValidatedNode>,
    edges: &[ValidatedEdge],
) -> Result<(), ValidationError> {
    let mut incoming: HashMap<(&String, &str), bool> = HashMap::new();
    for edge in edges {
        let Endpoint::NodePort {
            node_id: to,
            port_name,
        } = &edge.to;
        incoming.insert((to, port_name.as_str()), true);
    }

    for node in nodes.values() {
        for input in node.required_inputs() {
            if !incoming.contains_key(&(&node.runtime_id, input.name.as_str())) {
                return Err(ValidationError::MissingRequiredInput {
                    node: node.runtime_id.clone(),
                    input: input.name.clone(),
                });
            }
        }
    }
    Ok(())
}

fn enforce_types(
    nodes: &HashMap<String, ValidatedNode>,
    edges: &[ValidatedEdge],
) -> Result<(), ValidationError> {
    for edge in edges {
        let Endpoint::NodePort {
            node_id: from,
            port_name: from_port,
        } = &edge.from;
        let Endpoint::NodePort {
            node_id: to,
            port_name: to_port,
        } = &edge.to;

        let from_node = nodes
            .get(from)
            .ok_or_else(|| ValidationError::UnknownNode(from.clone()))?;
        let to_node = nodes
            .get(to)
            .ok_or_else(|| ValidationError::UnknownNode(to.clone()))?;

        let from_type = from_node
            .outputs
            .get(from_port)
            .ok_or_else(|| ValidationError::MissingOutputMetadata {
                node: from.clone(),
                output: from_port.clone(),
            })?
            .value_type
            .clone();

        let expected = to_node
            .inputs
            .iter()
            .find(|i| i.name == *to_port)
            .ok_or_else(|| ValidationError::MissingInputMetadata {
                node: to.clone(),
                input: to_port.clone(),
            })?
            .value_type
            .clone();

        if from_type != expected {
            return Err(ValidationError::TypeMismatch {
                from: from.clone(),
                output: from_port.clone(),
                to: to.clone(),
                input: to_port.clone(),
                expected,
                got: from_type,
            });
        }
    }

    Ok(())
}

fn enforce_action_gating(
    nodes: &HashMap<String, ValidatedNode>,
    edges: &[ValidatedEdge],
) -> Result<(), ValidationError> {
    let mut action_inputs: HashMap<String, bool> = HashMap::new();

    for edge in edges {
        let Endpoint::NodePort { node_id: to, .. } = &edge.to;
        if let Some(target) = nodes.get(to) {
            if target.kind == PrimitiveKind::Action {
                let Endpoint::NodePort {
                    node_id: from,
                    port_name: from_port,
                } = &edge.from;
                if let Some(src) = nodes.get(from) {
                    if src.kind == PrimitiveKind::Trigger {
                        if let Some(meta) = src.outputs.get(from_port) {
                            if meta.value_type == ValueType::Event {
                                action_inputs.insert(to.clone(), true);
                            }
                        }
                    }
                }
            }
        }
    }

    for (id, node) in nodes {
        if node.kind == PrimitiveKind::Action {
            if !action_inputs.get(id).copied().unwrap_or(false) {
                return Err(ValidationError::ActionNotGated(id.clone()));
            }
        }
    }

    Ok(())
}

fn enforce_boundary_outputs(
    nodes: &HashMap<String, ValidatedNode>,
    boundary_outputs: &[crate::cluster::OutputPortSpec],
) -> Result<(), ValidationError> {
    for output in boundary_outputs {
        let target_node = nodes
            .get(&output.maps_to.node_id)
            .ok_or_else(|| ValidationError::UnknownNode(output.maps_to.node_id.clone()))?;

        if !target_node.outputs.contains_key(&output.maps_to.port_name) {
            return Err(ValidationError::MissingOutputMetadata {
                node: output.maps_to.node_id.clone(),
                output: output.maps_to.port_name.clone(),
            });
        }
    }

    Ok(())
}

fn wiring_allowed(from: &PrimitiveKind, to: &PrimitiveKind) -> bool {
    match (from, to) {
        (PrimitiveKind::Source, PrimitiveKind::Compute) => true,

        (PrimitiveKind::Compute, PrimitiveKind::Compute) => true,
        (PrimitiveKind::Compute, PrimitiveKind::Trigger) => true,

        (PrimitiveKind::Trigger, PrimitiveKind::Trigger) => true,
        (PrimitiveKind::Trigger, PrimitiveKind::Action) => true,

        _ => false,
    }
}
