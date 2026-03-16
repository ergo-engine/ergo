use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ergo_runtime::catalog::{CorePrimitiveCatalog, CoreRegistries};
use ergo_runtime::cluster::{ExpandedGraph, PrimitiveCatalog, PrimitiveKind};
use ergo_runtime::common::Value;
use ergo_runtime::runtime::{
    execute_with_metadata, validate as runtime_validate, ExecError,
    ExecutionContext as RuntimeExecutionContext, Registries,
};
use serde::{Deserialize, Serialize};

pub mod capture;
pub mod composition;
pub mod errors;
pub mod event_binding;
pub mod fixture;
pub mod host;
pub mod manifest;
pub mod provenance;
pub mod provides;
pub mod registry;
mod schema_materialization;
pub mod validate;

pub use composition::{
    validate_action_adapter_composition, validate_capture_format,
    validate_source_adapter_composition, CompositionError, ContextRequirement, SourceRequires,
};
pub use errors::InvalidAdapter;
pub use event_binding::{
    bind_semantic_event_with_binder, compile_event_binder, EventBinder, EventBindingError,
};
pub use manifest::{
    AcceptsSpec, AdapterManifest, CaptureSpec, ContextKeySpec, EffectSpec, EventKindSpec,
};
pub use provenance::fingerprint as adapter_fingerprint;
pub use provides::{AdapterProvides, ContextKeyProvision};
pub use registry::register;
pub use validate::validate_adapter;

/// Validates that no source in the graph requires context keys when adapter provides are empty.
/// Used by demo/fixture runners that don't have a real adapter.
pub fn ensure_demo_sources_have_no_required_context(
    graph: &ExpandedGraph,
    catalog: &CorePrimitiveCatalog,
    registries: &CoreRegistries,
) -> Result<(), String> {
    for node in graph.nodes.values() {
        let meta = catalog
            .get(&node.implementation.impl_id, &node.implementation.version)
            .ok_or_else(|| {
                format!(
                    "missing catalog metadata for primitive '{}'",
                    node.implementation.impl_id
                )
            })?;
        if meta.kind != PrimitiveKind::Source {
            continue;
        }

        let Some(primitive) = registries.sources.get(&node.implementation.impl_id) else {
            return Err(format!(
                "missing source primitive '{}' in registry",
                node.implementation.impl_id
            ));
        };

        if let Some(req) = primitive
            .manifest()
            .requires
            .context
            .iter()
            .find(|req| req.required)
        {
            return Err(format!(
                "source '{}' requires context key '{}' but adapter provides are empty",
                node.implementation.impl_id, req.name
            ));
        }
    }

    Ok(())
}

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
    /// Deterministic semantic failures that should not be retried.
    ///
    /// Examples: DivisionByZero, NonFiniteOutput.
    /// These will fail identically on retry, so retrying is pathological.
    ///
    /// See: B.2 in PHASE_INVARIANTS.md
    SemanticError,
    DeadlineExceeded,
    Cancelled,
}

/// Result of a runtime invocation, carrying termination status and any effects.
#[derive(Debug, Clone)]
pub struct RunResult {
    pub termination: RunTermination,
    pub effects: Vec<ergo_runtime::common::ActionEffect>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct EventPayload {
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalEventPayloadError {
    InvalidJson { detail: String },
    PayloadMustBeJsonObject { got: String },
}

impl fmt::Display for ExternalEventPayloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson { detail } => write!(f, "payload bytes are not valid JSON: {detail}"),
            Self::PayloadMustBeJsonObject { got } => {
                write!(f, "payload must be a JSON object, got {got}")
            }
        }
    }
}

