/**
 * Inspector panel for viewing and editing node properties.
 *
 * Shows parameters and boundary outputs for the selected node.
 */

import React, { useState, useCallback } from 'react';
import type { UINode, UIParamValue, UIBoundaryOutput } from '../graph/internalModel';

// ============================================================================
// Numeric Validation (UI-COERCION-1, UI-INT-GUARD-1)
// ============================================================================

/**
 * Parse a string to a finite number, returning undefined for invalid input.
 * UI-COERCION-1: No silent fallback to 0.
 */
function parseFiniteNumber(s: string): number | undefined {
  if (s.trim() === '') return undefined;
  const n = Number(s);
  return Number.isFinite(n) ? n : undefined;
}

/**
 * Parse a string to a safe integer, returning undefined for invalid input.
 * UI-INT-GUARD-1: Validates |i| <= 2^53 per X.11.
 */
function parseSafeInteger(s: string): number | undefined {
  if (s.trim() === '') return undefined;
  const n = Number(s);
  if (!Number.isFinite(n)) return undefined;
  if (!Number.isSafeInteger(n)) return undefined;
  return n;
}

type ValidationError = string | null;

export interface InspectorProps {
  selectedNode: UINode | null;
  boundaryOutputs: UIBoundaryOutput[];
  onParamChange?: (nodeId: string, paramKey: string, value: UIParamValue) => void;
  onAddBoundaryOutput?: (nodeId: string, portName: string, outputName: string) => void;
  onRemoveBoundaryOutput?: (outputName: string) => void;
}

