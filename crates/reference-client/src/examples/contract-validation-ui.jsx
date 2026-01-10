import React, { useState, useCallback, useRef, memo, useMemo } from 'react';
import { serializeToExpandedGraph } from '../graph/serialize';
import { validateAndRun } from '../runtime/adapter';

// ═══════════════════════════════════════════════════════════════════════════════
// CONTRACT VALIDATION UI
// Purpose: Prove that a UI can emit ExpandedGraph and drive the runtime unchanged.
// This is NOT product UX. This is a validation vertical.
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────────
// COLORS (minimal, functional)
// ─────────────────────────────────────────────────────────────────────────────────

const COLORS = {
  bg: { 
    void: '#0a0a09',
    base: '#141413',
    elevated: '#1a1a18',
    surface: '#222220',
  },
  border: { 
    subtle: 'rgba(255,252,245,0.06)',
    default: 'rgba(255,252,245,0.12)',
    strong: 'rgba(255,252,245,0.25)',
  },
  text: { 
    primary: 'rgba(255,252,245,0.90)',
    secondary: 'rgba(255,252,245,0.55)',
    tertiary: 'rgba(255,252,245,0.35)',
  },
  kind: {
    source: '#7ca3c4',
    compute: '#98b5a0',
    trigger: '#c4b998',
    action: '#a89db3',
  },
  status: {
    success: '#98b5a0',
    error: '#c49898',
  }
};

// ─────────────────────────────────────────────────────────────────────────────────
// HELLO-WORLD PRIMITIVE CATALOG (hardcoded, matches runtime exactly)
// This will be replaced with catalog feed after contract is proven.
// ─────────────────────────────────────────────────────────────────────────────────

const PRIMITIVE_CATALOG = {
  number_source: {
    impl_id: 'number_source',
    version: '0.1.0',
    kind: 'source',
    label: 'Number Source',
    inputs: [],
    outputs: [{ name: 'value', type: 'number' }],
    parameters: [{ name: 'value', type: 'number', required: true, default: 0.0 }],
  },
  gt: {
    impl_id: 'gt',
    version: '0.1.0',
    kind: 'compute',
    label: 'Greater Than',
    inputs: [
      { name: 'a', type: 'number' },
      { name: 'b', type: 'number' },
    ],
    outputs: [{ name: 'result', type: 'bool' }],
    parameters: [],
  },
  emit_if_true: {
    impl_id: 'emit_if_true',
    version: '0.1.0',
    kind: 'trigger',
    label: 'Emit If True',
    inputs: [{ name: 'input', type: 'bool' }],
    outputs: [{ name: 'event', type: 'event' }],
    parameters: [],
  },
  ack_action: {
    impl_id: 'ack_action',
    version: '0.1.0',
    kind: 'action',
    label: 'Ack Action',
    inputs: [{ name: 'event', type: 'event' }],
    outputs: [{ name: 'outcome', type: 'event' }],
    parameters: [{ name: 'accept', type: 'bool', required: true, default: true }],
  },
};

// ─────────────────────────────────────────────────────────────────────────────────
// SERIALIZATION: Internal Model → ExpandedGraph (via shared serializer)
// ─────────────────────────────────────────────────────────────────────────────────

function buildUIGraph(nodes, connections, boundaryOutputs) {
  const uiNodes = [];

  for (const node of nodes) {
    const primitive = PRIMITIVE_CATALOG[node.impl_id];
    if (!primitive) continue;

    const params = {};
    for (const paramSpec of primitive.parameters) {
      if (node.params && Object.prototype.hasOwnProperty.call(node.params, paramSpec.name)) {
        const value = node.params[paramSpec.name];
        if (value !== undefined) {
          params[paramSpec.name] = { type: paramSpec.type, value };
        }
      }
    }

    uiNodes.push({
      id: node.runtime_id,
      type: primitive.impl_id,
      version: primitive.version,
      params,
      x: node.x,
      y: node.y,
    });
  }

  const uiEdges = connections.map((conn, idx) => ({
    id: `edge_${idx}`,
    fromNodeId: conn.from.nodeId,
    fromPort: conn.from.port,
    toNodeId: conn.to.nodeId,
    toPort: conn.to.port,
  }));

  const uiBoundaryOutputs = boundaryOutputs.map(bo => ({
    name: bo.name,
    nodeId: bo.nodeId,
    portName: bo.port,
  }));

  return {
    nodes: uiNodes,
    edges: uiEdges,
    boundaryOutputs: uiBoundaryOutputs,
  };
}

