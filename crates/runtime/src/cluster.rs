use std::collections::{HashMap, HashSet};

pub type Version = String;
pub type NodeId = String;

#[derive(Debug, Clone, PartialEq)]
pub struct ClusterDefinition {
    pub id: String,
    pub version: Version,
    pub nodes: HashMap<NodeId, NodeInstance>,
    pub edges: Vec<Edge>,
    pub input_ports: Vec<InputPortSpec>,
    pub output_ports: Vec<OutputPortSpec>,
    pub parameters: Vec<ParameterSpec>,
    pub declared_signature: Option<Signature>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeInstance {
    pub id: NodeId,
    pub kind: NodeKind,
    pub parameter_bindings: HashMap<String, ParameterBinding>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    Impl {
        impl_id: String,
        version: Version,
    },
    Cluster {
        cluster_id: String,
        version: Version,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub from: OutputRef,
    pub to: InputRef,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputRef {
    pub node_id: NodeId,
    pub port_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputRef {
    pub node_id: NodeId,
    pub port_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputPortSpec {
    pub name: String,
    pub maps_to: GraphInputPlaceholder,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputPortSpec {
    pub name: String,
    pub maps_to: OutputRef,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphInputPlaceholder {
    pub name: String,
    pub ty: ValueType,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParameterSpec {
    pub name: String,
    pub ty: ParameterType,
    pub default: Option<ParameterValue>,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParameterBinding {
    Literal { value: ParameterValue },
    Exposed { parent_param: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Signature {
    pub kind: BoundaryKind,
    pub inputs: Vec<PortSpec>,
    pub outputs: Vec<PortSpec>,
    pub has_side_effects: bool,
    pub is_origin: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PortSpec {
    pub name: String,
    pub ty: ValueType,
    pub cardinality: Cardinality,
    pub wireable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BoundaryKind {
    SourceLike,
    ComputeLike,
    TriggerLike,
    ActionLike,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueType {
    Number,
    Series,
    Bool,
    Event,
    String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cardinality {
    Single,
    Multiple,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParameterType {
    Int,
    Number,
    Bool,
    String,
    Enum,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParameterValue {
    Int(i64),
    Number(f64),
    Bool(bool),
    String(String),
    Enum(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrimitiveKind {
    Source,
    Compute,
    Trigger,
    Action,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputMetadata {
    pub value_type: ValueType,
    pub cardinality: Cardinality,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrimitiveMetadata {
    pub kind: PrimitiveKind,
    pub inputs: Vec<InputMetadata>,
    pub outputs: HashMap<String, OutputMetadata>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputMetadata {
    pub name: String,
    pub value_type: ValueType,
    pub required: bool,
}

/// Expansion output. Contains only topology, primitive identity, and authoring trace.
/// `boundary_inputs` and `boundary_outputs` are retained for signature inference only
/// and must not influence runtime execution.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedGraph {
    pub nodes: HashMap<String, ExpandedNode>,
    pub edges: Vec<ExpandedEdge>,
    pub boundary_inputs: Vec<InputPortSpec>,
    pub boundary_outputs: Vec<OutputPortSpec>,
}

/// X.9 enforcement: Clusters compile away here.
///
/// `ExpandedNode` holds only `ImplementationInstance` — no `NodeKind` enum.
/// Execution graphs (`ComputeGraph`, `TriggerGraph`, `ActionGraph`, `SourceGraph`)
/// have no cluster representation. The type system guarantees authoring
/// constructs cannot reach execution.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedNode {
    pub runtime_id: String,
    pub authoring_path: Vec<(String, NodeId)>,
    pub implementation: ImplementationInstance,
    pub parameters: HashMap<String, ParameterValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImplementationInstance {
    // Identity-only; no semantic or configuration fields.
    pub impl_id: String,
    pub version: Version,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedEdge {
    pub from: ExpandedEndpoint,
    pub to: ExpandedEndpoint,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExpandedEndpoint {
    NodePort { node_id: String, port_name: String },
    ExternalInput { name: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExpandError {
    EmptyCluster,
    MissingCluster {
        id: String,
        version: Version,
    },
    DuplicateInputPort {
        name: String,
    },
    DuplicateOutputPort {
        name: String,
    },
    DuplicateParameter {
        name: String,
    },
    ParameterDefaultTypeMismatch {
        name: String,
        expected: ParameterType,
        got: ParameterType,
    },
    SignatureInferenceFailed(SignatureInferenceError),
    DeclaredSignatureInvalid(ClusterValidationError),
    /// I.3: Required parameter has no binding and no default
    MissingRequiredParameter {
        cluster_id: String,
        parameter: String,
    },
    /// I.4: Literal binding has wrong type
    ParameterBindingTypeMismatch {
        cluster_id: String,
        parameter: String,
        expected: ParameterType,
        got: ParameterType,
    },
    /// I.5: Exposed binding references nonexistent parent parameter
    ExposedParameterNotFound {
        cluster_id: String,
        parameter: String,
        referenced: String,
    },
    /// I.4: Exposed binding has incompatible type with parent parameter
    ExposedParameterTypeMismatch {
        cluster_id: String,
        parameter: String,
        expected: ParameterType,
        got: ParameterType,
    },
    /// Exposed binding was not resolved during expansion (no parent provided a value)
    UnresolvedExposedBinding {
        node_id: String,
        parameter: String,
        referenced: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum SignatureInferenceError {
    MissingPrimitive {
        id: String,
        version: Version,
    },
    MissingNode(String),
    MissingOutput {
        impl_id: String,
        version: Version,
        output: String,
    },
}

/// D.11: Errors arising from declared signature validation
#[derive(Debug, Clone, PartialEq)]
pub enum ClusterValidationError {
    /// Declared wireability exceeds inferred wireability (D.11 violation)
    WireabilityExceedsInferred { port_name: String },
}

pub trait ClusterLoader {
    fn load(&self, id: &str, version: &Version) -> Option<ClusterDefinition>;
}

pub trait PrimitiveCatalog {
    fn get(&self, id: &str, version: &Version) -> Option<PrimitiveMetadata>;
}

pub fn expand<L: ClusterLoader>(
    cluster_def: &ClusterDefinition,
    loader: &L,
    catalog: &impl PrimitiveCatalog,
) -> Result<ExpandedGraph, ExpandError> {
    validate_cluster_definition(cluster_def)?;

    let mut ctx = ExpandContext::new();
    let build = expand_with_context(cluster_def, loader, catalog, &mut ctx, &[], &HashMap::new())?;

    let mut graph = build.graph;
    graph.boundary_inputs = cluster_def.input_ports.clone();
    graph.boundary_outputs = map_boundary_outputs(&cluster_def.output_ports, &build.node_mapping);

    // E.3 invariant: ExternalInput must not appear as edge target (sink) after expansion
    for edge in &graph.edges {
        debug_assert!(
            !matches!(&edge.to, ExpandedEndpoint::ExternalInput { .. }),
            "Invariant E.3 violated: ExternalInput '{}' cannot be edge sink after expansion",
            match &edge.to {
                ExpandedEndpoint::ExternalInput { name } => name.as_str(),
                _ => unreachable!(),
            }
        );
    }

    if let Some(declared) = &cluster_def.declared_signature {
        let inferred =
            infer_signature(&graph, catalog).map_err(ExpandError::SignatureInferenceFailed)?;
        validate_declared_signature(declared, &inferred)
            .map_err(ExpandError::DeclaredSignatureInvalid)?;
    }

    Ok(graph)
}

fn validate_cluster_definition(cluster_def: &ClusterDefinition) -> Result<(), ExpandError> {
    let mut input_names = HashSet::new();
    for input in &cluster_def.input_ports {
        if !input_names.insert(input.name.clone()) {
            return Err(ExpandError::DuplicateInputPort {
                name: input.name.clone(),
            });
        }
    }

    let mut output_names = HashSet::new();
    for output in &cluster_def.output_ports {
        if !output_names.insert(output.name.clone()) {
            return Err(ExpandError::DuplicateOutputPort {
                name: output.name.clone(),
            });
        }
    }

    let mut parameter_names = HashSet::new();
    for param in &cluster_def.parameters {
        if !parameter_names.insert(param.name.clone()) {
            return Err(ExpandError::DuplicateParameter {
                name: param.name.clone(),
            });
        }

        if let Some(default) = &param.default {
            let got = parameter_value_type(default);
            if got != param.ty {
                return Err(ExpandError::ParameterDefaultTypeMismatch {
                    name: param.name.clone(),
                    expected: param.ty.clone(),
                    got,
                });
            }
        }
    }

    Ok(())
}

fn parameter_value_type(value: &ParameterValue) -> ParameterType {
    match value {
        ParameterValue::Int(_) => ParameterType::Int,
        ParameterValue::Number(_) => ParameterType::Number,
        ParameterValue::Bool(_) => ParameterType::Bool,
        ParameterValue::String(_) => ParameterType::String,
        ParameterValue::Enum(_) => ParameterType::Enum,
    }
}

/// Infers the cluster's signature from its expanded graph.
///
/// F.6 invariant: Inference depends only on:
/// - Graph structure (nodes, edges, boundary ports)
/// - Catalog (primitive metadata for node kind lookup)
///
/// Inference must NOT depend on runtime state, execution context,
/// or any mutable external state. This guarantees deterministic,
/// reproducible signatures for the same graph definition.
pub fn infer_signature<C: PrimitiveCatalog>(
    graph: &ExpandedGraph,
    catalog: &C,
) -> Result<Signature, SignatureInferenceError> {
    let mut node_meta: HashMap<String, PrimitiveMetadata> = HashMap::new();
    let mut has_side_effects = false;

    for (node_id, node) in &graph.nodes {
        let meta = catalog
            .get(&node.implementation.impl_id, &node.implementation.version)
            .ok_or_else(|| SignatureInferenceError::MissingPrimitive {
                id: node.implementation.impl_id.clone(),
                version: node.implementation.version.clone(),
            })?;
        if meta.kind == PrimitiveKind::Action {
            has_side_effects = true;
        }
        node_meta.insert(node_id.clone(), meta);
    }

    let mut inputs: Vec<PortSpec> = Vec::new();
    for input in &graph.boundary_inputs {
        let port = PortSpec {
            name: input.name.clone(),
            ty: input.maps_to.ty.clone(),
            cardinality: Cardinality::Single,
            wireable: false, // F.1: Input ports are never wireable
        };
        // F.1 invariant: Input ports must never be wireable (CLUSTER_SPEC.md §3.2)
        debug_assert!(
            !port.wireable,
            "Invariant F.1 violated: input port '{}' must not be wireable",
            port.name
        );
        inputs.push(port);
    }

    let mut outputs: Vec<PortSpec> = Vec::new();
    let mut has_wireable_outputs = false;
    let mut wireable_out_types: Vec<ValueType> = Vec::new();

    for output in &graph.boundary_outputs {
        let meta = node_meta
            .get(&output.maps_to.node_id)
            .ok_or_else(|| SignatureInferenceError::MissingNode(output.maps_to.node_id.clone()))?;

        let out_meta = meta.outputs.get(&output.maps_to.port_name).ok_or_else(|| {
            SignatureInferenceError::MissingOutput {
                impl_id: graph
                    .nodes
                    .get(&output.maps_to.node_id)
                    .map(|n| n.implementation.impl_id.clone())
                    .unwrap_or_default(),
                version: graph
                    .nodes
                    .get(&output.maps_to.node_id)
                    .map(|n| n.implementation.version.clone())
                    .unwrap_or_default(),
                output: output.maps_to.port_name.clone(),
            }
        })?;

        let wireable = meta.kind != PrimitiveKind::Action;
        if wireable {
            has_wireable_outputs = true;
            wireable_out_types.push(out_meta.value_type.clone());
        }

        outputs.push(PortSpec {
            name: output.name.clone(),
            ty: out_meta.value_type.clone(),
            cardinality: out_meta.cardinality.clone(),
            wireable,
        });
    }

    let has_wireable_event_out = wireable_out_types
        .iter()
        .any(|t| matches!(t, ValueType::Event));

    let kind = if !has_wireable_outputs {
        BoundaryKind::ActionLike
    } else if graph.boundary_inputs.is_empty()
        && wireable_out_types.iter().all(|t| {
            matches!(
                t,
                ValueType::Number | ValueType::Series | ValueType::Bool | ValueType::String
            )
        })
    {
        BoundaryKind::SourceLike
    } else if has_wireable_event_out {
        BoundaryKind::TriggerLike
    } else {
        BoundaryKind::ComputeLike
    };

    let is_origin = graph.boundary_inputs.is_empty() && roots_are_sources(graph, &node_meta);

    Ok(Signature {
        kind,
        inputs,
        outputs,
        has_side_effects,
        is_origin,
    })
}

/// D.11: Validate that declared signature wireability does not exceed inferred wireability.
/// Declared wireability can restrict (true → false) but cannot grant (false → true).
pub fn validate_declared_signature(
    declared: &Signature,
    inferred: &Signature,
) -> Result<(), ClusterValidationError> {
    // Check output ports: declared.wireable cannot exceed inferred.wireable
    for declared_port in &declared.outputs {
        if let Some(inferred_port) = inferred
            .outputs
            .iter()
            .find(|p| p.name == declared_port.name)
        {
            // D.11: If declared.wireable == true but inferred.wireable == false, reject
            if declared_port.wireable && !inferred_port.wireable {
                return Err(ClusterValidationError::WireabilityExceedsInferred {
                    port_name: declared_port.name.clone(),
                });
            }
        }
    }

    // Check input ports: declared.wireable cannot exceed inferred.wireable
    // Note: Per F.1, inferred inputs always have wireable: false, so any declared wireable: true is invalid
    for declared_port in &declared.inputs {
        if let Some(inferred_port) = inferred
            .inputs
            .iter()
            .find(|p| p.name == declared_port.name)
        {
            if declared_port.wireable && !inferred_port.wireable {
                return Err(ClusterValidationError::WireabilityExceedsInferred {
                    port_name: declared_port.name.clone(),
                });
            }
        }
    }

    Ok(())
}

fn roots_are_sources(graph: &ExpandedGraph, meta: &HashMap<String, PrimitiveMetadata>) -> bool {
    let mut incoming: HashSet<&String> = HashSet::new();
    for edge in &graph.edges {
        if let (
            ExpandedEndpoint::NodePort { node_id: _from, .. },
            ExpandedEndpoint::NodePort { node_id: to, .. },
        ) = (&edge.from, &edge.to)
        {
            incoming.insert(to);
        }
    }

    for node_id in graph.nodes.keys() {
        if !incoming.contains(node_id) {
            if let Some(m) = meta.get(node_id) {
                if m.kind != PrimitiveKind::Source {
                    return false;
                }
            } else {
                return false;
            }
        }
    }

    true
}

#[derive(Debug)]
struct ExpandContext {
    next_id: usize,
}

impl ExpandContext {
    fn new() -> Self {
        Self { next_id: 0 }
    }

    fn next_runtime_id(&mut self) -> String {
        let id = format!("n{}", self.next_id);
        self.next_id += 1;
        id
    }
}

/// I.3/I.4/I.5: Validate parameter bindings for a nested cluster instantiation.
fn validate_parameter_bindings(
    nested_def: &ClusterDefinition,
    bindings: &HashMap<String, ParameterBinding>,
    parent_parameters: &[ParameterSpec],
) -> Result<(), ExpandError> {
    for param_spec in &nested_def.parameters {
        match bindings.get(&param_spec.name) {
            None => {
                // I.3: Required parameter with no default must have a binding
                if param_spec.required && param_spec.default.is_none() {
                    return Err(ExpandError::MissingRequiredParameter {
                        cluster_id: nested_def.id.clone(),
                        parameter: param_spec.name.clone(),
                    });
                }
            }
            Some(ParameterBinding::Literal { value }) => {
                // I.4: Literal binding must have correct type
                let got = parameter_value_type(value);
                if got != param_spec.ty {
                    return Err(ExpandError::ParameterBindingTypeMismatch {
                        cluster_id: nested_def.id.clone(),
                        parameter: param_spec.name.clone(),
                        expected: param_spec.ty.clone(),
                        got,
                    });
                }
            }
            Some(ParameterBinding::Exposed { parent_param }) => {
                // I.5: Exposed binding must reference existing parent parameter
                // I.4: Exposed binding must have compatible type
                let parent_spec = parent_parameters.iter().find(|p| &p.name == parent_param);
                match parent_spec {
                    None => {
                        return Err(ExpandError::ExposedParameterNotFound {
                            cluster_id: nested_def.id.clone(),
                            parameter: param_spec.name.clone(),
                            referenced: parent_param.clone(),
                        });
                    }
                    Some(spec) if spec.ty != param_spec.ty => {
                        return Err(ExpandError::ExposedParameterTypeMismatch {
                            cluster_id: nested_def.id.clone(),
                            parameter: param_spec.name.clone(),
                            expected: param_spec.ty.clone(),
                            got: spec.ty.clone(),
                        });
                    }
                    Some(_) => {} // Type matches, binding valid
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct ExpandBuild {
    graph: ExpandedGraph,
    node_mapping: HashMap<NodeId, String>,
    placeholder_map: HashMap<String, String>,
}

fn expand_with_context<L: ClusterLoader>(
    cluster_def: &ClusterDefinition,
    loader: &L,
    catalog: &impl PrimitiveCatalog,
    ctx: &mut ExpandContext,
    authoring_prefix: &[(String, NodeId)],
    resolved_params: &HashMap<String, ParameterValue>,
) -> Result<ExpandBuild, ExpandError> {
    if cluster_def.nodes.is_empty() {
        return Err(ExpandError::EmptyCluster);
    }

    let placeholder_map =
        build_placeholder_map(authoring_prefix, &cluster_def.id, &cluster_def.input_ports);

    let mut graph = ExpandedGraph {
        nodes: HashMap::new(),
        edges: Vec::new(),
        boundary_inputs: Vec::new(),
        boundary_outputs: Vec::new(),
    };
    let mut node_mapping: HashMap<NodeId, String> = HashMap::new();
    let mut cluster_output_map: HashMap<NodeId, HashMap<String, ExpandedEndpoint>> = HashMap::new();
    let mut cluster_input_map: HashMap<NodeId, HashMap<String, String>> = HashMap::new();

    for node in cluster_def.nodes.values() {
        match &node.kind {
            NodeKind::Impl { impl_id, version } => {
                let runtime_id = ctx.next_runtime_id();
                let mut authoring_path = authoring_prefix.to_vec();
                authoring_path.push((cluster_def.id.clone(), node.id.clone()));

                // Resolve Exposed bindings using parent's resolved parameters
                let resolved_bindings =
                    resolve_bindings_with_context(&node.parameter_bindings, resolved_params)?;

                // Validate no Exposed bindings remain unresolved
                for (name, binding) in &node.parameter_bindings {
                    if let ParameterBinding::Exposed { parent_param } = binding {
                        if !resolved_params.contains_key(parent_param) {
                            return Err(ExpandError::UnresolvedExposedBinding {
                                node_id: node.id.clone(),
                                parameter: name.clone(),
                                referenced: parent_param.clone(),
                            });
                        }
                    }
                }

                graph.nodes.insert(
                    runtime_id.clone(),
                    ExpandedNode {
                        runtime_id: runtime_id.clone(),
                        authoring_path,
                        implementation: ImplementationInstance {
                            impl_id: impl_id.clone(),
                            version: version.clone(),
                        },
                        parameters: resolved_bindings,
                    },
                );

                node_mapping.insert(node.id.clone(), runtime_id);
            }
            NodeKind::Cluster {
                cluster_id,
                version,
            } => {
                let nested_def = loader.load(cluster_id, version).ok_or_else(|| {
                    ExpandError::MissingCluster {
                        id: cluster_id.clone(),
                        version: version.clone(),
                    }
                })?;

                // I.3/I.4/I.5: Validate parameter bindings before expansion
                validate_parameter_bindings(
                    &nested_def,
                    &node.parameter_bindings,
                    &cluster_def.parameters,
                )?;

                let bound_nested = apply_literal_bindings(&nested_def, &node.parameter_bindings);

                // Build resolved parameter values for the nested cluster:
                // - Literal bindings use their value directly
                // - Exposed bindings look up the value from our resolved_params
                let nested_resolved_params =
                    build_resolved_params(&node.parameter_bindings, resolved_params);

                let mut nested_prefix = authoring_prefix.to_vec();
                nested_prefix.push((cluster_def.id.clone(), node.id.clone()));

                let nested_build = expand_with_context(
                    &bound_nested,
                    loader,
                    catalog,
                    ctx,
                    &nested_prefix,
                    &nested_resolved_params,
                )?;

                merge_graph(&mut graph, nested_build.graph);

                let mut input_map: HashMap<String, String> = HashMap::new();
                for input_port in &bound_nested.input_ports {
                    if let Some(mapped) = nested_build.placeholder_map.get(&input_port.maps_to.name)
                    {
                        input_map.insert(input_port.name.clone(), mapped.clone());
                    }
                }
                cluster_input_map.insert(node.id.clone(), input_map);

                let mut output_map: HashMap<String, ExpandedEndpoint> = HashMap::new();
                for output_port in &bound_nested.output_ports {
                    if let Some(node_id) =
                        nested_build.node_mapping.get(&output_port.maps_to.node_id)
                    {
                        output_map.insert(
                            output_port.name.clone(),
                            ExpandedEndpoint::NodePort {
                                node_id: node_id.clone(),
                                port_name: output_port.maps_to.port_name.clone(),
                            },
                        );
                    }
                }
                cluster_output_map.insert(node.id.clone(), output_map);

                for (k, v) in nested_build.node_mapping {
                    node_mapping.insert(k, v);
                }
            }
        }
    }

    for edge in &cluster_def.edges {
        let from = resolve_output_endpoint(
            &edge.from,
            &node_mapping,
            &cluster_output_map,
            authoring_prefix,
            &cluster_def.id,
        );
        let to = resolve_input_endpoint(
            &edge.to,
            &node_mapping,
            &cluster_input_map,
            &placeholder_map,
            authoring_prefix,
            &cluster_def.id,
        );

        if let ExpandedEndpoint::ExternalInput { name } = &to {
            let replaced = redirect_placeholder_edges(&mut graph.edges, name, &from);
            if !replaced {
                graph.edges.push(ExpandedEdge {
                    from: from.clone(),
                    to: to.clone(),
                });
            }
        } else {
            graph.edges.push(ExpandedEdge { from, to });
        }
    }

    Ok(ExpandBuild {
        graph,
        node_mapping,
        placeholder_map,
    })
}

fn build_placeholder_map(
    authoring_prefix: &[(String, NodeId)],
    cluster_id: &str,
    input_ports: &[InputPortSpec],
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for input in input_ports {
        let key = external_key(authoring_prefix, cluster_id, &input.maps_to.name);
        map.insert(input.maps_to.name.clone(), key);
    }
    map
}

fn external_key(authoring_prefix: &[(String, NodeId)], cluster_id: &str, name: &str) -> String {
    let mut parts: Vec<String> = authoring_prefix
        .iter()
        .map(|(c, n)| format!("{}:{}", c, n))
        .collect();
    parts.push(cluster_id.to_string());
    parts.push(name.to_string());
    parts.join("/")
}

fn merge_graph(target: &mut ExpandedGraph, nested: ExpandedGraph) {
    for (id, node) in nested.nodes {
        target.nodes.insert(id, node);
    }
    target.edges.extend(nested.edges);
}

fn resolve_output_endpoint(
    output: &OutputRef,
    node_mapping: &HashMap<NodeId, String>,
    cluster_output_map: &HashMap<NodeId, HashMap<String, ExpandedEndpoint>>,
    authoring_prefix: &[(String, NodeId)],
    cluster_id: &str,
) -> ExpandedEndpoint {
    if let Some(node_id) = node_mapping.get(&output.node_id) {
        return ExpandedEndpoint::NodePort {
            node_id: node_id.clone(),
            port_name: output.port_name.clone(),
        };
    }

    if let Some(map) = cluster_output_map.get(&output.node_id) {
        if let Some(ep) = map.get(&output.port_name) {
            return ep.clone();
        }
    }

    ExpandedEndpoint::ExternalInput {
        name: external_key(authoring_prefix, cluster_id, &output.node_id),
    }
}

fn resolve_input_endpoint(
    input: &InputRef,
    node_mapping: &HashMap<NodeId, String>,
    cluster_input_map: &HashMap<NodeId, HashMap<String, String>>,
    placeholder_map: &HashMap<String, String>,
    authoring_prefix: &[(String, NodeId)],
    cluster_id: &str,
) -> ExpandedEndpoint {
    if let Some(node_id) = node_mapping.get(&input.node_id) {
        return ExpandedEndpoint::NodePort {
            node_id: node_id.clone(),
            port_name: input.port_name.clone(),
        };
    }

    if let Some(map) = cluster_input_map.get(&input.node_id) {
        if let Some(name) = map.get(&input.port_name) {
            return ExpandedEndpoint::ExternalInput { name: name.clone() };
        }
    }

    if let Some(name) = placeholder_map.get(&input.node_id) {
        return ExpandedEndpoint::ExternalInput { name: name.clone() };
    }

    ExpandedEndpoint::ExternalInput {
        name: external_key(authoring_prefix, cluster_id, &input.node_id),
    }
}

fn redirect_placeholder_edges(
    edges: &mut [ExpandedEdge],
    placeholder: &str,
    source: &ExpandedEndpoint,
) -> bool {
    let mut replaced = false;
    for edge in edges.iter_mut() {
        if let ExpandedEndpoint::ExternalInput { name } = &edge.from {
            if name == placeholder {
                edge.from = source.clone();
                replaced = true;
            }
        }
    }
    replaced
}

fn apply_literal_bindings(
    cluster_def: &ClusterDefinition,
    bindings: &HashMap<String, ParameterBinding>,
) -> ClusterDefinition {
    // Clone is local to this call; the original ClusterDefinition is never mutated.
    let mut updated = cluster_def.clone();
    for node in updated.nodes.values_mut() {
        for binding in node.parameter_bindings.values_mut() {
            if let ParameterBinding::Exposed { parent_param } = binding {
                if let Some(ParameterBinding::Literal { value }) = bindings.get(parent_param) {
                    *binding = ParameterBinding::Literal {
                        value: value.clone(),
                    };
                }
            }
        }
    }
    updated
}

/// Resolves parameter bindings for a primitive node using the parent's resolved parameters.
/// - Literal bindings use their value directly
/// - Exposed bindings look up the value from resolved_params
fn resolve_bindings_with_context(
    bindings: &HashMap<String, ParameterBinding>,
    resolved_params: &HashMap<String, ParameterValue>,
) -> Result<HashMap<String, ParameterValue>, ExpandError> {
    let mut result = HashMap::new();
    for (name, binding) in bindings {
        match binding {
            ParameterBinding::Literal { value } => {
                result.insert(name.clone(), value.clone());
            }
            ParameterBinding::Exposed { parent_param } => {
                // Look up the value from parent's resolved parameters
                if let Some(value) = resolved_params.get(parent_param) {
                    result.insert(name.clone(), value.clone());
                }
                // If not found, validation will catch it separately
            }
        }
    }
    Ok(result)
}

/// Builds resolved parameter values for a nested cluster instantiation.
/// - Literal bindings use their value directly
/// - Exposed bindings look up the value from the current resolved_params
fn build_resolved_params(
    bindings: &HashMap<String, ParameterBinding>,
    resolved_params: &HashMap<String, ParameterValue>,
) -> HashMap<String, ParameterValue> {
    let mut result = HashMap::new();
    for (name, binding) in bindings {
        match binding {
            ParameterBinding::Literal { value } => {
                result.insert(name.clone(), value.clone());
            }
            ParameterBinding::Exposed { parent_param } => {
                if let Some(value) = resolved_params.get(parent_param) {
                    result.insert(name.clone(), value.clone());
                }
            }
        }
    }
    result
}

fn map_boundary_outputs(
    outputs: &[OutputPortSpec],
    mapping: &HashMap<NodeId, String>,
) -> Vec<OutputPortSpec> {
    outputs
        .iter()
        .map(|o| OutputPortSpec {
            name: o.name.clone(),
            maps_to: OutputRef {
                node_id: mapping
                    .get(&o.maps_to.node_id)
                    .cloned()
                    .unwrap_or_else(|| o.maps_to.node_id.clone()),
                port_name: o.maps_to.port_name.clone(),
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    struct TestLoader {
        clusters: HashMap<(String, Version), ClusterDefinition>,
    }

    impl TestLoader {
        fn new() -> Self {
            Self {
                clusters: HashMap::new(),
            }
        }

        fn with_cluster(mut self, def: ClusterDefinition) -> Self {
            self.clusters
                .insert((def.id.clone(), def.version.clone()), def);
            self
        }
    }

    impl ClusterLoader for TestLoader {
        fn load(&self, id: &str, version: &Version) -> Option<ClusterDefinition> {
            self.clusters
                .get(&(id.to_string(), version.clone()))
                .cloned()
        }
    }

    fn empty_parameters() -> Vec<ParameterSpec> {
        Vec::new()
    }

    fn meta(kind: PrimitiveKind, outputs: &[(&str, ValueType)]) -> PrimitiveMetadata {
        let outputs_map = outputs
            .iter()
            .map(|(name, ty)| {
                (
                    name.to_string(),
                    OutputMetadata {
                        value_type: ty.clone(),
                        cardinality: Cardinality::Single,
                    },
                )
            })
            .collect();
        PrimitiveMetadata {
            kind,
            inputs: Vec::new(),
            outputs: outputs_map,
        }
    }

    #[derive(Default)]
    struct TestCatalog {
        metadata: HashMap<(String, Version), PrimitiveMetadata>,
    }

    impl TestCatalog {
        fn with_metadata(mut self, id: &str, version: &str, meta: PrimitiveMetadata) -> Self {
            self.metadata
                .insert((id.to_string(), version.to_string()), meta);
            self
        }
    }

    impl PrimitiveCatalog for TestCatalog {
        fn get(&self, id: &str, version: &Version) -> Option<PrimitiveMetadata> {
            self.metadata
                .get(&(id.to_string(), version.clone()))
                .cloned()
        }
    }

    #[test]
    fn expands_primitive_cluster() {
        let mut nodes = HashMap::new();
        nodes.insert(
            "p1".to_string(),
            NodeInstance {
                id: "p1".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "prim".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        let cluster = ClusterDefinition {
            id: "root".to_string(),
            version: "v1".to_string(),
            nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            parameters: empty_parameters(),
            declared_signature: None,
        };

        let loader = TestLoader::new();
        let catalog = TestCatalog::default();
        let expanded = expand(&cluster, &loader, &catalog).unwrap();

        assert_eq!(expanded.nodes.len(), 1);
        assert!(expanded.edges.is_empty());

        let node = expanded.nodes.values().next().unwrap();
        assert_eq!(
            node.authoring_path,
            vec![("root".to_string(), "p1".to_string())]
        );
        assert_eq!(node.implementation.impl_id, "prim");
    }

    #[test]
    fn expands_nested_cluster_and_rewires_inputs() {
        let mut inner_nodes = HashMap::new();
        inner_nodes.insert(
            "leaf".to_string(),
            NodeInstance {
                id: "leaf".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "leaf_prim".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        let inner = ClusterDefinition {
            id: "inner".to_string(),
            version: "v1".to_string(),
            nodes: inner_nodes,
            edges: vec![Edge {
                from: OutputRef {
                    node_id: "in".to_string(),
                    port_name: "out".to_string(),
                },
                to: InputRef {
                    node_id: "leaf".to_string(),
                    port_name: "input".to_string(),
                },
            }],
            input_ports: vec![InputPortSpec {
                name: "in_port".to_string(),
                maps_to: GraphInputPlaceholder {
                    name: "in".to_string(),
                    ty: ValueType::Number,
                    required: true,
                },
            }],
            output_ports: vec![OutputPortSpec {
                name: "out_port".to_string(),
                maps_to: OutputRef {
                    node_id: "leaf".to_string(),
                    port_name: "out".to_string(),
                },
            }],
            parameters: empty_parameters(),
            declared_signature: None,
        };

        let mut outer_nodes = HashMap::new();
        outer_nodes.insert(
            "src".to_string(),
            NodeInstance {
                id: "src".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "src_prim".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );
        outer_nodes.insert(
            "nested".to_string(),
            NodeInstance {
                id: "nested".to_string(),
                kind: NodeKind::Cluster {
                    cluster_id: "inner".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );
        outer_nodes.insert(
            "sink".to_string(),
            NodeInstance {
                id: "sink".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "sink_prim".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        let outer = ClusterDefinition {
            id: "outer".to_string(),
            version: "v1".to_string(),
            nodes: outer_nodes,
            edges: vec![
                Edge {
                    from: OutputRef {
                        node_id: "src".to_string(),
                        port_name: "emit".to_string(),
                    },
                    to: InputRef {
                        node_id: "nested".to_string(),
                        port_name: "in_port".to_string(),
                    },
                },
                Edge {
                    from: OutputRef {
                        node_id: "nested".to_string(),
                        port_name: "out_port".to_string(),
                    },
                    to: InputRef {
                        node_id: "sink".to_string(),
                        port_name: "input".to_string(),
                    },
                },
            ],
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            parameters: empty_parameters(),
            declared_signature: None,
        };

        let loader = TestLoader::new().with_cluster(inner);
        let catalog = TestCatalog::default();
        let expanded = expand(&outer, &loader, &catalog).unwrap();

        assert_eq!(expanded.nodes.len(), 3);

        let mut external_edges = Vec::new();
        let mut node_edges = Vec::new();
        for edge in expanded.edges {
            match (&edge.from, &edge.to) {
                (ExpandedEndpoint::ExternalInput { .. }, _)
                | (_, ExpandedEndpoint::ExternalInput { .. }) => external_edges.push(edge),
                _ => node_edges.push(edge),
            }
        }

        assert!(external_edges.is_empty());
        assert_eq!(node_edges.len(), 2);
    }

    #[test]
    fn infers_source_like_signature() {
        let mut nodes = HashMap::new();
        nodes.insert(
            "s".to_string(),
            NodeInstance {
                id: "s".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "source".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        let cluster = ClusterDefinition {
            id: "root".to_string(),
            version: "v1".to_string(),
            nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: vec![OutputPortSpec {
                name: "out".to_string(),
                maps_to: OutputRef {
                    node_id: "s".to_string(),
                    port_name: "value".to_string(),
                },
            }],
            parameters: empty_parameters(),
            declared_signature: None,
        };

        let loader = TestLoader::new();
        let catalog = TestCatalog::default();
        let expanded = expand(&cluster, &loader, &catalog).unwrap();

        let catalog = TestCatalog::default().with_metadata(
            "source",
            "v1",
            meta(PrimitiveKind::Source, &[("value", ValueType::Number)]),
        );

        let sig = infer_signature(&expanded, &catalog).unwrap();

        assert_eq!(sig.kind, BoundaryKind::SourceLike);
        assert!(sig.is_origin);
        assert_eq!(sig.outputs.len(), 1);
        assert_eq!(sig.outputs[0].wireable, true);
        assert_eq!(sig.outputs[0].ty, ValueType::Number);
    }

    #[test]
    fn infers_action_like_signature_when_outputs_not_wireable() {
        let mut nodes = HashMap::new();
        nodes.insert(
            "a".to_string(),
            NodeInstance {
                id: "a".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "action".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        let cluster = ClusterDefinition {
            id: "root".to_string(),
            version: "v1".to_string(),
            nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: vec![OutputPortSpec {
                name: "outcome".to_string(),
                maps_to: OutputRef {
                    node_id: "a".to_string(),
                    port_name: "outcome".to_string(),
                },
            }],
            parameters: empty_parameters(),
            declared_signature: None,
        };

        let loader = TestLoader::new();
        let catalog = TestCatalog::default();
        let expanded = expand(&cluster, &loader, &catalog).unwrap();

        let catalog = TestCatalog::default().with_metadata(
            "action",
            "v1",
            meta(PrimitiveKind::Action, &[("outcome", ValueType::Event)]),
        );

        let sig = infer_signature(&expanded, &catalog).unwrap();

        assert_eq!(sig.kind, BoundaryKind::ActionLike);
        assert!(sig.has_side_effects);
        assert_eq!(sig.outputs[0].wireable, false);
    }

    #[test]
    fn infers_trigger_like_signature_with_event_output() {
        let mut nodes = HashMap::new();
        nodes.insert(
            "t".to_string(),
            NodeInstance {
                id: "t".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "trigger".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        let cluster = ClusterDefinition {
            id: "root".to_string(),
            version: "v1".to_string(),
            nodes,
            edges: Vec::new(),
            input_ports: vec![InputPortSpec {
                name: "in".to_string(),
                maps_to: GraphInputPlaceholder {
                    name: "in".to_string(),
                    ty: ValueType::Number,
                    required: true,
                },
            }],
            output_ports: vec![OutputPortSpec {
                name: "out".to_string(),
                maps_to: OutputRef {
                    node_id: "t".to_string(),
                    port_name: "emitted".to_string(),
                },
            }],
            parameters: empty_parameters(),
            declared_signature: None,
        };

        let loader = TestLoader::new();
        let catalog = TestCatalog::default();
        let expanded = expand(&cluster, &loader, &catalog).unwrap();

        let catalog = TestCatalog::default().with_metadata(
            "trigger",
            "v1",
            meta(PrimitiveKind::Trigger, &[("emitted", ValueType::Event)]),
        );

        let sig = infer_signature(&expanded, &catalog).unwrap();

        assert_eq!(sig.kind, BoundaryKind::TriggerLike);
        assert!(!sig.is_origin);
        assert_eq!(sig.outputs[0].wireable, true);
    }

    #[test]
    fn infers_compute_like_signature() {
        let mut nodes = HashMap::new();
        nodes.insert(
            "c".to_string(),
            NodeInstance {
                id: "c".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "compute".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        let cluster = ClusterDefinition {
            id: "root".to_string(),
            version: "v1".to_string(),
            nodes,
            edges: Vec::new(),
            input_ports: vec![InputPortSpec {
                name: "in".to_string(),
                maps_to: GraphInputPlaceholder {
                    name: "in".to_string(),
                    ty: ValueType::Number,
                    required: true,
                },
            }],
            output_ports: vec![OutputPortSpec {
                name: "out".to_string(),
                maps_to: OutputRef {
                    node_id: "c".to_string(),
                    port_name: "value".to_string(),
                },
            }],
            parameters: empty_parameters(),
            declared_signature: None,
        };

        let loader = TestLoader::new();
        let catalog = TestCatalog::default();
        let expanded = expand(&cluster, &loader, &catalog).unwrap();

        let catalog = TestCatalog::default().with_metadata(
            "compute",
            "v1",
            meta(PrimitiveKind::Compute, &[("value", ValueType::Number)]),
        );

        let sig = infer_signature(&expanded, &catalog).unwrap();

        assert_eq!(sig.kind, BoundaryKind::ComputeLike);
        assert!(!sig.is_origin);
        assert!(!sig.has_side_effects);
    }

    /// F.1 invariant test: Input ports must never be wireable (CLUSTER_SPEC.md §3.2)
    #[test]
    fn input_ports_are_never_wireable() {
        // Setup: Create a cluster with input ports
        let mut nodes = HashMap::new();
        nodes.insert(
            "c".to_string(),
            NodeInstance {
                id: "c".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "compute".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        let cluster = ClusterDefinition {
            id: "root".to_string(),
            version: "v1".to_string(),
            nodes,
            edges: Vec::new(),
            input_ports: vec![
                InputPortSpec {
                    name: "input_a".to_string(),
                    maps_to: GraphInputPlaceholder {
                        name: "input_a".to_string(),
                        ty: ValueType::Number,
                        required: true,
                    },
                },
                InputPortSpec {
                    name: "input_b".to_string(),
                    maps_to: GraphInputPlaceholder {
                        name: "input_b".to_string(),
                        ty: ValueType::Series,
                        required: false,
                    },
                },
            ],
            output_ports: vec![OutputPortSpec {
                name: "out".to_string(),
                maps_to: OutputRef {
                    node_id: "c".to_string(),
                    port_name: "value".to_string(),
                },
            }],
            parameters: empty_parameters(),
            declared_signature: None,
        };

        let loader = TestLoader::new();
        let catalog = TestCatalog::default();
        let expanded = expand(&cluster, &loader, &catalog).unwrap();

        let catalog = TestCatalog::default().with_metadata(
            "compute",
            "v1",
            meta(PrimitiveKind::Compute, &[("value", ValueType::Number)]),
        );

        let sig = infer_signature(&expanded, &catalog).unwrap();

        // F.1: Input ports must never be wireable
        assert!(
            sig.inputs.iter().all(|p| !p.wireable),
            "Invariant F.1 violated: Input ports must never be wireable"
        );

        // Verify we actually tested multiple inputs
        assert_eq!(
            sig.inputs.len(),
            2,
            "Test should verify multiple input ports"
        );
    }

    /// E.3 invariant test: ExternalInput must not appear as edge sink after expansion
    #[test]
    #[should_panic(expected = "Invariant E.3 violated")]
    fn external_input_cannot_be_edge_sink() {
        // Setup: Create a cluster with an edge targeting a non-existent node
        // This will cause ExternalInput to appear as edge sink, violating E.3
        let mut nodes = HashMap::new();
        nodes.insert(
            "source_node".to_string(),
            NodeInstance {
                id: "source_node".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "source".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        // Edge targets "nonexistent_node" which doesn't exist in nodes
        // This will resolve to ExternalInput as the sink, violating E.3
        let cluster = ClusterDefinition {
            id: "malformed".to_string(),
            version: "v1".to_string(),
            nodes,
            edges: vec![Edge {
                from: OutputRef {
                    node_id: "source_node".to_string(),
                    port_name: "out".to_string(),
                },
                to: InputRef {
                    node_id: "nonexistent_node".to_string(),
                    port_name: "in".to_string(),
                },
            }],
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            parameters: empty_parameters(),
            declared_signature: None,
        };

        let loader = TestLoader::new();
        let catalog = TestCatalog::default();
        // This should panic due to E.3 assertion
        let _ = expand(&cluster, &loader, &catalog);
    }

    /// D.11 invariant test: Declared wireability cannot exceed inferred wireability
    #[test]
    fn declared_wireability_cannot_exceed_inferred() {
        // Setup: Create cluster with Action output (inferred wireable: false)
        let mut nodes = HashMap::new();
        nodes.insert(
            "action_node".to_string(),
            NodeInstance {
                id: "action_node".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "action".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        let cluster = ClusterDefinition {
            id: "root".to_string(),
            version: "v1".to_string(),
            nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: vec![OutputPortSpec {
                name: "outcome".to_string(),
                maps_to: OutputRef {
                    node_id: "action_node".to_string(),
                    port_name: "outcome".to_string(),
                },
            }],
            parameters: empty_parameters(),
            declared_signature: Some(Signature {
                kind: BoundaryKind::ActionLike,
                inputs: Vec::new(),
                outputs: vec![PortSpec {
                    name: "outcome".to_string(),
                    ty: ValueType::Event,
                    cardinality: Cardinality::Single,
                    wireable: true, // D.11 violation: cannot grant wireability
                }],
                has_side_effects: true,
                is_origin: false,
            }),
        };

        let loader = TestLoader::new();
        let catalog = TestCatalog::default().with_metadata(
            "action",
            "v1",
            meta(PrimitiveKind::Action, &[("outcome", ValueType::Event)]),
        );

        let result = expand(&cluster, &loader, &catalog);

        assert!(
            matches!(
                result,
                Err(ExpandError::DeclaredSignatureInvalid(
                    ClusterValidationError::WireabilityExceedsInferred { ref port_name }
                )) if port_name == "outcome"
            ),
            "D.11: Declared wireability exceeding inferred must be rejected in production path"
        );
    }

    #[test]
    fn validate_declared_signature_rejects_wireability_grant() {
        let inferred = Signature {
            kind: BoundaryKind::ActionLike,
            inputs: Vec::new(),
            outputs: vec![PortSpec {
                name: "outcome".to_string(),
                ty: ValueType::Event,
                cardinality: Cardinality::Single,
                wireable: false,
            }],
            has_side_effects: true,
            is_origin: false,
        };

        let declared = Signature {
            kind: BoundaryKind::ActionLike,
            inputs: Vec::new(),
            outputs: vec![PortSpec {
                name: "outcome".to_string(),
                ty: ValueType::Event,
                cardinality: Cardinality::Single,
                wireable: true,
            }],
            has_side_effects: true,
            is_origin: false,
        };

        let result = validate_declared_signature(&declared, &inferred);

        assert!(matches!(
            result,
            Err(ClusterValidationError::WireabilityExceedsInferred { port_name })
                if port_name == "outcome"
        ));
    }

    #[test]
    fn duplicate_input_ports_rejected() {
        let mut nodes = HashMap::new();
        nodes.insert(
            "impl".to_string(),
            NodeInstance {
                id: "impl".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "compute".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        let cluster = ClusterDefinition {
            id: "dup_inputs".to_string(),
            version: "v1".to_string(),
            nodes,
            edges: Vec::new(),
            input_ports: vec![
                InputPortSpec {
                    name: "in".to_string(),
                    maps_to: GraphInputPlaceholder {
                        name: "in_a".to_string(),
                        ty: ValueType::Number,
                        required: true,
                    },
                },
                InputPortSpec {
                    name: "in".to_string(),
                    maps_to: GraphInputPlaceholder {
                        name: "in_b".to_string(),
                        ty: ValueType::Number,
                        required: true,
                    },
                },
            ],
            output_ports: Vec::new(),
            parameters: empty_parameters(),
            declared_signature: None,
        };

        let loader = TestLoader::new();
        let catalog = TestCatalog::default();
        let result = expand(&cluster, &loader, &catalog);

        assert!(matches!(
            result,
            Err(ExpandError::DuplicateInputPort { name }) if name == "in"
        ));
    }

    #[test]
    fn duplicate_output_ports_rejected() {
        let mut nodes = HashMap::new();
        nodes.insert(
            "impl".to_string(),
            NodeInstance {
                id: "impl".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "compute".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        let cluster = ClusterDefinition {
            id: "dup_outputs".to_string(),
            version: "v1".to_string(),
            nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: vec![
                OutputPortSpec {
                    name: "out".to_string(),
                    maps_to: OutputRef {
                        node_id: "impl".to_string(),
                        port_name: "value".to_string(),
                    },
                },
                OutputPortSpec {
                    name: "out".to_string(),
                    maps_to: OutputRef {
                        node_id: "impl".to_string(),
                        port_name: "value".to_string(),
                    },
                },
            ],
            parameters: empty_parameters(),
            declared_signature: None,
        };

        let loader = TestLoader::new();
        let catalog = TestCatalog::default();
        let result = expand(&cluster, &loader, &catalog);

        assert!(matches!(
            result,
            Err(ExpandError::DuplicateOutputPort { name }) if name == "out"
        ));
    }

    #[test]
    fn duplicate_parameters_rejected() {
        let mut nodes = HashMap::new();
        nodes.insert(
            "impl".to_string(),
            NodeInstance {
                id: "impl".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "compute".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        let cluster = ClusterDefinition {
            id: "dup_params".to_string(),
            version: "v1".to_string(),
            nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            parameters: vec![
                ParameterSpec {
                    name: "p".to_string(),
                    ty: ParameterType::Number,
                    default: None,
                    required: true,
                },
                ParameterSpec {
                    name: "p".to_string(),
                    ty: ParameterType::Number,
                    default: None,
                    required: true,
                },
            ],
            declared_signature: None,
        };

        let loader = TestLoader::new();
        let catalog = TestCatalog::default();
        let result = expand(&cluster, &loader, &catalog);

        assert!(matches!(
            result,
            Err(ExpandError::DuplicateParameter { name }) if name == "p"
        ));
    }

    #[test]
    fn parameter_default_type_mismatch_rejected() {
        let mut nodes = HashMap::new();
        nodes.insert(
            "impl".to_string(),
            NodeInstance {
                id: "impl".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "compute".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        let cluster = ClusterDefinition {
            id: "bad_default".to_string(),
            version: "v1".to_string(),
            nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            parameters: vec![ParameterSpec {
                name: "flag".to_string(),
                ty: ParameterType::Bool,
                default: Some(ParameterValue::Number(1.0)),
                required: false,
            }],
            declared_signature: None,
        };

        let loader = TestLoader::new();
        let catalog = TestCatalog::default();
        let result = expand(&cluster, &loader, &catalog);

        assert!(matches!(
            result,
            Err(ExpandError::ParameterDefaultTypeMismatch {
                name,
                expected,
                got
            }) if name == "flag" && expected == ParameterType::Bool && got == ParameterType::Number
        ));
    }

    /// I.3: Required parameter with no default and no binding must be rejected
    #[test]
    fn required_parameter_missing_rejected() {
        // Inner cluster has a required parameter with no default
        let mut inner_nodes = HashMap::new();
        inner_nodes.insert(
            "leaf".to_string(),
            NodeInstance {
                id: "leaf".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "prim".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        let inner = ClusterDefinition {
            id: "inner".to_string(),
            version: "v1".to_string(),
            nodes: inner_nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            parameters: vec![ParameterSpec {
                name: "required_param".to_string(),
                ty: ParameterType::Number,
                default: None, // No default
                required: true,
            }],
            declared_signature: None,
        };

        // Outer cluster instantiates inner without providing the required binding
        let mut outer_nodes = HashMap::new();
        outer_nodes.insert(
            "nested".to_string(),
            NodeInstance {
                id: "nested".to_string(),
                kind: NodeKind::Cluster {
                    cluster_id: "inner".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(), // No binding provided
            },
        );

        let outer = ClusterDefinition {
            id: "outer".to_string(),
            version: "v1".to_string(),
            nodes: outer_nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            parameters: Vec::new(),
            declared_signature: None,
        };

        let loader = TestLoader::new().with_cluster(inner);
        let catalog = TestCatalog::default();
        let result = expand(&outer, &loader, &catalog);

        assert!(matches!(
            result,
            Err(ExpandError::MissingRequiredParameter {
                cluster_id,
                parameter
            }) if cluster_id == "inner" && parameter == "required_param"
        ));
    }

    /// I.4: Literal binding with wrong type must be rejected
    #[test]
    fn parameter_binding_type_mismatch_rejected() {
        // Inner cluster expects a Number parameter
        let mut inner_nodes = HashMap::new();
        inner_nodes.insert(
            "leaf".to_string(),
            NodeInstance {
                id: "leaf".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "prim".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        let inner = ClusterDefinition {
            id: "inner".to_string(),
            version: "v1".to_string(),
            nodes: inner_nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            parameters: vec![ParameterSpec {
                name: "num_param".to_string(),
                ty: ParameterType::Number,
                default: None,
                required: true,
            }],
            declared_signature: None,
        };

        // Outer cluster provides a Bool when Number is expected
        let mut outer_nodes = HashMap::new();
        outer_nodes.insert(
            "nested".to_string(),
            NodeInstance {
                id: "nested".to_string(),
                kind: NodeKind::Cluster {
                    cluster_id: "inner".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::from([(
                    "num_param".to_string(),
                    ParameterBinding::Literal {
                        value: ParameterValue::Bool(true), // Wrong type!
                    },
                )]),
            },
        );

        let outer = ClusterDefinition {
            id: "outer".to_string(),
            version: "v1".to_string(),
            nodes: outer_nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            parameters: Vec::new(),
            declared_signature: None,
        };

        let loader = TestLoader::new().with_cluster(inner);
        let catalog = TestCatalog::default();
        let result = expand(&outer, &loader, &catalog);

        assert!(matches!(
            result,
            Err(ExpandError::ParameterBindingTypeMismatch {
                cluster_id,
                parameter,
                expected,
                got
            }) if cluster_id == "inner"
                && parameter == "num_param"
                && expected == ParameterType::Number
                && got == ParameterType::Bool
        ));
    }

    /// I.5: Exposed binding referencing nonexistent parent parameter must be rejected
    #[test]
    fn exposed_parameter_not_in_parent_rejected() {
        // Inner cluster has a parameter that will be exposed
        let mut inner_nodes = HashMap::new();
        inner_nodes.insert(
            "leaf".to_string(),
            NodeInstance {
                id: "leaf".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "prim".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        let inner = ClusterDefinition {
            id: "inner".to_string(),
            version: "v1".to_string(),
            nodes: inner_nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            parameters: vec![ParameterSpec {
                name: "inner_param".to_string(),
                ty: ParameterType::Number,
                default: None,
                required: true,
            }],
            declared_signature: None,
        };

        // Outer cluster exposes to a parent parameter that doesn't exist
        let mut outer_nodes = HashMap::new();
        outer_nodes.insert(
            "nested".to_string(),
            NodeInstance {
                id: "nested".to_string(),
                kind: NodeKind::Cluster {
                    cluster_id: "inner".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::from([(
                    "inner_param".to_string(),
                    ParameterBinding::Exposed {
                        parent_param: "nonexistent_param".to_string(), // Doesn't exist!
                    },
                )]),
            },
        );

        let outer = ClusterDefinition {
            id: "outer".to_string(),
            version: "v1".to_string(),
            nodes: outer_nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            parameters: Vec::new(), // No parameters defined!
            declared_signature: None,
        };

        let loader = TestLoader::new().with_cluster(inner);
        let catalog = TestCatalog::default();
        let result = expand(&outer, &loader, &catalog);

        assert!(matches!(
            result,
            Err(ExpandError::ExposedParameterNotFound {
                cluster_id,
                parameter,
                referenced
            }) if cluster_id == "inner"
                && parameter == "inner_param"
                && referenced == "nonexistent_param"
        ));
    }

    /// I.4: Exposed binding with incompatible type must be rejected
    #[test]
    fn exposed_parameter_type_mismatch_rejected() {
        // Inner cluster expects a Number parameter
        let mut inner_nodes = HashMap::new();
        inner_nodes.insert(
            "leaf".to_string(),
            NodeInstance {
                id: "leaf".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "prim".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::new(),
            },
        );

        let inner = ClusterDefinition {
            id: "inner".to_string(),
            version: "v1".to_string(),
            nodes: inner_nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            parameters: vec![ParameterSpec {
                name: "threshold".to_string(),
                ty: ParameterType::Number, // Expects Number
                default: None,
                required: true,
            }],
            declared_signature: None,
        };

        // Outer cluster has an Int parameter but inner expects Number
        let mut outer_nodes = HashMap::new();
        outer_nodes.insert(
            "nested".to_string(),
            NodeInstance {
                id: "nested".to_string(),
                kind: NodeKind::Cluster {
                    cluster_id: "inner".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::from([(
                    "threshold".to_string(),
                    ParameterBinding::Exposed {
                        parent_param: "count".to_string(), // Exposes Int as Number
                    },
                )]),
            },
        );

        let outer = ClusterDefinition {
            id: "outer".to_string(),
            version: "v1".to_string(),
            nodes: outer_nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            parameters: vec![ParameterSpec {
                name: "count".to_string(),
                ty: ParameterType::Int, // Int, not Number!
                default: None,
                required: true,
            }],
            declared_signature: None,
        };

        let loader = TestLoader::new().with_cluster(inner);
        let catalog = TestCatalog::default();
        let result = expand(&outer, &loader, &catalog);

        assert!(matches!(
            result,
            Err(ExpandError::ExposedParameterTypeMismatch {
                cluster_id,
                parameter,
                expected,
                got
            }) if cluster_id == "inner"
                && parameter == "threshold"
                && expected == ParameterType::Number
                && got == ParameterType::Int
        ));
    }

    /// PR-B: Exposed binding propagates through nested clusters to leaf primitive
    #[test]
    fn exposed_binding_propagates_to_leaf_primitive() {
        // Structure:
        //   Parent cluster (param "threshold": Number)
        //     └── Nested cluster "middle" (param "inner_t" exposed to "threshold")
        //           └── Leaf impl (param "leaf_t" exposed to "inner_t")
        //
        // Instantiate parent with threshold = Literal(7.0)
        // Assert: leaf impl receives parameters = { "leaf_t": Number(7.0) }

        // Innermost cluster with a leaf primitive that exposes a parameter
        let mut inner_nodes = HashMap::new();
        inner_nodes.insert(
            "leaf".to_string(),
            NodeInstance {
                id: "leaf".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "prim".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::from([(
                    "leaf_t".to_string(),
                    ParameterBinding::Exposed {
                        parent_param: "inner_t".to_string(),
                    },
                )]),
            },
        );

        let inner = ClusterDefinition {
            id: "inner".to_string(),
            version: "v1".to_string(),
            nodes: inner_nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            parameters: vec![ParameterSpec {
                name: "inner_t".to_string(),
                ty: ParameterType::Number,
                default: None,
                required: true,
            }],
            declared_signature: None,
        };

        // Middle cluster that instantiates inner with an exposed binding
        let mut middle_nodes = HashMap::new();
        middle_nodes.insert(
            "nested_inner".to_string(),
            NodeInstance {
                id: "nested_inner".to_string(),
                kind: NodeKind::Cluster {
                    cluster_id: "inner".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::from([(
                    "inner_t".to_string(),
                    ParameterBinding::Exposed {
                        parent_param: "middle_t".to_string(),
                    },
                )]),
            },
        );

        let middle = ClusterDefinition {
            id: "middle".to_string(),
            version: "v1".to_string(),
            nodes: middle_nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            parameters: vec![ParameterSpec {
                name: "middle_t".to_string(),
                ty: ParameterType::Number,
                default: None,
                required: true,
            }],
            declared_signature: None,
        };

        // Outer cluster that provides a literal binding
        let mut outer_nodes = HashMap::new();
        outer_nodes.insert(
            "nested_middle".to_string(),
            NodeInstance {
                id: "nested_middle".to_string(),
                kind: NodeKind::Cluster {
                    cluster_id: "middle".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::from([(
                    "middle_t".to_string(),
                    ParameterBinding::Literal {
                        value: ParameterValue::Number(7.0),
                    },
                )]),
            },
        );

        let outer = ClusterDefinition {
            id: "outer".to_string(),
            version: "v1".to_string(),
            nodes: outer_nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            parameters: Vec::new(),
            declared_signature: None,
        };

        let loader = TestLoader::new().with_cluster(inner).with_cluster(middle);
        let catalog = TestCatalog::default();
        let expanded = expand(&outer, &loader, &catalog).unwrap();

        // Verify the leaf primitive received the propagated value
        assert_eq!(expanded.nodes.len(), 1);
        let leaf_node = expanded.nodes.values().next().unwrap();
        assert_eq!(leaf_node.implementation.impl_id, "prim");
        assert_eq!(
            leaf_node.parameters.get("leaf_t"),
            Some(&ParameterValue::Number(7.0))
        );
    }

    /// PR-B: Unresolved Exposed binding on primitive node is rejected
    #[test]
    fn unresolved_exposed_binding_rejected() {
        // Structure:
        //   Parent cluster (NO params)
        //     └── Leaf impl (param "x" exposed to "nonexistent")
        //
        // Expand should fail with UnresolvedExposedBinding

        let mut nodes = HashMap::new();
        nodes.insert(
            "leaf".to_string(),
            NodeInstance {
                id: "leaf".to_string(),
                kind: NodeKind::Impl {
                    impl_id: "prim".to_string(),
                    version: "v1".to_string(),
                },
                parameter_bindings: HashMap::from([(
                    "x".to_string(),
                    ParameterBinding::Exposed {
                        parent_param: "nonexistent".to_string(),
                    },
                )]),
            },
        );

        let cluster = ClusterDefinition {
            id: "root".to_string(),
            version: "v1".to_string(),
            nodes,
            edges: Vec::new(),
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            parameters: Vec::new(), // No parameters!
            declared_signature: None,
        };

        let loader = TestLoader::new();
        let catalog = TestCatalog::default();
        let result = expand(&cluster, &loader, &catalog);

        assert!(matches!(
            result,
            Err(ExpandError::UnresolvedExposedBinding {
                node_id,
                parameter,
                referenced
            }) if node_id == "leaf"
                && parameter == "x"
                && referenced == "nonexistent"
        ));
    }
}
