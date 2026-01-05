use std::collections::HashMap;

use crate::action::{
    implementations::{ack_action_manifest, annotate_action_manifest},
    AckAction, ActionRegistry, ActionValidationError, ActionValueType, AnnotateAction,
};
use crate::cluster::{
    Cardinality, InputMetadata, OutputMetadata, ParameterMetadata, ParameterType, ParameterValue,
    PrimitiveCatalog, PrimitiveKind, PrimitiveMetadata, ValueType, Version,
};
use crate::common;
use crate::common::ValidationError;
use crate::compute::implementations::{
    add::add_manifest, and::and_manifest, const_bool::const_bool_manifest,
    const_number::const_number_manifest, divide::divide_manifest, eq::eq_manifest, gt::gt_manifest,
    lt::lt_manifest, multiply::multiply_manifest, negate::negate_manifest, neq::neq_manifest,
    not::not_manifest, or::or_manifest, select::select_manifest, subtract::subtract_manifest, Add,
    And, ConstBool, ConstNumber, Divide, Eq, Gt, Lt, Multiply, Negate, Neq, Not, Or, Select,
    Subtract,
};
use crate::compute::{ComputePrimitiveManifest, PrimitiveRegistry as ComputeRegistry};
use crate::source::{
    implementations::{boolean_source_manifest, number_source_manifest},
    BooleanSource, NumberSource, SourceRegistry, SourceValidationError,
};
use crate::trigger::{
    implementations::emit_if_true::emit_if_true_manifest, EmitIfTrue, TriggerRegistry,
    TriggerValidationError, TriggerValueType,
};

#[derive(Debug)]
pub enum CoreRegistrationError {
    Source(SourceValidationError),
    Compute(ValidationError),
    Trigger(TriggerValidationError),
    Action(ActionValidationError),
}

pub struct CoreRegistries {
    pub sources: SourceRegistry,
    pub computes: ComputeRegistry,
    pub triggers: TriggerRegistry,
    pub actions: ActionRegistry,
}

impl CoreRegistries {
    pub fn new(
        sources: SourceRegistry,
        computes: ComputeRegistry,
        triggers: TriggerRegistry,
        actions: ActionRegistry,
    ) -> Self {
        Self {
            sources,
            computes,
            triggers,
            actions,
        }
    }
}

pub fn core_registries() -> Result<CoreRegistries, CoreRegistrationError> {
    let mut sources = SourceRegistry::new();
    sources
        .register(Box::new(NumberSource::new()))
        .map_err(CoreRegistrationError::Source)?;
    sources
        .register(Box::new(BooleanSource::new()))
        .map_err(CoreRegistrationError::Source)?;

    let mut computes = ComputeRegistry::new();
    computes
        .register(Box::new(ConstNumber::new()))
        .map_err(CoreRegistrationError::Compute)?;
    computes
        .register(Box::new(ConstBool::new()))
        .map_err(CoreRegistrationError::Compute)?;
    computes
        .register(Box::new(Add::new()))
        .map_err(CoreRegistrationError::Compute)?;
    computes
        .register(Box::new(Subtract::new()))
        .map_err(CoreRegistrationError::Compute)?;
    computes
        .register(Box::new(Multiply::new()))
        .map_err(CoreRegistrationError::Compute)?;
    computes
        .register(Box::new(Divide::new()))
        .map_err(CoreRegistrationError::Compute)?;
    computes
        .register(Box::new(Negate::new()))
        .map_err(CoreRegistrationError::Compute)?;
    computes
        .register(Box::new(Gt::new()))
        .map_err(CoreRegistrationError::Compute)?;
    computes
        .register(Box::new(Lt::new()))
        .map_err(CoreRegistrationError::Compute)?;
    computes
        .register(Box::new(Eq::new()))
        .map_err(CoreRegistrationError::Compute)?;
    computes
        .register(Box::new(Neq::new()))
        .map_err(CoreRegistrationError::Compute)?;
    computes
        .register(Box::new(And::new()))
        .map_err(CoreRegistrationError::Compute)?;
    computes
        .register(Box::new(Or::new()))
        .map_err(CoreRegistrationError::Compute)?;
    computes
        .register(Box::new(Not::new()))
        .map_err(CoreRegistrationError::Compute)?;
    computes
        .register(Box::new(Select::new()))
        .map_err(CoreRegistrationError::Compute)?;

    let mut triggers = TriggerRegistry::new();
    triggers
        .register(Box::new(EmitIfTrue::new()))
        .map_err(CoreRegistrationError::Trigger)?;

    let mut actions = ActionRegistry::new();
    actions
        .register(Box::new(AckAction::new()))
        .map_err(CoreRegistrationError::Action)?;
    actions
        .register(Box::new(AnnotateAction::new()))
        .map_err(CoreRegistrationError::Action)?;

    Ok(CoreRegistries::new(sources, computes, triggers, actions))
}

