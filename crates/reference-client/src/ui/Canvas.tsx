/**
 * Main canvas component for graph authoring.
 *
 * Renders nodes and edges, handles pan/zoom and selection.
 */

import React, { useState, useCallback, useRef } from 'react';
import type { UIGraph, UINode, UIEdge } from '../graph/internalModel';
import { Node } from './Node';
import { Edge } from './Edge';

export interface CanvasProps {
  graph: UIGraph;
  onNodeMove?: (nodeId: string, x: number, y: number) => void;
  onNodeSelect?: (nodeId: string | null) => void;
  onEdgeSelect?: (edgeId: string | null) => void;
  onConnect?: (fromNodeId: string, fromPort: string, toNodeId: string, toPort: string) => void;
  selectedNodeId?: string | null;
  selectedEdgeId?: string | null;
}

export const Canvas: React.FC<CanvasProps> = ({
  graph,
  onNodeMove,
  onNodeSelect,
  onEdgeSelect,
  onConnect,
  selectedNodeId,
  selectedEdgeId,
}) => {
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [scale, setScale] = useState(1);
  const [isPanning, setIsPanning] = useState(false);
  const lastMousePos = useRef({ x: 0, y: 0 });

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      setIsPanning(true);
      lastMousePos.current = { x: e.clientX, y: e.clientY };
      onNodeSelect?.(null);
      onEdgeSelect?.(null);
    }
  }, [onNodeSelect, onEdgeSelect]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (isPanning) {
      const dx = e.clientX - lastMousePos.current.x;
      const dy = e.clientY - lastMousePos.current.y;
      setOffset(prev => ({ x: prev.x + dx, y: prev.y + dy }));
      lastMousePos.current = { x: e.clientX, y: e.clientY };
    }
  }, [isPanning]);

  const handleMouseUp = useCallback(() => {
    setIsPanning(false);
  }, []);

  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    const delta = e.deltaY > 0 ? 0.9 : 1.1;
    setScale(prev => Math.min(Math.max(prev * delta, 0.1), 3));
  }, []);

  return (
    <div
      style={{
        width: '100%',
        height: '100%',
        overflow: 'hidden',
        backgroundColor: '#1a1a2e',
        cursor: isPanning ? 'grabbing' : 'default',
      }}
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      onMouseLeave={handleMouseUp}
      onWheel={handleWheel}
    >
      <svg
        style={{
          width: '100%',
          height: '100%',
          transform: `translate(${offset.x}px, ${offset.y}px) scale(${scale})`,
          transformOrigin: '0 0',
        }}
      >
        {/* Render edges first (behind nodes) */}
        {graph.edges.map(edge => (
          <Edge
            key={edge.id}
            edge={edge}
            nodes={graph.nodes}
            selected={edge.id === selectedEdgeId}
            onSelect={() => onEdgeSelect?.(edge.id)}
          />
        ))}
      </svg>

      {/* Render nodes as DOM elements for better interaction */}
      <div
        style={{
          position: 'absolute',
          top: 0,
          left: 0,
          transform: `translate(${offset.x}px, ${offset.y}px) scale(${scale})`,
          transformOrigin: '0 0',
        }}
      >
        {graph.nodes.map(node => (
          <Node
            key={node.id}
            node={node}
            selected={node.id === selectedNodeId}
            onSelect={() => onNodeSelect?.(node.id)}
            onMove={(x, y) => onNodeMove?.(node.id, x, y)}
          />
        ))}
      </div>
    </div>
  );
};
