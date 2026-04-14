//! catalog
//!
//! Purpose:
//! - Assemble the kernel core primitive catalog and registries, and expose the
//!   typed registration boundary for core primitive construction.
//!
//! Owns:
//! - `CoreRegistrationError` as the wrapper over source/compute/trigger/action
//!   registration failures.
//! - The core primitive registry/catalog assembly functions used by higher layers.
//!
//! Does not own:
//! - Product-facing rendering of registration errors.
//! - Primitive-specific validation semantics already owned by the wrapped kernel
//!   validation enums.
//!
//! Connects to:
//! - `ergo_host` and SDK setup paths that materialize runtime surfaces.
//! - Source/compute/trigger/action registration modules whose errors chain through
//!   this file.
//!
//! Safety notes:
//! - `CoreRegistrationError` must preserve source chaining so higher layers can
//!   carry typed kernel registration failures instead of formatting them with
//!   `Debug`.

use std::collections::HashMap;
use std::fmt;

use crate::action::{
    AckAction, ActionPrimitive, ActionRegistry, ActionValidationError, ActionValueType,
    AnnotateAction, ContextSetBoolAction, ContextSetNumberAction, ContextSetSeriesAction,
    ContextSetStringAction,
};
use crate::cluster::{
    Cardinality, InputMetadata, OutputMetadata, ParameterMetadata, ParameterType, ParameterValue,
    PrimitiveCatalog, PrimitiveKind, PrimitiveMetadata, PrimitiveVersionIndex, ValueType, Version,
};
use crate::common;
use crate::common::ValidationError;
use crate::compute::implementations::{
    Abs, Add, And, Append, ConstBool, ConstNumber, Divide, Eq, Gt, Gte, Len, Lt, Lte, Max, Mean,
    Min, Multiply, Negate, Neq, Not, Or, SafeDivide, Select, SelectBool, Subtract, Sum, Window,
};
use crate::compute::{
    ComputePrimitive, ComputePrimitiveManifest, PrimitiveRegistry as ComputeRegistry,
};
use crate::source::{
    BooleanSource, ContextBoolSource, ContextNumberSource, ContextSeriesSource,
    ContextStringSource, NumberSource, SourcePrimitive, SourceRegistry, SourceValidationError,
    StringSource,
};
use crate::trigger::{
    EmitIfEventAndTrue, EmitIfTrue, TriggerPrimitive, TriggerRegistry, TriggerValidationError,
    TriggerValueType,
};

#[derive(Debug)]
pub enum CoreRegistrationError {
    Source(SourceValidationError),
    Compute(ValidationError),
    Trigger(TriggerValidationError),
    Action(ActionValidationError),
}

impl fmt::Display for CoreRegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(err) => write!(f, "source registration failed: {err}"),
            Self::Compute(err) => write!(f, "compute registration failed: {err}"),
            Self::Trigger(err) => write!(f, "trigger registration failed: {err}"),
            Self::Action(err) => write!(f, "action registration failed: {err}"),
        }
    }
}

impl std::error::Error for CoreRegistrationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(err) => Some(err),
            Self::Compute(err) => Some(err),
            Self::Trigger(err) => Some(err),
            Self::Action(err) => Some(err),
        }
    }
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

fn core_source_primitives() -> Vec<Box<dyn SourcePrimitive>> {
    vec![
        Box::new(NumberSource::new()),
        Box::new(ContextNumberSource::new()),
        Box::new(ContextSeriesSource::new()),
        Box::new(ContextBoolSource::new()),
        Box::new(ContextStringSource::new()),
        Box::new(BooleanSource::new()),
        Box::new(StringSource::new()),
    ]
}

fn core_compute_primitives() -> Vec<Box<dyn ComputePrimitive>> {
    vec![
        Box::new(ConstNumber::new()),
        Box::new(ConstBool::new()),
        Box::new(Abs::new()),
        Box::new(Add::new()),
        Box::new(Subtract::new()),
        Box::new(Multiply::new()),
        Box::new(Divide::new()),
        Box::new(SafeDivide::new()),
        Box::new(Negate::new()),
        Box::new(Gt::new()),
        Box::new(Gte::new()),
        Box::new(Lt::new()),
        Box::new(Lte::new()),
        Box::new(Min::new()),
        Box::new(Max::new()),
        Box::new(Eq::new()),
        Box::new(Neq::new()),
        Box::new(And::new()),
        Box::new(Or::new()),
        Box::new(Not::new()),
        Box::new(Select::new()),
        Box::new(SelectBool::new()),
        Box::new(Append::new()),
        Box::new(Window::new()),
        Box::new(Mean::new()),
        Box::new(Sum::new()),
        Box::new(Len::new()),
    ]
}