export const Inspector: React.FC<InspectorProps> = ({
  selectedNode,
  boundaryOutputs,
  onParamChange,
  onAddBoundaryOutput,
  onRemoveBoundaryOutput,
}) => {
  // Track validation errors per parameter (UI-COERCION-1, UI-INT-GUARD-1)
  const [paramErrors, setParamErrors] = useState<Record<string, ValidationError>>({});

  // Validate and update a numeric parameter
  const handleNumericChange = useCallback((
    nodeId: string,
    paramKey: string,
    paramType: 'number' | 'int',
    inputValue: string
  ) => {
    if (!onParamChange) return;

    if (paramType === 'int') {
      // UI-INT-GUARD-1: Validate safe integer range
      const parsed = parseSafeInteger(inputValue);
      if (parsed === undefined) {
        setParamErrors(prev => ({
          ...prev,
          [paramKey]: inputValue.trim() === ''
            ? 'Required'
            : 'Invalid integer (must be |i| ≤ 2^53)'
        }));
        return;
      }
      setParamErrors(prev => ({ ...prev, [paramKey]: null }));
      onParamChange(nodeId, paramKey, { type: 'int', value: parsed });
    } else {
      // UI-COERCION-1: No silent fallback to 0
      const parsed = parseFiniteNumber(inputValue);
      if (parsed === undefined) {
        setParamErrors(prev => ({
          ...prev,
          [paramKey]: inputValue.trim() === '' ? 'Required' : 'Invalid number'
        }));
        return;
      }
      setParamErrors(prev => ({ ...prev, [paramKey]: null }));
      onParamChange(nodeId, paramKey, { type: 'number', value: parsed });
    }
  }, [onParamChange]);

  if (!selectedNode) {
    return (
      <div style={styles.container}>
        <div style={styles.header}>Inspector</div>
        <div style={styles.empty}>Select a node to inspect</div>
      </div>
    );
  }

  const nodeOutputs = boundaryOutputs.filter(o => o.nodeId === selectedNode.id);

  return (
    <div style={styles.container}>
      <div style={styles.header}>Inspector</div>

      {/* Node Info */}
      <div style={styles.section}>
        <div style={styles.sectionTitle}>Node</div>
        <div style={styles.field}>
          <span style={styles.label}>ID:</span>
          <span style={styles.value}>{selectedNode.id}</span>
        </div>
        <div style={styles.field}>
          <span style={styles.label}>Type:</span>
          <span style={styles.value}>{selectedNode.type}</span>
        </div>
        <div style={styles.field}>
          <span style={styles.label}>Version:</span>
          <span style={styles.value}>{selectedNode.version}</span>
        </div>
      </div>

      {/* Parameters */}
      <div style={styles.section}>
        <div style={styles.sectionTitle}>Parameters</div>
        {Object.entries(selectedNode.params).length === 0 ? (
          <div style={styles.empty}>No parameters</div>
        ) : (
          Object.entries(selectedNode.params).map(([key, param]) => {
            const error = paramErrors[key];
            const isNumeric = param.type === 'number' || param.type === 'int';

            return (
              <div key={key} style={styles.paramRow}>
                <span style={styles.label}>{key}</span>
                <div style={styles.inputWrapper}>
                  <input
                    style={{
                      ...styles.input,
                      ...(error ? styles.inputError : {}),
                    }}
                    type={param.type === 'bool' ? 'checkbox' : 'text'}
                    defaultValue={param.type === 'bool' ? undefined : String(param.value)}
                    checked={param.type === 'bool' ? param.value : undefined}
                    onChange={(e) => {
                      if (!onParamChange) return;

                      if (param.type === 'bool') {
                        onParamChange(selectedNode.id, key, { type: 'bool', value: e.target.checked });
                      } else if (isNumeric) {
                        handleNumericChange(selectedNode.id, key, param.type, e.target.value);
                      } else {
                        onParamChange(selectedNode.id, key, { type: param.type, value: e.target.value });
                      }
                    }}
                  />
                  {error && <div style={styles.errorText}>{error}</div>}
                </div>
              </div>
            );
          })
        )}
      </div>

      {/* Boundary Outputs */}
      <div style={styles.section}>
        <div style={styles.sectionTitle}>Boundary Outputs</div>
        {nodeOutputs.length === 0 ? (
          <div style={styles.empty}>No outputs exposed</div>
        ) : (
          nodeOutputs.map(output => (
            <div key={output.name} style={styles.outputRow}>
              <span style={styles.outputName}>{output.name}</span>
              <span style={styles.outputPort}>:{output.portName}</span>
              {onRemoveBoundaryOutput && (
                <button
                  style={styles.removeButton}
                  onClick={() => onRemoveBoundaryOutput(output.name)}
                >
                  ×
                </button>
              )}
            </div>
          ))
        )}
      </div>
    </div>
  );
};

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
  section: {
    padding: '12px 16px',
    borderBottom: '1px solid #3d3d5c',
  },
  sectionTitle: {
    fontSize: 11,
    fontWeight: 600,
    textTransform: 'uppercase',
    color: '#64748b',
    marginBottom: 8,
  },
  field: {
    marginBottom: 6,
  },
  label: {
    color: '#94a3b8',
    marginRight: 8,
  },
  value: {
    color: '#e2e8f0',
  },
  empty: {
    color: '#64748b',
    fontStyle: 'italic',
    padding: '8px 0',
  },
  paramRow: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginBottom: 8,
  },
  input: {
    backgroundColor: '#2d2d44',
    border: '1px solid #4a4a6a',
    borderRadius: 4,
    color: '#e2e8f0',
    padding: '4px 8px',
    width: 100,
  },
  inputError: {
    borderColor: '#ef4444',
    backgroundColor: '#2d2233',
  },
  inputWrapper: {
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'flex-end',
  },
  errorText: {
    color: '#ef4444',
    fontSize: 10,
    marginTop: 2,
  },
  outputRow: {
    display: 'flex',
    alignItems: 'center',
    marginBottom: 6,
  },
  outputName: {
    color: '#6366f1',
    fontWeight: 500,
  },
  outputPort: {
    color: '#64748b',
    marginLeft: 4,
    flex: 1,
  },
  removeButton: {
    background: 'none',
    border: 'none',
    color: '#94a3b8',
    cursor: 'pointer',
    fontSize: 16,
    padding: '0 4px',
  },
};
