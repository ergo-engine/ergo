//! host::buffering_invoker
//!
//! Purpose:
//! - Hold the host-owned runtime buffer shim that captures `RunResult.effects`
//!   from `RuntimeHandle::run(...)` while presenting a termination-only
//!   `RuntimeInvoker` surface to the supervisor.
//!
//! Owns:
//! - `BufferingRuntimeInvoker` and its replace-and-drain buffer lifecycle.
//! - The private `RuntimeResultProvider` helper seam used by local tests.
//!
//! Does not own:
//! - The public `RuntimeInvoker` contract or `RuntimeHandle` semantics; those
//!   remain in `ergo_adapter`.
//! - Effect application or capture enrichment; `runner.rs` owns those later
//!   host steps.
//!
//! Connects to:
//! - `runner.rs`, which drains pending effects after each supervisor step.
//! - `ergo_adapter::RuntimeHandle`, which remains the engine behind the shim.
//!
//! Safety notes:
//! - Each `run(...)` call replaces the pending-effect buffer rather than
//!   extending it, so retries preserve the latest attempt only.
//! - `drain_pending_effects()` is single-use and clears the buffer.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use ergo_adapter::{
    EventId, ExecutionContext, GraphId, RunResult, RunTermination, RuntimeHandle, RuntimeInvoker,
};
use ergo_runtime::common::ActionEffect;

trait RuntimeResultProvider {
    fn run_result(
        &self,
        graph_id: &GraphId,
        event_id: &EventId,
        ctx: &ExecutionContext,
        deadline: Option<Duration>,
    ) -> RunResult;
}

impl RuntimeResultProvider for RuntimeHandle {
    fn run_result(
        &self,
        graph_id: &GraphId,
        event_id: &EventId,
        ctx: &ExecutionContext,
        deadline: Option<Duration>,
    ) -> RunResult {
        self.run(graph_id, event_id, ctx, deadline)
    }
}

#[derive(Debug, Default)]
struct BufferState {
    pending_effects: Vec<ActionEffect>,
    run_call_count: u64,
}

#[derive(Clone)]
pub struct BufferingRuntimeInvoker {
    engine: Arc<dyn RuntimeResultProvider>,
    state: Arc<Mutex<BufferState>>,
}

impl BufferingRuntimeInvoker {
    // Allow non-Send/Sync in Arc: RuntimeHandle contains non-Send/Sync trait object types.
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new(inner: RuntimeHandle) -> Self {
        Self::new_with_provider(Arc::new(inner))
    }

    fn new_with_provider(engine: Arc<dyn RuntimeResultProvider>) -> Self {
        Self {
            engine,
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
}

impl RuntimeInvoker for BufferingRuntimeInvoker {
    fn run(
        &self,
        graph_id: &GraphId,
        event_id: &EventId,
        ctx: &ExecutionContext,
        deadline: Option<Duration>,
    ) -> RunTermination {
        let result = self.engine.run_result(graph_id, event_id, ctx, deadline);

        let mut guard = self.state.lock().expect("buffering runtime state poisoned");
        guard.run_call_count = guard.run_call_count.saturating_add(1);
        guard.pending_effects = result.effects;

        result.termination
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ergo_adapter::{ErrKind, EventTime, ExternalEvent, ExternalEventKind};
    use ergo_runtime::common::{EffectWrite, Value};

    struct ScriptedProvider {
        queue: Mutex<Vec<RunResult>>,
    }

    impl ScriptedProvider {
        fn new(queue: Vec<RunResult>) -> Self {
            Self {
                queue: Mutex::new(queue),
            }
        }
    }

    impl RuntimeResultProvider for ScriptedProvider {
        fn run_result(
            &self,
            _graph_id: &GraphId,
            _event_id: &EventId,
            _ctx: &ExecutionContext,
            _deadline: Option<Duration>,
        ) -> RunResult {
            let mut guard = self.queue.lock().expect("scripted queue poisoned");
            if guard.is_empty() {
                return RunResult {
                    termination: RunTermination::Completed,
                    effects: vec![],
                };
            }
            guard.remove(0)
        }
    }

    fn effect_for_key(key: &str, value: f64) -> ActionEffect {
        ActionEffect {
            kind: "set_context".to_string(),
            writes: vec![EffectWrite {
                key: key.to_string(),
                value: Value::Number(value),
            }],
            intents: vec![],
        }
    }

    #[test]
    fn replaces_pending_effects_on_retry_attempt() {
        let provider = Arc::new(ScriptedProvider::new(vec![
            RunResult {
                termination: RunTermination::Failed(ErrKind::NetworkTimeout),
                effects: vec![effect_for_key("first", 1.0)],
            },
            RunResult {
                termination: RunTermination::Completed,
                effects: vec![effect_for_key("second", 2.0)],
            },
        ]));
        let invoker = BufferingRuntimeInvoker::new_with_provider(provider);
        let ctx = ExternalEvent::mechanical_at(
            EventId::new("seed"),
            ExternalEventKind::Command,
            EventTime::default(),
        )
        .context()
        .clone();
        let graph_id = GraphId::new("g");
        let event_id = EventId::new("e");

        let first = invoker.run(&graph_id, &event_id, &ctx, None);
        assert_eq!(
            first,
            RunTermination::Failed(ErrKind::NetworkTimeout)
        );
        assert_eq!(invoker.pending_effect_count(), 1);

        let second = invoker.run(&graph_id, &event_id, &ctx, None);
        assert_eq!(second, RunTermination::Completed);
        assert_eq!(invoker.pending_effect_count(), 1);
        assert_eq!(invoker.run_call_count(), 2);

        let drained = invoker.drain_pending_effects();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].writes[0].key, "second");
    }

    #[test]
    fn drain_pending_effects_is_single_use_and_clears_buffer() {
        let provider = Arc::new(ScriptedProvider::new(vec![RunResult {
            termination: RunTermination::Completed,
            effects: vec![effect_for_key("k", 42.0)],
        }]));
        let invoker = BufferingRuntimeInvoker::new_with_provider(provider);
        let ctx = ExternalEvent::mechanical_at(
            EventId::new("seed"),
            ExternalEventKind::Command,
            EventTime::default(),
        )
        .context()
        .clone();

        let _ = invoker.run(&GraphId::new("g"), &EventId::new("e"), &ctx, None);
        assert_eq!(invoker.pending_effect_count(), 1);

        let first = invoker.drain_pending_effects();
        assert_eq!(first.len(), 1);
        assert_eq!(invoker.pending_effect_count(), 0);

        let second = invoker.drain_pending_effects();
        assert!(second.is_empty());
    }
}
