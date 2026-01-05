/**
 * Hello World example - builds graph via UI state, serializes to ExpandedGraph.
 *
 * This demonstrates the complete flow from UI model to runtime contract.
 *
 * Graph structure:
 *   number_source(value=3.0) → gt:a
 *   number_source(value=1.0) → gt:b
 *   gt.result → emit_if_true.input
 *   emit_if_true.event → ack_action.event
 *
 * Expected result: action_outcome = Event(Action(Completed))
 */

import {
  createGraph,
  createNode,
  createEdge,
  resetIdCounters,
  type UIGraph,
} from '../graph/internalModel';
import { serializeToExpandedGraph } from '../graph/serialize';

/**
 * Build the hello-world graph using the UI model.
 */
export function buildHelloWorldGraph(): UIGraph {
  // Reset for deterministic IDs
  resetIdCounters();

  const graph = createGraph();

  // Create nodes
  const srcA = createNode('number_source', '0.1.0', 50, 50, {
    value: { type: 'number', value: 3.0 },
  });
  srcA.id = 'src_a';

  const srcB = createNode('number_source', '0.1.0', 50, 150, {
    value: { type: 'number', value: 1.0 },
  });
  srcB.id = 'src_b';

  const gt1 = createNode('gt', '0.1.0', 250, 100);
  gt1.id = 'gt1';

  const emit = createNode('emit_if_true', '0.1.0', 450, 100);
  emit.id = 'emit';

  const act = createNode('ack_action', '0.1.0', 650, 100, {
    accept: { type: 'bool', value: true },
  });
  act.id = 'act';

  graph.nodes.push(srcA, srcB, gt1, emit, act);

  // Create edges
  graph.edges.push(
    createEdge('src_a', 'value', 'gt1', 'a'),
    createEdge('src_b', 'value', 'gt1', 'b'),
    createEdge('gt1', 'result', 'emit', 'input'),
    createEdge('emit', 'event', 'act', 'event'),
  );

  // Declare boundary output
  graph.boundaryOutputs.push({
    name: 'action_outcome',
    nodeId: 'act',
    portName: 'outcome',
  });

  graph.name = 'Hello World';
  graph.description = 'A simple graph that compares two numbers and triggers an action';

  return graph;
}

/**
 * Serialize the hello-world graph to ExpandedGraph format.
 */
export function getHelloWorldExpandedGraph() {
  const uiGraph = buildHelloWorldGraph();
  return serializeToExpandedGraph(uiGraph);
}

/**
 * Log the serialized graph for debugging.
 */
export function debugHelloWorld(): void {
  const expanded = getHelloWorldExpandedGraph();
  console.log('ExpandedGraph:', JSON.stringify(expanded, null, 2));
}
