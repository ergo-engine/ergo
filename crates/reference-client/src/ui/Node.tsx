/**
 * Node component for graph authoring.
 *
 * Renders a single node with its ports and handles drag.
 */

import React, { useState, useCallback, useRef } from 'react';
import type { UINode } from '../graph/internalModel';

export interface NodeProps {
  node: UINode;
  selected?: boolean;
  onSelect?: () => void;
  onMove?: (x: number, y: number) => void;
}

export const Node: React.FC<NodeProps> = ({
  node,
  selected = false,
  onSelect,
  onMove,
}) => {
  const [isDragging, setIsDragging] = useState(false);
  const dragStart = useRef({ x: 0, y: 0, nodeX: 0, nodeY: 0 });

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onSelect?.();
    setIsDragging(true);
    dragStart.current = {
      x: e.clientX,
      y: e.clientY,
      nodeX: node.x,
      nodeY: node.y,
    };

    const handleMouseMove = (moveEvent: MouseEvent) => {
      const dx = moveEvent.clientX - dragStart.current.x;
      const dy = moveEvent.clientY - dragStart.current.y;
      onMove?.(dragStart.current.nodeX + dx, dragStart.current.nodeY + dy);
    };

    const handleMouseUp = () => {
      setIsDragging(false);
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  }, [node.x, node.y, onSelect, onMove]);

  return (
    <div
      style={{
        position: 'absolute',
        left: node.x,
        top: node.y,
        minWidth: 120,
        backgroundColor: selected ? '#3d3d5c' : '#2d2d44',
        border: `2px solid ${selected ? '#6366f1' : '#4a4a6a'}`,
        borderRadius: 8,
        cursor: isDragging ? 'grabbing' : 'grab',
        userSelect: 'none',
      }}
      onMouseDown={handleMouseDown}
    >
      {/* Header */}
      <div
        style={{
          padding: '8px 12px',
          borderBottom: '1px solid #4a4a6a',
          fontSize: 12,
          fontWeight: 600,
          color: '#e2e8f0',
        }}
      >
        {node.type}
      </div>

      {/* Parameters */}
      {Object.entries(node.params).length > 0 && (
        <div style={{ padding: '8px 12px' }}>
          {Object.entries(node.params).map(([key, param]) => (
            <div
              key={key}
              style={{
                fontSize: 11,
                color: '#94a3b8',
                marginBottom: 4,
              }}
            >
              <span style={{ color: '#64748b' }}>{key}:</span>{' '}
              <span>{String(param.value)}</span>
            </div>
          ))}
        </div>
      )}

      {/* Node ID */}
      <div
        style={{
          padding: '4px 12px 8px',
          fontSize: 10,
          color: '#64748b',
        }}
      >
        {node.id}
      </div>
    </div>
  );
};
