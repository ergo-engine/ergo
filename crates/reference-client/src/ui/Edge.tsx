/**
 * Edge component for graph authoring.
 *
 * Renders a connection between two node ports.
 */

import React from 'react';
import type { UIEdge, UINode } from '../graph/internalModel';

export interface EdgeProps {
  edge: UIEdge;
  nodes: UINode[];
  selected?: boolean;
  onSelect?: () => void;
}

// Node dimensions for port position calculation
const NODE_WIDTH = 120;
const NODE_HEADER_HEIGHT = 33;

export const Edge: React.FC<EdgeProps> = ({
  edge,
  nodes,
  selected = false,
  onSelect,
}) => {
  const fromNode = nodes.find(n => n.id === edge.fromNodeId);
  const toNode = nodes.find(n => n.id === edge.toNodeId);

  if (!fromNode || !toNode) {
    return null;
  }

  // Calculate port positions (simplified: right side of from, left side of to)
  const fromX = fromNode.x + NODE_WIDTH;
  const fromY = fromNode.y + NODE_HEADER_HEIGHT / 2;
  const toX = toNode.x;
  const toY = toNode.y + NODE_HEADER_HEIGHT / 2;

  // Calculate control points for a smooth bezier curve
  const dx = Math.abs(toX - fromX);
  const controlOffset = Math.max(dx * 0.5, 50);

  const path = `
    M ${fromX} ${fromY}
    C ${fromX + controlOffset} ${fromY},
      ${toX - controlOffset} ${toY},
      ${toX} ${toY}
  `;

  return (
    <g onClick={onSelect} style={{ cursor: 'pointer' }}>
      {/* Hit area (wider, invisible) */}
      <path
        d={path}
        fill="none"
        stroke="transparent"
        strokeWidth={12}
      />
      {/* Visible edge */}
      <path
        d={path}
        fill="none"
        stroke={selected ? '#6366f1' : '#64748b'}
        strokeWidth={selected ? 3 : 2}
      />
      {/* Arrow head */}
      <circle
        cx={toX}
        cy={toY}
        r={4}
        fill={selected ? '#6366f1' : '#64748b'}
      />
    </g>
  );
};
