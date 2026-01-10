/**
 * Authoring UI entry point.
 */

import React from 'react';
import { createRoot } from 'react-dom/client';

// Re-export contract types for consumers
export * from './contract/contractTypes';

// Re-export graph model and serialization
export * from './graph/internalModel';
export * from './graph/serialize';
export * from './graph/validateLocal';

// Re-export runtime adapter
export * from './runtime/adapter';
export * from './runtime/types';

// Re-export UI components
export { Canvas } from './ui/Canvas';
export { Node } from './ui/Node';
export { Edge } from './ui/Edge';
export { Inspector } from './ui/Inspector';
export { RunPanel } from './ui/RunPanel';

// Re-export examples
export * from './examples/helloWorld';

// App placeholder
const App: React.FC = () => {
  return (
    <div style={{ padding: 20, fontFamily: 'system-ui, sans-serif' }}>
      <h1>Authoring UI</h1>
      <p>Graph authoring surface for Primitive Library.</p>
      <p style={{ color: '#666' }}>
        See <code>src/examples/helloWorld.ts</code> for usage example.
      </p>
    </div>
  );
};

// Mount if we're in a browser with a root element
if (typeof document !== 'undefined') {
  const rootEl = document.getElementById('root');
  if (rootEl) {
    const root = createRoot(rootEl);
    root.render(<App />);
  }
}