// ─────────────────────────────────────────────────────────────────────────────────
// UTILITY
// ─────────────────────────────────────────────────────────────────────────────────

const uid = () => Math.random().toString(36).substr(2, 9);

// ─────────────────────────────────────────────────────────────────────────────────
// NODE COMPONENT
// ─────────────────────────────────────────────────────────────────────────────────

const Node = memo(({ 
  node, 
  selected, 
  onSelect, 
  onDragStart,
  onParamChange,
  onDelete,
  onPortMouseDown,
  onPortMouseEnter,
  onPortMouseLeave,
  dragPos,
  isDragging,
}) => {
  const primitive = PRIMITIVE_CATALOG[node.impl_id];
  if (!primitive) return null;
  
  const kindColor = COLORS.kind[primitive.kind] || COLORS.text.secondary;
  const x = isDragging ? dragPos.x : node.x;
  const y = isDragging ? dragPos.y : node.y;
  
  return (
    <div
      className="absolute select-none"
      style={{
        left: x,
        top: y,
        width: 180,
        zIndex: selected ? 100 : 1,
      }}
      onMouseDown={(e) => {
        if (e.target.closest('[data-port]') || e.target.closest('input')) return;
        e.stopPropagation();
        onSelect(node.runtime_id);
        onDragStart(node.runtime_id, e);
      }}
    >
      <div
        style={{
          background: COLORS.bg.elevated,
          border: `1px solid ${selected ? kindColor : COLORS.border.default}`,
          borderRadius: 8,
          overflow: 'hidden',
        }}
      >
        {/* Header */}
        <div
          style={{
            padding: '8px 12px',
            borderBottom: `1px solid ${COLORS.border.subtle}`,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <div
              style={{
                width: 8,
                height: 8,
                borderRadius: '50%',
                background: kindColor,
              }}
            />
            <span style={{ color: COLORS.text.primary, fontSize: 12, fontWeight: 600 }}>
              {primitive.label}
            </span>
          </div>
          <button
            onClick={(e) => { e.stopPropagation(); onDelete(node.runtime_id); }}
            style={{
              background: 'none',
              border: 'none',
              color: COLORS.text.tertiary,
              cursor: 'pointer',
              fontSize: 14,
              lineHeight: 1,
            }}
          >
            ×
          </button>
        </div>
        
        {/* Runtime ID (for debugging) */}
        <div style={{ padding: '4px 12px', background: COLORS.bg.surface }}>
          <span style={{ color: COLORS.text.tertiary, fontSize: 9, fontFamily: 'monospace' }}>
            {node.runtime_id}
          </span>
        </div>
        
        {/* Parameters */}
        {primitive.parameters.length > 0 && (
          <div style={{ padding: '8px 12px', borderBottom: `1px solid ${COLORS.border.subtle}` }}>
            {primitive.parameters.map(param => (
              <div key={param.name} style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 4 }}>
                <span style={{ color: COLORS.text.tertiary, fontSize: 10, width: 50 }}>
                  {param.name}
                </span>
                {param.type === 'bool' ? (
                  <input
                    type="checkbox"
                    checked={Boolean(
                      node.params && Object.prototype.hasOwnProperty.call(node.params, param.name)
                        ? node.params[param.name]
                        : false
                    )}
                    onChange={(e) => onParamChange(node.runtime_id, param.name, e.target.checked)}
                    onClick={(e) => e.stopPropagation()}
                  />
                ) : (
                  <input
                    type="number"
                    step="any"
                    value={
                      node.params && Object.prototype.hasOwnProperty.call(node.params, param.name)
                        ? node.params[param.name]
                        : ''
                    }
                    onChange={(e) => {
                      const raw = e.target.value;
                      const nextValue = raw === '' ? undefined : Number(raw);
                      onParamChange(node.runtime_id, param.name, nextValue);
                    }}
                    onClick={(e) => e.stopPropagation()}
                    style={{
                      flex: 1,
                      background: COLORS.bg.surface,
                      border: `1px solid ${COLORS.border.subtle}`,
                      borderRadius: 4,
                      padding: '2px 6px',
                      color: COLORS.text.primary,
                      fontSize: 11,
                    }}
                  />
                )}
              </div>
            ))}
          </div>
        )}
        
        {/* Inputs */}
        {primitive.inputs.length > 0 && (
          <div style={{ padding: '6px 0' }}>
            {primitive.inputs.map(input => (
              <div
                key={input.name}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                  padding: '2px 12px',
                }}
              >
                <div
                  data-port
                  onMouseDown={(e) => { e.stopPropagation(); onPortMouseDown(node.runtime_id, 'in', input.name, input.type); }}
                  onMouseEnter={() => onPortMouseEnter(node.runtime_id, 'in', input.name, input.type)}
                  onMouseLeave={onPortMouseLeave}
                  style={{
                    width: 10,
                    height: 10,
                    borderRadius: '50%',
                    background: COLORS.border.default,
                    marginLeft: -17,
                    cursor: 'crosshair',
                  }}
                />
                <span style={{ color: COLORS.text.tertiary, fontSize: 10 }}>
                  {input.name} <span style={{ opacity: 0.5 }}>({input.type})</span>
                </span>
              </div>
            ))}
          </div>
        )}
        
        {/* Outputs */}
        {primitive.outputs.length > 0 && (
          <div style={{ padding: '6px 0', borderTop: primitive.inputs.length > 0 ? `1px solid ${COLORS.border.subtle}` : 'none' }}>
            {primitive.outputs.map(output => (
              <div
                key={output.name}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'flex-end',
                  gap: 8,
                  padding: '2px 12px',
                }}
              >
                <span style={{ color: COLORS.text.tertiary, fontSize: 10 }}>
                  <span style={{ opacity: 0.5 }}>({output.type})</span> {output.name}
                </span>
                <div
                  data-port
                  onMouseDown={(e) => { e.stopPropagation(); onPortMouseDown(node.runtime_id, 'out', output.name, output.type); }}
                  style={{
                    width: 10,
                    height: 10,
                    borderRadius: '50%',
                    background: kindColor,
                    marginRight: -17,
                    cursor: 'crosshair',
                  }}
                />
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
});

// ─────────────────────────────────────────────────────────────────────────────────
// CONNECTION COMPONENT
// ─────────────────────────────────────────────────────────────────────────────────

const Connection = memo(({ conn, nodes, isTemp, mousePos, dragNodeId, dragPos }) => {
  const fromNode = nodes.find(n => n.runtime_id === conn.from.nodeId);
  if (!fromNode) return null;
  
  const fromPrimitive = PRIMITIVE_CATALOG[fromNode.impl_id];
  if (!fromPrimitive) return null;
  
  // Calculate port positions
  const fromX = dragNodeId === fromNode.runtime_id ? dragPos.x : fromNode.x;
  const fromY = dragNodeId === fromNode.runtime_id ? dragPos.y : fromNode.y;
  
  const outputIdx = fromPrimitive.outputs.findIndex(o => o.name === conn.from.port);
  const headerHeight = 32 + 20; // header + runtime_id
  const paramHeight = fromPrimitive.parameters.length > 0 ? 8 + fromPrimitive.parameters.length * 24 : 0;
  const inputHeight = fromPrimitive.inputs.length * 20 + (fromPrimitive.inputs.length > 0 ? 12 : 0);
  
  const startX = fromX + 180;
  const startY = fromY + headerHeight + paramHeight + inputHeight + (outputIdx + 0.5) * 20 + 6;
  
  let endX, endY;
  
  if (isTemp && mousePos) {
    endX = mousePos.x;
    endY = mousePos.y;
  } else if (conn.to) {
    const toNode = nodes.find(n => n.runtime_id === conn.to.nodeId);
    if (!toNode) return null;
    
    const toPrimitive = PRIMITIVE_CATALOG[toNode.impl_id];
    if (!toPrimitive) return null;
    
    const toX = dragNodeId === toNode.runtime_id ? dragPos.x : toNode.x;
    const toY = dragNodeId === toNode.runtime_id ? dragPos.y : toNode.y;
    
    const inputIdx = toPrimitive.inputs.findIndex(i => i.name === conn.to.port);
    const toHeaderHeight = 32 + 20;
    const toParamHeight = toPrimitive.parameters.length > 0 ? 8 + toPrimitive.parameters.length * 24 : 0;
    
    endX = toX;
    endY = toY + toHeaderHeight + toParamHeight + (inputIdx + 0.5) * 20 + 6;
  } else {
    return null;
  }
  
  const dx = endX - startX;
  const controlOffset = Math.max(Math.min(Math.abs(dx) * 0.5, 80), 40);
  const path = `M ${startX} ${startY} C ${startX + controlOffset} ${startY}, ${endX - controlOffset} ${endY}, ${endX} ${endY}`;
  
  return (
    <g>
      <path
        d={path}
        fill="none"
        stroke={isTemp ? COLORS.text.tertiary : COLORS.border.strong}
        strokeWidth={isTemp ? 1 : 2}
        strokeDasharray={isTemp ? "4 4" : "none"}
        opacity={isTemp ? 0.5 : 0.8}
      />
      <circle cx={startX} cy={startY} r={3} fill={COLORS.border.strong} />
      {!isTemp && <circle cx={endX} cy={endY} r={3} fill={COLORS.border.strong} />}
    </g>
  );
});

// ─────────────────────────────────────────────────────────────────────────────────
// BOUNDARY OUTPUT PANEL
// ─────────────────────────────────────────────────────────────────────────────────

const BoundaryOutputPanel = memo(({ nodes, boundaryOutputs, onAdd, onRemove, onChange }) => {
  const [newName, setNewName] = useState('');
  const [selectedNode, setSelectedNode] = useState('');
  const [selectedPort, setSelectedPort] = useState('');
  
  const availableOutputs = useMemo(() => {
    const outputs = [];
    for (const node of nodes) {
      const primitive = PRIMITIVE_CATALOG[node.impl_id];
      if (primitive) {
        for (const output of primitive.outputs) {
          outputs.push({
            nodeId: node.runtime_id,
            port: output.name,
            label: `${node.runtime_id}:${output.name}`,
          });
        }
      }
    }
    return outputs;
  }, [nodes]);
  
  const handleAdd = () => {
    if (newName && selectedNode && selectedPort) {
      onAdd({ name: newName, nodeId: selectedNode, port: selectedPort });
      setNewName('');
      setSelectedNode('');
      setSelectedPort('');
    }
  };
  
  return (
    <div style={{ padding: 12 }}>
      <div style={{ fontSize: 11, fontWeight: 600, color: COLORS.text.primary, marginBottom: 8 }}>
        Boundary Outputs
      </div>
      <div style={{ fontSize: 9, color: COLORS.text.tertiary, marginBottom: 12 }}>
        Name which node:port to observe in ExecutionReport
      </div>
      
      {/* Existing outputs */}
      {boundaryOutputs.map((bo, i) => (
        <div
          key={i}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 8,
            padding: '4px 8px',
            background: COLORS.bg.surface,
            borderRadius: 4,
            marginBottom: 4,
          }}
        >
          <span style={{ color: COLORS.text.primary, fontSize: 10, flex: 1 }}>{bo.name}</span>
          <span style={{ color: COLORS.text.tertiary, fontSize: 9 }}>→ {bo.nodeId}:{bo.port}</span>
          <button
            onClick={() => onRemove(i)}
            style={{ background: 'none', border: 'none', color: COLORS.text.tertiary, cursor: 'pointer' }}
          >
            ×
          </button>
        </div>
      ))}
      
      {/* Add new */}
      <div style={{ marginTop: 8, display: 'flex', flexDirection: 'column', gap: 4 }}>
        <input
          type="text"
          placeholder="output name"
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
          style={{
            background: COLORS.bg.surface,
            border: `1px solid ${COLORS.border.subtle}`,
            borderRadius: 4,
            padding: '4px 8px',
            color: COLORS.text.primary,
            fontSize: 10,
          }}
        />
        <select
          value={`${selectedNode}:${selectedPort}`}
          onChange={(e) => {
            const [nodeId, port] = e.target.value.split(':');
            setSelectedNode(nodeId);
            setSelectedPort(port);
          }}
          style={{
            background: COLORS.bg.surface,
            border: `1px solid ${COLORS.border.subtle}`,
            borderRadius: 4,
            padding: '4px 8px',
            color: COLORS.text.primary,
            fontSize: 10,
          }}
        >
          <option value=":">select node:port</option>
          {availableOutputs.map(o => (
            <option key={o.label} value={`${o.nodeId}:${o.port}`}>{o.label}</option>
          ))}
        </select>
        <button
          onClick={handleAdd}
          disabled={!newName || !selectedNode || !selectedPort}
          style={{
            background: COLORS.kind.source,
            border: 'none',
            borderRadius: 4,
            padding: '6px 12px',
            color: '#000',
            fontSize: 10,
            fontWeight: 600,
            cursor: newName && selectedNode && selectedPort ? 'pointer' : 'not-allowed',
            opacity: newName && selectedNode && selectedPort ? 1 : 0.5,
          }}
        >
          Add Boundary Output
        </button>
      </div>
    </div>
  );
});