pub struct CorePrimitiveCatalog {
    metadata: HashMap<(String, Version), PrimitiveMetadata>,
}

impl CorePrimitiveCatalog {
    pub fn new() -> Self {
        Self {
            metadata: HashMap::new(),
        }
    }

    pub fn register_compute(
        &mut self,
        manifest: ComputePrimitiveManifest,
    ) -> Result<(), ValidationError> {
        let inputs = manifest
            .inputs
            .into_iter()
            .map(|i| InputMetadata {
                name: i.name,
                value_type: map_common_value_type(i.value_type),
                required: i.required,
            })
            .collect();

        let outputs = manifest
            .outputs
            .into_iter()
            .map(|o| {
                (
                    o.name,
                    OutputMetadata {
                        value_type: map_common_value_type(o.value_type),
                        cardinality: Cardinality::Single,
                    },
                )
            })
            .collect();

        // A.1: Extract parameter specs with defaults
        // X.10: Reject Series parameter types at registration
        let mut parameters = Vec::new();
        for p in manifest.parameters {
            let param_value_type = p.value_type.clone();
            let ty = map_compute_param_type(p.value_type).ok_or_else(|| {
                ValidationError::UnsupportedParameterType {
                    primitive: manifest.id.clone(),
                    version: manifest.version.clone(),
                    parameter: p.name.clone(),
                    got: param_value_type, // common::ValueType, not cluster::ValueType
                }
            })?;
            let required = p.default.is_none();
            parameters.push(ParameterMetadata {
                name: p.name,
                ty,
                default: p.default.map(map_compute_param_value),
                required,
            });
        }

        self.metadata.insert(
            (manifest.id.clone(), manifest.version.clone()),
            PrimitiveMetadata {
                kind: PrimitiveKind::Compute,
                inputs,
                outputs,
                parameters,
            },
        );
        Ok(())
    }

    pub fn register_trigger(&mut self, manifest: crate::trigger::TriggerPrimitiveManifest) {
        let inputs = manifest
            .inputs
            .into_iter()
            .map(|i| InputMetadata {
                name: i.name,
                value_type: map_trigger_value_type(i.value_type),
                required: i.required,
            })
            .collect();

        let outputs = manifest
            .outputs
            .into_iter()
            .map(|o| {
                (
                    o.name,
                    OutputMetadata {
                        value_type: map_trigger_value_type(o.value_type),
                        cardinality: Cardinality::Single,
                    },
                )
            })
            .collect();

        // A.1: Extract parameter specs with defaults
        let parameters = manifest
            .parameters
            .into_iter()
            .map(|p| {
                let required = p.default.is_none();
                ParameterMetadata {
                    name: p.name,
                    ty: map_trigger_param_type(p.value_type),
                    default: p.default.map(map_trigger_param_value),
                    required,
                }
            })
            .collect();

        self.metadata.insert(
            (manifest.id.clone(), manifest.version.clone()),
            PrimitiveMetadata {
                kind: PrimitiveKind::Trigger,
                inputs,
                outputs,
                parameters,
            },
        );
    }

    pub fn register_source(&mut self, manifest: crate::source::SourcePrimitiveManifest) {
        let inputs = vec![];
        let outputs = manifest
            .outputs
            .into_iter()
            .map(|o| {
                (
                    o.name,
                    OutputMetadata {
                        value_type: map_common_value_type(o.value_type),
                        cardinality: Cardinality::Single,
                    },
                )
            })
            .collect();

        // A.1: Extract parameter specs with defaults
        let parameters = manifest
            .parameters
            .into_iter()
            .map(|p| {
                let required = p.default.is_none();
                ParameterMetadata {
                    name: p.name,
                    ty: map_source_param_type(p.value_type),
                    default: p.default.map(map_source_param_value),
                    required,
                }
            })
            .collect();

        self.metadata.insert(
            (manifest.id.clone(), manifest.version.clone()),
            PrimitiveMetadata {
                kind: PrimitiveKind::Source,
                inputs,
                outputs,
                parameters,
            },
        );
    }

