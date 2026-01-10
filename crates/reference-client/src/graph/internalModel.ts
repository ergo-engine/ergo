/**
 * Internal UI model for graph authoring.
 *
 * This is the UI's working representation of a graph.
 * It includes visual properties (x, y) that are not part of the runtime contract.
 *
 * The serialize module converts this to ExpandedGraph for runtime execution.
 */

// ============================================================================
// UI Node Model
// ============================================================================

export interface UINode {
  id: string;                    // Unique ID (becomes runtime_id)
  type: string;                  // impl_id (e.g., "number_source", "gt")
  version: string;               // impl version (e.g., "0.1.0")
  params: Record<string, UIParamValue>;

  // Visual properties (not sent to runtime)
  x: number;
  y: number;
  selected?: boolean;
}

export type UIParamValue =
  | { type: 'int'; value: number }
  | { type: 'number'; value: number }
  | { type: 'bool'; value: boolean }
  | { type: 'string'; value: string }
  | { type: 'enum'; value: string };

// ============================================================================
// UI Edge Model
// ============================================================================

export interface UIEdge {
  id: string;                    // UI-only identifier
  fromNodeId: string;
  fromPort: string;
  toNodeId: string;
  toPort: string;

  // Visual properties
  selected?: boolean;
}

// ============================================================================
// UI Boundary Output
// ============================================================================

export interface UIBoundaryOutput {
  name: string;                  // External name for this output
  nodeId: string;                // Which node produces it
  portName: string;              // Which port on that node
}

// ============================================================================
// Complete UI Graph
// ============================================================================

export interface UIGraph {
  nodes: UINode[];
  edges: UIEdge[];
  boundaryOutputs: UIBoundaryOutput[];

  // Metadata
  name?: string;
  description?: string;
}

// ============================================================================
// Factory Functions
// ============================================================================

let nodeIdCounter = 0;
let edgeIdCounter = 0;

export function createNode(
  type: string,
  version: string,
  x: number,
  y: number,
  params: Record<string, UIParamValue> = {}
): UINode {
  return {
    id: `node_${++nodeIdCounter}`,
    type,
    version,
    params,
    x,
    y,
  };
}

export function createEdge(
  fromNodeId: string,
  fromPort: string,
  toNodeId: string,
  toPort: string
): UIEdge {
  return {
    id: `edge_${++edgeIdCounter}`,
    fromNodeId,
    fromPort,
    toNodeId,
    toPort,
  };
}

export function createGraph(): UIGraph {
  return {
    nodes: [],
    edges: [],
    boundaryOutputs: [],
  };
}

// Reset counters (useful for testing)
export function resetIdCounters(): void {
  nodeIdCounter = 0;
  edgeIdCounter = 0;
}
