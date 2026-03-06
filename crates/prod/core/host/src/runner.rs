use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::{Arc, Mutex};

use ergo_adapter::host::{
    ensure_handler_coverage, AppliedWrite, BufferingRuntimeInvoker, ContextStore, EffectHandler,
    SetContextHandler,
};
use ergo_adapter::{
    bind_semantic_event_with_binder, AdapterProvides, EventId, EventTime, ExternalEvent,
    ExternalEventKind, GraphId, RunTermination, RuntimeHandle,
};
use ergo_runtime::common::ActionEffect;
use ergo_supervisor::{
    CaptureBundle, CapturingSession, Constraints, Decision, DecisionLog, DecisionLogEntry,
    NO_ADAPTER_PROVENANCE,
};

use crate::capture_enrichment::{enrich_bundle_with_effects, AppliedEffectsByDecision};
use crate::error::HostedStepError;

#[derive(Debug, Clone)]
pub struct HostedEvent {
    pub event_id: String,
    pub kind: ExternalEventKind,
    pub at: EventTime,
    pub semantic_kind: Option<String>,
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct HostedStepOutcome {
    pub decision: Decision,
    pub termination: Option<RunTermination>,
    pub retry_count: usize,
    pub effects: Vec<ActionEffect>,
    pub applied_writes: Vec<AppliedWrite>,
}

#[derive(Debug)]
pub struct HostedAdapterConfig {
    pub provides: AdapterProvides,
    pub binder: ergo_adapter::EventBinder,
    pub adapter_provenance: String,
}

#[derive(Clone, Default)]
struct HostDecisionLog {
    entries: Arc<Mutex<Vec<DecisionLogEntry>>>,
}

impl DecisionLog for HostDecisionLog {
    fn log(&self, entry: DecisionLogEntry) {
        let mut guard = self.entries.lock().expect("host decision log poisoned");
        guard.push(entry);
    }
}

impl HostDecisionLog {
    fn len(&self) -> usize {
        let guard = self.entries.lock().expect("host decision log poisoned");
        guard.len()
    }

    fn get(&self, index: usize) -> Option<DecisionLogEntry> {
        let guard = self.entries.lock().expect("host decision log poisoned");
        guard.get(index).cloned()
    }
}

struct AdapterMode {
    provides: AdapterProvides,
    binder: ergo_adapter::EventBinder,
}

pub struct HostedRunner {
    session: CapturingSession<HostDecisionLog, BufferingRuntimeInvoker>,
    decision_log: HostDecisionLog,
    runtime: BufferingRuntimeInvoker,
    context_store: ContextStore,
    seen_event_ids: HashSet<String>,
    adapter: Option<AdapterMode>,
    handlers: BTreeMap<String, Arc<dyn EffectHandler>>,
    applied_effects: AppliedEffectsByDecision,
}

impl HostedRunner {
    pub fn new(
        graph_id: GraphId,
        constraints: Constraints,
        runtime: RuntimeHandle,
        runtime_provenance: String,
        adapter: Option<HostedAdapterConfig>,
    ) -> Result<Self, HostedStepError> {
        let graph_emittable_effect_kinds = runtime.graph_emittable_effect_kinds();
        let mut handlers: BTreeMap<String, Arc<dyn EffectHandler>> = BTreeMap::new();
        handlers.insert("set_context".to_string(), Arc::new(SetContextHandler));

        if let Some(config) = &adapter {
            let handler_kinds: BTreeSet<String> = handlers.keys().cloned().collect();
            ensure_handler_coverage(
                &config.provides,
                &graph_emittable_effect_kinds,
                &handler_kinds,
            )?;
        }

        let runtime = BufferingRuntimeInvoker::new(runtime);
        let decision_log = HostDecisionLog::default();

        let adapter_provenance = adapter
            .as_ref()
            .map(|config| config.adapter_provenance.clone())
            .unwrap_or_else(|| NO_ADAPTER_PROVENANCE.to_string());

        let session = CapturingSession::new_with_provenance(
            graph_id,
            constraints,
            decision_log.clone(),
            runtime.clone(),
            adapter_provenance,
            runtime_provenance,
        );

        let adapter = adapter.map(|config| AdapterMode {
            provides: config.provides,
            binder: config.binder,
        });

        Ok(Self {
            session,
            decision_log,
            runtime,
            context_store: ContextStore::new(),
            seen_event_ids: HashSet::new(),
            adapter,
            handlers,
            applied_effects: AppliedEffectsByDecision::default(),
        })
    }