// ─────────────────────────────────────────────────────────────────────────────────
// EXECUTION PANEL
// ─────────────────────────────────────────────────────────────────────────────────

const ExecutionPanel = memo(({ onRun, result, isRunning }) => {
  let statusLabel = null;
  let statusColor = COLORS.text.secondary;
  let payload = null;

  if (result) {
    if (result.status === 'error') {
      statusLabel = '✗ Error';
      statusColor = COLORS.status.error;
      payload = result.error;
    } else if (result.response) {
      const { validation, execution } = result.response;
      if (validation && !validation.success) {
        statusLabel = '✗ Validation failed';
        statusColor = COLORS.status.error;
        payload = validation;
      } else if (execution && execution.success === false) {
        statusLabel = '✗ Execution failed';
        statusColor = COLORS.status.error;
        payload = execution;
      } else {
        statusLabel = '✓ Success';
        statusColor = COLORS.status.success;
        payload = { validation, execution };
      }
    }
  }

  return (
    <div style={{ padding: 12, borderTop: `1px solid ${COLORS.border.subtle}` }}>
      <button
        onClick={onRun}
        disabled={isRunning}
        style={{
          width: '100%',
          background: COLORS.kind.source,
          border: 'none',
          borderRadius: 4,
          padding: '10px 16px',
          color: '#000',
          fontSize: 12,
          fontWeight: 600,
          cursor: isRunning ? 'wait' : 'pointer',
          opacity: isRunning ? 0.7 : 1,
        }}
      >
        {isRunning ? 'Running...' : '▶ Run Graph'}
      </button>
      
      {result && (
        <div style={{ marginTop: 12 }}>
          <div
            style={{
              fontSize: 10,
              fontWeight: 600,
              color: statusColor,
              marginBottom: 4,
            }}
          >
            {statusLabel}
          </div>
          
          <pre style={{
            background: COLORS.bg.surface,
            borderRadius: 4,
            padding: 8,
            fontSize: 9,
            color: COLORS.text.secondary,
            overflow: 'auto',
            maxHeight: 200,
          }}>
            {JSON.stringify(payload, null, 2)}
          </pre>
        </div>
      )}
    </div>
  );
});