fn core_trigger_primitives() -> Vec<Box<dyn TriggerPrimitive>> {
    vec![
        Box::new(EmitIfTrue::new()),
        Box::new(EmitIfEventAndTrue::new()),
    ]
}

fn core_action_primitives() -> Vec<Box<dyn ActionPrimitive>> {
    vec![
        Box::new(AckAction::new()),
        Box::new(AnnotateAction::new()),
        Box::new(ContextSetNumberAction::new()),
        Box::new(ContextSetSeriesAction::new()),
        Box::new(ContextSetBoolAction::new()),
        Box::new(ContextSetStringAction::new()),
    ]
}

struct PrimitiveInventory {
    sources: Vec<Box<dyn SourcePrimitive>>,
    computes: Vec<Box<dyn ComputePrimitive>>,
    triggers: Vec<Box<dyn TriggerPrimitive>>,
    actions: Vec<Box<dyn ActionPrimitive>>,
}

impl PrimitiveInventory {
    fn with_core() -> Self {
        Self {
            sources: core_source_primitives(),
            computes: core_compute_primitives(),
            triggers: core_trigger_primitives(),
            actions: core_action_primitives(),
        }
    }
}

pub struct CatalogBuilder {
    inventory: PrimitiveInventory,
}

impl CatalogBuilder {
    pub fn new() -> Self {
        Self {
            inventory: PrimitiveInventory::with_core(),
        }
    }

    pub fn add_source(&mut self, primitive: Box<dyn SourcePrimitive>) -> &mut Self {
        self.inventory.sources.push(primitive);
        self
    }

    pub fn add_compute(&mut self, primitive: Box<dyn ComputePrimitive>) -> &mut Self {
        self.inventory.computes.push(primitive);
        self
    }

    pub fn add_trigger(&mut self, primitive: Box<dyn TriggerPrimitive>) -> &mut Self {
        self.inventory.triggers.push(primitive);
        self
    }

    pub fn add_action(&mut self, primitive: Box<dyn ActionPrimitive>) -> &mut Self {
        self.inventory.actions.push(primitive);
        self
    }

    pub fn build(self) -> Result<(CoreRegistries, CorePrimitiveCatalog), CoreRegistrationError> {
        build_from_inventory(self.inventory)
    }
}

impl Default for CatalogBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub fn build_core() -> Result<(CoreRegistries, CorePrimitiveCatalog), CoreRegistrationError> {
    CatalogBuilder::new().build()
}

fn build_from_inventory(
    inventory: PrimitiveInventory,
) -> Result<(CoreRegistries, CorePrimitiveCatalog), CoreRegistrationError> {
    let PrimitiveInventory {
        sources: source_primitives,
        computes: compute_primitives,
        triggers: trigger_primitives,
        actions: action_primitives,
    } = inventory;

    let mut sources = SourceRegistry::new();
    let mut computes = ComputeRegistry::new();
    let mut triggers = TriggerRegistry::new();
    let mut actions = ActionRegistry::new();
    let mut catalog = CorePrimitiveCatalog::new();

    for primitive in source_primitives {
        let manifest = primitive.manifest().clone();
        sources
            .register(primitive)
            .map_err(CoreRegistrationError::Source)?;
        catalog.register_source(manifest);
    }

    for primitive in compute_primitives {
        let manifest = primitive.manifest().clone();
        computes
            .register(primitive)
            .map_err(CoreRegistrationError::Compute)?;
        catalog
            .register_compute(manifest)
            .map_err(CoreRegistrationError::Compute)?;
    }

    for primitive in trigger_primitives {
        let manifest = primitive.manifest().clone();
        triggers
            .register(primitive)
            .map_err(CoreRegistrationError::Trigger)?;
        catalog.register_trigger(manifest);
    }

    for primitive in action_primitives {
        let manifest = primitive.manifest().clone();
        actions
            .register(primitive)
            .map_err(CoreRegistrationError::Action)?;
        catalog.register_action(manifest);
    }

    let registries = CoreRegistries::new(sources, computes, triggers, actions);
    debug_assert_registry_catalog_key_parity(&registries, &catalog);

    Ok((registries, catalog))
}

pub fn core_registries() -> Result<CoreRegistries, CoreRegistrationError> {
    let (registries, _catalog) = build_core()?;
    Ok(registries)
}

pub struct CorePrimitiveCatalog {
    metadata: HashMap<(String, Version), PrimitiveMetadata>,
}

impl CorePrimitiveCatalog {
    pub(crate) fn new() -> Self {
        Self {
            metadata: HashMap::new(),
        }
    }