    pub fn register_action(&mut self, manifest: crate::action::ActionPrimitiveManifest) {
        let inputs = manifest
            .inputs
            .into_iter()
            .map(|i| InputMetadata {
                name: i.name,
                value_type: map_action_value_type(i.value_type),
                required: i.required,
            })
            .collect();

        let outputs = manifest
            .outputs
            .into_iter()
            .map(|o| {
                (
                    o.name,
                    OutputMetadata {
                        value_type: map_action_value_type(o.value_type),
                        cardinality: Cardinality::Single,
                    },
                )
            })
            .collect();

        // A.1: Extract parameter specs with defaults
        let parameters = manifest
            .parameters
            .into_iter()
            .map(|p| {
                let required = p.default.is_none();
                ParameterMetadata {
                    name: p.name,
                    ty: map_action_param_type(p.value_type),
                    default: p.default.map(map_action_param_value),
                    required,
                }
            })
            .collect();

        self.metadata.insert(
            (manifest.id.clone(), manifest.version.clone()),
            PrimitiveMetadata {
                kind: PrimitiveKind::Action,
                inputs,
                outputs,
                parameters,
            },
        );
    }
}

impl PrimitiveCatalog for CorePrimitiveCatalog {
    fn get(&self, id: &str, version: &Version) -> Option<PrimitiveMetadata> {
        self.metadata
            .get(&(id.to_string(), version.clone()))
            .cloned()
    }
}

pub fn build_core_catalog() -> CorePrimitiveCatalog {
    let mut catalog = CorePrimitiveCatalog::new();

    // Sources
    catalog.register_source(number_source_manifest());
    catalog.register_source(boolean_source_manifest());

    // Computes (X.10: all core compute manifests use supported parameter types)
    catalog
        .register_compute(const_number_manifest())
        .expect("const_number manifest is valid");
    catalog
        .register_compute(const_bool_manifest())
        .expect("const_bool manifest is valid");
    catalog
        .register_compute(add_manifest())
        .expect("add manifest is valid");
    catalog
        .register_compute(subtract_manifest())
        .expect("subtract manifest is valid");
    catalog
        .register_compute(multiply_manifest())
        .expect("multiply manifest is valid");
    catalog
        .register_compute(divide_manifest())
        .expect("divide manifest is valid");
    catalog
        .register_compute(negate_manifest())
        .expect("negate manifest is valid");
    catalog
        .register_compute(gt_manifest())
        .expect("gt manifest is valid");
    catalog
        .register_compute(lt_manifest())
        .expect("lt manifest is valid");
    catalog
        .register_compute(eq_manifest())
        .expect("eq manifest is valid");
    catalog
        .register_compute(neq_manifest())
        .expect("neq manifest is valid");
    catalog
        .register_compute(and_manifest())
        .expect("and manifest is valid");
    catalog
        .register_compute(or_manifest())
        .expect("or manifest is valid");
    catalog
        .register_compute(not_manifest())
        .expect("not manifest is valid");
    catalog
        .register_compute(select_manifest())
        .expect("select manifest is valid");

    // Triggers
    catalog.register_trigger(emit_if_true_manifest());

    // Actions
    catalog.register_action(ack_action_manifest());
    catalog.register_action(annotate_action_manifest());

    catalog
}

fn map_common_value_type(value_type: common::ValueType) -> ValueType {
    match value_type {
        common::ValueType::Number => ValueType::Number,
        common::ValueType::Series => ValueType::Series,
        common::ValueType::Bool => ValueType::Bool,
    }
}

fn map_trigger_value_type(value_type: TriggerValueType) -> ValueType {
    match value_type {
        TriggerValueType::Number => ValueType::Number,
        TriggerValueType::Series => ValueType::Series,
        TriggerValueType::Bool => ValueType::Bool,
        TriggerValueType::Event => ValueType::Event,
    }
}

fn map_action_value_type(value_type: ActionValueType) -> ValueType {
    match value_type {
        ActionValueType::Event => ValueType::Event,
        ActionValueType::Number => ValueType::Number,
        ActionValueType::Bool => ValueType::Bool,
        ActionValueType::String => ValueType::String,
    }
}

