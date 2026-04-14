use super::*;
use crate::action::{
    ActionEffects, ActionKind, ActionPrimitiveManifest, Cardinality as ActionCardinality,
    ExecutionSpec as ActionExecutionSpec, InputSpec as ActionInputSpec,
    OutputSpec as ActionOutputSpec, ParameterSpec as ActionParameterSpec,
    ParameterValue as ActionParameterValue, StateSpec as ActionStateSpec,
};
use crate::common::{Value, ValueType};
use crate::compute::{
    Cadence, Cardinality, ComputePrimitive, ComputePrimitiveManifest, ErrorSpec, ExecutionSpec,
    InputSpec, OutputSpec, ParameterSpec, PrimitiveState, StateSpec,
};
use crate::runtime::ExecutionContext;
use crate::source::{
    Cadence as SourceCadence, ExecutionSpec as SourceExecutionSpec, OutputSpec as SourceOutputSpec,
    ParameterSpec as SourceParameterSpec, SourceKind, SourcePrimitiveManifest, SourceRequires,
    StateSpec as SourceStateSpec,
};
use crate::trigger::{
    Cadence as TriggerCadence, Cardinality as TriggerCardinality,
    ExecutionSpec as TriggerExecutionSpec, InputSpec as TriggerInputSpec,
    OutputSpec as TriggerOutputSpec, StateSpec as TriggerStateSpec, TriggerEvent, TriggerKind,
    TriggerPrimitiveManifest, TriggerValue, TriggerValueType,
};
use std::collections::HashMap;

struct TestSource {
    manifest: SourcePrimitiveManifest,
    output: f64,
}

impl TestSource {
    fn new(id: &str, version: &str, output: f64) -> Self {
        Self {
            manifest: SourcePrimitiveManifest {
                id: id.to_string(),
                version: version.to_string(),
                kind: SourceKind::Source,
                inputs: vec![],
                outputs: vec![SourceOutputSpec {
                    name: "value".to_string(),
                    value_type: ValueType::Number,
                }],
                parameters: vec![SourceParameterSpec {
                    name: "unused".to_string(),
                    value_type: crate::source::ParameterType::String,
                    default: Some(crate::source::ParameterValue::String("ok".to_string())),
                    bounds: None,
                }],
                requires: SourceRequires {
                    context: Vec::new(),
                },
                execution: SourceExecutionSpec {
                    deterministic: true,
                    cadence: SourceCadence::Continuous,
                },
                state: SourceStateSpec { allowed: false },
                side_effects: false,
            },
            output,
        }
    }
}

impl SourcePrimitive for TestSource {
    fn manifest(&self) -> &SourcePrimitiveManifest {
        &self.manifest
    }

    fn produce(
        &self,
        _parameters: &HashMap<String, crate::source::ParameterValue>,
        _ctx: &ExecutionContext,
    ) -> HashMap<String, Value> {
        HashMap::from([("value".to_string(), Value::Number(self.output))])
    }
}

struct TestCompute {
    manifest: ComputePrimitiveManifest,
}

impl TestCompute {
    fn new(id: &str, version: &str) -> Self {
        Self {
            manifest: ComputePrimitiveManifest {
                id: id.to_string(),
                version: version.to_string(),
                kind: common::PrimitiveKind::Compute,
                inputs: vec![InputSpec {
                    name: "x".to_string(),
                    value_type: ValueType::Number,
                    required: true,
                    cardinality: Cardinality::Single,
                }],
                outputs: vec![OutputSpec {
                    name: "result".to_string(),
                    value_type: ValueType::Number,
                }],
                parameters: vec![],
                execution: ExecutionSpec {
                    deterministic: true,
                    cadence: Cadence::Continuous,
                    may_error: false,
                },
                errors: ErrorSpec {
                    allowed: false,
                    types: vec![],
                    deterministic: true,
                },
                state: StateSpec {
                    allowed: false,
                    resettable: false,
                    description: None,
                },
                side_effects: false,
            },
        }
    }
}

impl ComputePrimitive for TestCompute {
    fn manifest(&self) -> &ComputePrimitiveManifest {
        &self.manifest
    }