    pub(crate) fn register_compute(
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
                    got: param_value_type.clone(),
                }
            })?;
            let default = match p.default {
                Some(v) => Some(map_compute_param_value(v).map_err(|_| {
                    ValidationError::UnsupportedParameterType {
                        primitive: manifest.id.clone(),
                        version: manifest.version.clone(),
                        parameter: p.name.clone(),
                        got: param_value_type,
                    }
                })?),
                None => None,
            };
            parameters.push(ParameterMetadata {
                name: p.name,
                ty,
                default,
                required: p.required,
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

    pub(crate) fn register_trigger(&mut self, manifest: crate::trigger::TriggerPrimitiveManifest) {
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
            .map(|p| ParameterMetadata {
                name: p.name,
                ty: map_trigger_param_type(p.value_type),
                default: p.default.map(map_trigger_param_value),
                required: p.required,
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

    pub(crate) fn register_source(&mut self, manifest: crate::source::SourcePrimitiveManifest) {
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

    pub(crate) fn register_action(&mut self, manifest: crate::action::ActionPrimitiveManifest) {
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
            .map(|p| ParameterMetadata {
                name: p.name,
                ty: map_action_param_type(p.value_type),
                default: p.default.map(map_action_param_value),
                required: p.required,
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

    pub(crate) fn keys_for_kind(&self, kind: PrimitiveKind) -> Vec<(String, String)> {
        let mut keys: Vec<(String, String)> = self
            .metadata
            .iter()
            .filter_map(|((id, version), meta)| {
                if meta.kind == kind {
                    Some((id.clone(), version.to_string()))
                } else {
                    None
                }
            })
            .collect();
        keys.sort();
        keys
    }
}

impl PrimitiveCatalog for CorePrimitiveCatalog {
    fn get(&self, id: &str, version: &Version) -> Option<PrimitiveMetadata> {
        self.metadata
            .get(&(id.to_string(), version.clone()))
            .cloned()
    }
}

impl PrimitiveVersionIndex for CorePrimitiveCatalog {
    fn available_versions(&self, id: &str) -> Vec<Version> {
        let mut versions = self
            .metadata
            .keys()
            .filter_map(|(candidate_id, version)| {
                if candidate_id == id {
                    Some(version.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        versions.sort();
        versions
    }
}

fn debug_assert_registry_catalog_key_parity(
    registries: &CoreRegistries,
    catalog: &CorePrimitiveCatalog,
) {
    debug_assert_eq!(
        registries.sources.keys(),
        catalog.keys_for_kind(PrimitiveKind::Source),
        "registry/catalog key drift for sources"
    );
    debug_assert_eq!(
        registries.computes.keys(),
        catalog.keys_for_kind(PrimitiveKind::Compute),
        "registry/catalog key drift for computes"
    );
    debug_assert_eq!(
        registries.triggers.keys(),
        catalog.keys_for_kind(PrimitiveKind::Trigger),
        "registry/catalog key drift for triggers"
    );
    debug_assert_eq!(
        registries.actions.keys(),
        catalog.keys_for_kind(PrimitiveKind::Action),
        "registry/catalog key drift for actions"
    );
}

pub fn build_core_catalog() -> CorePrimitiveCatalog {
    let (_registries, catalog) = build_core().expect("core registries/catalog should build");
    catalog
}

fn map_common_value_type(value_type: common::ValueType) -> ValueType {
    match value_type {
        common::ValueType::Number => ValueType::Number,
        common::ValueType::Series => ValueType::Series,
        common::ValueType::Bool => ValueType::Bool,
        common::ValueType::String => ValueType::String,
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
        ActionValueType::Series => ValueType::Series,
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

/// X.10: Returns None for Series/String (unsupported parameter types for compute primitives).
fn map_compute_param_type(ty: common::ValueType) -> Option<ParameterType> {
    match ty {
        common::ValueType::Number => Some(ParameterType::Number),
        common::ValueType::Series => None, // X.10: Series params not supported
        common::ValueType::Bool => Some(ParameterType::Bool),
        common::ValueType::String => None, // X.10: String params not supported
    }
}

fn map_compute_param_value(val: common::Value) -> Result<ParameterValue, &'static str> {
    match val {
        common::Value::Number(n) => Ok(ParameterValue::Number(n)),
        // X.10: Series is rejected at type check; if this arm is reached,
        // it indicates the type gate was bypassed.
        common::Value::Series(_) => {
            Err("X.10: Series parameter type should be rejected at registration")
        }
        common::Value::Bool(b) => Ok(ParameterValue::Bool(b)),
        // X.10: String default values are valid — they pass through to
        // the compute parameter as-is.
        common::Value::String(s) => Ok(ParameterValue::String(s)),
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
mod tests;