    pub fn step(&mut self, event: HostedEvent) -> Result<HostedStepOutcome, HostedStepError> {
        let external_event = self.build_external_event(event)?;
        self.execute_step(external_event)
    }

    pub fn replay_step(
        &mut self,
        external_event: ExternalEvent,
    ) -> Result<HostedStepOutcome, HostedStepError> {
        self.execute_step(external_event)
    }

    fn execute_step(
        &mut self,
        external_event: ExternalEvent,
    ) -> Result<HostedStepOutcome, HostedStepError> {
        let event_id = external_event.event_id().as_str().to_string();
        if !self.seen_event_ids.insert(event_id.clone()) {
            return Err(HostedStepError::DuplicateEventId { event_id });
        }

        if self.runtime.pending_effect_count() != 0 {
            return Err(HostedStepError::LifecycleViolation {
                detail: "pending effect buffer must be drained before next on_event".to_string(),
            });
        }

        let pre_entry_len = self.decision_log.len();
        let pre_run_calls = self.runtime.run_call_count();

        self.session.on_event(external_event);

        let post_entry_len = self.decision_log.len();
        if post_entry_len != pre_entry_len + 1 {
            return Err(HostedStepError::LifecycleViolation {
                detail: format!(
                    "expected exactly one decision entry for step, got {} new entries",
                    post_entry_len.saturating_sub(pre_entry_len)
                ),
            });
        }

        let decision_index = post_entry_len - 1;
        let entry = self
            .decision_log
            .get(decision_index)
            .ok_or(HostedStepError::MissingDecisionEntry)?;

        let run_calls = self.runtime.run_call_count().saturating_sub(pre_run_calls);

        let drained_effects = self.runtime.drain_pending_effects();
        let mut applied_writes = Vec::new();

        if entry.decision == Decision::Invoke {
            let expected_calls = (entry.retry_count as u64).saturating_add(1);
            if run_calls != expected_calls {
                return Err(HostedStepError::LifecycleViolation {
                    detail: format!(
                        "run call count mismatch: expected {expected_calls}, got {run_calls}"
                    ),
                });
            }

            if let Some(adapter) = &self.adapter {
                for effect in &drained_effects {
                    let Some(handler) = self.handlers.get(&effect.kind) else {
                        return Err(HostedStepError::from(
                            ergo_adapter::host::EffectApplyError::UnhandledEffectKind {
                                kind: effect.kind.clone(),
                            },
                        ));
                    };

                    // SUP-6 alignment: no rollback on handler failure.
                    let writes =
                        handler.apply(effect, &mut self.context_store, &adapter.provides)?;
                    applied_writes.extend(writes);
                }
            } else if !drained_effects.is_empty() {
                return Err(HostedStepError::EffectsWithoutAdapter);
            }

            if !drained_effects.is_empty() {
                self.applied_effects
                    .record(decision_index, drained_effects.clone());
            }
        } else {
            if run_calls != 0 {
                return Err(HostedStepError::LifecycleViolation {
                    detail: format!(
                        "non-invoke decision {:?} unexpectedly performed {run_calls} run calls",
                        entry.decision
                    ),
                });
            }

            if !drained_effects.is_empty() {
                return Err(HostedStepError::LifecycleViolation {
                    detail: "non-invoke decision produced pending effects".to_string(),
                });
            }
        }

        Ok(HostedStepOutcome {
            decision: entry.decision,
            termination: entry.termination,
            retry_count: entry.retry_count,
            effects: drained_effects,
            applied_writes,
        })
    }

    pub fn into_capture_bundle(self) -> CaptureBundle {
        let mut bundle = self.session.into_bundle();
        enrich_bundle_with_effects(&mut bundle, self.applied_effects.effects());
        bundle
    }

