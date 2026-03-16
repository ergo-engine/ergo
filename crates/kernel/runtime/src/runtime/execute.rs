use std::collections::HashMap;

use crate::action::{ActionOutcome, ActionValue};
use crate::cluster::{PrimitiveKind, ValueType};
use crate::common::{ActionEffect, EffectWrite};
use crate::trigger::{TriggerEvent, TriggerValue};

use super::types::{
    Endpoint, ExecError, ExecutionContext, ExecutionReport, Registries, RuntimeEvent, RuntimeValue,
    ValidatedEdge, ValidatedGraph, ValidatedNode,
};

pub fn execute(
    graph: &ValidatedGraph,
    registries: &Registries,
    ctx: &ExecutionContext,
) -> Result<ExecutionReport, ExecError> {
    let mut node_outputs: HashMap<String, HashMap<String, RuntimeValue>> = HashMap::new();
    let mut effects: Vec<ActionEffect> = Vec::new();

    for node_id in &graph.topo_order {
        let node = graph
            .nodes
            .get(node_id)
            .ok_or_else(|| ExecError::MissingNode {
                node: node_id.clone(),
            })?;

        let inputs = collect_inputs(node_id, &node.inputs, &graph.edges, &node_outputs)?;

        let outputs = match node.kind {
            PrimitiveKind::Source => execute_source(node, inputs, registries, ctx)?,
            PrimitiveKind::Compute => execute_compute(node, inputs, registries)?,
            PrimitiveKind::Trigger => execute_trigger(node, inputs, registries)?,
            PrimitiveKind::Action => {
                // R.7: Actions execute only when all trigger event inputs are Emitted.
                // If any Event input is TriggerEvent::NotEmitted, skip execution.
                if should_skip_action(&inputs) {
                    produce_skipped_outputs(node)
                } else {
                    let (action_outputs, action_effects) =
                        execute_action(node, inputs, registries)?;
                    effects.extend(action_effects);
                    action_outputs
                }
            }
        };

        node_outputs.insert(node_id.clone(), outputs);
    }

    let mut outputs: HashMap<String, RuntimeValue> = HashMap::new();
    for out in &graph.boundary_outputs {
        if let Some(node_outs) = node_outputs.get(&out.maps_to.node_id) {
            if let Some(val) = node_outs.get(&out.maps_to.port_name) {
                outputs.insert(out.name.clone(), val.clone());
            } else {
                return Err(ExecError::MissingOutput {
                    node: out.maps_to.node_id.clone(),
                    output: out.maps_to.port_name.clone(),
                });
            }
        } else {
            return Err(ExecError::MissingOutput {
                node: out.maps_to.node_id.clone(),
                output: out.maps_to.port_name.clone(),
            });
        }
    }

    Ok(ExecutionReport { outputs, effects })
}

fn collect_inputs(
    target: &str,
    input_specs: &[crate::cluster::InputMetadata],
    edges: &[ValidatedEdge],
    node_outputs: &HashMap<String, HashMap<String, RuntimeValue>>,
) -> Result<HashMap<String, RuntimeValue>, ExecError> {
    let mut inputs: HashMap<String, RuntimeValue> = HashMap::new();

    for edge in edges {
        let Endpoint::NodePort {
            node_id: to_node,
            port_name: to_port,
        } = &edge.to;
        if to_node == target {
            let Endpoint::NodePort {
                node_id: from,
                port_name: from_port,
            } = &edge.from;
            let outs = node_outputs
                .get(from)
                .ok_or_else(|| ExecError::MissingOutput {
                    node: from.clone(),
                    output: from_port.clone(),
                })?;
            let val = outs
                .get(from_port)
                .ok_or_else(|| ExecError::MissingOutput {
                    node: from.clone(),
                    output: from_port.clone(),
                })?;
            inputs.insert(to_port.clone(), val.clone());
        }
    }

    // fill missing required
    for spec in input_specs {
        if spec.required && !inputs.contains_key(&spec.name) {
            return Err(ExecError::MissingOutput {
                node: target.to_string(),
                output: spec.name.clone(),
            });
        }
    }

    Ok(inputs)
}