    fn compute(
        &self,
        inputs: &HashMap<String, Value>,
        _parameters: &HashMap<String, Value>,
        _state: Option<&mut PrimitiveState>,
    ) -> Result<HashMap<String, Value>, crate::compute::ComputeError> {
        Ok(HashMap::from([(
            "result".to_string(),
            inputs.get("x").cloned().unwrap_or(Value::Number(0.0)),
        )]))
    }
}

struct TestTrigger {
    manifest: TriggerPrimitiveManifest,
}

impl TestTrigger {
    fn new(id: &str, version: &str) -> Self {
        Self {
            manifest: TriggerPrimitiveManifest {
                id: id.to_string(),
                version: version.to_string(),
                kind: TriggerKind::Trigger,
                inputs: vec![TriggerInputSpec {
                    name: "gate".to_string(),
                    value_type: TriggerValueType::Bool,
                    required: true,
                    cardinality: TriggerCardinality::Single,
                }],
                outputs: vec![TriggerOutputSpec {
                    name: "event".to_string(),
                    value_type: TriggerValueType::Event,
                }],
                parameters: vec![],
                execution: TriggerExecutionSpec {
                    deterministic: true,
                    cadence: TriggerCadence::Continuous,
                },
                state: TriggerStateSpec {
                    allowed: false,
                    description: None,
                },
                side_effects: false,
            },
        }
    }
}

impl TriggerPrimitive for TestTrigger {
    fn manifest(&self) -> &TriggerPrimitiveManifest {
        &self.manifest
    }

    fn evaluate(
        &self,
        inputs: &HashMap<String, TriggerValue>,
        _parameters: &HashMap<String, crate::trigger::ParameterValue>,
    ) -> HashMap<String, TriggerValue> {
        let emitted = matches!(inputs.get("gate"), Some(TriggerValue::Bool(true)));
        let event = if emitted {
            TriggerEvent::Emitted
        } else {
            TriggerEvent::NotEmitted
        };
        HashMap::from([("event".to_string(), TriggerValue::Event(event))])
    }
}

struct TestAction {
    manifest: ActionPrimitiveManifest,
}

impl TestAction {
    fn new(id: &str, version: &str) -> Self {
        Self {
            manifest: ActionPrimitiveManifest {
                id: id.to_string(),
                version: version.to_string(),
                kind: ActionKind::Action,
                inputs: vec![ActionInputSpec {
                    name: "event".to_string(),
                    value_type: ActionValueType::Event,
                    required: true,
                    cardinality: ActionCardinality::Single,
                }],
                outputs: vec![ActionOutputSpec {
                    name: "outcome".to_string(),
                    value_type: ActionValueType::Event,
                }],
                parameters: vec![ActionParameterSpec {
                    name: "tag".to_string(),
                    value_type: crate::action::ParameterType::String,
                    default: Some(ActionParameterValue::String("ok".to_string())),
                    required: false,
                    bounds: None,
                }],
                effects: ActionEffects {
                    writes: Vec::new(),
                    intents: Vec::new(),
                },
                execution: ActionExecutionSpec {
                    deterministic: true,
                    retryable: false,
                },
                state: ActionStateSpec { allowed: false },
                side_effects: true,
            },
        }
    }
}

impl ActionPrimitive for TestAction {
    fn manifest(&self) -> &ActionPrimitiveManifest {
        &self.manifest
    }

    fn execute(
        &self,
        _inputs: &HashMap<String, crate::action::ActionValue>,
        _parameters: &HashMap<String, crate::action::ParameterValue>,
    ) -> HashMap<String, crate::action::ActionValue> {
        HashMap::from([(
            "outcome".to_string(),
            crate::action::ActionValue::Event(crate::action::ActionOutcome::Completed),
        )])
    }
}

