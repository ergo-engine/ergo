/**
 * Serialization from UI model to ExpandedGraph.
 *
 * This is the critical translation layer between UI state and runtime contract.
 */

import type { UIGraph, UINode, UIEdge, UIBoundaryOutput, UIParamValue } from './internalModel';
import type {
  ExpandedGraph,
  ExpandedNode,
  ExpandedEdge,
  ExpandedEndpoint,
  ParameterValue,
  OutputPortSpec,
} from '../contract/contractTypes';

// ============================================================================
// Parameter Conversion
// ============================================================================

function convertParam(param: UIParamValue): ParameterValue {
  switch (param.type) {
    case 'int':
      return { type: 'Int', value: param.value };
    case 'number':
      return { type: 'Number', value: param.value };
    case 'bool':
      return { type: 'Bool', value: param.value };
    case 'string':
      return { type: 'String', value: param.value };
    case 'enum':
      return { type: 'Enum', value: param.value };
  }
}

// ============================================================================
// Node Conversion
// ============================================================================

function convertNode(node: UINode): ExpandedNode {
  const parameters: Record<string, ParameterValue> = {};

  for (const [key, value] of Object.entries(node.params)) {
    parameters[key] = convertParam(value);
  }

  return {
    runtime_id: node.id,
    authoring_path: [],  // Empty for flat graphs
    implementation: {
      impl_id: node.type,
      version: node.version,
    },
    parameters,
  };
}

// ============================================================================
// Edge Conversion
// ============================================================================

function convertEdge(edge: UIEdge): ExpandedEdge {
  const from: ExpandedEndpoint = {
    type: 'NodePort',
    node_id: edge.fromNodeId,
    port_name: edge.fromPort,
  };

  const to: ExpandedEndpoint = {
    type: 'NodePort',
    node_id: edge.toNodeId,
    port_name: edge.toPort,
  };

  return { from, to };
}

// ============================================================================
// Boundary Output Conversion
// ============================================================================

function convertBoundaryOutput(output: UIBoundaryOutput): OutputPortSpec {
  return {
    name: output.name,
    maps_to: {
      node_id: output.nodeId,
      port_name: output.portName,
    },
  };
}

// ============================================================================
// Main Serialization Function
// ============================================================================

/**
 * Convert a UIGraph to an ExpandedGraph for runtime execution.
 *
 * This strips visual properties and converts to the contract format.
 */
export function serializeToExpandedGraph(uiGraph: UIGraph): ExpandedGraph {
  // Convert nodes to a Record keyed by runtime_id
  const nodes: Record<string, ExpandedNode> = {};
  for (const node of uiGraph.nodes) {
    const expandedNode = convertNode(node);
    nodes[expandedNode.runtime_id] = expandedNode;
  }

  // Convert edges
  const edges: ExpandedEdge[] = uiGraph.edges.map(convertEdge);

  // Convert boundary outputs
  const boundary_outputs: OutputPortSpec[] = uiGraph.boundaryOutputs.map(convertBoundaryOutput);

  return {
    nodes,
    edges,
    boundary_inputs: [],  // Empty for runtime execution
    boundary_outputs,
  };
}