// ─────────────────────────────────────────────────────────────────────────────────
// MAIN COMPONENT
// ─────────────────────────────────────────────────────────────────────────────────

export default function ContractValidationUI() {
  // ─────────────────────────────────────────────────────────────────────────────
  // STATE
  // ─────────────────────────────────────────────────────────────────────────────
  
  const [nodes, setNodes] = useState([]);
  const [connections, setConnections] = useState([]);
  const [boundaryOutputs, setBoundaryOutputs] = useState([]);
  const [selectedNode, setSelectedNode] = useState(null);
  
  // Canvas state
  const [canvasOffset, setCanvasOffset] = useState({ x: 0, y: 0 });
  const canvasOffsetRef = useRef({ x: 0, y: 0 });
  const [isPanning, setIsPanning] = useState(false);
  const panStart = useRef({ x: 0, y: 0 });
  const canvasRef = useRef(null);
  const contentRef = useRef(null);
  
  // Dragging state
  const [draggingNodeId, setDraggingNodeId] = useState(null);
  const [dragOffset, setDragOffset] = useState({ x: 0, y: 0 });
  const [dragPos, setDragPos] = useState({ x: 0, y: 0 });
  
  // Connection state
  const [connecting, setConnecting] = useState(null);
  const [mousePos, setMousePos] = useState({ x: 0, y: 0 });
  const hoverPort = useRef(null);
  
  // Execution state
  const [isRunning, setIsRunning] = useState(false);
  const [result, setResult] = useState(null);
  
  // ─────────────────────────────────────────────────────────────────────────────
  // HANDLERS
  // ─────────────────────────────────────────────────────────────────────────────
  
  const handleAddNode = useCallback((impl_id) => {
    const primitive = PRIMITIVE_CATALOG[impl_id];
    if (!primitive) return;
    
    const runtime_id = `${impl_id}_${uid()}`;
    const params = {};
    
    setNodes(prev => [...prev, {
      runtime_id,
      impl_id,
      x: 200 - canvasOffsetRef.current.x + Math.random() * 100,
      y: 150 - canvasOffsetRef.current.y + Math.random() * 100,
      params,
    }]);
    setSelectedNode(runtime_id);
  }, []);
  
  const handleDeleteNode = useCallback((runtime_id) => {
    setNodes(prev => prev.filter(n => n.runtime_id !== runtime_id));
    setConnections(prev => prev.filter(c => 
      c.from.nodeId !== runtime_id && c.to?.nodeId !== runtime_id
    ));
    setBoundaryOutputs(prev => prev.filter(bo => bo.nodeId !== runtime_id));
    if (selectedNode === runtime_id) setSelectedNode(null);
  }, [selectedNode]);
  
  const handleParamChange = useCallback((runtime_id, paramName, value) => {
    setNodes(prev => prev.map(n => {
      if (n.runtime_id !== runtime_id) return n;
      const nextParams = { ...n.params };
      if (value === undefined) {
        delete nextParams[paramName];
      } else {
        nextParams[paramName] = value;
      }
      return { ...n, params: nextParams };
    }));
  }, []);
  
  const handleDragStart = useCallback((runtime_id, e) => {
    const node = nodes.find(n => n.runtime_id === runtime_id);
    if (!node) return;
    setDraggingNodeId(runtime_id);
    setDragOffset({
      x: e.clientX - node.x - canvasOffsetRef.current.x,
      y: e.clientY - node.y - canvasOffsetRef.current.y,
    });
    setDragPos({ x: node.x, y: node.y });
  }, [nodes]);
  
  const handlePortMouseDown = useCallback((nodeId, direction, portName, portType) => {
    if (direction === 'out') {
      setConnecting({ from: { nodeId, port: portName, type: portType } });
    }
  }, []);
  
  const handlePortMouseEnter = useCallback((nodeId, direction, portName, portType) => {
    if (direction === 'in' && connecting) {
      hoverPort.current = { nodeId, port: portName, type: portType };
    }
  }, [connecting]);
  
  const handlePortMouseLeave = useCallback(() => {
    hoverPort.current = null;
  }, []);
  
  const handleCanvasMouseDown = useCallback((e) => {
    if (e.target.closest('[data-port]') || e.target.closest('input') || e.target.closest('select') || e.target.closest('button')) return;
    setIsPanning(true);
    panStart.current = { x: e.clientX - canvasOffsetRef.current.x, y: e.clientY - canvasOffsetRef.current.y };
    setSelectedNode(null);
  }, []);
  
  const handleMouseMove = useCallback((e) => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;
    
    if (isPanning) {
      const newX = e.clientX - panStart.current.x;
      const newY = e.clientY - panStart.current.y;
      canvasOffsetRef.current = { x: newX, y: newY };
      if (contentRef.current) {
        contentRef.current.style.transform = `translate(${newX}px, ${newY}px)`;
      }
    } else if (draggingNodeId) {
      setDragPos({
        x: e.clientX - dragOffset.x - canvasOffsetRef.current.x,
        y: e.clientY - dragOffset.y - canvasOffsetRef.current.y,
      });
    } else if (connecting) {
      setMousePos({
        x: e.clientX - rect.left - canvasOffsetRef.current.x,
        y: e.clientY - rect.top - canvasOffsetRef.current.y,
      });
    }
  }, [isPanning, draggingNodeId, connecting, dragOffset]);
  
  const handleMouseUp = useCallback(() => {
    if (isPanning) {
      setCanvasOffset({ ...canvasOffsetRef.current });
    }
    
    if (draggingNodeId) {
      setNodes(prev => prev.map(n => 
        n.runtime_id === draggingNodeId ? { ...n, x: dragPos.x, y: dragPos.y } : n
      ));
      setDraggingNodeId(null);
    }
    
    if (connecting && hoverPort.current) {
      // Create connection (no type checking - let runtime validate)
      setConnections(prev => [...prev, {
        from: connecting.from,
        to: { nodeId: hoverPort.current.nodeId, port: hoverPort.current.port },
      }]);
    }
    
    setIsPanning(false);
    setConnecting(null);
    hoverPort.current = null;
  }, [isPanning, draggingNodeId, connecting, dragPos]);
  
  const handleRun = useCallback(async () => {
    setIsRunning(true);
    setResult(null);
    
    try {
      const uiGraph = buildUIGraph(nodes, connections, boundaryOutputs);
      const expandedGraph = serializeToExpandedGraph(uiGraph);
      const response = await validateAndRun(expandedGraph);
      setResult({ status: 'ok', response });
    } catch (err) {
      setResult({ status: 'error', error: { message: err.message } });
    } finally {
      setIsRunning(false);
    }
  }, [nodes, connections, boundaryOutputs]);
  
  // ─────────────────────────────────────────────────────────────────────────────
  // RENDER
  // ─────────────────────────────────────────────────────────────────────────────
  
  return (
    <div style={{ display: 'flex', height: '100vh', width: '100vw', background: COLORS.bg.void }}>
      {/* Sidebar */}
      <div style={{
        width: 220,
        background: COLORS.bg.base,
        borderRight: `1px solid ${COLORS.border.subtle}`,
        display: 'flex',
        flexDirection: 'column',
      }}>
        {/* Header */}
        <div style={{ padding: 12, borderBottom: `1px solid ${COLORS.border.subtle}` }}>
          <div style={{ fontSize: 12, fontWeight: 600, color: COLORS.text.primary }}>
            Contract Validation UI
          </div>
          <div style={{ fontSize: 9, color: COLORS.text.tertiary, marginTop: 4 }}>
            Emit ExpandedGraph → Drive Runtime
          </div>
        </div>
        
        {/* Primitives */}
        <div style={{ padding: 12, borderBottom: `1px solid ${COLORS.border.subtle}` }}>
          <div style={{ fontSize: 10, fontWeight: 600, color: COLORS.text.secondary, marginBottom: 8 }}>
            Hello-World Primitives
          </div>
          {Object.entries(PRIMITIVE_CATALOG).map(([id, primitive]) => (
            <button
              key={id}
              onClick={() => handleAddNode(id)}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 8,
                width: '100%',
                padding: '8px 10px',
                marginBottom: 4,
                background: COLORS.bg.elevated,
                border: `1px solid ${COLORS.border.subtle}`,
                borderRadius: 6,
                cursor: 'pointer',
                textAlign: 'left',
              }}
            >
              <div
                style={{
                  width: 8,
                  height: 8,
                  borderRadius: '50%',
                  background: COLORS.kind[primitive.kind],
                }}
              />
              <div>
                <div style={{ fontSize: 11, color: COLORS.text.primary }}>{primitive.label}</div>
                <div style={{ fontSize: 9, color: COLORS.text.tertiary }}>{primitive.kind}</div>
              </div>
            </button>
          ))}
        </div>
        
        {/* Boundary Outputs */}
        <div style={{ flex: 1, overflow: 'auto' }}>
          <BoundaryOutputPanel
            nodes={nodes}
            boundaryOutputs={boundaryOutputs}
            onAdd={(bo) => setBoundaryOutputs(prev => [...prev, bo])}
            onRemove={(i) => setBoundaryOutputs(prev => prev.filter((_, idx) => idx !== i))}
            onChange={(i, bo) => setBoundaryOutputs(prev => prev.map((b, idx) => idx === i ? bo : b))}
          />
        </div>
        
        {/* Execution */}
        <ExecutionPanel onRun={handleRun} result={result} isRunning={isRunning} />
      </div>
      
      {/* Canvas */}
      <div
        ref={canvasRef}
        style={{
          flex: 1,
          position: 'relative',
          overflow: 'hidden',
          cursor: isPanning ? 'grabbing' : connecting ? 'crosshair' : 'default',
        }}
        onMouseDown={handleCanvasMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
      >
        {/* Grid background */}
        <div style={{
          position: 'absolute',
          inset: 0,
          backgroundImage: `radial-gradient(circle at center, ${COLORS.border.subtle} 1px, transparent 1px)`,
          backgroundSize: '32px 32px',
          backgroundPosition: `${canvasOffset.x}px ${canvasOffset.y}px`,
        }} />
        
        {/* Content */}
        <div
          ref={contentRef}
          style={{
            position: 'absolute',
            transform: `translate(${canvasOffset.x}px, ${canvasOffset.y}px)`,
          }}
        >
          {/* Connections SVG */}
          <svg style={{ position: 'absolute', width: 5000, height: 5000, left: -2500, top: -2500, pointerEvents: 'none' }}>
            <g transform="translate(2500, 2500)">
              {connections.map((conn, i) => (
                <Connection
                  key={i}
                  conn={conn}
                  nodes={nodes}
                  dragNodeId={draggingNodeId}
                  dragPos={dragPos}
                />
              ))}
              {connecting && (
                <Connection
                  conn={{ from: connecting.from, to: null }}
                  nodes={nodes}
                  isTemp
                  mousePos={mousePos}
                  dragNodeId={draggingNodeId}
                  dragPos={dragPos}
                />
              )}
            </g>
          </svg>
          
          {/* Nodes */}
          {nodes.map(node => (
            <Node
              key={node.runtime_id}
              node={node}
              selected={selectedNode === node.runtime_id}
              onSelect={setSelectedNode}
              onDragStart={handleDragStart}
              onParamChange={handleParamChange}
              onDelete={handleDeleteNode}
              onPortMouseDown={handlePortMouseDown}
              onPortMouseEnter={handlePortMouseEnter}
              onPortMouseLeave={handlePortMouseLeave}
              dragPos={dragPos}
              isDragging={draggingNodeId === node.runtime_id}
            />
          ))}
        </div>
        
        {/* Empty state */}
        {nodes.length === 0 && (
          <div style={{
            position: 'absolute',
            inset: 0,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            pointerEvents: 'none',
          }}>
            <div style={{ textAlign: 'center' }}>
              <div style={{ fontSize: 14, color: COLORS.text.tertiary, marginBottom: 8 }}>
                Add nodes from the sidebar
              </div>
              <div style={{ fontSize: 11, color: COLORS.text.tertiary, opacity: 0.6 }}>
                Build the hello-world graph to validate the contract
              </div>
            </div>
          </div>
        )}
        
        {/* Stats */}
        <div style={{
          position: 'absolute',
          bottom: 12,
          left: 12,
          padding: '8px 12px',
          background: COLORS.bg.elevated,
          borderRadius: 6,
          border: `1px solid ${COLORS.border.subtle}`,
          display: 'flex',
          gap: 16,
          fontSize: 10,
        }}>
          <span style={{ color: COLORS.text.tertiary }}>
            Nodes: <span style={{ color: COLORS.text.secondary }}>{nodes.length}</span>
          </span>
          <span style={{ color: COLORS.text.tertiary }}>
            Edges: <span style={{ color: COLORS.text.secondary }}>{connections.length}</span>
          </span>
          <span style={{ color: COLORS.text.tertiary }}>
            Outputs: <span style={{ color: COLORS.text.secondary }}>{boundaryOutputs.length}</span>
          </span>
        </div>
      </div>
    </div>
  );
}