/// X.10: Compute primitives must not declare Series parameter types.
#[test]
fn series_parameter_type_rejected() {
    let manifest = ComputePrimitiveManifest {
        id: "test_series_param".to_string(),
        version: "0.1.0".to_string(),
        kind: common::PrimitiveKind::Compute,
        inputs: vec![InputSpec {
            name: "x".to_string(),
            value_type: common::ValueType::Number,
            required: true,
            cardinality: Cardinality::Single,
        }],
        outputs: vec![OutputSpec {
            name: "result".to_string(),
            value_type: common::ValueType::Number,
        }],
        parameters: vec![ParameterSpec {
            name: "series_param".to_string(),
            value_type: common::ValueType::Series, // X.10: unsupported
            default: None,
            required: true,
            bounds: None,
        }],
        execution: ExecutionSpec {
            deterministic: true,
            cadence: Cadence::Continuous,
            may_error: false,
        },
        errors: ErrorSpec {
            allowed: false,
            types: vec![],
            deterministic: true,
        },
        state: StateSpec {
            allowed: false,
            resettable: false,
            description: None,
        },
        side_effects: false,
    };

    let mut catalog = CorePrimitiveCatalog::new();
    let result = catalog.register_compute(manifest);

    assert!(matches!(
        result,
        Err(ValidationError::UnsupportedParameterType {
            primitive,
            version,
            parameter,
            got
        }) if primitive == "test_series_param"
            && version == "0.1.0"
            && parameter == "series_param"
            && got == common::ValueType::Series
    ));
}

/// REG-SYNC-1: registry and catalog key sets must be identical per primitive kind.
#[test]
fn registry_catalog_key_parity() {
    let (registries, catalog) = build_core().expect("core registries/catalog should build");

    assert_eq!(
        registries.sources.keys(),
        catalog.keys_for_kind(PrimitiveKind::Source),
        "source registry/catalog keys differ"
    );
    assert_eq!(
        registries.computes.keys(),
        catalog.keys_for_kind(PrimitiveKind::Compute),
        "compute registry/catalog keys differ"
    );
    assert_eq!(
        registries.triggers.keys(),
        catalog.keys_for_kind(PrimitiveKind::Trigger),
        "trigger registry/catalog keys differ"
    );
    assert_eq!(
        registries.actions.keys(),
        catalog.keys_for_kind(PrimitiveKind::Action),
        "action registry/catalog keys differ"
    );
}

#[test]
fn catalog_builder_admits_external_implementations_by_kind() {
    let mut builder = CatalogBuilder::new();
    builder.add_source(Box::new(TestSource::new("test_source", "0.1.0", 1.0)));
    builder.add_compute(Box::new(TestCompute::new("test_compute", "0.1.0")));
    builder.add_trigger(Box::new(TestTrigger::new("test_trigger", "0.1.0")));
    builder.add_action(Box::new(TestAction::new("test_action", "0.1.0")));

    let (registries, catalog) = builder
        .build()
        .expect("external implementations should register");

    assert!(registries.sources.get("test_source").is_some());
    assert!(registries.computes.get("test_compute").is_some());
    assert!(registries.triggers.get("test_trigger").is_some());
    assert!(registries.actions.get("test_action").is_some());
    assert!(catalog
        .get("test_source", &Version::from("0.1.0"))
        .is_some());
    assert!(catalog
        .get("test_compute", &Version::from("0.1.0"))
        .is_some());
    assert!(catalog
        .get("test_trigger", &Version::from("0.1.0"))
        .is_some());
    assert!(catalog
        .get("test_action", &Version::from("0.1.0"))
        .is_some());
}

#[test]
fn catalog_builder_rejects_invalid_manifest_via_existing_validation() {
    let mut builder = CatalogBuilder::new();
    builder.add_source(Box::new(TestSource::new("BadId", "0.1.0", 1.0)));

    let result = builder.build();

    assert!(matches!(
        result,
        Err(CoreRegistrationError::Source(SourceValidationError::InvalidId { id }))
            if id == "BadId"
    ));
}

#[test]
fn catalog_builder_rejects_duplicate_external_id_even_with_new_version() {
    let mut builder = CatalogBuilder::new();
    builder.add_source(Box::new(TestSource::new("dup_source", "0.1.0", 1.0)));
    builder.add_source(Box::new(TestSource::new("dup_source", "0.2.0", 2.0)));

    let result = builder.build();

    assert!(matches!(
        result,
        Err(CoreRegistrationError::Source(SourceValidationError::DuplicateId(id)))
            if id == "dup_source"
    ));
}

#[test]
fn catalog_builder_rejects_duplicate_core_id_even_with_new_version() {
    let mut builder = CatalogBuilder::new();
    builder.add_source(Box::new(TestSource::new("number_source", "9.9.9", 9.0)));

    let result = builder.build();

    assert!(matches!(
        result,
        Err(CoreRegistrationError::Source(SourceValidationError::DuplicateId(id)))
            if id == "number_source"
    ));
}
