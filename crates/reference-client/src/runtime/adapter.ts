/**
 * Runtime adapter.
 *
 * Handles communication with the Primitive Library runtime backend.
 * The UI calls these functions; the adapter handles serialization and transport.
 */

import type { ExpandedGraph } from '../contract/contractTypes';
import type { ValidationResult, ExecutionResult, RuntimeResponse } from './types';

// ============================================================================
// Configuration
// ============================================================================

export interface RuntimeConfig {
  baseUrl: string;
  timeout?: number;
}

const DEFAULT_CONFIG: RuntimeConfig = {
  baseUrl: 'http://localhost:3001',
  timeout: 30000,
};

let config: RuntimeConfig = { ...DEFAULT_CONFIG };

export function configureRuntime(newConfig: Partial<RuntimeConfig>): void {
  config = { ...config, ...newConfig };
}

// ============================================================================
// API Functions
// ============================================================================

/**
 * Validate a graph against the runtime catalog.
 *
 * This checks:
 * - All primitives exist
 * - Wiring matrix is respected
 * - Required inputs are connected
 * - Types are compatible
 * - Actions are gated by triggers
 */
export async function validateGraph(graph: ExpandedGraph): Promise<ValidationResult> {
  const response = await fetch(`${config.baseUrl}/validate`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(graph),
    signal: AbortSignal.timeout(config.timeout ?? DEFAULT_CONFIG.timeout!),
  });

  if (!response.ok) {
    throw new Error(`Validation request failed: ${response.status}`);
  }

  return response.json();
}

/**
 * Execute a graph and return the results.
 *
 * This first validates, then runs the graph if valid.
 * Results are returned via boundary_outputs.
 */
export async function runGraph(graph: ExpandedGraph): Promise<RuntimeResponse> {
  const response = await fetch(`${config.baseUrl}/run`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(graph),
    signal: AbortSignal.timeout(config.timeout ?? DEFAULT_CONFIG.timeout!),
  });

  if (!response.ok) {
    throw new Error(`Execution request failed: ${response.status}`);
  }

  return response.json();
}

/**
 * Validate and run in a single call.
 *
 * Convenience function that handles the common case.
 */
export async function validateAndRun(graph: ExpandedGraph): Promise<{
  validation: ValidationResult;
  execution?: ExecutionResult;
}> {
  // First validate
  const validation = await validateGraph(graph);

  if (!validation.success) {
    return { validation };
  }

  // Then run
  const response = await runGraph(graph);
  return {
    validation: response.validation,
    execution: response.execution,
  };
}