fn execute_source(
    node: &ValidatedNode,
    _inputs: HashMap<String, RuntimeValue>,
    registries: &Registries,
    ctx: &ExecutionContext,
) -> Result<HashMap<String, RuntimeValue>, ExecError> {
    let primitive =
        registries
            .sources
            .get(&node.impl_id)
            .ok_or_else(|| ExecError::UnknownPrimitive {
                id: node.impl_id.clone(),
                version: node.version.clone(),
            })?;

    let manifest = primitive.manifest();
    let manifest_name_parameters = source_manifest_name_parameters(node, manifest);
    for req in &manifest.requires.context {
        // Resolve $key bindings before the required check so optional
        // parameter-bound keys are still resolved.
        let resolved_name =
            crate::common::resolve_manifest_name(&req.name, &manifest_name_parameters).map_err(
                |_| ExecError::MissingRequiredContextKey {
                    node: node.runtime_id.clone(),
                    key: req.name.clone(),
                },
            )?;
        if !req.required {
            continue;
        }
        match ctx.value(&resolved_name) {
            None => {
                return Err(ExecError::MissingRequiredContextKey {
                    node: node.runtime_id.clone(),
                    key: resolved_name,
                });
            }
            Some(val) => {
                if val.value_type() != req.ty {
                    return Err(ExecError::ContextKeyTypeMismatch {
                        node: node.runtime_id.clone(),
                        key: resolved_name,
                        expected: req.ty.clone(),
                        got: val.value_type(),
                    });
                }
            }
        }
    }

    let mut mapped_parameters: HashMap<String, crate::source::ParameterValue> = HashMap::new();
    for (name, val) in &node.parameters {
        let mapped = map_to_source_parameter_value(val).ok_or_else(|| {
            ExecError::ParameterTypeConversionFailed {
                node: node.runtime_id.clone(),
                parameter: name.clone(),
            }
        })?;
        mapped_parameters.insert(name.clone(), mapped);
    }

    let outputs = primitive.produce(&mapped_parameters, ctx);
    ensure_finite(&node.runtime_id, &outputs)?;
    Ok(outputs
        .into_iter()
        .map(|(k, v)| (k, map_common_value(v)))
        .collect())
}

fn execute_compute(
    node: &ValidatedNode,
    inputs: HashMap<String, RuntimeValue>,
    registries: &Registries,
) -> Result<HashMap<String, RuntimeValue>, ExecError> {
    let primitive =
        registries
            .computes
            .get(&node.impl_id)
            .ok_or_else(|| ExecError::UnknownPrimitive {
                id: node.impl_id.clone(),
                version: node.version.clone(),
            })?;

    let mut mapped_inputs: HashMap<String, crate::common::Value> = HashMap::new();
    for (name, val) in inputs {
        let mapped = map_to_compute_value(&val).ok_or_else(|| ExecError::TypeConversionFailed {
            node: node.runtime_id.clone(),
            port: name.clone(),
        })?;
        mapped_inputs.insert(name, mapped);
    }

    let mut mapped_parameters: HashMap<String, crate::common::Value> = HashMap::new();
    for (name, val) in &node.parameters {
        let mapped = map_to_compute_parameter_value(val).ok_or_else(|| {
            // X.11: If Int conversion failed, it's out of range; otherwise type mismatch
            if let crate::cluster::ParameterValue::Int(i) = val {
                ExecError::ParameterOutOfRange {
                    node: node.runtime_id.clone(),
                    parameter: name.clone(),
                    value: *i,
                }
            } else {
                ExecError::ParameterTypeConversionFailed {
                    node: node.runtime_id.clone(),
                    parameter: name.clone(),
                }
            }
        })?;
        mapped_parameters.insert(name.clone(), mapped);
    }

    let outputs = primitive
        .compute(&mapped_inputs, &mapped_parameters, None)
        .map_err(|error| ExecError::ComputeFailed {
            node: node.runtime_id.clone(),
            id: node.impl_id.clone(),
            version: node.version.clone(),
            error,
        })?;
    for output_name in node.outputs.keys() {
        if !outputs.contains_key(output_name) {
            return Err(ExecError::MissingOutput {
                node: node.runtime_id.clone(),
                output: output_name.clone(),
            });
        }
    }
    ensure_finite(&node.runtime_id, &outputs)?;
    Ok(outputs
        .into_iter()
        .map(|(k, v)| (k, map_common_value(v)))
        .collect())
}