// A.1: Parameter type/value mapping functions for expansion-time default resolution

fn map_source_param_type(ty: crate::source::ParameterType) -> ParameterType {
    match ty {
        crate::source::ParameterType::Int => ParameterType::Int,
        crate::source::ParameterType::Number => ParameterType::Number,
        crate::source::ParameterType::Bool => ParameterType::Bool,
        crate::source::ParameterType::String => ParameterType::String,
        crate::source::ParameterType::Enum => ParameterType::Enum,
    }
}

fn map_source_param_value(val: crate::source::ParameterValue) -> ParameterValue {
    match val {
        crate::source::ParameterValue::Int(i) => ParameterValue::Int(i),
        crate::source::ParameterValue::Number(n) => ParameterValue::Number(n),
        crate::source::ParameterValue::Bool(b) => ParameterValue::Bool(b),
        crate::source::ParameterValue::String(s) => ParameterValue::String(s),
        crate::source::ParameterValue::Enum(e) => ParameterValue::Enum(e),
    }
}

/// X.10: Returns None for Series (unsupported parameter type for compute primitives).
fn map_compute_param_type(ty: common::ValueType) -> Option<ParameterType> {
    match ty {
        common::ValueType::Number => Some(ParameterType::Number),
        common::ValueType::Series => None, // X.10: Series params not supported
        common::ValueType::Bool => Some(ParameterType::Bool),
    }
}

fn map_compute_param_value(val: common::Value) -> ParameterValue {
    match val {
        common::Value::Number(n) => ParameterValue::Number(n),
        // X.10: Series is rejected at type check; this arm is unreachable for valid registrations.
        common::Value::Series(_) => {
            unreachable!("X.10: Series parameter type should be rejected at registration")
        }
        common::Value::Bool(b) => ParameterValue::Bool(b),
    }
}

fn map_trigger_param_type(ty: crate::trigger::ParameterType) -> ParameterType {
    match ty {
        crate::trigger::ParameterType::Int => ParameterType::Int,
        crate::trigger::ParameterType::Number => ParameterType::Number,
        crate::trigger::ParameterType::Bool => ParameterType::Bool,
        crate::trigger::ParameterType::String => ParameterType::String,
        crate::trigger::ParameterType::Enum => ParameterType::Enum,
    }
}

fn map_trigger_param_value(val: crate::trigger::ParameterValue) -> ParameterValue {
    match val {
        crate::trigger::ParameterValue::Int(i) => ParameterValue::Int(i),
        crate::trigger::ParameterValue::Number(n) => ParameterValue::Number(n),
        crate::trigger::ParameterValue::Bool(b) => ParameterValue::Bool(b),
        crate::trigger::ParameterValue::String(s) => ParameterValue::String(s),
        crate::trigger::ParameterValue::Enum(e) => ParameterValue::Enum(e),
    }
}

fn map_action_param_type(ty: crate::action::ParameterType) -> ParameterType {
    match ty {
        crate::action::ParameterType::Int => ParameterType::Int,
        crate::action::ParameterType::Number => ParameterType::Number,
        crate::action::ParameterType::Bool => ParameterType::Bool,
        crate::action::ParameterType::String => ParameterType::String,
        crate::action::ParameterType::Enum => ParameterType::Enum,
    }
}

fn map_action_param_value(val: crate::action::ParameterValue) -> ParameterValue {
    match val {
        crate::action::ParameterValue::Int(i) => ParameterValue::Int(i),
        crate::action::ParameterValue::Number(n) => ParameterValue::Number(n),
        crate::action::ParameterValue::Bool(b) => ParameterValue::Bool(b),
        crate::action::ParameterValue::String(s) => ParameterValue::String(s),
        crate::action::ParameterValue::Enum(e) => ParameterValue::Enum(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compute::{Cadence, ExecutionSpec, InputSpec, OutputSpec, ParameterSpec, StateSpec};

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
            }],
            outputs: vec![OutputSpec {
                name: "result".to_string(),
                value_type: common::ValueType::Number,
            }],
            parameters: vec![ParameterSpec {
                name: "series_param".to_string(),
                value_type: common::ValueType::Series, // X.10: unsupported
                default: None,
            }],
            execution: ExecutionSpec {
                deterministic: true,
                cadence: Cadence::Continuous,
            },
            state: StateSpec {
                stateful: false,
                rolling_window: None,
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
}
