/**
 * Run panel for executing graphs and viewing results.
 *
 * Handles validation, execution, and displays outputs.
 */

import React, { useState } from 'react';
import type { UIGraph } from '../graph/internalModel';
import type { ValidationResult, ExecutionResult, RuntimeValue } from '../runtime/types';
import { serializeToExpandedGraph } from '../graph/serialize';
import { validateLocal } from '../graph/validateLocal';
import { validateGraph, runGraph } from '../runtime/adapter';

export interface RunPanelProps {
  graph: UIGraph;
}

type RunState =
  | { status: 'idle' }
  | { status: 'validating' }
  | { status: 'running' }
  | { status: 'validation_error'; result: ValidationResult }
  | { status: 'execution_error'; error: string }
  | { status: 'success'; outputs: Record<string, RuntimeValue> };

export const RunPanel: React.FC<RunPanelProps> = ({ graph }) => {
  const [state, setState] = useState<RunState>({ status: 'idle' });

  const handleValidate = async () => {
    // First do local validation for quick feedback
    const localResult = validateLocal(graph);
    if (!localResult.isValid) {
      setState({
        status: 'validation_error',
        result: {
          success: false,
          errors: localResult.issues
            .filter(i => i.severity === 'error')
            .map(i => ({
              kind: 'InvalidWiring' as const,
              message: i.message,
              location: i.nodeId ? { node_id: i.nodeId } : undefined,
            })),
        },
      });
      return;
    }

    setState({ status: 'validating' });

    try {
      const expanded = serializeToExpandedGraph(graph);
      const result = await validateGraph(expanded);

      if (!result.success) {
        setState({ status: 'validation_error', result });
      } else {
        setState({ status: 'idle' });
      }
    } catch (err) {
      setState({
        status: 'execution_error',
        error: err instanceof Error ? err.message : 'Unknown error',
      });
    }
  };

  const handleRun = async () => {
    // Local pre-check
    const localResult = validateLocal(graph);
    if (!localResult.isValid) {
      setState({
        status: 'validation_error',
        result: {
          success: false,
          errors: localResult.issues
            .filter(i => i.severity === 'error')
            .map(i => ({
              kind: 'InvalidWiring' as const,
              message: i.message,
            })),
        },
      });
      return;
    }

    setState({ status: 'running' });

    try {
      const expanded = serializeToExpandedGraph(graph);
      const response = await runGraph(expanded);

      if (!response.validation.success) {
        setState({ status: 'validation_error', result: response.validation });
      } else if (!response.execution?.success) {
        setState({
          status: 'execution_error',
          error: response.execution?.error?.message ?? 'Execution failed',
        });
      } else {
        setState({
          status: 'success',
          outputs: response.execution.report.outputs,
        });
      }
    } catch (err) {
      setState({
        status: 'execution_error',
        error: err instanceof Error ? err.message : 'Unknown error',
      });
    }
  };

  return (
    <div style={styles.container}>
      <div style={styles.header}>Run</div>

      {/* Action Buttons */}
      <div style={styles.actions}>
        <button
          style={styles.button}
          onClick={handleValidate}
          disabled={state.status === 'validating' || state.status === 'running'}
        >
          Validate
        </button>
        <button
          style={{ ...styles.button, ...styles.runButton }}
          onClick={handleRun}
          disabled={state.status === 'validating' || state.status === 'running'}
        >
          {state.status === 'running' ? 'Running...' : 'Run'}
        </button>
      </div>

      {/* Status Display */}
      <div style={styles.status}>
        {state.status === 'idle' && (
          <div style={styles.statusIdle}>Ready</div>
        )}

        {state.status === 'validating' && (
          <div style={styles.statusRunning}>Validating...</div>
        )}

        {state.status === 'running' && (
          <div style={styles.statusRunning}>Executing...</div>
        )}

        {state.status === 'validation_error' && (
          <div style={styles.errorSection}>
            <div style={styles.errorTitle}>Validation Errors</div>
            {state.result.errors.map((err, i) => (
              <div key={i} style={styles.errorItem}>
                <span style={styles.errorKind}>{err.kind}</span>
                <span style={styles.errorMessage}>{err.message}</span>
              </div>
            ))}
          </div>
        )}

        {state.status === 'execution_error' && (
          <div style={styles.errorSection}>
            <div style={styles.errorTitle}>Execution Error</div>
            <div style={styles.errorMessage}>{state.error}</div>
          </div>
        )}

        {state.status === 'success' && (
          <div style={styles.successSection}>
            <div style={styles.successTitle}>Outputs</div>
            {Object.entries(state.outputs).map(([name, value]) => (
              <div key={name} style={styles.outputItem}>
                <span style={styles.outputName}>{name}</span>
                <span style={styles.outputValue}>
                  {formatRuntimeValue(value)}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};

function formatRuntimeValue(value: RuntimeValue): string {
  switch (value.type) {
    case 'Number':
      return String(value.value);
    case 'Bool':
      return String(value.value);
    case 'String':
      return `"${value.value}"`;
    case 'Series':
      return `[${value.value.slice(0, 3).join(', ')}${value.value.length > 3 ? '...' : ''}]`;
    case 'Event': {
      // UI-CONTRACT-ALIGN-1: Handle tagged union RuntimeEvent
      const event = value.value;
      if (event.type === 'Trigger') {
        return `Event(Trigger: ${event.event.triggered ? 'fired' : 'not fired'})`;
      } else {
        return `Event(Action: ${event.outcome.status})`;
      }
    }
  }
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    width: 280,
    backgroundColor: '#1e1e2e',
    borderLeft: '1px solid #3d3d5c',
    display: 'flex',
    flexDirection: 'column',
    color: '#e2e8f0',
    fontSize: 13,
  },
  header: {
    padding: '12px 16px',
    borderBottom: '1px solid #3d3d5c',
    fontWeight: 600,
    fontSize: 14,
  },
  actions: {
    display: 'flex',
    gap: 8,
    padding: 16,
    borderBottom: '1px solid #3d3d5c',
  },
  button: {
    flex: 1,
    padding: '8px 16px',
    backgroundColor: '#3d3d5c',
    border: 'none',
    borderRadius: 6,
    color: '#e2e8f0',
    cursor: 'pointer',
    fontSize: 13,
    fontWeight: 500,
  },
  runButton: {
    backgroundColor: '#6366f1',
  },
  status: {
    flex: 1,
    padding: 16,
    overflow: 'auto',
  },
  statusIdle: {
    color: '#64748b',
  },
  statusRunning: {
    color: '#6366f1',
  },
  errorSection: {
    backgroundColor: 'rgba(239, 68, 68, 0.1)',
    borderRadius: 6,
    padding: 12,
  },
  errorTitle: {
    color: '#ef4444',
    fontWeight: 600,
    marginBottom: 8,
  },
  errorItem: {
    marginBottom: 6,
  },
  errorKind: {
    color: '#fca5a5',
    fontSize: 11,
    display: 'block',
  },
  errorMessage: {
    color: '#e2e8f0',
  },
  successSection: {
    backgroundColor: 'rgba(34, 197, 94, 0.1)',
    borderRadius: 6,
    padding: 12,
  },
  successTitle: {
    color: '#22c55e',
    fontWeight: 600,
    marginBottom: 8,
  },
  outputItem: {
    marginBottom: 8,
  },
  outputName: {
    color: '#6366f1',
    fontWeight: 500,
    display: 'block',
  },
  outputValue: {
    color: '#e2e8f0',
    fontFamily: 'monospace',
  },
};
