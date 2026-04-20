//! host::buffering_invoker
//!
//! Purpose:
//! - Hold the host-owned runtime buffer shim that captures reported effects
//!   from `ReportingRuntimeHandle::run_reporting(...)` while presenting a
//!   termination-only
//!   `RuntimeInvoker` surface to the supervisor.
//!
//! Owns:
//! - `BufferingRuntimeInvoker` and its replace-and-drain buffer lifecycle.
//! - The private reporting-runtime helper seam used by local tests.
//!
//! Does not own:
//! - The public `RuntimeInvoker` contract or `RuntimeHandle` semantics; those
//!   remain in `ergo_adapter`.
//! - Effect application or capture enrichment; `runner.rs` owns those later
//!   host steps.
//!
//! Connects to:
//! - `runner.rs`, which drains pending effects after each supervisor step.
//! - `ergo_adapter::ReportingRuntimeHandle`, which remains the low-level
//!   engine behind the shim.
//!
//! Safety notes:
//! - Each `run(...)` call replaces the pending-effect buffer rather than
//!   extending it, so retries preserve the latest attempt only.
//! - `drain_pending_effects()` is single-use and clears the buffer.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ergo_adapter::{
    EventId, ExecutionContext, GraphId, ReportingRuntimeHandle, RunTermination, RuntimeInvoker,
};
use ergo_runtime::common::ActionEffect;

trait ReportingRuntime {
    fn run_reporting(
        &self,
        graph_id: &GraphId,
        event_id: &EventId,
        ctx: &ExecutionContext,
        deadline: Option<Duration>,
        effects_out: &mut Vec<ActionEffect>,
    ) -> RunTermination;

    fn graph_emittable_effect_kinds(&self) -> HashSet<String>;
}

impl ReportingRuntime for ReportingRuntimeHandle {
    fn run_reporting(
        &self,
        graph_id: &GraphId,
        event_id: &EventId,
        ctx: &ExecutionContext,
        deadline: Option<Duration>,
        effects_out: &mut Vec<ActionEffect>,
    ) -> RunTermination {
        ReportingRuntimeHandle::run_reporting(self, graph_id, event_id, ctx, deadline, effects_out)
    }

    fn graph_emittable_effect_kinds(&self) -> HashSet<String> {
        ReportingRuntimeHandle::graph_emittable_effect_kinds(self)
    }
}

#[derive(Debug, Default)]
struct BufferState {
    pending_effects: Vec<ActionEffect>,
    run_call_count: u64,
}

#[derive(Clone)]
pub struct BufferingRuntimeInvoker {
    engine: Arc<dyn ReportingRuntime>,
    graph_emittable_effect_kinds: Arc<HashSet<String>>,
    state: Arc<Mutex<BufferState>>,
}

impl BufferingRuntimeInvoker {
    // Allow non-Send/Sync in Arc: ReportingRuntimeHandle contains non-Send/Sync trait object types.
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new(inner: ReportingRuntimeHandle) -> Self {
        Self::new_with_provider(Arc::new(inner))
    }

    fn new_with_provider(engine: Arc<dyn ReportingRuntime>) -> Self {
        let graph_emittable_effect_kinds = Arc::new(engine.graph_emittable_effect_kinds());
        Self {
            engine,
            graph_emittable_effect_kinds,
            state: Arc::new(Mutex::new(BufferState::default())),
        }
    }

    pub fn drain_pending_effects(&self) -> Vec<ActionEffect> {
        let mut guard = self.state.lock().expect("buffering runtime state poisoned");
        std::mem::take(&mut guard.pending_effects)
    }

    pub fn pending_effect_count(&self) -> usize {
        let guard = self.state.lock().expect("buffering runtime state poisoned");
        guard.pending_effects.len()
    }

    pub fn run_call_count(&self) -> u64 {
        let guard = self.state.lock().expect("buffering runtime state poisoned");
        guard.run_call_count
    }

    pub fn graph_emittable_effect_kinds(&self) -> &HashSet<String> {
        self.graph_emittable_effect_kinds.as_ref()
    }
}

impl RuntimeInvoker for BufferingRuntimeInvoker {
    fn run(
        &self,
        graph_id: &GraphId,
        event_id: &EventId,
        ctx: &ExecutionContext,
        deadline: Option<Duration>,
    ) -> RunTermination {
        let mut effects = vec![];
        let termination =
            self.engine
                .run_reporting(graph_id, event_id, ctx, deadline, &mut effects);

        let mut guard = self.state.lock().expect("buffering runtime state poisoned");
        guard.run_call_count = guard.run_call_count.saturating_add(1);
        guard.pending_effects = effects;

        termination
    }
}

#[cfg(test)]
mod tests;
