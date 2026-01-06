/**
 * TypeScript mirror of the ExpandedGraph contract.
 *
 * These types exactly match the Rust structures defined in docs/CONTRACTS/UI_RUNTIME_CONTRACT.md.
 * This file is the authoritative TypeScript representation of the contract.
 */

// ============================================================================
// Parameter Values
// ============================================================================

export type ParameterValue =
  | { type: 'Int'; value: number }
  | { type: 'Number'; value: number }
  | { type: 'Bool'; value: boolean }
  | { type: 'String'; value: string }
  | { type: 'Enum'; value: string };

// ============================================================================
// Implementation Reference
// ============================================================================

export interface ImplementationInstance {
  impl_id: string;   // e.g., "number_source", "gt", "emit_if_true"
  version: string;   // e.g., "0.1.0"
}

// ============================================================================
// Node Types
// ============================================================================

export interface ExpandedNode {
  runtime_id: string;
  authoring_path: Array<[string, string]>;  // Empty for flat graphs
  implementation: ImplementationInstance;
  parameters: Record<string, ParameterValue>;
}

// ============================================================================
// Edge Types
// ============================================================================

export type ExpandedEndpoint =
  | { type: 'NodePort'; node_id: string; port_name: string }
  | { type: 'ExternalInput'; name: string };

export interface ExpandedEdge {
  from: ExpandedEndpoint;
  to: ExpandedEndpoint;
}

// ============================================================================
// Boundary Ports
// ============================================================================

export interface OutputRef {
  node_id: string;
  port_name: string;
}

export interface OutputPortSpec {
  name: string;
  maps_to: OutputRef;
}

export interface InputPortSpec {
  name: string;
  // Additional fields as needed
}

// ============================================================================
// Top-Level Graph
// ============================================================================

export interface ExpandedGraph {
  nodes: Record<string, ExpandedNode>;
  edges: ExpandedEdge[];
  boundary_inputs: InputPortSpec[];
  boundary_outputs: OutputPortSpec[];
}

// ============================================================================
// Runtime Values (for execution results)
// ============================================================================

export type RuntimeValue =
  | { type: 'Number'; value: number }
  | { type: 'Series'; value: number[] }
  | { type: 'Bool'; value: boolean }
  | { type: 'Event'; value: RuntimeEvent }
  | { type: 'String'; value: string };

/**
 * RuntimeEvent is a tagged union matching Rust's RuntimeEvent enum.
 * UI-CONTRACT-ALIGN-1: Aligned with Rust enum structure.
 */
export type RuntimeEvent =
  | { type: 'Trigger'; event: TriggerEvent }
  | { type: 'Action'; outcome: ActionOutcome };

export interface TriggerEvent {
  triggered: boolean;
}

export type ActionOutcome =
  | { status: 'Completed' }
  | { status: 'Skipped' }
  | { status: 'Failed'; error: string };

// ============================================================================
// Execution Report
// ============================================================================

export interface ExecutionReport {
  outputs: Record<string, RuntimeValue>;
}
