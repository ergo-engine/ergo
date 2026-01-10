/**
 * Runtime response types.
 *
 * These mirror the Rust types returned by the runtime's validate() and run() functions.
 */

import type { ExecutionReport, RuntimeValue } from '../contract/contractTypes';

// Re-export for convenience
export type { ExecutionReport, RuntimeValue };

// ============================================================================
// Validation Errors
// ============================================================================

export type ValidationErrorKind =
  | 'UnknownPrimitive'
  | 'MissingRequiredInput'
  | 'TypeMismatch'
  | 'InvalidWiring'
  | 'UngatedAction'
  | 'CycleDetected';

export interface ValidationError {
  kind: ValidationErrorKind;
  message: string;
  location?: {
    node_id?: string;
    port_name?: string;
    edge_index?: number;
  };
}

export interface ValidationResult {
  success: boolean;
  errors: ValidationError[];
}

// ============================================================================
// Execution Errors
// ============================================================================

export type ExecutionErrorKind =
  | 'RuntimePanic'
  | 'TypeCoercionFailed'
  | 'MissingInput'
  | 'ActionFailed';

export interface ExecutionError {
  kind: ExecutionErrorKind;
  message: string;
  node_id?: string;
}

export type ExecutionResult =
  | { success: true; report: ExecutionReport }
  | { success: false; error: ExecutionError };

// ============================================================================
// Combined API Response
// ============================================================================

export interface RuntimeResponse {
  validation: ValidationResult;
  execution?: ExecutionResult;
}