fn execute_trigger(
    node: &ValidatedNode,
    inputs: HashMap<String, RuntimeValue>,
    registries: &Registries,
) -> Result<HashMap<String, RuntimeValue>, ExecError> {
    let primitive =
        registries
            .triggers
            .get(&node.impl_id)
            .ok_or_else(|| ExecError::UnknownPrimitive {
                id: node.impl_id.clone(),
                version: node.version.clone(),
            })?;

    let mut mapped_inputs: HashMap<String, TriggerValue> = HashMap::new();
    for (name, val) in inputs {
        let mapped = map_to_trigger_value(&val).ok_or_else(|| ExecError::TypeConversionFailed {
            node: node.runtime_id.clone(),
            port: name.clone(),
        })?;
        mapped_inputs.insert(name, mapped);
    }

    let mut mapped_parameters: HashMap<String, crate::trigger::ParameterValue> = HashMap::new();
    for (name, val) in &node.parameters {
        let mapped = map_to_trigger_parameter_value(val).ok_or_else(|| {
            ExecError::ParameterTypeConversionFailed {
                node: node.runtime_id.clone(),
                parameter: name.clone(),
            }
        })?;
        mapped_parameters.insert(name.clone(), mapped);
    }

    let outputs = primitive.evaluate(&mapped_inputs, &mapped_parameters);
    Ok(outputs
        .into_iter()
        .map(|(k, v)| (k, map_trigger_value(v)))
        .collect())
}

fn execute_action(
    node: &ValidatedNode,
    inputs: HashMap<String, RuntimeValue>,
    registries: &Registries,
) -> Result<(HashMap<String, RuntimeValue>, Vec<ActionEffect>), ExecError> {
    let primitive =
        registries
            .actions
            .get(&node.impl_id)
            .ok_or_else(|| ExecError::UnknownPrimitive {
                id: node.impl_id.clone(),
                version: node.version.clone(),
            })?;

    let mut mapped_inputs: HashMap<String, ActionValue> = HashMap::new();
    for (name, val) in &inputs {
        let mapped = map_to_action_value(val, &node.runtime_id, name)?;
        mapped_inputs.insert(name.clone(), mapped);
    }

    let mut mapped_parameters: HashMap<String, crate::action::ParameterValue> = HashMap::new();
    for (name, val) in &node.parameters {
        let mapped = map_to_action_parameter_value(val).ok_or_else(|| {
            ExecError::ParameterTypeConversionFailed {
                node: node.runtime_id.clone(),
                parameter: name.clone(),
            }
        })?;
        mapped_parameters.insert(name.clone(), mapped);
    }

    let outputs = primitive.execute(&mapped_inputs, &mapped_parameters);

    // Build effects from manifest write declarations
    let manifest = primitive.manifest();
    let mut effects = Vec::new();
    if !manifest.effects.writes.is_empty() {
        let mut writes = Vec::new();
        for spec in &manifest.effects.writes {
            // Resolve $key via Decision 2 infrastructure
            let resolved_name = crate::common::resolve_manifest_name(&spec.name, &node.parameters)
                .map_err(|_| ExecError::ParameterTypeConversionFailed {
                    node: node.runtime_id.clone(),
                    parameter: spec.name.clone(),
                })?;

            // Read the write value from the action input snapshot
            let input_val =
                inputs
                    .get(&spec.from_input)
                    .ok_or_else(|| ExecError::MissingOutput {
                        node: node.runtime_id.clone(),
                        output: spec.from_input.clone(),
                    })?;

            let value = map_runtime_value_to_common(input_val).ok_or_else(|| {
                ExecError::TypeConversionFailed {
                    node: node.runtime_id.clone(),
                    port: spec.from_input.clone(),
                }
            })?;

            writes.push(EffectWrite {
                key: resolved_name,
                value,
            });
        }
        effects.push(ActionEffect {
            kind: "set_context".to_string(),
            writes,
            intents: vec![],
        });
    }

    let runtime_outputs = outputs
        .into_iter()
        .map(|(k, v)| (k, map_action_value(v)))
        .collect();

    Ok((runtime_outputs, effects))
}

