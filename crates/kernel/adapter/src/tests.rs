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
            effect_schemas: HashMap::new(),
            event_schemas: HashMap::new(),
            capture_format_version: String::new(),
            adapter_fingerprint: String::new(),
        },
    );

    let rt_ctx = RuntimeExecutionContext::default();
    let ctx = ExecutionContext::new(rt_ctx);
    let result = runtime.run(&GraphId::new("g"), &EventId::new("e"), &ctx, None);

    assert_eq!(result, RunTermination::Failed(ErrKind::ValidationFailed));
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
            core_registries().expect("core registries should initialize for capture format test"),
        ),
        AdapterProvides {
            context: HashMap::new(),
            events: HashSet::new(),
            effects: HashSet::new(),
            effect_schemas: HashMap::new(),
            event_schemas: HashMap::new(),
            capture_format_version: "999".to_string(),
            adapter_fingerprint: "adapter:test@1.0.0;sha256:test".to_string(),
        },
    );

    let ctx = ExecutionContext::new(RuntimeExecutionContext::default());
    let result = runtime.run(&GraphId::new("g"), &EventId::new("e"), &ctx, None);
    assert_eq!(result, RunTermination::Failed(ErrKind::ValidationFailed));
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
