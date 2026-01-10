/**
 * Local validation for UX feedback.
 *
 * IMPORTANT: This is NOT authoritative validation.
 * The runtime is the only source of truth for validation.
 *
 * These checks exist solely to provide fast UX feedback before
 * round-tripping to the backend. They may be incomplete or
 * slightly out of sync with runtime rules.
 */

import type { UIGraph, UINode, UIEdge } from './internalModel';

// ============================================================================
// Local Validation Result
// ============================================================================

export interface LocalValidationIssue {
  severity: 'error' | 'warning';
  message: string;
  nodeId?: string;
  edgeId?: string;
}

export interface LocalValidationResult {
  issues: LocalValidationIssue[];
  get isValid(): boolean;
}

// ============================================================================
// Validation Checks
// ============================================================================

function checkDuplicateNodeIds(graph: UIGraph): LocalValidationIssue[] {
  const issues: LocalValidationIssue[] = [];
  const seen = new Set<string>();

  for (const node of graph.nodes) {
    if (seen.has(node.id)) {
      issues.push({
        severity: 'error',
        message: `Duplicate node ID: ${node.id}`,
        nodeId: node.id,
      });
    }
    seen.add(node.id);
  }

  return issues;
}

function checkEdgeReferences(graph: UIGraph): LocalValidationIssue[] {
  const issues: LocalValidationIssue[] = [];
  const nodeIds = new Set(graph.nodes.map(n => n.id));

  for (const edge of graph.edges) {
    if (!nodeIds.has(edge.fromNodeId)) {
      issues.push({
        severity: 'error',
        message: `Edge references non-existent source node: ${edge.fromNodeId}`,
        edgeId: edge.id,
      });
    }
    if (!nodeIds.has(edge.toNodeId)) {
      issues.push({
        severity: 'error',
        message: `Edge references non-existent target node: ${edge.toNodeId}`,
        edgeId: edge.id,
      });
    }
  }

  return issues;
}

function checkBoundaryOutputReferences(graph: UIGraph): LocalValidationIssue[] {
  const issues: LocalValidationIssue[] = [];
  const nodeIds = new Set(graph.nodes.map(n => n.id));

  for (const output of graph.boundaryOutputs) {
    if (!nodeIds.has(output.nodeId)) {
      issues.push({
        severity: 'error',
        message: `Boundary output "${output.name}" references non-existent node: ${output.nodeId}`,
      });
    }
  }

  return issues;
}

function checkSelfLoops(graph: UIGraph): LocalValidationIssue[] {
  const issues: LocalValidationIssue[] = [];

  for (const edge of graph.edges) {
    if (edge.fromNodeId === edge.toNodeId) {
      issues.push({
        severity: 'error',
        message: `Self-loop detected on node: ${edge.fromNodeId}`,
        edgeId: edge.id,
        nodeId: edge.fromNodeId,
      });
    }
  }

  return issues;
}

function checkEmptyGraph(graph: UIGraph): LocalValidationIssue[] {
  if (graph.nodes.length === 0) {
    return [{
      severity: 'warning',
      message: 'Graph is empty',
    }];
  }
  return [];
}

// ============================================================================
// Main Validation Function
// ============================================================================

/**
 * Perform local validation for UX feedback.
 *
 * Remember: This is non-authoritative. Always validate against the runtime.
 */
export function validateLocal(graph: UIGraph): LocalValidationResult {
  const issues: LocalValidationIssue[] = [
    ...checkEmptyGraph(graph),
    ...checkDuplicateNodeIds(graph),
    ...checkEdgeReferences(graph),
    ...checkBoundaryOutputReferences(graph),
    ...checkSelfLoops(graph),
  ];

  return {
    issues,
    get isValid() {
      return !issues.some(i => i.severity === 'error');
    },
  };
}