impl std::error::Error for ExternalEventPayloadError {}

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
    ) -> Result<Self, ExternalEventPayloadError> {
        let context = ExecutionContext::new(context_from_payload(&payload)?);
        Ok(Self::new(event_id, kind, context, at, payload))
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

fn context_from_payload(
    payload: &EventPayload,
) -> Result<RuntimeExecutionContext, ExternalEventPayloadError> {
    let values = payload_values(payload)?;
    Ok(RuntimeExecutionContext::from_values(values))
}

fn payload_values(
    payload: &EventPayload,
) -> Result<HashMap<String, Value>, ExternalEventPayloadError> {
    if payload.data.is_empty() {
        return Ok(HashMap::new());
    }

    let parsed: serde_json::Value = match serde_json::from_slice(&payload.data) {
        Ok(value) => value,
        Err(err) => {
            return Err(ExternalEventPayloadError::InvalidJson {
                detail: err.to_string(),
            });
        }
    };

    let Some(object) = parsed.as_object() else {
        return Err(ExternalEventPayloadError::PayloadMustBeJsonObject {
            got: json_type_name(&parsed).to_string(),
        });
    };

    let mut values = HashMap::new();
    for (key, value) in object {
        if let Some(mapped) = json_to_value(value) {
            values.insert(key.clone(), mapped);
        }
    }

    Ok(values)
}

fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
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

    let items = value.as_array()?;

    let mut series = Vec::with_capacity(items.len());
    for item in items {
        let number = item.as_f64()?;
        series.push(number);
    }

    Some(Value::Series(series))
}

/// RuntimeHandle holds the execution dependencies needed to invoke the runtime.
/// It is constructed with an expanded graph, primitive catalog, registries, and adapter provides.
#[derive(Clone)]
pub struct RuntimeHandle {
    graph: Arc<ExpandedGraph>,
    catalog: Arc<CorePrimitiveCatalog>,
    registries: Arc<CoreRegistries>,
    adapter_provides: AdapterProvides,
}

impl RuntimeHandle {
    pub fn new(
        graph: Arc<ExpandedGraph>,
        catalog: Arc<CorePrimitiveCatalog>,
        registries: Arc<CoreRegistries>,
        adapter_provides: AdapterProvides,
    ) -> Self {
        Self {
            graph,
            catalog,
            registries,
            adapter_provides,
        }
    }

    pub fn run(
        &self,
        graph_id: &GraphId,
        event_id: &EventId,
        ctx: &ExecutionContext,
        deadline: Option<Duration>,
    ) -> RunResult {
        let _ = graph_id;
        let _ = event_id;

        if matches!(deadline, Some(d) if d.is_zero()) {
            return RunResult {
                termination: RunTermination::Aborted,
                effects: vec![],
            };
        }

        let validated = match runtime_validate(&self.graph, &*self.catalog) {
            Ok(graph) => graph,
            Err(_) => {
                return RunResult {
                    termination: RunTermination::Failed(ErrKind::ValidationFailed),
                    effects: vec![],
                }
            }
        };

        if self.validate_composition(&validated).is_err() {
            return RunResult {
                termination: RunTermination::Failed(ErrKind::ValidationFailed),
                effects: vec![],
            };
        }

        // Create temporary Registries reference from owned CoreRegistries
        let registries = Registries {
            sources: &self.registries.sources,
            computes: &self.registries.computes,
            triggers: &self.registries.triggers,
            actions: &self.registries.actions,
        };

        // Call runtime::execute, surface effects through the boundary
        match execute_with_metadata(
            &validated,
            &registries,
            ctx.inner(),
            graph_id.as_str(),
            event_id.as_str(),
        ) {
            Ok(report) => RunResult {
                termination: RunTermination::Completed,
                effects: report.effects,
            },
            Err(exec_err) => {
                let termination = match exec_err {
                    ExecError::ComputeFailed { .. }
                    | ExecError::NonFiniteOutput { .. }
                    | ExecError::MissingRequiredContextKey { .. }
                    | ExecError::ContextKeyTypeMismatch { .. } => {
                        RunTermination::Failed(ErrKind::SemanticError)
                    }
                    _ => RunTermination::Failed(ErrKind::RuntimeError),
                };
                RunResult {
                    termination,
                    effects: vec![],
                }
            }
        }
    }

