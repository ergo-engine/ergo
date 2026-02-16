use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use ergo_adapter::{
    validate_action_adapter_composition, validate_source_adapter_composition, AdapterManifest,
    AdapterProvides,
};
use ergo_runtime::catalog::{
    build_core_catalog, core_registries, CorePrimitiveCatalog, CoreRegistries,
};
use ergo_runtime::cluster::{
    expand, BoundaryKind, Cardinality, ClusterDefinition, ClusterLoader, Edge,
    GraphInputPlaceholder, InputPortSpec, InputRef, NodeInstance, NodeKind, OutputPortSpec,
    OutputRef, ParameterBinding, ParameterSpec, ParameterType, ParameterValue, PortSpec,
    PrimitiveCatalog, Signature, ValueType, Version,
};
use ergo_runtime::common::ErrorInfo;
use ergo_runtime::runtime::{
    ExecutionContext, Registries, RuntimeError, RuntimeEvent, RuntimeValue,
};
use serde::Deserialize;

pub fn run_graph_command(graph_path: &Path, args: &[String]) -> Result<(), String> {
    let opts = parse_run_options(args)?;
    let root = parse_graph_file(graph_path)?;
    let clusters = load_cluster_tree(graph_path, &root, &opts.cluster_paths)?;
    let loader = PreloadedClusterLoader::new(clusters);

    let catalog = build_core_catalog();
    let registries = core_registries().map_err(|err| format!("core registries: {err:?}"))?;
    let expanded = expand(&root, &loader, &catalog)
        .map_err(|err| format!("graph expansion failed: {}", render_error_info(&err)))?;

    if let Some(adapter_path) = &opts.adapter_path {
        let adapter = parse_adapter_manifest(adapter_path)?;
        ergo_adapter::validate_adapter(&adapter)
            .map_err(|err| format!("adapter invalid: {}", render_error_info(&err)))?;
        let provides = AdapterProvides::from_manifest(&adapter);
        validate_adapter_composition(&expanded, &catalog, &registries, &provides)?;
    }

    let refs = Registries {
        sources: &registries.sources,
        computes: &registries.computes,
        triggers: &registries.triggers,
        actions: &registries.actions,
    };
    let ctx = ExecutionContext::default();
    let report = ergo_runtime::runtime::run(&expanded, &catalog, &refs, &ctx)
        .map_err(format_runtime_error)?;

    print_outputs(&report.outputs);
    Ok(())
}

#[derive(Debug, Default)]
struct RunGraphOptions {
    adapter_path: Option<PathBuf>,
    cluster_paths: Vec<PathBuf>,
}

fn parse_run_options(args: &[String]) -> Result<RunGraphOptions, String> {
    let mut options = RunGraphOptions::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--adapter" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--adapter requires a path".to_string())?;
                options.adapter_path = Some(PathBuf::from(value));
                i += 2;
            }
            "--cluster-path" | "--search-path" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| format!("{} requires a path", args[i]))?;
                options.cluster_paths.push(PathBuf::from(value));
                i += 2;
            }
            other => {
                return Err(format!(
                    "unknown run option '{other}'. expected --adapter, --cluster-path, or --search-path"
                ))
            }
        }
    }
    Ok(options)
}

fn parse_adapter_manifest(path: &Path) -> Result<AdapterManifest, String> {
    let data = fs::read_to_string(path)
        .map_err(|err| format!("read adapter manifest '{}': {err}", path.display()))?;
    let value = serde_yaml::from_str::<serde_json::Value>(&data)
        .map_err(|err| format!("parse adapter manifest '{}': {err}", path.display()))?;
    serde_json::from_value::<AdapterManifest>(value)
        .map_err(|err| format!("decode adapter manifest '{}': {err}", path.display()))
}

