use std::collections::BTreeSet;
use std::fmt;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::cluster::{
    Cardinality, ExpandedEndpoint, ExpandedGraph, ExpandedNode, ParameterType, ParameterValue,
    PrimitiveCatalog, PrimitiveKind, ValueType,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProvenanceScheme {
    Rpv1,
}

impl RuntimeProvenanceScheme {
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Rpv1 => "rpv1",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeProvenanceError {
    MissingPrimitiveMetadata { impl_id: String, version: String },
    NonFiniteFloat { context: String },
    Serialize(String),
}

impl fmt::Display for RuntimeProvenanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPrimitiveMetadata { impl_id, version } => {
                write!(
                    f,
                    "missing primitive metadata for '{}@{}'",
                    impl_id, version
                )
            }
            Self::NonFiniteFloat { context } => {
                write!(f, "non-finite float in runtime provenance ({context})")
            }
            Self::Serialize(msg) => write!(f, "runtime provenance serialization failed: {msg}"),
        }
    }
}

impl std::error::Error for RuntimeProvenanceError {}

pub fn compute_runtime_provenance<C: PrimitiveCatalog>(
    scheme: RuntimeProvenanceScheme,
    graph_id: &str,
    graph: &ExpandedGraph,
    catalog: &C,
) -> Result<String, RuntimeProvenanceError> {
    match scheme {
        RuntimeProvenanceScheme::Rpv1 => compute_rpv1(graph_id, graph, catalog),
    }
}

