use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use semver::{Version as SemverVersion, VersionReq};
use serde::Deserialize;

use crate::io::{LoaderDecodeError, LoaderError, LoaderIoError};
use crate::DecodedAuthoringGraph;
use ergo_runtime::cluster::{
    BoundaryKind, Cardinality, ClusterDefinition, Edge, GraphInputPlaceholder, InputPortSpec,
    InputRef, NodeInstance, NodeKind, OutputPortSpec, OutputRef, ParameterBinding,
    ParameterDefault, ParameterSpec, ParameterType, ParameterValue, PortSpec, Signature, ValueType,
};

pub fn decode_graph_yaml(input: &str) -> Result<DecodedAuthoringGraph, LoaderError> {
    parse_graph_str(input, Path::new("<memory>"))
}

pub fn parse_graph_file(path: &Path) -> Result<DecodedAuthoringGraph, LoaderError> {
    let data = fs::read_to_string(path).map_err(|err| {
        LoaderError::Io(LoaderIoError {
            path: path.to_path_buf(),
            message: format!("read graph '{}': {err}", path.display()),
        })
    })?;
    parse_graph_str(&data, path)
}

fn parse_graph_str(input: &str, source: &Path) -> Result<DecodedAuthoringGraph, LoaderError> {
    let raw: RawClusterDefinition = serde_yaml::from_str(input)
        .map_err(|err| decode_error(format!("parse YAML '{}': {err}", source.display())))?;
    raw.into_cluster_definition()
        .map_err(|err| decode_error(format!("graph '{}': {err}", source.display())))
}

fn decode_error(message: String) -> LoaderError {
    LoaderError::Decode(LoaderDecodeError { message })
}
#[derive(Debug, Deserialize)]
struct RawClusterDefinition {
    kind: String,
    id: String,
    version: serde_yaml::Value,
    nodes: HashMap<String, RawNodeSpec>,
    edges: Vec<RawEdge>,
    #[serde(default)]
    outputs: HashMap<String, String>,
    #[serde(default)]
    inputs: Vec<RawInputPortSpec>,
    #[serde(default)]
    parameters: Vec<RawParameterSpec>,
    #[serde(default, alias = "signature")]
    declared_signature: Option<RawSignature>,
}