/// NUM-FINITE-1: Reject non-finite numeric outputs before propagation.
///
/// This guard ensures that NaN, inf, and -inf cannot reach triggers or actions,
/// preventing counterintuitive behavior (e.g., NaN comparisons always return false).
///
/// Called after compute and source outputs are produced, before values enter
/// the node_outputs map.
///
/// See: NUM-FINITE-1 in PHASE_INVARIANTS.md
fn ensure_finite(
    node: &str,
    outputs: &HashMap<String, crate::common::Value>,
) -> Result<(), ExecError> {
    for (port, value) in outputs {
        match value {
            crate::common::Value::Number(n) if !n.is_finite() => {
                return Err(ExecError::NonFiniteOutput {
                    node: node.to_string(),
                    port: port.to_string(),
                });
            }
            crate::common::Value::Series(values)
                if values.iter().any(|value| !value.is_finite()) =>
            {
                return Err(ExecError::NonFiniteOutput {
                    node: node.to_string(),
                    port: port.to_string(),
                });
            }
            _ => {}
        }
    }

    Ok(())
}

fn map_common_value(v: crate::common::Value) -> RuntimeValue {
    match v {
        crate::common::Value::Number(n) => RuntimeValue::Number(n),
        crate::common::Value::Series(s) => RuntimeValue::Series(s),
        crate::common::Value::Bool(b) => RuntimeValue::Bool(b),
        crate::common::Value::String(s) => RuntimeValue::String(s),
    }
}

fn map_to_compute_value(v: &RuntimeValue) -> Option<crate::common::Value> {
    match v {
        RuntimeValue::Number(n) => Some(crate::common::Value::Number(*n)),
        RuntimeValue::Series(s) => Some(crate::common::Value::Series(s.clone())),
        RuntimeValue::Bool(b) => Some(crate::common::Value::Bool(*b)),
        _ => None,
    }
}

/// X.11: Maximum safe integer for exact f64 representation (2^53).
const MAX_SAFE_INT: i64 = 9_007_199_254_740_992;

fn map_to_compute_parameter_value(
    v: &crate::cluster::ParameterValue,
) -> Option<crate::common::Value> {
    match v {
        crate::cluster::ParameterValue::Int(i) => {
            // X.11: Guard against precision loss for |i| > 2^53
            // Note: Use explicit bounds instead of i.abs() to avoid overflow panic on i64::MIN
            if *i >= -MAX_SAFE_INT && *i <= MAX_SAFE_INT {
                Some(crate::common::Value::Number(*i as f64))
            } else {
                None // Caller will produce ParameterOutOfRange with full context
            }
        }
        crate::cluster::ParameterValue::Number(n) => Some(crate::common::Value::Number(*n)),
        crate::cluster::ParameterValue::Bool(b) => Some(crate::common::Value::Bool(*b)),
        _ => None,
    }
}

fn map_trigger_value(v: TriggerValue) -> RuntimeValue {
    match v {
        TriggerValue::Number(n) => RuntimeValue::Number(n),
        TriggerValue::Series(s) => RuntimeValue::Series(s),
        TriggerValue::Bool(b) => RuntimeValue::Bool(b),
        TriggerValue::Event(e) => RuntimeValue::Event(RuntimeEvent::Trigger(e)),
    }
}

fn map_to_trigger_value(v: &RuntimeValue) -> Option<TriggerValue> {
    match v {
        RuntimeValue::Number(n) => Some(TriggerValue::Number(*n)),
        RuntimeValue::Series(s) => Some(TriggerValue::Series(s.clone())),
        RuntimeValue::Bool(b) => Some(TriggerValue::Bool(*b)),
        RuntimeValue::Event(RuntimeEvent::Trigger(e)) => Some(TriggerValue::Event(e.clone())),
        _ => None,
    }
}