fn compute_rpv1<C: PrimitiveCatalog>(
    graph_id: &str,
    graph: &ExpandedGraph,
    catalog: &C,
) -> Result<String, RuntimeProvenanceError> {
    let input = RuntimeProvenanceV1Input::from_graph(graph_id, graph, catalog)?;
    let bytes = serde_json::to_vec(&input)
        .map_err(|err| RuntimeProvenanceError::Serialize(err.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    Ok(format!(
        "{}:sha256:{}",
        RuntimeProvenanceScheme::Rpv1.prefix(),
        to_hex(&digest)
    ))
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

#[derive(Debug, Serialize)]
struct RuntimeProvenanceV1Input {
    graph_id: String,
    nodes: Vec<ProvenanceNode>,
    edges: Vec<ProvenanceEdge>,
    primitives: Vec<ProvenancePrimitiveMeta>,
}

impl RuntimeProvenanceV1Input {
    fn from_graph<C: PrimitiveCatalog>(
        graph_id: &str,
        graph: &ExpandedGraph,
        catalog: &C,
    ) -> Result<Self, RuntimeProvenanceError> {
        let mut nodes = graph.nodes.values().collect::<Vec<_>>();
        nodes.sort_by(|a, b| a.runtime_id.cmp(&b.runtime_id));
        let nodes = nodes
            .into_iter()
            .map(ProvenanceNode::from_expanded_node)
            .collect::<Result<Vec<_>, _>>()?;

        let mut edges = graph
            .edges
            .iter()
            .map(ProvenanceEdge::from_expanded_edge)
            .collect::<Vec<_>>();
        edges.sort_by_key(|a| a.sort_key());

        let mut used = BTreeSet::<(String, String)>::new();
        for node in graph.nodes.values() {
            used.insert((
                node.implementation.impl_id.clone(),
                node.implementation.version.clone(),
            ));
        }
        let mut primitives = Vec::with_capacity(used.len());
        for (impl_id, resolved_version) in used {
            let meta = catalog.get(&impl_id, &resolved_version).ok_or_else(|| {
                RuntimeProvenanceError::MissingPrimitiveMetadata {
                    impl_id: impl_id.clone(),
                    version: resolved_version.clone(),
                }
            })?;
            primitives.push(ProvenancePrimitiveMeta::from_meta(
                impl_id,
                resolved_version,
                &meta,
            )?);
        }

        Ok(Self {
            graph_id: graph_id.to_string(),
            nodes,
            edges,
            primitives,
        })
    }
}

#[derive(Debug, Serialize)]
struct ProvenanceNode {
    runtime_id: String,
    impl_id: String,
    requested_version: String,
    resolved_version: String,
    parameters: Vec<ProvenanceBoundParam>,
}

impl ProvenanceNode {
    fn from_expanded_node(node: &ExpandedNode) -> Result<Self, RuntimeProvenanceError> {
        let mut params = node
            .parameters
            .iter()
            .map(|(name, value)| ProvenanceBoundParam::new(name, value))
            .collect::<Result<Vec<_>, _>>()?;
        params.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Self {
            runtime_id: node.runtime_id.clone(),
            impl_id: node.implementation.impl_id.clone(),
            requested_version: node.implementation.requested_version.clone(),
            resolved_version: node.implementation.version.clone(),
            parameters: params,
        })
    }
}

#[derive(Debug, Serialize)]
struct ProvenanceBoundParam {
    name: String,
    value: CanonicalParameterValue,
}

impl ProvenanceBoundParam {
    fn new(name: &str, value: &ParameterValue) -> Result<Self, RuntimeProvenanceError> {
        Ok(Self {
            name: name.to_string(),
            value: CanonicalParameterValue::from_parameter_value(
                value,
                format!("bound parameter '{name}'"),
            )?,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "value")]
enum CanonicalParameterValue {
    Int(i64),
    Number(f64),
    Bool(bool),
    String(String),
    Enum(String),
}

impl CanonicalParameterValue {
    fn from_parameter_value(
        value: &ParameterValue,
        context: String,
    ) -> Result<Self, RuntimeProvenanceError> {
        Ok(match value {
            ParameterValue::Int(v) => Self::Int(*v),
            ParameterValue::Number(v) => {
                if !v.is_finite() {
                    return Err(RuntimeProvenanceError::NonFiniteFloat { context });
                }
                Self::Number(*v)
            }
            ParameterValue::Bool(v) => Self::Bool(*v),
            ParameterValue::String(v) => Self::String(v.clone()),
            ParameterValue::Enum(v) => Self::Enum(v.clone()),
        })
    }
}

#[derive(Debug, Serialize)]
struct ProvenanceEdge {
    from: ProvenanceEndpoint,
    to: ProvenanceEndpoint,
}

impl ProvenanceEdge {
    fn from_expanded_edge(edge: &crate::cluster::ExpandedEdge) -> Self {
        Self {
            from: ProvenanceEndpoint::from_expanded_endpoint(&edge.from),
            to: ProvenanceEndpoint::from_expanded_endpoint(&edge.to),
        }
    }

    fn sort_key(&self) -> (String, String) {
        (self.from.sort_key(), self.to.sort_key())
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
enum ProvenanceEndpoint {
    NodePort { node_id: String, port_name: String },
    ExternalInput { name: String },
}

impl ProvenanceEndpoint {
    fn from_expanded_endpoint(endpoint: &ExpandedEndpoint) -> Self {
        match endpoint {
            ExpandedEndpoint::NodePort { node_id, port_name } => Self::NodePort {
                node_id: node_id.clone(),
                port_name: port_name.clone(),
            },
            ExpandedEndpoint::ExternalInput { name } => Self::ExternalInput { name: name.clone() },
        }
    }

    fn sort_key(&self) -> String {
        match self {
            Self::NodePort { node_id, port_name } => format!("node:{node_id}.{port_name}"),
            Self::ExternalInput { name } => format!("ext:{name}"),
        }
    }
}

#[derive(Debug, Serialize)]
struct ProvenancePrimitiveMeta {
    impl_id: String,
    resolved_version: String,
    kind: String,
    inputs: Vec<ProvenanceInputMeta>,
    outputs: Vec<ProvenanceOutputMeta>,
    parameters: Vec<ProvenanceParameterMeta>,
}

impl ProvenancePrimitiveMeta {
    fn from_meta(
        impl_id: String,
        resolved_version: String,
        meta: &crate::cluster::PrimitiveMetadata,
    ) -> Result<Self, RuntimeProvenanceError> {
        let mut inputs = meta
            .inputs
            .iter()
            .map(|input| ProvenanceInputMeta {
                name: input.name.clone(),
                value_type: value_type_name(&input.value_type).to_string(),
                required: input.required,
            })
            .collect::<Vec<_>>();
        inputs.sort_by(|a, b| a.name.cmp(&b.name));

        let mut outputs = meta
            .outputs
            .iter()
            .map(|(name, output)| ProvenanceOutputMeta {
                name: name.clone(),
                value_type: value_type_name(&output.value_type).to_string(),
                cardinality: cardinality_name(&output.cardinality).to_string(),
            })
            .collect::<Vec<_>>();
        outputs.sort_by(|a, b| a.name.cmp(&b.name));

        let mut parameters = meta
            .parameters
            .iter()
            .map(|param| {
                Ok(ProvenanceParameterMeta {
                    name: param.name.clone(),
                    ty: parameter_type_name(&param.ty).to_string(),
                    required: param.required,
                    default: match &param.default {
                        Some(value) => Some(CanonicalParameterValue::from_parameter_value(
                            value,
                            format!("default parameter '{}'", param.name),
                        )?),
                        None => None,
                    },
                })
            })
            .collect::<Result<Vec<_>, RuntimeProvenanceError>>()?;
        parameters.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(Self {
            impl_id,
            resolved_version,
            kind: primitive_kind_name(&meta.kind).to_string(),
            inputs,
            outputs,
            parameters,
        })
    }
}

#[derive(Debug, Serialize)]
struct ProvenanceInputMeta {
    name: String,
    value_type: String,
    required: bool,
}

#[derive(Debug, Serialize)]
struct ProvenanceOutputMeta {
    name: String,
    value_type: String,
    cardinality: String,
}

#[derive(Debug, Serialize)]
struct ProvenanceParameterMeta {
    name: String,
    ty: String,
    required: bool,
    default: Option<CanonicalParameterValue>,
}

fn value_type_name(value: &ValueType) -> &'static str {
    match value {
        ValueType::Number => "Number",
        ValueType::Series => "Series",
        ValueType::Bool => "Bool",
        ValueType::Event => "Event",
        ValueType::String => "String",
    }
}

fn cardinality_name(cardinality: &Cardinality) -> &'static str {
    match cardinality {
        Cardinality::Single => "Single",
        Cardinality::Multiple => "Multiple",
    }
}

fn parameter_type_name(ty: &ParameterType) -> &'static str {
    match ty {
        ParameterType::Int => "Int",
        ParameterType::Number => "Number",
        ParameterType::Bool => "Bool",
        ParameterType::String => "String",
        ParameterType::Enum => "Enum",
    }
}

fn primitive_kind_name(kind: &PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Source => "Source",
        PrimitiveKind::Compute => "Compute",
        PrimitiveKind::Trigger => "Trigger",
        PrimitiveKind::Action => "Action",
    }
}

#[cfg(test)]
mod tests;