    /// Derive effect kinds that this composed graph can emit based on registered action manifests.
    pub fn graph_emittable_effect_kinds(&self) -> HashSet<String> {
        let mut kinds = HashSet::new();

        for node in self.graph.nodes.values() {
            let Some(meta) = self
                .catalog
                .get(&node.implementation.impl_id, &node.implementation.version)
            else {
                continue;
            };
            if meta.kind != PrimitiveKind::Action {
                continue;
            }

            let Some(action) = self.registries.actions.get(&node.implementation.impl_id) else {
                continue;
            };

            // Current runtime action surface emits `set_context` for write specs.
            if !action.manifest().effects.writes.is_empty() {
                kinds.insert("set_context".to_string());
            }

            for intent in &action.manifest().effects.intents {
                kinds.insert(intent.name.clone());
            }
        }

        kinds
    }

    fn validate_composition(
        &self,
        graph: &ergo_runtime::runtime::ValidatedGraph,
    ) -> Result<(), CompositionError> {
        // COMP-3: only adapter-bound runs have a capture format to validate.
        if !self.adapter_provides.capture_format_version.is_empty() {
            validate_capture_format(&self.adapter_provides.capture_format_version)?;
        }

        for node in graph.nodes.values() {
            if node.kind != PrimitiveKind::Source {
                continue;
            }

            let Some(primitive) = self.registries.sources.get(&node.impl_id) else {
                continue;
            };

            let manifest = primitive.manifest();
            let source_params =
                source_parameters_with_manifest_defaults(manifest, &node.parameters);
            validate_source_adapter_composition(
                &manifest.requires,
                &self.adapter_provides,
                &source_params,
            )?;
        }

        for node in graph.nodes.values() {
            if node.kind != PrimitiveKind::Action {
                continue;
            }

            let Some(primitive) = self.registries.actions.get(&node.impl_id) else {
                continue;
            };

            let manifest = primitive.manifest();
            validate_action_adapter_composition(
                &manifest.effects,
                &self.adapter_provides,
                &node.parameters,
            )?;
        }

        Ok(())
    }
}

fn source_parameters_with_manifest_defaults(
    manifest: &ergo_runtime::source::SourcePrimitiveManifest,
    node_parameters: &HashMap<String, ergo_runtime::cluster::ParameterValue>,
) -> HashMap<String, ergo_runtime::cluster::ParameterValue> {
    let mut resolved = node_parameters.clone();

    for spec in &manifest.parameters {
        if resolved.contains_key(&spec.name) {
            continue;
        }
        let Some(default) = &spec.default else {
            continue;
        };

        let mapped = match default {
            ergo_runtime::source::ParameterValue::Int(i) => {
                ergo_runtime::cluster::ParameterValue::Int(*i)
            }
            ergo_runtime::source::ParameterValue::Number(n) => {
                ergo_runtime::cluster::ParameterValue::Number(*n)
            }
            ergo_runtime::source::ParameterValue::Bool(b) => {
                ergo_runtime::cluster::ParameterValue::Bool(*b)
            }
            ergo_runtime::source::ParameterValue::String(s) => {
                ergo_runtime::cluster::ParameterValue::String(s.clone())
            }
            ergo_runtime::source::ParameterValue::Enum(e) => {
                ergo_runtime::cluster::ParameterValue::Enum(e.clone())
            }
        };
        resolved.insert(spec.name.clone(), mapped);
    }

    resolved
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
        RuntimeHandle::run(self, graph_id, event_id, ctx, deadline).termination
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
            queue.remove(0)
        } else {
            self.default.clone()
        }
    }
}

