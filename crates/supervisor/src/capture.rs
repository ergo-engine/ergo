use std::sync::{Arc, Mutex};

use ergo_adapter::capture::ExternalEventRecord;
use ergo_adapter::{ExternalEvent, GraphId, RuntimeInvoker};

use crate::{
    CaptureBundle, Constraints, DecisionLog, DecisionLogEntry, EpisodeInvocationRecord, Supervisor,
};

pub struct CapturingDecisionLog<L: DecisionLog> {
    inner: L,
    bundle: Arc<Mutex<CaptureBundle>>,
}

impl<L: DecisionLog> CapturingDecisionLog<L> {
    pub fn new(inner: L, bundle: Arc<Mutex<CaptureBundle>>) -> Self {
        Self { inner, bundle }
    }
}

impl<L: DecisionLog> DecisionLog for CapturingDecisionLog<L> {
    fn log(&self, entry: DecisionLogEntry) {
        self.inner.log(entry.clone());

        let mut guard = self.bundle.lock().expect("capture bundle poisoned");
        guard.decisions.push(EpisodeInvocationRecord::from(&entry));
    }
}

pub struct CapturingSession<L: DecisionLog, R: RuntimeInvoker> {
    supervisor: Supervisor<CapturingDecisionLog<L>, R>,
    bundle: Arc<Mutex<CaptureBundle>>,
}

impl<L: DecisionLog, R: RuntimeInvoker> CapturingSession<L, R> {
    pub fn new(graph_id: GraphId, constraints: Constraints, inner_log: L, runtime: R) -> Self {
        let bundle = Arc::new(Mutex::new(CaptureBundle {
            capture_version: "v0".to_string(),
            graph_id: graph_id.clone(),
            config: constraints.clone(),
            events: Vec::new(),
            decisions: Vec::new(),
            adapter_version: None,
        }));

        let capturing_log = CapturingDecisionLog::new(inner_log, Arc::clone(&bundle));
        let supervisor = Supervisor::with_runtime(graph_id, constraints, capturing_log, runtime);

        Self { supervisor, bundle }
    }

    pub fn on_event(&mut self, event: ExternalEvent) {
        {
            let mut guard = self.bundle.lock().expect("capture bundle poisoned");
            guard.events.push(ExternalEventRecord::from_event(&event));
        }

        self.supervisor.on_event(event);
    }

    pub fn into_bundle(self) -> CaptureBundle {
        let CapturingSession { supervisor, bundle } = self;
        drop(supervisor);

        match Arc::try_unwrap(bundle) {
            Ok(mutex) => mutex.into_inner().expect("capture bundle poisoned"),
            Err(shared) => shared.lock().expect("capture bundle poisoned").clone(),
        }
    }
}