fn map_to_trigger_parameter_value(
    v: &crate::cluster::ParameterValue,
) -> Option<crate::trigger::ParameterValue> {
    match v {
        crate::cluster::ParameterValue::Int(i) => Some(crate::trigger::ParameterValue::Int(*i)),
        crate::cluster::ParameterValue::Number(n) => {
            Some(crate::trigger::ParameterValue::Number(*n))
        }
        crate::cluster::ParameterValue::Bool(b) => Some(crate::trigger::ParameterValue::Bool(*b)),
        crate::cluster::ParameterValue::String(s) => {
            Some(crate::trigger::ParameterValue::String(s.clone()))
        }
        crate::cluster::ParameterValue::Enum(e) => {
            Some(crate::trigger::ParameterValue::Enum(e.clone()))
        }
    }
}

fn map_action_value(v: ActionValue) -> RuntimeValue {
    match v {
        ActionValue::Event(e) => RuntimeValue::Event(RuntimeEvent::Action(e)),
        ActionValue::Number(n) => RuntimeValue::Number(n),
        ActionValue::Series(s) => RuntimeValue::Series(s),
        ActionValue::Bool(b) => RuntimeValue::Bool(b),
        ActionValue::String(s) => RuntimeValue::String(s),
    }
}

fn map_to_action_value(
    v: &RuntimeValue,
    _node: &str,
    _port: &str,
) -> Result<ActionValue, ExecError> {
    Ok(match v {
        RuntimeValue::Event(RuntimeEvent::Action(e)) => ActionValue::Event(e.clone()),
        RuntimeValue::Event(RuntimeEvent::Trigger(TriggerEvent::Emitted)) => {
            ActionValue::Event(crate::action::ActionOutcome::Attempted)
        }
        RuntimeValue::Event(RuntimeEvent::Trigger(TriggerEvent::NotEmitted)) => {
            // R.7: should_skip_action() must catch NotEmitted before this point.
            unreachable!("R.7 violation: NotEmitted must be caught by should_skip_action")
        }
        RuntimeValue::Number(n) => ActionValue::Number(*n),
        RuntimeValue::Series(s) => ActionValue::Series(s.clone()),
        RuntimeValue::Bool(b) => ActionValue::Bool(*b),
        RuntimeValue::String(s) => ActionValue::String(s.clone()),
    })
}

fn map_runtime_value_to_common(v: &RuntimeValue) -> Option<crate::common::Value> {
    match v {
        RuntimeValue::Number(n) => Some(crate::common::Value::Number(*n)),
        RuntimeValue::Bool(b) => Some(crate::common::Value::Bool(*b)),
        RuntimeValue::String(s) => Some(crate::common::Value::String(s.clone())),
        RuntimeValue::Series(s) => Some(crate::common::Value::Series(s.clone())),
        RuntimeValue::Event(_) => None,
    }
}

fn map_to_action_parameter_value(
    v: &crate::cluster::ParameterValue,
) -> Option<crate::action::ParameterValue> {
    match v {
        crate::cluster::ParameterValue::Int(i) => Some(crate::action::ParameterValue::Int(*i)),
        crate::cluster::ParameterValue::Number(n) => {
            Some(crate::action::ParameterValue::Number(*n))
        }
        crate::cluster::ParameterValue::Bool(b) => Some(crate::action::ParameterValue::Bool(*b)),
        crate::cluster::ParameterValue::String(s) => {
            Some(crate::action::ParameterValue::String(s.clone()))
        }
        crate::cluster::ParameterValue::Enum(e) => {
            Some(crate::action::ParameterValue::Enum(e.clone()))
        }
    }
}