#[cfg(test)]
#[allow(clippy::arc_with_non_send_sync)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    use ergo_runtime::action::ActionRegistry;
    use ergo_runtime::catalog::{build_core_catalog, core_registries, CoreRegistries};
    use ergo_runtime::cluster::{ExpandedGraph, ExpandedNode, ImplementationInstance};
    use ergo_runtime::compute::PrimitiveRegistry as ComputeRegistry;
    use ergo_runtime::runtime::ExecutionContext as RuntimeExecutionContext;
    use ergo_runtime::source::{
        Cadence, ContextRequirement, ExecutionSpec, OutputSpec, SourceKind, SourcePrimitive,
        SourcePrimitiveManifest, SourceRegistry, SourceRequires, StateSpec,
    };
    use ergo_runtime::trigger::TriggerRegistry;

    #[test]
    fn fault_runtime_handle_aborts_when_deadline_zero() {
        // FaultRuntimeHandle (the test double) should respect deadline=zero
        let handle = FaultRuntimeHandle::new(RunTermination::Completed);
        let rt_ctx = ergo_runtime::runtime::ExecutionContext::default();
        let ctx = ExecutionContext::new(rt_ctx);
        let result = handle.run(
            &GraphId::new("g"),
            &EventId::new("e"),
            &ctx,
            Some(Duration::ZERO),
        );

        assert_eq!(result, RunTermination::Aborted);
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
        let result = handle.run(&GraphId::new("g"), &EventId::new("e1"), &ctx, None);
        assert_eq!(result, RunTermination::Failed(ErrKind::NetworkTimeout));

        // Second call returns default
        let result = handle.run(&GraphId::new("g"), &EventId::new("e1"), &ctx, None);
        assert_eq!(result, RunTermination::Completed);
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

    #[test]
    fn external_event_with_payload_rejects_non_object_json() {
        let err = ExternalEvent::with_payload(
            EventId::new("e1"),
            ExternalEventKind::Command,
            EventTime::default(),
            EventPayload {
                data: br#"[1,2,3]"#.to_vec(),
            },
        )
        .expect_err("top-level array payload must be rejected");

        assert!(matches!(
            err,
            ExternalEventPayloadError::PayloadMustBeJsonObject { ref got } if got == "array"
        ));
    }

    #[test]
    fn external_event_with_payload_rejects_invalid_json_bytes() {
        let err = ExternalEvent::with_payload(
            EventId::new("e1"),
            ExternalEventKind::Command,
            EventTime::default(),
            EventPayload {
                data: b"not-json".to_vec(),
            },
        )
        .expect_err("invalid JSON bytes must be rejected");

        assert!(matches!(err, ExternalEventPayloadError::InvalidJson { .. }));
    }

    #[test]
    fn runtime_handle_rejects_required_context_when_provides_empty() {
        #[derive(Clone)]
        struct RequiredContextSource {
            manifest: SourcePrimitiveManifest,
        }

        impl SourcePrimitive for RequiredContextSource {
            fn manifest(&self) -> &SourcePrimitiveManifest {
                &self.manifest
            }

            fn produce(
                &self,
                _parameters: &HashMap<String, ergo_runtime::source::ParameterValue>,
                _ctx: &RuntimeExecutionContext,
            ) -> HashMap<String, Value> {
                HashMap::from([("out".to_string(), Value::Number(0.0))])
            }
        }

        let manifest = SourcePrimitiveManifest {
            id: "context_number_source".to_string(),
            version: "0.1.0".to_string(),
            kind: SourceKind::Source,
            inputs: vec![],
            outputs: vec![OutputSpec {
                name: "out".to_string(),
                value_type: ergo_runtime::common::ValueType::Number,
            }],
            parameters: vec![],
            requires: SourceRequires {
                context: vec![ContextRequirement {
                    name: "x".to_string(),
                    ty: ergo_runtime::common::ValueType::Number,
                    required: true,
                }],
            },
            execution: ExecutionSpec {
                deterministic: true,
                cadence: Cadence::Continuous,
            },
            state: StateSpec { allowed: false },
            side_effects: false,
        };

        let mut sources = SourceRegistry::new();
        sources
            .register(Box::new(RequiredContextSource {
                manifest: manifest.clone(),
            }))
            .expect("source registration should succeed");

        let registries = CoreRegistries::new(
            sources,
            ComputeRegistry::new(),
            TriggerRegistry::new(),
            ActionRegistry::new(),
        );

        let catalog = build_core_catalog();

        let graph = ExpandedGraph {
            nodes: HashMap::from([(
                "src".to_string(),
                ExpandedNode {
                    runtime_id: "src".to_string(),
                    authoring_path: vec![],
                    implementation: ImplementationInstance {
                        impl_id: manifest.id.clone(),
                        requested_version: manifest.version.clone(),
                        version: manifest.version.clone(),
                    },
                    parameters: HashMap::new(),
                },
            )]),
            edges: vec![],
            boundary_inputs: vec![],
            boundary_outputs: vec![],
        };

        let runtime = RuntimeHandle::new(
            Arc::new(graph),
            Arc::new(catalog),
            Arc::new(registries),
            AdapterProvides {
                context: HashMap::new(),
                events: HashSet::new(),
                effects: HashSet::new(),
                event_schemas: HashMap::new(),
                capture_format_version: String::new(),
                adapter_fingerprint: String::new(),
            },
        );

        let rt_ctx = RuntimeExecutionContext::default();
        let ctx = ExecutionContext::new(rt_ctx);
        let result = runtime.run(&GraphId::new("g"), &EventId::new("e"), &ctx, None);

        assert_eq!(
            result.termination,
            RunTermination::Failed(ErrKind::ValidationFailed)
        );
    }

    #[test]
    fn runtime_handle_rejects_unsupported_capture_format() {
        let graph = ExpandedGraph {
            nodes: HashMap::from([(
                "src".to_string(),
                ExpandedNode {
                    runtime_id: "src".to_string(),
                    authoring_path: vec![],
                    implementation: ImplementationInstance {
                        impl_id: "number_source".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::from([(
                        "value".to_string(),
                        ergo_runtime::cluster::ParameterValue::Number(1.0),
                    )]),
                },
            )]),
            edges: vec![],
            boundary_inputs: vec![],
            boundary_outputs: vec![],
        };

        let runtime = RuntimeHandle::new(
            Arc::new(graph),
            Arc::new(build_core_catalog()),
            Arc::new(
                core_registries()
                    .expect("core registries should initialize for capture format test"),
            ),
            AdapterProvides {
                context: HashMap::new(),
                events: HashSet::new(),
                effects: HashSet::new(),
                event_schemas: HashMap::new(),
                capture_format_version: "999".to_string(),
                adapter_fingerprint: "adapter:test@1.0.0;sha256:test".to_string(),
            },
        );

        let ctx = ExecutionContext::new(RuntimeExecutionContext::default());
        let result = runtime.run(&GraphId::new("g"), &EventId::new("e"), &ctx, None);
        assert_eq!(
            result.termination,
            RunTermination::Failed(ErrKind::ValidationFailed)
        );
    }

    #[test]
    fn runtime_handle_derives_graph_emittable_effect_kinds() {
        let graph = ExpandedGraph {
            nodes: HashMap::from([(
                "act".to_string(),
                ExpandedNode {
                    runtime_id: "act".to_string(),
                    authoring_path: vec![],
                    implementation: ImplementationInstance {
                        impl_id: "context_set_bool".to_string(),
                        requested_version: "0.1.0".to_string(),
                        version: "0.1.0".to_string(),
                    },
                    parameters: HashMap::from([(
                        "key".to_string(),
                        ergo_runtime::cluster::ParameterValue::String("armed".to_string()),
                    )]),
                },
            )]),
            edges: vec![],
            boundary_inputs: vec![],
            boundary_outputs: vec![],
        };

        let runtime = RuntimeHandle::new(
            Arc::new(graph),
            Arc::new(build_core_catalog()),
            Arc::new(core_registries().expect("core registries should initialize")),
            AdapterProvides::default(),
        );

        let kinds = runtime.graph_emittable_effect_kinds();
        assert!(
            kinds.contains("set_context"),
            "context_set_* actions should derive set_context as emittable"
        );
    }
}
