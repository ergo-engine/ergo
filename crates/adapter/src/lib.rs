use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ergo_runtime::catalog::{CorePrimitiveCatalog, CoreRegistries};
use ergo_runtime::cluster::ExpandedGraph;
use ergo_runtime::common::Value;
use ergo_runtime::runtime::ExecutionContext as RuntimeExecutionContext;
use ergo_runtime::runtime::Registries;
use serde::{Deserialize, Serialize};

pub mod capture;
pub mod fixture;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GraphId(String);

impl GraphId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(String);

impl EventId {
    pub fn new(id: impl Into<String>) -> Self {
        EventId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrKind {
    NetworkTimeout,
    AdapterUnavailable,
    ValidationFailed,
    RuntimeError,
    DeadlineExceeded,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunTermination {
    Completed,
    TimedOut,
    Aborted,
    Failed(ErrKind),
}

/// ExecutionContext is intentionally opaque to non-adapter callers.
/// Its internals are owned by the runtime and are not constructible
/// outside this crate to satisfy CXT-1.
///
/// ```compile_fail
/// use ergo_adapter::ExecutionContext;
/// use ergo_runtime::runtime::ExecutionContext as RuntimeExecutionContext;
///
/// // Constructor is not visible outside ergo-adapter.
/// let runtime_ctx = RuntimeExecutionContext::default();
/// let _ctx = ExecutionContext::new(runtime_ctx);
/// ```
///
/// ```compile_fail
/// use ergo_adapter::ExecutionContext;
/// use ergo_runtime::runtime::ExecutionContext as RuntimeExecutionContext;
///
/// // Opaque fields cannot be set directly.
/// let runtime_ctx = RuntimeExecutionContext::default();
/// let _ctx = ExecutionContext { inner: runtime_ctx };
/// ```
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    inner: RuntimeExecutionContext,
}

impl ExecutionContext {
    pub(crate) fn new(inner: RuntimeExecutionContext) -> Self {
        Self { inner }
    }

    pub(crate) fn inner(&self) -> &RuntimeExecutionContext {
        &self.inner
    }
}

/// Opaque absolute time used for deterministic scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventTime(Duration);

impl EventTime {
    pub fn from_duration(duration: Duration) -> Self {
        Self(duration)
    }

    pub fn as_duration(&self) -> Duration {
        self.0
    }

    pub fn saturating_add(&self, duration: Duration) -> Self {
        Self(self.0.saturating_add(duration))
    }
}

impl From<Duration> for EventTime {
    fn from(value: Duration) -> Self {
        EventTime::from_duration(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventPayload {
    pub data: Vec<u8>,
}

impl Default for EventPayload {
    fn default() -> Self {
        Self { data: Vec::new() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalEventKind {
    /// Periodic event for graph evaluation cycles.
    /// Renamed from `Tick` for domain neutrality (see TERMINOLOGY.md §9).
    #[serde(alias = "Tick")]
    Pump,
    DataAvailable,
    Command,
}

#[derive(Debug, Clone)]
pub struct ExternalEvent {
    event_id: EventId,
    kind: ExternalEventKind,
    context: ExecutionContext,
    at: EventTime,
    payload: EventPayload,
}

impl ExternalEvent {
    pub(crate) fn new(
        event_id: EventId,
        kind: ExternalEventKind,
        context: ExecutionContext,
        at: EventTime,
        payload: EventPayload,
    ) -> Self {
        Self {
            event_id,
            kind,
            context,
            at,
            payload,
        }
    }

    pub fn mechanical_at(event_id: EventId, kind: ExternalEventKind, at: EventTime) -> Self {
        let context = ExecutionContext::new(RuntimeExecutionContext::default());
        Self::new(event_id, kind, context, at, EventPayload::default())
    }

    pub fn mechanical(event_id: EventId, kind: ExternalEventKind) -> Self {
        Self::mechanical_at(event_id, kind, EventTime::default())
    }

    pub fn with_payload(
        event_id: EventId,
        kind: ExternalEventKind,
        at: EventTime,
        payload: EventPayload,
    ) -> Self {
        let context = ExecutionContext::new(context_from_payload(&payload));
        Self::new(event_id, kind, context, at, payload)
    }

    pub fn context(&self) -> &ExecutionContext {
        &self.context
    }

    pub fn kind(&self) -> ExternalEventKind {
        self.kind
    }

    pub fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub fn at(&self) -> EventTime {
        self.at
    }

    pub fn payload(&self) -> &EventPayload {
        &self.payload
    }
}

fn context_from_payload(payload: &EventPayload) -> RuntimeExecutionContext {
    let values = payload_values(payload);
    RuntimeExecutionContext::from_values(values)
}

fn payload_values(payload: &EventPayload) -> HashMap<String, Value> {
    if payload.data.is_empty() {
        return HashMap::new();
    }

    let parsed: serde_json::Value = match serde_json::from_slice(&payload.data) {
        Ok(value) => value,
        Err(_) => return HashMap::new(),
    };

    let Some(object) = parsed.as_object() else {
        return HashMap::new();
    };

    let mut values = HashMap::new();
    for (key, value) in object {
        if let Some(mapped) = json_to_value(value) {
            values.insert(key.clone(), mapped);
        }
    }

    values
}

fn json_to_value(value: &serde_json::Value) -> Option<Value> {
    if let Some(number) = value.as_f64() {
        return Some(Value::Number(number));
    }

    if let Some(text) = value.as_str() {
        return Some(Value::String(text.to_string()));
    }

    if let Some(flag) = value.as_bool() {
        return Some(Value::Bool(flag));
    }

    let Some(items) = value.as_array() else {
        return None;
    };

    let mut series = Vec::with_capacity(items.len());
    for item in items {
        let Some(number) = item.as_f64() else {
            return None;
        };
        series.push(number);
    }

    Some(Value::Series(series))
}

/// RuntimeHandle holds the execution dependencies needed to invoke the runtime.
/// It is constructed with an expanded graph, primitive catalog, and registries.
#[derive(Clone)]
pub struct RuntimeHandle {
    graph: Arc<ExpandedGraph>,
    catalog: Arc<CorePrimitiveCatalog>,
    registries: Arc<CoreRegistries>,
}

impl RuntimeHandle {
    pub fn new(
        graph: Arc<ExpandedGraph>,
        catalog: Arc<CorePrimitiveCatalog>,
        registries: Arc<CoreRegistries>,
    ) -> Self {
        Self {
            graph,
            catalog,
            registries,
        }
    }

    pub fn run(
        &self,
        graph_id: &GraphId,
        event_id: &EventId,
        ctx: &ExecutionContext,
        deadline: Option<Duration>,
    ) -> RunTermination {
        let _ = graph_id;
        let _ = event_id;

        if matches!(deadline, Some(d) if d.is_zero()) {
            return RunTermination::Aborted;
        }

        // Create temporary Registries reference from owned CoreRegistries
        let registries = Registries {
            sources: &self.registries.sources,
            computes: &self.registries.computes,
            triggers: &self.registries.triggers,
            actions: &self.registries.actions,
        };

        // Call runtime::run, consume ExecutionReport internally (SUP-2)
        match ergo_runtime::runtime::run(&self.graph, &*self.catalog, &registries, ctx.inner()) {
            Ok(_report) => RunTermination::Completed,
            Err(_) => RunTermination::Failed(ErrKind::RuntimeError),
        }
    }
}

pub trait RuntimeInvoker {
    fn run(
        &self,
        graph_id: &GraphId,
        event_id: &EventId,
        ctx: &ExecutionContext,
        deadline: Option<Duration>,
    ) -> RunTermination;
}

impl RuntimeInvoker for RuntimeHandle {
    fn run(
        &self,
        graph_id: &GraphId,
        event_id: &EventId,
        ctx: &ExecutionContext,
        deadline: Option<Duration>,
    ) -> RunTermination {
        Self::run(self, graph_id, event_id, ctx, deadline)
    }
}

#[derive(Clone)]
pub struct FaultRuntimeHandle {
    schedule: Arc<Mutex<HashMap<EventId, Vec<RunTermination>>>>,
    default: RunTermination,
}

impl Default for FaultRuntimeHandle {
    fn default() -> Self {
        Self::new(RunTermination::Completed)
    }
}

impl FaultRuntimeHandle {
    pub fn new(default: RunTermination) -> Self {
        Self {
            schedule: Arc::new(Mutex::new(HashMap::new())),
            default,
        }
    }

    pub fn with_schedule(
        default: RunTermination,
        schedule: HashMap<EventId, Vec<RunTermination>>,
    ) -> Self {
        Self {
            schedule: Arc::new(Mutex::new(schedule)),
            default,
        }
    }

    pub fn push_outcomes(&self, event_id: EventId, outcomes: Vec<RunTermination>) {
        let mut guard = self.schedule.lock().expect("fault schedule poisoned");
        guard.insert(event_id, outcomes);
    }
}

impl RuntimeInvoker for FaultRuntimeHandle {
    fn run(
        &self,
        graph_id: &GraphId,
        event_id: &EventId,
        ctx: &ExecutionContext,
        deadline: Option<Duration>,
    ) -> RunTermination {
        let _ = graph_id;
        let _ = ctx.inner();

        if matches!(deadline, Some(d) if d.is_zero()) {
            return RunTermination::Aborted;
        }

        let mut guard = self.schedule.lock().expect("fault schedule poisoned");
        let queue = guard.entry(event_id.clone()).or_default();
        if !queue.is_empty() {
            return queue.remove(0);
        }

        self.default.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fault_runtime_handle_aborts_when_deadline_zero() {
        // FaultRuntimeHandle (the test double) should respect deadline=zero
        let handle = FaultRuntimeHandle::new(RunTermination::Completed);
        let rt_ctx = ergo_runtime::runtime::ExecutionContext::default();
        let ctx = ExecutionContext::new(rt_ctx);
        let term = handle.run(
            &GraphId::new("g"),
            &EventId::new("e"),
            &ctx,
            Some(Duration::ZERO),
        );

        assert_eq!(term, RunTermination::Aborted);
    }

    #[test]
    fn fault_runtime_handle_returns_scheduled_outcome() {
        let handle = FaultRuntimeHandle::new(RunTermination::Completed);
        handle.push_outcomes(
            EventId::new("e1"),
            vec![RunTermination::Failed(ErrKind::NetworkTimeout)],
        );

        let rt_ctx = ergo_runtime::runtime::ExecutionContext::default();
        let ctx = ExecutionContext::new(rt_ctx);

        // First call returns scheduled outcome
        let term = handle.run(&GraphId::new("g"), &EventId::new("e1"), &ctx, None);
        assert_eq!(term, RunTermination::Failed(ErrKind::NetworkTimeout));

        // Second call returns default
        let term = handle.run(&GraphId::new("g"), &EventId::new("e1"), &ctx, None);
        assert_eq!(term, RunTermination::Completed);
    }

    /// TEST-PUMP-SERDE-1: Verify Pump serializes as "Pump" not "Tick".
    /// The serde(alias = "Tick") allows deserialization of legacy data,
    /// but serialization must produce the canonical name.
    #[test]
    fn pump_serializes_as_pump_not_tick() {
        let serialized = serde_json::to_string(&ExternalEventKind::Pump).unwrap();
        assert_eq!(
            serialized, "\"Pump\"",
            "Pump must serialize as 'Pump', not legacy 'Tick'"
        );

        // Also verify the alias still works for deserialization (backward compat)
        let from_pump: ExternalEventKind = serde_json::from_str("\"Pump\"").unwrap();
        let from_tick: ExternalEventKind = serde_json::from_str("\"Tick\"").unwrap();
        assert_eq!(from_pump, ExternalEventKind::Pump);
        assert_eq!(from_tick, ExternalEventKind::Pump);
    }
}