fn map_to_source_parameter_value(
    v: &crate::cluster::ParameterValue,
) -> Option<crate::source::ParameterValue> {
    match v {
        crate::cluster::ParameterValue::Int(i) => Some(crate::source::ParameterValue::Int(*i)),
        crate::cluster::ParameterValue::Number(n) => {
            Some(crate::source::ParameterValue::Number(*n))
        }
        crate::cluster::ParameterValue::Bool(b) => Some(crate::source::ParameterValue::Bool(*b)),
        crate::cluster::ParameterValue::String(s) => {
            Some(crate::source::ParameterValue::String(s.clone()))
        }
        crate::cluster::ParameterValue::Enum(e) => {
            Some(crate::source::ParameterValue::Enum(e.clone()))
        }
    }
}

fn source_manifest_name_parameters(
    node: &ValidatedNode,
    manifest: &crate::source::SourcePrimitiveManifest,
) -> HashMap<String, crate::cluster::ParameterValue> {
    let mut resolved = node.parameters.clone();

    for spec in &manifest.parameters {
        if resolved.contains_key(&spec.name) {
            continue;
        }
        let Some(default) = &spec.default else {
            continue;
        };
        if let Some(mapped) = map_source_default_to_cluster_parameter_value(default) {
            resolved.insert(spec.name.clone(), mapped);
        }
    }

    resolved
}

fn map_source_default_to_cluster_parameter_value(
    v: &crate::source::ParameterValue,
) -> Option<crate::cluster::ParameterValue> {
    match v {
        crate::source::ParameterValue::Int(i) => Some(crate::cluster::ParameterValue::Int(*i)),
        crate::source::ParameterValue::Number(n) => {
            Some(crate::cluster::ParameterValue::Number(*n))
        }
        crate::source::ParameterValue::Bool(b) => Some(crate::cluster::ParameterValue::Bool(*b)),
        crate::source::ParameterValue::String(s) => {
            Some(crate::cluster::ParameterValue::String(s.clone()))
        }
        crate::source::ParameterValue::Enum(e) => {
            Some(crate::cluster::ParameterValue::Enum(e.clone()))
        }
    }
}

/// R.7 gating: Returns true if any Event input is TriggerEvent::NotEmitted.
/// Uses AND semantics: all trigger events must be Emitted for action to execute.
fn should_skip_action(inputs: &HashMap<String, RuntimeValue>) -> bool {
    inputs.values().any(|v| {
        matches!(
            v,
            RuntimeValue::Event(RuntimeEvent::Trigger(TriggerEvent::NotEmitted))
        )
    })
}

/// Produce outputs for a skipped action. Event outputs get ActionOutcome::Skipped.
fn produce_skipped_outputs(node: &ValidatedNode) -> HashMap<String, RuntimeValue> {
    node.outputs
        .iter()
        .map(|(name, meta)| {
            let value = match meta.value_type {
                ValueType::Event => {
                    RuntimeValue::Event(RuntimeEvent::Action(ActionOutcome::Skipped))
                }
                // Non-event outputs use sensible defaults (actions are terminal per F.2).
                ValueType::Number => RuntimeValue::Number(0.0),
                ValueType::Bool => RuntimeValue::Bool(false),
                ValueType::String => RuntimeValue::String(String::new()),
                ValueType::Series => RuntimeValue::Series(vec![]),
            };
            (name.clone(), value)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::ensure_finite;
    use crate::common::Value;

    #[test]
    fn num_finite_guard_rejects_nan() {
        let outputs =
            std::collections::HashMap::from([("result".to_string(), Value::Number(f64::NAN))]);
        let result = ensure_finite("test_node", &outputs);
        assert!(matches!(
            result,
            Err(super::ExecError::NonFiniteOutput { .. })
        ));
    }

    #[test]
    fn num_finite_guard_rejects_infinity() {
        let outputs =
            std::collections::HashMap::from([("result".to_string(), Value::Number(f64::INFINITY))]);
        let result = ensure_finite("test_node", &outputs);
        assert!(matches!(
            result,
            Err(super::ExecError::NonFiniteOutput { .. })
        ));
    }

    #[test]
    fn num_finite_guard_allows_finite() {
        let outputs =
            std::collections::HashMap::from([("result".to_string(), Value::Number(42.0))]);
        let result = ensure_finite("test_node", &outputs);
        assert!(result.is_ok());
    }
}