    pub fn context_snapshot(&self) -> &BTreeMap<String, serde_json::Value> {
        self.context_store.snapshot()
    }

    fn build_external_event(&self, event: HostedEvent) -> Result<ExternalEvent, HostedStepError> {
        if let Some(adapter) = &self.adapter {
            let semantic_kind = event
                .semantic_kind
                .as_deref()
                .ok_or(HostedStepError::MissingSemanticKind)?;

            let incoming_payload = event.payload.ok_or(HostedStepError::MissingPayload)?;
            let incoming_object = incoming_payload
                .as_object()
                .ok_or(HostedStepError::PayloadMustBeObject)?;

            let allowed_store_keys = allowed_schema_keys(adapter, semantic_kind)?;

            let mut merged = serde_json::Map::new();
            for (key, value) in self.context_store.snapshot() {
                if adapter.provides.context.contains_key(key) && allowed_store_keys.contains(key) {
                    merged.insert(key.clone(), value.clone());
                }
            }

            for (key, value) in incoming_object {
                merged.insert(key.clone(), value.clone());
            }

            return bind_semantic_event_with_binder(
                &adapter.binder,
                EventId::new(event.event_id),
                event.kind,
                event.at,
                semantic_kind,
                serde_json::Value::Object(merged),
            )
            .map_err(|err| HostedStepError::BindingError(err.to_string()));
        }

        match event.payload {
            Some(payload) => {
                let object = payload
                    .as_object()
                    .ok_or(HostedStepError::PayloadMustBeObject)?;
                let bytes = serde_json::to_vec(object)
                    .map_err(|err| HostedStepError::EventBuildError(err.to_string()))?;
                ExternalEvent::with_payload(
                    EventId::new(event.event_id),
                    event.kind,
                    event.at,
                    ergo_adapter::EventPayload { data: bytes },
                )
                .map_err(|err| HostedStepError::EventBuildError(err.to_string()))
            }
            None => Ok(ExternalEvent::mechanical_at(
                EventId::new(event.event_id),
                event.kind,
                event.at,
            )),
        }
    }
}

fn allowed_schema_keys(
    adapter: &AdapterMode,
    semantic_kind: &str,
) -> Result<HashSet<String>, HostedStepError> {
    let Some(schema) = adapter.provides.event_schemas.get(semantic_kind) else {
        return Err(HostedStepError::UnknownSemanticKind {
            kind: semantic_kind.to_string(),
        });
    };

    let mut keys = HashSet::new();
    if let Some(properties) = schema.get("properties").and_then(|value| value.as_object()) {
        for key in properties.keys() {
            keys.insert(key.clone());
        }
    }

    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ergo_adapter::{compile_event_binder, ContextKeyProvision, RuntimeHandle};
    use ergo_runtime::catalog::{build_core_catalog, core_registries};
    use ergo_runtime::cluster::{
        ExpandedEdge, ExpandedEndpoint, ExpandedGraph, ExpandedNode, ImplementationInstance,
        OutputPortSpec, OutputRef, ParameterValue,
    };
    use ergo_supervisor::Constraints;
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    fn build_context_set_bool_graph() -> ExpandedGraph {
        let mut nodes = HashMap::new();

        nodes.insert(
            "gate".to_string(),
            ExpandedNode {
                runtime_id: "gate".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "const_bool".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([("value".to_string(), ParameterValue::Bool(true))]),
            },
        );

        nodes.insert(
            "payload".to_string(),
            ExpandedNode {
                runtime_id: "payload".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "boolean_source".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([("value".to_string(), ParameterValue::Bool(false))]),
            },
        );

        nodes.insert(
            "emit".to_string(),
            ExpandedNode {
                runtime_id: "emit".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "emit_if_true".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::new(),
            },
        );

        nodes.insert(
            "ctx_set".to_string(),
            ExpandedNode {
                runtime_id: "ctx_set".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "context_set_bool".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([(
                    "key".to_string(),
                    ParameterValue::String("armed".to_string()),
                )]),
            },
        );

        let edges = vec![
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "gate".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "input".to_string(),
                },
            },
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "event".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "ctx_set".to_string(),
                    port_name: "event".to_string(),
                },
            },
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "payload".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "ctx_set".to_string(),
                    port_name: "value".to_string(),
                },
            },
        ];

        ExpandedGraph {
            nodes,
            edges,
            boundary_inputs: vec![],
            boundary_outputs: vec![OutputPortSpec {
                name: "outcome".to_string(),
                maps_to: OutputRef {
                    node_id: "ctx_set".to_string(),
                    port_name: "outcome".to_string(),
                },
            }],
        }
    }

    fn build_number_source_graph() -> ExpandedGraph {
        let nodes = HashMap::from([(
            "src".to_string(),
            ExpandedNode {
                runtime_id: "src".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "number_source".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([("value".to_string(), ParameterValue::Number(1.0))]),
            },
        )]);

        ExpandedGraph {
            nodes,
            edges: vec![],
            boundary_inputs: vec![],
            boundary_outputs: vec![OutputPortSpec {
                name: "value".to_string(),
                maps_to: OutputRef {
                    node_id: "src".to_string(),
                    port_name: "value".to_string(),
                },
            }],
        }
    }

    fn build_context_set_number_from_price_graph() -> ExpandedGraph {
        let mut nodes = HashMap::new();

        nodes.insert(
            "gate".to_string(),
            ExpandedNode {
                runtime_id: "gate".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "const_bool".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([("value".to_string(), ParameterValue::Bool(true))]),
            },
        );

        nodes.insert(
            "emit".to_string(),
            ExpandedNode {
                runtime_id: "emit".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "emit_if_true".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::new(),
            },
        );

        nodes.insert(
            "price_source".to_string(),
            ExpandedNode {
                runtime_id: "price_source".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "context_number_source".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([(
                    "key".to_string(),
                    ParameterValue::String("price".to_string()),
                )]),
            },
        );

        nodes.insert(
            "ctx_set".to_string(),
            ExpandedNode {
                runtime_id: "ctx_set".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "context_set_number".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([(
                    "key".to_string(),
                    ParameterValue::String("ema".to_string()),
                )]),
            },
        );

        let edges = vec![
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "gate".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "input".to_string(),
                },
            },
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "event".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "ctx_set".to_string(),
                    port_name: "event".to_string(),
                },
            },
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "price_source".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "ctx_set".to_string(),
                    port_name: "value".to_string(),
                },
            },
        ];

        ExpandedGraph {
            nodes,
            edges,
            boundary_inputs: vec![],
            boundary_outputs: vec![OutputPortSpec {
                name: "outcome".to_string(),
                maps_to: OutputRef {
                    node_id: "ctx_set".to_string(),
                    port_name: "outcome".to_string(),
                },
            }],
        }
    }

    fn build_merge_precedence_graph() -> ExpandedGraph {
        let mut nodes = HashMap::new();

        nodes.insert(
            "armed_src".to_string(),
            ExpandedNode {
                runtime_id: "armed_src".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "context_bool_source".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([(
                    "key".to_string(),
                    ParameterValue::String("armed".to_string()),
                )]),
            },
        );

        nodes.insert(
            "not_state".to_string(),
            ExpandedNode {
                runtime_id: "not_state".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "not".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::new(),
            },
        );

        nodes.insert(
            "emit".to_string(),
            ExpandedNode {
                runtime_id: "emit".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "emit_if_true".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::new(),
            },
        );

        nodes.insert(
            "set_value".to_string(),
            ExpandedNode {
                runtime_id: "set_value".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "boolean_source".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([("value".to_string(), ParameterValue::Bool(true))]),
            },
        );

        nodes.insert(
            "set_armed".to_string(),
            ExpandedNode {
                runtime_id: "set_armed".to_string(),
                authoring_path: vec![],
                implementation: ImplementationInstance {
                    impl_id: "context_set_bool".to_string(),
                    requested_version: "0.1.0".to_string(),
                    version: "0.1.0".to_string(),
                },
                parameters: HashMap::from([(
                    "key".to_string(),
                    ParameterValue::String("armed".to_string()),
                )]),
            },
        );

        let edges = vec![
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "armed_src".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "not_state".to_string(),
                    port_name: "value".to_string(),
                },
            },
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "not_state".to_string(),
                    port_name: "result".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "input".to_string(),
                },
            },
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "emit".to_string(),
                    port_name: "event".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "set_armed".to_string(),
                    port_name: "event".to_string(),
                },
            },
            ExpandedEdge {
                from: ExpandedEndpoint::NodePort {
                    node_id: "set_value".to_string(),
                    port_name: "value".to_string(),
                },
                to: ExpandedEndpoint::NodePort {
                    node_id: "set_armed".to_string(),
                    port_name: "value".to_string(),
                },
            },
        ];

        ExpandedGraph {
            nodes,
            edges,
            boundary_inputs: vec![],
            boundary_outputs: vec![OutputPortSpec {
                name: "outcome".to_string(),
                maps_to: OutputRef {
                    node_id: "set_armed".to_string(),
                    port_name: "outcome".to_string(),
                },
            }],
        }
    }

    // Allow non-Send/Sync in Arc: CoreRegistries and CorePrimitiveCatalog contain non-Send/Sync types.
    #[allow(clippy::arc_with_non_send_sync)]
    fn runtime_for_graph(graph: ExpandedGraph, provides: AdapterProvides) -> RuntimeHandle {
        RuntimeHandle::new(
            Arc::new(graph),
            Arc::new(build_core_catalog()),
            Arc::new(core_registries().expect("core registries must initialize for host tests")),
            provides,
        )
    }

    fn adapter_provides_with_effects(extra_effects: &[&str]) -> AdapterProvides {
        let mut context = HashMap::new();
        context.insert(
            "armed".to_string(),
            ContextKeyProvision {
                ty: "Bool".to_string(),
                required: false,
                writable: true,
            },
        );
        context.insert(
            "price".to_string(),
            ContextKeyProvision {
                ty: "Number".to_string(),
                required: false,
                writable: false,
            },
        );

        let mut effects = HashSet::from(["set_context".to_string()]);
        for effect in extra_effects {
            effects.insert((*effect).to_string());
        }

        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "price": { "type": "number" },
                "armed": { "type": "boolean" }
            },
            "additionalProperties": false
        });
        let mut event_schemas = HashMap::new();
        event_schemas.insert("price_bar".to_string(), schema);

        AdapterProvides {
            context,
            events: HashSet::from(["price_bar".to_string()]),
            effects,
            event_schemas,
            capture_format_version: "1".to_string(),
            adapter_fingerprint: "adapter:test@1.0.0;sha256:test".to_string(),
        }
    }

    fn adapter_config(provides: AdapterProvides) -> HostedAdapterConfig {
        let binder = compile_event_binder(&provides).expect("event binder should compile");
        HostedAdapterConfig {
            provides,
            binder,
            adapter_provenance: "adapter:test@1.0.0;sha256:test".to_string(),
        }
    }

    fn adapter_provides_for_number_effect() -> AdapterProvides {
        let context = HashMap::from([
            (
                "price".to_string(),
                ContextKeyProvision {
                    ty: "Number".to_string(),
                    required: false,
                    writable: false,
                },
            ),
            (
                "ema".to_string(),
                ContextKeyProvision {
                    ty: "Number".to_string(),
                    required: false,
                    writable: true,
                },
            ),
        ]);

        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "price": { "type": "number" },
                "ema": { "type": "number" }
            },
            "additionalProperties": false
        });
        let mut event_schemas = HashMap::new();
        event_schemas.insert("price_bar".to_string(), schema);

        AdapterProvides {
            context,
            events: HashSet::from(["price_bar".to_string()]),
            effects: HashSet::from(["set_context".to_string()]),
            event_schemas,
            capture_format_version: "1".to_string(),
            adapter_fingerprint: "adapter:test@1.0.0;sha256:test".to_string(),
        }
    }

    #[test]
    fn adapter_bound_step_applies_effects_and_enriches_capture() {
        let provides = adapter_provides_with_effects(&[]);
        let runtime = runtime_for_graph(build_context_set_bool_graph(), provides.clone());
        let adapter = adapter_config(provides);

        let mut runner = HostedRunner::new(
            GraphId::new("g"),
            Constraints::default(),
            runtime,
            "runtime:test".to_string(),
            Some(adapter),
        )
        .expect("hosted runner should initialize");

        let outcome = runner
            .step(HostedEvent {
                event_id: "e1".to_string(),
                kind: ExternalEventKind::Command,
                at: EventTime::default(),
                semantic_kind: Some("price_bar".to_string()),
                payload: Some(serde_json::json!({"price": 101.5})),
            })
            .expect("adapter-bound step should execute");

        assert_eq!(outcome.decision, Decision::Invoke);
        assert_eq!(outcome.termination, Some(RunTermination::Completed));
        assert_eq!(outcome.retry_count, 0);
        assert_eq!(outcome.effects.len(), 1);
        assert_eq!(outcome.effects[0].kind, "set_context");
        assert_eq!(outcome.applied_writes.len(), 1);
        assert_eq!(outcome.applied_writes[0].key, "armed");
        assert_eq!(
            runner.context_snapshot().get("armed"),
            Some(&serde_json::json!(false))
        );

        let bundle = runner.into_capture_bundle();
        let effects = &bundle.decisions[0].effects;
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].effect.kind, "set_context");
    }

    #[test]
    fn non_invoke_decision_has_no_effects_or_applied_writes() {
        let provides = adapter_provides_with_effects(&[]);
        let runtime = runtime_for_graph(build_context_set_bool_graph(), provides.clone());
        let adapter = adapter_config(provides);

        let mut runner = HostedRunner::new(
            GraphId::new("g"),
            Constraints {
                max_in_flight: Some(0),
                ..Constraints::default()
            },
            runtime,
            "runtime:test".to_string(),
            Some(adapter),
        )
        .expect("hosted runner should initialize");

        let outcome = runner
            .step(HostedEvent {
                event_id: "e_defer".to_string(),
                kind: ExternalEventKind::Command,
                at: EventTime::default(),
                semantic_kind: Some("price_bar".to_string()),
                payload: Some(serde_json::json!({"price": 10.0})),
            })
            .expect("defer decision should still produce a step outcome");

        assert_eq!(outcome.decision, Decision::Defer);
        assert!(outcome.termination.is_none());
        assert!(outcome.effects.is_empty());
        assert!(outcome.applied_writes.is_empty());
        assert!(runner.context_snapshot().is_empty());
    }

    #[test]
    fn adapter_independent_mode_executes_without_adapter_config() {
        let runtime = runtime_for_graph(build_number_source_graph(), AdapterProvides::default());
        let mut runner = HostedRunner::new(
            GraphId::new("g"),
            Constraints::default(),
            runtime,
            "runtime:test".to_string(),
            None,
        )
        .expect("hosted runner should initialize in adapter-independent mode");

        let outcome = runner
            .step(HostedEvent {
                event_id: "e1".to_string(),
                kind: ExternalEventKind::Command,
                at: EventTime::default(),
                semantic_kind: None,
                payload: Some(serde_json::json!({"foo": "bar"})),
            })
            .expect("adapter-independent step should execute");

        assert_eq!(outcome.decision, Decision::Invoke);
        assert_eq!(outcome.termination, Some(RunTermination::Completed));
        assert!(outcome.effects.is_empty());
        assert!(outcome.applied_writes.is_empty());
    }

    #[test]
    fn replay_step_runs_shared_lifecycle_and_effect_application() {
        let provides = adapter_provides_with_effects(&[]);
        let runtime = runtime_for_graph(build_context_set_bool_graph(), provides.clone());
        let adapter = adapter_config(provides);

        let mut runner = HostedRunner::new(
            GraphId::new("g"),
            Constraints::default(),
            runtime,
            "runtime:test".to_string(),
            Some(adapter),
        )
        .expect("hosted runner should initialize");

        let event = ergo_adapter::ExternalEvent::with_payload(
            EventId::new("e1"),
            ExternalEventKind::Command,
            EventTime::default(),
            ergo_adapter::EventPayload {
                data: br#"{"price":101.5}"#.to_vec(),
            },
        )
        .expect("payload should produce external event");

        let outcome = runner
            .replay_step(event)
            .expect("replay_step should execute");

        assert_eq!(outcome.decision, Decision::Invoke);
        assert_eq!(outcome.termination, Some(RunTermination::Completed));
        assert_eq!(outcome.retry_count, 0);
        assert_eq!(outcome.effects.len(), 1);
        assert_eq!(outcome.effects[0].kind, "set_context");
        assert_eq!(
            runner.context_snapshot().get("armed"),
            Some(&serde_json::json!(false))
        );
    }

    #[test]
    fn merged_payload_incoming_overrides_store() {
        let provides = adapter_provides_with_effects(&[]);
        let runtime = runtime_for_graph(build_merge_precedence_graph(), provides.clone());
        let adapter = adapter_config(provides);

        let mut runner = HostedRunner::new(
            GraphId::new("g"),
            Constraints::default(),
            runtime,
            "runtime:test".to_string(),
            Some(adapter),
        )
        .expect("hosted runner should initialize");

        let first = runner
            .step(HostedEvent {
                event_id: "e1".to_string(),
                kind: ExternalEventKind::Command,
                at: EventTime::default(),
                semantic_kind: Some("price_bar".to_string()),
                payload: Some(serde_json::json!({"armed": false, "price": 10.0})),
            })
            .expect("first step should execute");
        assert_eq!(first.effects.len(), 1);
        assert_eq!(
            runner.context_snapshot().get("armed"),
            Some(&serde_json::json!(true))
        );

        let second = runner
            .step(HostedEvent {
                event_id: "e2".to_string(),
                kind: ExternalEventKind::Command,
                at: EventTime::default(),
                semantic_kind: Some("price_bar".to_string()),
                payload: Some(serde_json::json!({"price": 11.0})),
            })
            .expect("second step should execute");
        assert!(
            second.effects.is_empty(),
            "store-sourced armed=true should suppress emit on second step"
        );

        let third = runner
            .step(HostedEvent {
                event_id: "e3".to_string(),
                kind: ExternalEventKind::Command,
                at: EventTime::default(),
                semantic_kind: Some("price_bar".to_string()),
                payload: Some(serde_json::json!({"armed": false, "price": 12.0})),
            })
            .expect("third step should execute");
        assert_eq!(
            third.effects.len(),
            1,
            "incoming armed=false must override stored armed=true"
        );
    }

    #[test]
    fn lifecycle_guard_rejects_step_when_pending_effects_exist() {
        let provides = adapter_provides_with_effects(&[]);
        let runtime = runtime_for_graph(build_context_set_bool_graph(), provides.clone());
        let adapter = adapter_config(provides);

        let mut runner = HostedRunner::new(
            GraphId::new("g"),
            Constraints::default(),
            runtime,
            "runtime:test".to_string(),
            Some(adapter),
        )
        .expect("hosted runner should initialize");

        let bypass = ergo_adapter::ExternalEvent::with_payload(
            EventId::new("bypass"),
            ExternalEventKind::Command,
            EventTime::default(),
            ergo_adapter::EventPayload {
                data: br#"{"price":101.5}"#.to_vec(),
            },
        )
        .expect("bypass event should construct");
        runner.session.on_event(bypass);

        let err = runner
            .step(HostedEvent {
                event_id: "e1".to_string(),
                kind: ExternalEventKind::Command,
                at: EventTime::default(),
                semantic_kind: Some("price_bar".to_string()),
                payload: Some(serde_json::json!({"price": 11.0})),
            })
            .expect_err("pending buffer should trigger lifecycle guard");

        match err {
            HostedStepError::LifecycleViolation { detail } => {
                assert!(
                    detail.contains("pending effect buffer must be drained"),
                    "unexpected detail: {detail}"
                );
            }
            other => panic!("expected lifecycle violation, got {:?}", other),
        }
    }

    #[test]
    fn handler_coverage_ignores_non_emittable_accepted_effect() {
        let provides = adapter_provides_with_effects(&["send_notification"]);

        let runtime_ok = runtime_for_graph(build_context_set_bool_graph(), provides.clone());
        let adapter_ok = adapter_config(provides.clone());
        let ok = HostedRunner::new(
            GraphId::new("g"),
            Constraints::default(),
            runtime_ok,
            "runtime:test".to_string(),
            Some(adapter_ok),
        );
        assert!(ok.is_ok());
    }

    #[test]
    fn decision_order_preserves_effects_across_steps() {
        let provides = adapter_provides_for_number_effect();
        let runtime = runtime_for_graph(
            build_context_set_number_from_price_graph(),
            provides.clone(),
        );
        let adapter = adapter_config(provides);

        let mut runner = HostedRunner::new(
            GraphId::new("g"),
            Constraints::default(),
            runtime,
            "runtime:test".to_string(),
            Some(adapter),
        )
        .expect("hosted runner should initialize");

        runner
            .step(HostedEvent {
                event_id: "evt_1".to_string(),
                kind: ExternalEventKind::Command,
                at: EventTime::default(),
                semantic_kind: Some("price_bar".to_string()),
                payload: Some(serde_json::json!({"price": 100.0})),
            })
            .expect("first duplicate-id step should execute");

        runner
            .step(HostedEvent {
                event_id: "evt_2".to_string(),
                kind: ExternalEventKind::Command,
                at: EventTime::default(),
                semantic_kind: Some("price_bar".to_string()),
                payload: Some(serde_json::json!({"price": 200.0})),
            })
            .expect("second duplicate-id step should execute");

        let bundle = runner.into_capture_bundle();
        assert_eq!(bundle.decisions.len(), 2);

        let first_writes = &bundle.decisions[0].effects[0].effect.writes;
        let second_writes = &bundle.decisions[1].effects[0].effect.writes;

        assert_eq!(first_writes[0].key, "ema");
        assert_eq!(second_writes[0].key, "ema");
        assert_eq!(
            first_writes[0].value,
            ergo_runtime::common::Value::Number(100.0)
        );
        assert_eq!(
            second_writes[0].value,
            ergo_runtime::common::Value::Number(200.0)
        );
    }

    #[test]
    fn step_rejects_duplicate_event_id() {
        let runtime = runtime_for_graph(build_number_source_graph(), AdapterProvides::default());
        let mut runner = HostedRunner::new(
            GraphId::new("g"),
            Constraints::default(),
            runtime,
            "runtime:test".to_string(),
            None,
        )
        .expect("hosted runner should initialize");

        runner
            .step(HostedEvent {
                event_id: "dup_evt".to_string(),
                kind: ExternalEventKind::Command,
                at: EventTime::default(),
                semantic_kind: None,
                payload: Some(serde_json::json!({"foo": "bar"})),
            })
            .expect("first event should execute");

        let err = runner
            .step(HostedEvent {
                event_id: "dup_evt".to_string(),
                kind: ExternalEventKind::Command,
                at: EventTime::default(),
                semantic_kind: None,
                payload: Some(serde_json::json!({"foo": "baz"})),
            })
            .expect_err("duplicate event id must fail");

        assert!(matches!(
            err,
            HostedStepError::DuplicateEventId { event_id } if event_id == "dup_evt"
        ));
    }

    #[test]
    fn replay_step_rejects_duplicate_event_id() {
        let runtime = runtime_for_graph(build_number_source_graph(), AdapterProvides::default());
        let mut runner = HostedRunner::new(
            GraphId::new("g"),
            Constraints::default(),
            runtime,
            "runtime:test".to_string(),
            None,
        )
        .expect("hosted runner should initialize");

        let event = ExternalEvent::mechanical(EventId::new("dup_evt"), ExternalEventKind::Command);
        runner
            .replay_step(event.clone())
            .expect("first replay event should execute");

        let err = runner
            .replay_step(event)
            .expect_err("duplicate replay event id must fail");
        assert!(matches!(
            err,
            HostedStepError::DuplicateEventId { event_id } if event_id == "dup_evt"
        ));
    }
}
