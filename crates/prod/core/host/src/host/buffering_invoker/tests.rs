use super::*;
use ergo_adapter::{ErrKind, EventTime, ExternalEvent, ExternalEventKind};
use ergo_runtime::common::{EffectWrite, Value};

struct ScriptedRun {
    termination: RunTermination,
    effects: Vec<ActionEffect>,
}

struct ScriptedProvider {
    queue: Mutex<Vec<ScriptedRun>>,
    graph_emittable_effect_kinds: HashSet<String>,
}

impl ScriptedProvider {
    fn new(queue: Vec<ScriptedRun>) -> Self {
        Self {
            queue: Mutex::new(queue),
            graph_emittable_effect_kinds: HashSet::new(),
        }
    }
}

impl ReportingRuntime for ScriptedProvider {
    fn run_reporting(
        &self,
        _graph_id: &GraphId,
        _event_id: &EventId,
        _ctx: &ExecutionContext,
        _deadline: Option<Duration>,
        effects_out: &mut Vec<ActionEffect>,
    ) -> RunTermination {
        let mut guard = self.queue.lock().expect("scripted queue poisoned");
        if guard.is_empty() {
            effects_out.clear();
            return RunTermination::Completed;
        }
        let scripted = guard.remove(0);
        *effects_out = scripted.effects;
        scripted.termination
    }

    fn graph_emittable_effect_kinds(&self) -> HashSet<String> {
        self.graph_emittable_effect_kinds.clone()
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
        ScriptedRun {
            termination: RunTermination::Failed(ErrKind::NetworkTimeout),
            effects: vec![effect_for_key("first", 1.0)],
        },
        ScriptedRun {
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
    assert_eq!(first, RunTermination::Failed(ErrKind::NetworkTimeout));
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
    let provider = Arc::new(ScriptedProvider::new(vec![ScriptedRun {
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