fn validate_adapter_composition(
    expanded: &ergo_runtime::cluster::ExpandedGraph,
    catalog: &CorePrimitiveCatalog,
    registries: &CoreRegistries,
    provides: &AdapterProvides,
) -> Result<(), String> {
    for (runtime_id, node) in &expanded.nodes {
        let meta = catalog
            .get(&node.implementation.impl_id, &node.implementation.version)
            .ok_or_else(|| {
                format!(
                    "missing catalog metadata for primitive '{}@{}'",
                    node.implementation.impl_id, node.implementation.version
                )
            })?;
        match meta.kind {
            ergo_runtime::cluster::PrimitiveKind::Source => {
                let source = registries
                    .sources
                    .get(&node.implementation.impl_id)
                    .ok_or_else(|| {
                        format!(
                            "source '{}' missing in core registry",
                            node.implementation.impl_id
                        )
                    })?;
                validate_source_adapter_composition(&source.manifest().requires, provides)
                    .map_err(|err| {
                        format!(
                            "source composition failed for node '{}': {}",
                            runtime_id,
                            render_error_info(&err)
                        )
                    })?;
            }
            ergo_runtime::cluster::PrimitiveKind::Action => {
                let action = registries
                    .actions
                    .get(&node.implementation.impl_id)
                    .ok_or_else(|| {
                        format!(
                            "action '{}' missing in core registry",
                            node.implementation.impl_id
                        )
                    })?;
                validate_action_adapter_composition(&action.manifest().effects, provides).map_err(
                    |err| {
                        format!(
                            "action composition failed for node '{}': {}",
                            runtime_id,
                            render_error_info(&err)
                        )
                    },
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn print_outputs(outputs: &HashMap<String, RuntimeValue>) {
    if outputs.is_empty() {
        println!("outputs: {{}}");
        return;
    }

    let mut keys: Vec<&String> = outputs.keys().collect();
    keys.sort();
    println!("outputs:");
    for key in keys {
        if let Some(value) = outputs.get(key) {
            println!("  {key}: {}", format_runtime_value(value));
        }
    }
}

fn format_runtime_value(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Number(n) => n.to_string(),
        RuntimeValue::Series(items) => format!("{items:?}"),
        RuntimeValue::Bool(b) => b.to_string(),
        RuntimeValue::String(s) => s.clone(),
        RuntimeValue::Event(RuntimeEvent::Trigger(event)) => format!("Trigger::{event:?}"),
        RuntimeValue::Event(RuntimeEvent::Action(outcome)) => format!("Action::{outcome:?}"),
    }
}

fn format_runtime_error(err: RuntimeError) -> String {
    match err {
        RuntimeError::Validation(err) => {
            format!("runtime validation failed: {}", render_error_info(&err))
        }
        RuntimeError::Execution(err) => {
            format!("runtime execution failed: {}", render_error_info(&err))
        }
    }
}

fn render_error_info(err: &impl ErrorInfo) -> String {
    let mut msg = format!("[{}] {}", err.rule_id(), err.summary());
    if let Some(path) = err.path() {
        msg.push_str(&format!(" (path: {path})"));
    }
    if let Some(fix) = err.fix() {
        msg.push_str(&format!("; fix: {fix}"));
    }
    msg
}

#[derive(Clone)]
struct PreloadedClusterLoader {
    clusters: HashMap<(String, Version), ClusterDefinition>,
}

impl PreloadedClusterLoader {
    fn new(clusters: HashMap<(String, Version), ClusterDefinition>) -> Self {
        Self { clusters }
    }
}

impl ClusterLoader for PreloadedClusterLoader {
    fn load(&self, id: &str, version: &Version) -> Option<ClusterDefinition> {
        self.clusters
            .get(&(id.to_string(), version.clone()))
            .cloned()
    }
}

fn load_cluster_tree(
    root_path: &Path,
    root: &ClusterDefinition,
    search_paths: &[PathBuf],
) -> Result<HashMap<(String, Version), ClusterDefinition>, String> {
    let mut builder = ClusterTreeBuilder {
        clusters: HashMap::new(),
        cluster_sources: HashMap::new(),
        visiting_paths: HashSet::new(),
        visiting_keys: HashSet::new(),
        search_paths: search_paths.to_vec(),
    };
    builder.visit(root_path, root.clone())?;
    Ok(builder.clusters)
}

struct ClusterTreeBuilder {
    clusters: HashMap<(String, Version), ClusterDefinition>,
    cluster_sources: HashMap<(String, Version), PathBuf>,
    visiting_paths: HashSet<PathBuf>,
    visiting_keys: HashSet<(String, Version)>,
    search_paths: Vec<PathBuf>,
}

impl ClusterTreeBuilder {
    fn visit(&mut self, path: &Path, def: ClusterDefinition) -> Result<(), String> {
        let canonical = canonicalize_or_self(path);
        let cluster_key = (def.id.clone(), def.version.clone());

        if let Some(existing_path) = self.cluster_sources.get(&cluster_key) {
            if existing_path != &canonical {
                return Err(format!(
                    "cluster '{}@{}' is defined by multiple files: '{}' and '{}'",
                    def.id,
                    def.version,
                    existing_path.display(),
                    canonical.display()
                ));
            }
        } else {
            self.cluster_sources
                .insert(cluster_key.clone(), canonical.clone());
        }

        if !self.visiting_paths.insert(canonical.clone()) {
            return Err(format!(
                "circular cluster reference detected at '{}'",
                path.display()
            ));
        }
        if !self.visiting_keys.insert(cluster_key.clone()) {
            return Err(format!(
                "circular cluster reference detected for '{}@{}' at '{}'",
                def.id,
                def.version,
                path.display()
            ));
        }

        self.clusters
            .entry(cluster_key.clone())
            .or_insert_with(|| def.clone());

        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        for node in def.nodes.values() {
            let NodeKind::Cluster {
                cluster_id,
                version,
            } = &node.kind
            else {
                continue;
            };
            let nested_key = (cluster_id.clone(), version.clone());

            if self.visiting_keys.contains(&nested_key) {
                return Err(format!(
                    "circular cluster reference detected for '{}@{}' referenced by node '{}' in '{}'",
                    cluster_id,
                    version,
                    node.id,
                    path.display()
                ));
            }

            let cluster_path = resolve_cluster_path(base_dir, cluster_id, &self.search_paths)
                .ok_or_else(|| {
                    format!(
                        "missing cluster file for '{}@{}' referenced by node '{}' in '{}'",
                        cluster_id,
                        version,
                        node.id,
                        path.display()
                    )
                })?;
            let canonical_nested_path = canonicalize_or_self(&cluster_path);

            if let Some(existing_path) = self.cluster_sources.get(&nested_key) {
                if existing_path != &canonical_nested_path {
                    return Err(format!(
                        "cluster '{}@{}' is defined by multiple files: '{}' and '{}'",
                        cluster_id,
                        version,
                        existing_path.display(),
                        canonical_nested_path.display()
                    ));
                }
            }

            if self.clusters.contains_key(&nested_key) {
                continue;
            }

            let nested = parse_graph_file(&cluster_path).map_err(|err| {
                format!(
                    "failed parsing nested cluster '{}@{}' at '{}': {}",
                    cluster_id,
                    version,
                    cluster_path.display(),
                    err
                )
            })?;

            if nested.id != *cluster_id {
                return Err(format!(
                    "cluster id mismatch in '{}': expected '{}', found '{}'",
                    cluster_path.display(),
                    cluster_id,
                    nested.id
                ));
            }

            if nested.version != *version {
                return Err(format!(
                    "cluster version mismatch in '{}': expected '{}', found '{}'",
                    cluster_path.display(),
                    version,
                    nested.version
                ));
            }

            self.visit(&cluster_path, nested)?;
        }

        self.visiting_paths.remove(&canonical);
        self.visiting_keys.remove(&cluster_key);
        Ok(())
    }
}

fn resolve_cluster_path(
    base_dir: &Path,
    cluster_id: &str,
    search_paths: &[PathBuf],
) -> Option<PathBuf> {
    let filename = format!("{cluster_id}.yaml");

    let mut candidates = vec![
        base_dir.join(&filename),
        base_dir.join("clusters").join(&filename),
    ];

    for path in search_paths {
        candidates.push(path.join(&filename));
        candidates.push(path.join("clusters").join(&filename));
    }

    candidates.into_iter().find(|candidate| candidate.exists())
}

fn canonicalize_or_self(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn parse_graph_file(path: &Path) -> Result<ClusterDefinition, String> {
    let data = fs::read_to_string(path)
        .map_err(|err| format!("read graph '{}': {err}", path.display()))?;
    parse_graph_str(&data, path)
}

fn parse_graph_str(input: &str, source: &Path) -> Result<ClusterDefinition, String> {
    let raw: RawClusterDefinition = serde_yaml::from_str(input)
        .map_err(|err| format!("parse YAML '{}': {err}", source.display()))?;
    raw.into_cluster_definition()
        .map_err(|err| format!("graph '{}': {err}", source.display()))
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
        if self.kind.to_ascii_lowercase() != "cluster" {
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
            .map(|value| parse_typed_parameter_value(value, &ty))
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
    Ok((id.to_string(), version.to_string()))
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
        serde_yaml::Value::String(s) => Ok(s.clone()),
        serde_yaml::Value::Number(n) => Ok(n.to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use ergo_runtime::action::ActionOutcome;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn write_temp_yaml(name: &str, contents: &str) -> PathBuf {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "ergo-graph-yaml-test-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join(name);
        fs::write(&path, contents).expect("write temp graph yaml");
        path
    }

    fn write_temp_tree(entries: &[(&str, &str)]) -> PathBuf {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "ergo-graph-yaml-tree-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&root).expect("create temp tree root");
        for (relative_path, contents) in entries {
            let full_path = root.join(relative_path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).expect("create temp tree parent");
            }
            fs::write(&full_path, contents).expect("write temp tree file");
        }
        root
    }

    #[test]
    fn parses_graph_with_external_input_edge() {
        let yaml = r#"
kind: cluster
id: threshold_gate
version: "0.1.0"

nodes:
  gt:
    impl: gt@0.1.0

edges:
  - $threshold -> gt.b

inputs:
  - name: threshold
    type: number
    required: true

outputs:
  result: gt.result
"#;
        let parsed = parse_graph_str(yaml, Path::new("inline.yaml")).expect("parse graph");
        assert_eq!(parsed.input_ports.len(), 1);
        assert_eq!(parsed.edges.len(), 1);
        assert_eq!(parsed.edges[0].from.node_id, "threshold");
        assert_eq!(parsed.edges[0].from.port_name, "threshold");
    }

    #[test]
    fn rejects_undeclared_external_input_reference() {
        let yaml = r#"
kind: cluster
id: bad
version: "0.1.0"
nodes:
  gt:
    impl: gt@0.1.0
edges:
  - $threshold -> gt.b
outputs: {}
"#;
        let err = parse_graph_str(yaml, Path::new("inline.yaml")).expect_err("parse should fail");
        assert!(err.contains("not declared in inputs"), "err: {err}");
    }

    #[test]
    fn rejects_external_input_source_without_dollar_prefix() {
        let yaml = r#"
kind: cluster
id: bad
version: "0.1.0"
nodes:
  gt:
    impl: gt@0.1.0
edges:
  - threshold.value -> gt.b
inputs:
  - name: threshold
    type: number
outputs:
  result: gt.result
"#;
        let err = parse_graph_str(yaml, Path::new("inline.yaml")).expect_err("parse should fail");
        assert!(
            err.contains("must use '$threshold' syntax"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_unknown_edge_source_node() {
        let yaml = r#"
kind: cluster
id: bad
version: "0.1.0"
nodes:
  gt:
    impl: gt@0.1.0
edges:
  - nope.value -> gt.b
outputs:
  result: gt.result
"#;
        let err = parse_graph_str(yaml, Path::new("inline.yaml")).expect_err("parse should fail");
        assert!(
            err.contains("source node 'nope' is not declared"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_unknown_edge_target_node() {
        let yaml = r#"
kind: cluster
id: bad
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 1
edges:
  - src.value -> missing.a
outputs:
  out: src.value
"#;
        let err = parse_graph_str(yaml, Path::new("inline.yaml")).expect_err("parse should fail");
        assert!(
            err.contains("target node 'missing' is not declared"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn run_graph_command_executes_simple_graph() {
        let graph = r#"
kind: cluster
id: basic
version: "0.1.0"

nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5

edges: []

outputs:
  value_out: src.value
"#;
        let graph_path = write_temp_yaml("basic.yaml", graph);
        run_graph_command(&graph_path, &[]).expect("graph should run");
    }

    #[test]
    fn run_graph_command_allows_boundary_output_from_cluster_node() {
        let root = write_temp_tree(&[
            (
                "child.yaml",
                r#"
kind: cluster
id: child
version: "0.1.0"

nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 7

edges: []
outputs:
  out: src.value
"#,
            ),
            (
                "root.yaml",
                r#"
kind: cluster
id: root
version: "0.1.0"

nodes:
  c:
    cluster: child@0.1.0

edges: []
outputs:
  result: c.out
"#,
            ),
        ]);
        let root_graph = root.join("root.yaml");
        run_graph_command(&root_graph, &[]).expect("root boundary output through cluster node");
    }

    #[test]
    fn demo_1_yaml_executes_end_to_end() {
        let yaml = r#"
kind: cluster
id: demo_1
version: "0.1.0"

nodes:
  src_left_a:
    impl: number_source@0.1.0
    params:
      value: 4.0

  src_left_b:
    impl: number_source@0.1.0
    params:
      value: 2.0

  src_right_a:
    impl: number_source@0.1.0
    params:
      value: 1.0

  src_right_b:
    impl: number_source@0.1.0
    params:
      value: 1.0

  src_ctx_x:
    impl: context_number_source@0.1.0

  add_left:
    impl: add@0.1.0

  add_right:
    impl: add@0.1.0

  add_right_ctx:
    impl: add@0.1.0

  add_total:
    impl: add@0.1.0

  gt_a:
    impl: gt@0.1.0

  gt_b:
    impl: gt@0.1.0

  emit_a:
    impl: emit_if_true@0.1.0

  emit_b:
    impl: emit_if_true@0.1.0

  act_a:
    impl: ack_action@0.1.0
    params:
      accept: true

  act_b:
    impl: ack_action@0.1.0
    params:
      accept: true

edges:
  - src_left_a.value -> add_left.a
  - src_left_b.value -> add_left.b
  - src_right_a.value -> add_right.a
  - src_right_b.value -> add_right.b
  - add_left.result -> add_total.a
  - add_right.result -> add_total.b
  - add_right.result -> add_right_ctx.a
  - src_ctx_x.value -> add_right_ctx.b
  - add_left.result -> gt_a.a
  - add_right_ctx.result -> gt_a.b
  - add_right_ctx.result -> gt_b.a
  - add_left.result -> gt_b.b
  - gt_a.result -> emit_a.input
  - gt_b.result -> emit_b.input
  - emit_a.event -> act_a.event
  - emit_b.event -> act_b.event

outputs:
  sum_left: add_left.result
  sum_total: add_total.result
  action_a_outcome: act_a.outcome
  action_b_outcome: act_b.outcome
"#;

        let cluster = parse_graph_str(yaml, Path::new("demo_1.yaml")).expect("parse demo_1");
        let loader = PreloadedClusterLoader::new(HashMap::new());
        let catalog = build_core_catalog();
        let registries = core_registries().expect("core registries");
        let expanded = expand(&cluster, &loader, &catalog).expect("expand demo_1");
        let refs = Registries {
            sources: &registries.sources,
            computes: &registries.computes,
            triggers: &registries.triggers,
            actions: &registries.actions,
        };

        let report =
            ergo_runtime::runtime::run(&expanded, &catalog, &refs, &ExecutionContext::default())
                .expect("run demo_1");

        assert_eq!(
            report.outputs.get("sum_left"),
            Some(&RuntimeValue::Number(6.0))
        );
        assert_eq!(
            report.outputs.get("sum_total"),
            Some(&RuntimeValue::Number(8.0))
        );
        assert_eq!(
            report.outputs.get("action_a_outcome"),
            Some(&RuntimeValue::Event(RuntimeEvent::Action(
                ActionOutcome::Completed
            )))
        );
        assert_eq!(
            report.outputs.get("action_b_outcome"),
            Some(&RuntimeValue::Event(RuntimeEvent::Action(
                ActionOutcome::Skipped
            )))
        );
    }

    #[test]
    fn version_accepts_number_and_string() {
        let yaml = r#"
kind: cluster
id: vcheck
version: 1.0
nodes: {}
edges: []
outputs: {}
"#;
        let parsed = parse_graph_str(yaml, Path::new("inline.yaml")).expect("parse graph");
        assert_eq!(parsed.version, "1.0");
    }

    #[test]
    fn run_graph_command_rejects_circular_cluster_references() {
        let root = write_temp_tree(&[(
            "root.yaml",
            r#"
kind: cluster
id: root
version: "0.1.0"

nodes:
  self_ref:
    cluster: root@0.1.0

edges: []
outputs: {}
"#,
        )]);
        let graph = root.join("root.yaml");
        let err = run_graph_command(&graph, &[]).expect_err("cycle should be rejected");
        assert!(
            err.contains("circular cluster reference"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn run_graph_command_rejects_duplicate_cluster_id_version_from_different_files() {
        let root = write_temp_tree(&[
            (
                "root/root.yaml",
                r#"
kind: cluster
id: root
version: "0.1.0"

nodes:
  a_node:
    cluster: a@0.1.0
  b_node:
    cluster: b@0.1.0
  add:
    impl: add@0.1.0

edges:
  - a_node.out -> add.a
  - b_node.out -> add.b

outputs:
  result: add.result
"#,
            ),
            (
                "a/a.yaml",
                r#"
kind: cluster
id: a
version: "0.1.0"

nodes:
  c:
    cluster: common@0.1.0
  z:
    impl: number_source@0.1.0
    params:
      value: 0
  pass:
    impl: add@0.1.0

edges:
  - c.out -> pass.a
  - z.value -> pass.b

outputs:
  out: pass.result
"#,
            ),
            (
                "b/b.yaml",
                r#"
kind: cluster
id: b
version: "0.1.0"

nodes:
  c:
    cluster: common@0.1.0
  z:
    impl: number_source@0.1.0
    params:
      value: 0
  pass:
    impl: add@0.1.0

edges:
  - c.out -> pass.a
  - z.value -> pass.b

outputs:
  out: pass.result
"#,
            ),
            (
                "a/common.yaml",
                r#"
kind: cluster
id: common
version: "0.1.0"

nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 10

edges: []
outputs:
  out: src.value
"#,
            ),
            (
                "b/common.yaml",
                r#"
kind: cluster
id: common
version: "0.1.0"

nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 100

edges: []
outputs:
  out: src.value
"#,
            ),
        ]);
        let graph = root.join("root/root.yaml");
        let search_a = root.join("a").to_string_lossy().to_string();
        let search_b = root.join("b").to_string_lossy().to_string();
        let args = vec![
            "--cluster-path".to_string(),
            search_a,
            "--cluster-path".to_string(),
            search_b,
        ];
        let err = run_graph_command(&graph, &args).expect_err("duplicate definitions must fail");
        assert!(
            err.contains("defined by multiple files"),
            "unexpected error: {err}"
        );
    }
}