impl RawClusterDefinition {
    fn into_cluster_definition(self) -> Result<ClusterDefinition, String> {
        if !self.kind.eq_ignore_ascii_case("cluster") {
            return Err(format!("kind must be 'cluster', got '{}'", self.kind));
        }
        validate_general_identifier(&self.id, "cluster id", true)?;
        let version = parse_version_value(&self.version)?;

        let input_ports = self
            .inputs
            .into_iter()
            .map(RawInputPortSpec::into_input_port)
            .collect::<Result<Vec<_>, _>>()?;

        let input_names: HashSet<String> = input_ports
            .iter()
            .map(|input| input.maps_to.name.clone())
            .collect();

        let mut nodes = HashMap::new();
        for (node_id, raw_node) in self.nodes {
            validate_node_identifier(&node_id, "node id")?;
            let node = raw_node.into_node_instance(node_id.clone())?;
            nodes.insert(node_id, node);
        }

        let mut edges = Vec::with_capacity(self.edges.len());
        for raw_edge in self.edges {
            let parsed = raw_edge.into_parsed_edge()?;
            if let Some(external_name) = parsed.external_source.as_deref() {
                if !input_names.contains(external_name) {
                    return Err(format!(
                        "edge references external input '${}' that is not declared in inputs",
                        external_name
                    ));
                }
            } else if !nodes.contains_key(&parsed.edge.from.node_id) {
                if input_names.contains(&parsed.edge.from.node_id) {
                    return Err(format!(
                        "edge source '{}.{}' must use '${}' syntax for external inputs",
                        parsed.edge.from.node_id,
                        parsed.edge.from.port_name,
                        parsed.edge.from.node_id
                    ));
                }
                return Err(format!(
                    "edge source node '{}' is not declared in nodes",
                    parsed.edge.from.node_id
                ));
            }

            if !nodes.contains_key(&parsed.edge.to.node_id) {
                return Err(format!(
                    "edge target node '{}' is not declared in nodes",
                    parsed.edge.to.node_id
                ));
            }
            edges.push(parsed.edge);
        }

        let mut output_ports = Vec::with_capacity(self.outputs.len());
        for (name, target) in self.outputs {
            validate_general_identifier(&name, "output name", false)?;
            let (node_id, port_name) = parse_node_port_ref(&target, "output target")?;
            output_ports.push(OutputPortSpec {
                name,
                maps_to: OutputRef { node_id, port_name },
            });
        }

        let parameters = self
            .parameters
            .into_iter()
            .map(RawParameterSpec::into_parameter_spec)
            .collect::<Result<Vec<_>, _>>()?;

        let declared_signature = self
            .declared_signature
            .map(RawSignature::into_signature)
            .transpose()?;

        Ok(ClusterDefinition {
            id: self.id,
            version,
            nodes,
            edges,
            input_ports,
            output_ports,
            parameters,
            declared_signature,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawNodeSpec {
    #[serde(rename = "impl")]
    impl_ref: Option<String>,
    cluster: Option<String>,
    #[serde(default)]
    params: HashMap<String, RawParamBinding>,
}

impl RawNodeSpec {
    fn into_node_instance(self, node_id: String) -> Result<NodeInstance, String> {
        let kind = match (self.impl_ref, self.cluster) {
            (Some(impl_ref), None) => {
                let (impl_id, version) = parse_packed_id_version(&impl_ref, "impl")?;
                NodeKind::Impl { impl_id, version }
            }
            (None, Some(cluster_ref)) => {
                let (cluster_id, version) = parse_packed_id_version(&cluster_ref, "cluster")?;
                NodeKind::Cluster {
                    cluster_id,
                    version,
                }
            }
            (Some(_), Some(_)) => {
                return Err(format!(
                    "node '{}' must define exactly one of 'impl' or 'cluster'",
                    node_id
                ))
            }
            (None, None) => {
                return Err(format!(
                    "node '{}' must define exactly one of 'impl' or 'cluster'",
                    node_id
                ))
            }
        };

        let mut parameter_bindings = HashMap::new();
        for (name, raw_binding) in self.params {
            validate_general_identifier(&name, "parameter binding name", false)?;
            let binding = raw_binding.into_binding()?;
            parameter_bindings.insert(name, binding);
        }

        Ok(NodeInstance {
            id: node_id,
            kind,
            parameter_bindings,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawParamBinding {
    Exposed { exposed: String },
    Scalar(serde_yaml::Value),
}

impl RawParamBinding {
    fn into_binding(self) -> Result<ParameterBinding, String> {
        match self {
            Self::Exposed { exposed } => {
                validate_general_identifier(&exposed, "exposed parameter name", false)?;
                Ok(ParameterBinding::Exposed {
                    parent_param: exposed,
                })
            }
            Self::Scalar(value) => Ok(ParameterBinding::Literal {
                value: parse_untyped_parameter_value(&value)?,
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawEdge {
    Shorthand(String),
    Structured {
        from: RawEndpointRef,
        to: RawEndpointRef,
    },
}

#[derive(Debug)]
struct ParsedEdge {
    edge: Edge,
    external_source: Option<String>,
}

impl RawEdge {
    fn into_parsed_edge(self) -> Result<ParsedEdge, String> {
        match self {
            Self::Shorthand(text) => parse_shorthand_edge(&text),
            Self::Structured { from, to } => {
                let (from, external_source) = parse_output_endpoint_ref(from)?;
                let to = parse_input_endpoint_ref(to)?;
                Ok(ParsedEdge {
                    edge: Edge { from, to },
                    external_source,
                })
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawEndpointRef {
    NodePort { node: String, port: String },
    External { external: String },
}

fn parse_output_endpoint_ref(raw: RawEndpointRef) -> Result<(OutputRef, Option<String>), String> {
    match raw {
        RawEndpointRef::NodePort { node, port } => {
            validate_node_identifier(&node, "edge source node")?;
            validate_port_identifier(&port, "edge source port")?;
            Ok((
                OutputRef {
                    node_id: node,
                    port_name: port,
                },
                None,
            ))
        }
        RawEndpointRef::External { external } => {
            validate_general_identifier(&external, "external input name", true)?;
            Ok((
                OutputRef {
                    node_id: external.clone(),
                    port_name: external.clone(),
                },
                Some(external),
            ))
        }
    }
}

fn parse_input_endpoint_ref(raw: RawEndpointRef) -> Result<InputRef, String> {
    match raw {
        RawEndpointRef::NodePort { node, port } => {
            validate_node_identifier(&node, "edge target node")?;
            validate_port_identifier(&port, "edge target port")?;
            Ok(InputRef {
                node_id: node,
                port_name: port,
            })
        }
        RawEndpointRef::External { external } => Err(format!(
            "external input '{}' cannot appear as an edge target",
            external
        )),
    }
}

fn parse_shorthand_edge(text: &str) -> Result<ParsedEdge, String> {
    let (from_raw, to_raw) = text
        .split_once("->")
        .ok_or_else(|| format!("invalid edge '{}': expected 'from -> to'", text))?;
    let from_raw = from_raw.trim();
    let to_raw = to_raw.trim();
    if from_raw.is_empty() || to_raw.is_empty() {
        return Err(format!("invalid edge '{}': expected 'from -> to'", text));
    }

    let (from, external_source) = if let Some(external) = from_raw.strip_prefix('$') {
        let name = external.trim();
        validate_general_identifier(name, "external input name", true)?;
        (
            OutputRef {
                node_id: name.to_string(),
                port_name: name.to_string(),
            },
            Some(name.to_string()),
        )
    } else {
        let (node_id, port_name) = parse_node_port_ref(from_raw, "edge source")?;
        (OutputRef { node_id, port_name }, None)
    };

    if to_raw.starts_with('$') {
        return Err(format!(
            "invalid edge '{}': external inputs can only appear on the source side",
            text
        ));
    }

    let (node_id, port_name) = parse_node_port_ref(to_raw, "edge target")?;
    Ok(ParsedEdge {
        edge: Edge {
            from,
            to: InputRef { node_id, port_name },
        },
        external_source,
    })
}

#[derive(Debug, Deserialize)]
struct RawInputPortSpec {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    required: Option<bool>,
}

impl RawInputPortSpec {
    fn into_input_port(self) -> Result<InputPortSpec, String> {
        validate_general_identifier(&self.name, "input name", true)?;
        let ty = parse_value_type(&self.ty)?;
        let required = self.required.unwrap_or(true);
        Ok(InputPortSpec {
            name: self.name.clone(),
            maps_to: GraphInputPlaceholder {
                name: self.name,
                ty,
                required,
            },
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawParameterSpec {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    default: Option<serde_yaml::Value>,
    required: Option<bool>,
}

impl RawParameterSpec {
    fn into_parameter_spec(self) -> Result<ParameterSpec, String> {
        validate_general_identifier(&self.name, "parameter name", false)?;
        let ty = parse_parameter_type(&self.ty)?;
        let default = self
            .default
            .as_ref()
            .map(|value| parse_cluster_parameter_default(value, &ty))
            .transpose()?;
        let required = self.required.unwrap_or(default.is_none());
        Ok(ParameterSpec {
            name: self.name,
            ty,
            default,
            required,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawSignature {
    kind: String,
    inputs: Vec<RawPortSpec>,
    outputs: Vec<RawPortSpec>,
    has_side_effects: bool,
    is_origin: bool,
}

impl RawSignature {
    fn into_signature(self) -> Result<Signature, String> {
        let kind = parse_boundary_kind(&self.kind)?;
        let inputs = self
            .inputs
            .into_iter()
            .map(RawPortSpec::into_port_spec)
            .collect::<Result<Vec<_>, _>>()?;
        let outputs = self
            .outputs
            .into_iter()
            .map(RawPortSpec::into_port_spec)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Signature {
            kind,
            inputs,
            outputs,
            has_side_effects: self.has_side_effects,
            is_origin: self.is_origin,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawPortSpec {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    cardinality: String,
    wireable: bool,
}

impl RawPortSpec {
    fn into_port_spec(self) -> Result<PortSpec, String> {
        validate_general_identifier(&self.name, "declared signature port name", false)?;
        let ty = parse_value_type(&self.ty)?;
        let cardinality = parse_cardinality(&self.cardinality)?;
        Ok(PortSpec {
            name: self.name,
            ty,
            cardinality,
            wireable: self.wireable,
        })
    }
}

fn version_migration_guidance() -> &'static str {
    "use strict semver (e.g. '1.2.3') or a semver constraint (e.g. '^1.2'); migrate legacy tags with tools/migrate_graph_versions.py --check"
}

fn normalize_numeric_semver(raw: &str) -> Option<String> {
    let parts: Vec<&str> = raw.split('.').collect();
    if parts.is_empty() || parts.len() > 2 {
        return None;
    }
    if !parts
        .iter()
        .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    match parts.len() {
        1 => Some(format!("{}.0.0", parts[0])),
        2 => Some(format!("{}.{}.0", parts[0], parts[1])),
        _ => None,
    }
}

fn parse_exact_semver(value: &str, context: &str) -> Result<String, String> {
    SemverVersion::parse(value)
        .map(|version| version.to_string())
        .map_err(|_| {
            format!(
                "invalid {context} '{value}': expected strict semver (x.y.z); {}",
                version_migration_guidance()
            )
        })
}

fn parse_version_selector(value: &str, context: &str) -> Result<String, String> {
    if let Ok(version) = SemverVersion::parse(value) {
        return Ok(version.to_string());
    }
    VersionReq::parse(value)
        .map(|_| value.to_string())
        .map_err(|_| {
            format!(
                "invalid {context} '{value}': expected strict semver or semver constraint; {}",
                version_migration_guidance()
            )
        })
}

pub(crate) fn selector_matches_version(selector: &str, version: &str) -> Result<bool, String> {
    let candidate = SemverVersion::parse(version).map_err(|_| {
        format!("cluster discovery found non-semver version '{version}' in nested cluster file")
    })?;
    if let Ok(exact) = SemverVersion::parse(selector) {
        return Ok(candidate == exact);
    }
    let req = VersionReq::parse(selector)
        .map_err(|_| format!("invalid cluster selector '{selector}' during discovery"))?;
    Ok(req.matches(&candidate))
}

fn parse_packed_id_version(value: &str, field: &str) -> Result<(String, String), String> {
    let (id, version) = value.rsplit_once('@').ok_or_else(|| {
        format!(
            "invalid {} reference '{}': expected '<id>@<version>'",
            field, value
        )
    })?;
    if id.is_empty() || version.is_empty() {
        return Err(format!(
            "invalid {} reference '{}': expected non-empty id and version",
            field, value
        ));
    }
    validate_general_identifier(id, &format!("{field} id"), false)?;
    let version = parse_version_selector(version, &format!("{field} version selector"))?;
    Ok((id.to_string(), version))
}

fn parse_node_port_ref(value: &str, label: &str) -> Result<(String, String), String> {
    let (node, port) = value
        .split_once('.')
        .ok_or_else(|| format!("invalid {label} '{}': expected 'node.port'", value))?;
    if node.is_empty() || port.is_empty() || port.contains('.') {
        return Err(format!("invalid {label} '{}': expected 'node.port'", value));
    }
    validate_node_identifier(node, &format!("{label} node"))?;
    validate_port_identifier(port, &format!("{label} port"))?;
    Ok((node.to_string(), port.to_string()))
}

fn parse_version_value(value: &serde_yaml::Value) -> Result<String, String> {
    match value {
        serde_yaml::Value::String(s) => parse_exact_semver(s, "cluster version"),
        serde_yaml::Value::Number(n) => {
            let raw = n.to_string();
            let normalized = normalize_numeric_semver(&raw).ok_or_else(|| {
                format!(
                    "invalid cluster version number '{raw}': expected semver-like number (e.g. 1 or 1.2) or use a quoted semver string"
                )
            })?;
            parse_exact_semver(&normalized, "cluster version")
        }
        _ => Err(format!(
            "version must be a string or number, got '{value:?}'"
        )),
    }
}

fn parse_untyped_parameter_value(value: &serde_yaml::Value) -> Result<ParameterValue, String> {
    match value {
        serde_yaml::Value::Bool(b) => Ok(ParameterValue::Bool(*b)),
        serde_yaml::Value::Number(n) => {
            if let Some(int) = n.as_i64() {
                return Ok(ParameterValue::Int(int));
            }
            if let Some(number) = n.as_f64() {
                return Ok(ParameterValue::Number(number));
            }
            Err(format!("unsupported numeric parameter value '{n}'"))
        }
        serde_yaml::Value::String(s) => Ok(ParameterValue::String(s.clone())),
        _ => Err(format!(
            "parameter values must be scalar (number, bool, string) or {{exposed: ...}}, got '{value:?}'"
        )),
    }
}

fn parse_cluster_parameter_default(
    value: &serde_yaml::Value,
    ty: &ParameterType,
) -> Result<ParameterDefault, String> {
    // Mapping with "derive_key" key → DeriveKey default
    if let Some(mapping) = value.as_mapping() {
        if mapping.len() != 1 {
            return Err(format!(
                "parameter default mapping must have exactly one key, got {}",
                mapping.len()
            ));
        }
        let (key, val) = mapping.iter().next().unwrap();
        let key_str = key
            .as_str()
            .ok_or_else(|| "parameter default mapping key must be a string".to_string())?;
        if key_str != "derive_key" {
            return Err(format!(
                "unknown parameter default key '{}', expected 'derive_key'",
                key_str
            ));
        }
        let slot_name = val
            .as_str()
            .ok_or_else(|| "derive_key value must be a string".to_string())?;
        if slot_name.is_empty() {
            return Err("derive_key slot_name must not be empty".to_string());
        }
        return Ok(ParameterDefault::DeriveKey {
            slot_name: slot_name.to_string(),
        });
    }

    // Scalar → Literal default
    parse_typed_parameter_value(value, ty).map(ParameterDefault::Literal)
}

fn parse_typed_parameter_value(
    value: &serde_yaml::Value,
    ty: &ParameterType,
) -> Result<ParameterValue, String> {
    match ty {
        ParameterType::Int => value
            .as_i64()
            .map(ParameterValue::Int)
            .ok_or_else(|| format!("expected Int default, got '{value:?}'")),
        ParameterType::Number => value
            .as_f64()
            .map(ParameterValue::Number)
            .ok_or_else(|| format!("expected Number default, got '{value:?}'")),
        ParameterType::Bool => value
            .as_bool()
            .map(ParameterValue::Bool)
            .ok_or_else(|| format!("expected Bool default, got '{value:?}'")),
        ParameterType::String => value
            .as_str()
            .map(|s| ParameterValue::String(s.to_string()))
            .ok_or_else(|| format!("expected String default, got '{value:?}'")),
        ParameterType::Enum => value
            .as_str()
            .map(|s| ParameterValue::Enum(s.to_string()))
            .ok_or_else(|| format!("expected Enum default (string), got '{value:?}'")),
    }
}

fn parse_value_type(raw: &str) -> Result<ValueType, String> {
    match raw.to_ascii_lowercase().as_str() {
        "number" => Ok(ValueType::Number),
        "series" => Ok(ValueType::Series),
        "bool" | "boolean" => Ok(ValueType::Bool),
        "event" => Ok(ValueType::Event),
        "string" => Ok(ValueType::String),
        _ => Err(format!("unknown value type '{}'", raw)),
    }
}

fn parse_parameter_type(raw: &str) -> Result<ParameterType, String> {
    match raw.to_ascii_lowercase().as_str() {
        "int" => Ok(ParameterType::Int),
        "number" => Ok(ParameterType::Number),
        "bool" | "boolean" => Ok(ParameterType::Bool),
        "string" => Ok(ParameterType::String),
        "enum" => Ok(ParameterType::Enum),
        _ => Err(format!("unknown parameter type '{}'", raw)),
    }
}

fn parse_cardinality(raw: &str) -> Result<Cardinality, String> {
    match raw.to_ascii_lowercase().as_str() {
        "single" => Ok(Cardinality::Single),
        "multiple" => Ok(Cardinality::Multiple),
        _ => Err(format!("unknown cardinality '{}'", raw)),
    }
}

fn parse_boundary_kind(raw: &str) -> Result<BoundaryKind, String> {
    match raw.to_ascii_lowercase().as_str() {
        "source_like" | "sourcelike" => Ok(BoundaryKind::SourceLike),
        "compute_like" | "computelike" => Ok(BoundaryKind::ComputeLike),
        "trigger_like" | "triggerlike" => Ok(BoundaryKind::TriggerLike),
        "action_like" | "actionlike" => Ok(BoundaryKind::ActionLike),
        _ => Err(format!("unknown boundary kind '{}'", raw)),
    }
}

fn validate_node_identifier(value: &str, label: &str) -> Result<(), String> {
    validate_general_identifier(value, label, true)
}

fn validate_port_identifier(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if value.contains('.') {
        return Err(format!("{label} must not contain '.'"));
    }
    if value.contains(' ') {
        return Err(format!("{label} must not contain spaces"));
    }
    Ok(())
}

fn validate_general_identifier(
    value: &str,
    label: &str,
    forbid_dollar: bool,
) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if value.contains('.') {
        return Err(format!("{label} must not contain '.'"));
    }
    if value.contains('@') {
        return Err(format!("{label} must not contain '@'"));
    }
    if value.contains(' ') {
        return Err(format!("{label} must not contain spaces"));
    }
    if forbid_dollar && value.contains('$') {
        return Err(format!("{label} must not contain '$'"));
    }
    Ok(())
}
