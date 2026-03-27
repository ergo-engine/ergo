use std::collections::HashMap;
use std::path::PathBuf;

use ergo_loader::PreparedGraphAssets;
use ergo_runtime::catalog::{build_core_catalog, CorePrimitiveCatalog};
use ergo_runtime::cluster::{
    expand, ClusterDefinition, ClusterLoader, ClusterVersionIndex, ExpandError, ExpandedEndpoint,
    ExpandedGraph, PrimitiveCatalog, PrimitiveKind, Version, VersionTargetKind,
};
use ergo_runtime::common::ErrorInfo;

pub struct GraphToDotFromPathsRequest {
    pub graph_path: PathBuf,
    pub cluster_paths: Vec<PathBuf>,
    pub show_ports: bool,
    pub show_impl: bool,
    pub show_runtime_id: bool,
}

pub struct GraphToDotFromAssetsRequest {
    pub assets: PreparedGraphAssets,
    pub show_ports: bool,
    pub show_impl: bool,
    pub show_runtime_id: bool,
}

pub fn graph_to_dot_from_paths(request: GraphToDotFromPathsRequest) -> Result<String, String> {
    let GraphToDotFromPathsRequest {
        graph_path,
        cluster_paths,
        show_ports,
        show_impl,
        show_runtime_id,
    } = request;

    let discovery = ergo_loader::discovery::discover_cluster_tree(&graph_path, &cluster_paths)
        .map_err(|err| err.to_string())?;
    let root = discovery.root.clone();
    let cluster_sources = discovery.cluster_sources;
    let clusters = discovery.clusters;
    let loader = PreloadedClusterLoader::new(clusters);

    let catalog = build_core_catalog();
    let expanded = expand(&root, &loader, &catalog).map_err(|err| {
        format!(
            "graph expansion failed: [{}] {}",
            err.rule_id(),
            summarize_expand_error_with_files(&err, &cluster_sources)
        )
    })?;

    Ok(render_graph_as_dot(
        &root.id,
        &expanded,
        &catalog,
        show_ports,
        show_impl,
        show_runtime_id,
    ))
}

pub fn graph_to_dot_from_assets(request: GraphToDotFromAssetsRequest) -> Result<String, String> {
    let GraphToDotFromAssetsRequest {
        assets,
        show_ports,
        show_impl,
        show_runtime_id,
    } = request;

    let root = assets.root().clone();
    let loader = PreloadedClusterLoader::new(assets.clusters().clone());

    let catalog = build_core_catalog();
    let expanded = expand(&root, &loader, &catalog).map_err(|err| {
        format!(
            "graph expansion failed: [{}] {}",
            err.rule_id(),
            summarize_expand_error_with_labels(&err, assets.cluster_diagnostic_labels())
        )
    })?;

    Ok(render_graph_as_dot(
        &root.id,
        &expanded,
        &catalog,
        show_ports,
        show_impl,
        show_runtime_id,
    ))
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

impl ClusterVersionIndex for PreloadedClusterLoader {
    fn available_versions(&self, id: &str) -> Vec<Version> {
        let mut versions = self
            .clusters
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

fn summarize_expand_error_with_files(
    err: &ExpandError,
    cluster_sources: &HashMap<(String, Version), PathBuf>,
) -> String {
    let base = err.summary().to_string();
    match err {
        ExpandError::UnsatisfiedVersionConstraint {
            target_kind: VersionTargetKind::Cluster,
            id,
            available_versions,
            ..
        } => {
            let available = available_versions
                .iter()
                .filter_map(|version| {
                    cluster_sources
                        .get(&(id.clone(), version.clone()))
                        .map(|path| format!("- {}@{} at {}", id, version, path.display()))
                })
                .collect::<Vec<_>>();
            if available.is_empty() {
                base
            } else {
                format!(
                    "{}\navailable cluster files:\n{}",
                    base,
                    available.join("\n")
                )
            }
        }
        _ => base,
    }
}

fn summarize_expand_error_with_labels(
    err: &ExpandError,
    cluster_diagnostic_labels: &HashMap<(String, Version), String>,
) -> String {
    let base = err.summary().to_string();
    match err {
        ExpandError::UnsatisfiedVersionConstraint {
            target_kind: VersionTargetKind::Cluster,
            id,
            available_versions,
            ..
        } => {
            let available = available_versions
                .iter()
                .map(|version| {
                    let label = cluster_diagnostic_labels
                        .get(&(id.clone(), version.clone()))
                        .cloned()
                        .unwrap_or_else(|| format!("{id}@{version}"));
                    format!("- {}@{} at {}", id, version, label)
                })
                .collect::<Vec<_>>();
            if available.is_empty() {
                base
            } else {
                format!(
                    "{}\navailable cluster sources:\n{}",
                    base,
                    available.join("\n")
                )
            }
        }
        _ => base,
    }
}

fn render_graph_as_dot(
    graph_id: &str,
    expanded: &ExpandedGraph,
    catalog: &CorePrimitiveCatalog,
    show_ports: bool,
    show_impl: bool,
    show_runtime_id: bool,
) -> String {
    let mut lines = Vec::new();
    let mut node_ids: Vec<&str> = expanded.nodes.keys().map(|id| id.as_str()).collect();
    node_ids.sort_unstable();

    lines.push(format!("digraph {} {{", quote_id(graph_id)));
    lines.push(format!(
        "  label={}",
        quote_label(&format!("{} (expanded)", graph_id))
    ));
    lines.push("  labelloc=t".to_string());
    lines.push("  rankdir=TB".to_string());
    lines.push("  nodesep=0.5".to_string());
    lines.push("  ranksep=0.7".to_string());
    lines.push(String::new());

    let mut source_nodes = Vec::new();
    let mut action_nodes = Vec::new();

    for node_id in &node_ids {
        let node = expanded
            .nodes
            .get(*node_id)
            .expect("node id list built from map keys");
        let primitive = catalog.get(&node.implementation.impl_id, &node.implementation.version);
        let kind = primitive
            .map(|metadata| metadata.kind)
            .unwrap_or(PrimitiveKind::Compute);
        let style = node_style(&kind);

        if kind == PrimitiveKind::Source {
            source_nodes.push(*node_id);
        } else if kind == PrimitiveKind::Action {
            action_nodes.push(*node_id);
        }

        let mut label = authoring_label(node);
        if show_runtime_id {
            label.push('\n');
            label.push_str("runtime: ");
            label.push_str(node_id);
        }
        if show_impl {
            label.push('\n');
            label.push_str("impl: ");
            label.push_str(&node.implementation.impl_id);
            label.push('@');
            label.push_str(&node.implementation.version);
        }

        lines.push(format!(
            "  {} [label={}, shape={}, style=filled, fillcolor={}, color={}]",
            quote_id(node_id),
            quote_label(&label),
            style.shape,
            quote_label(style.fillcolor),
            quote_label(style.color),
        ));
    }

    lines.push(String::new());

    let mut external_inputs = Vec::new();
    for edge in &expanded.edges {
        if let ExpandedEndpoint::ExternalInput { name } = &edge.from {
            if !external_inputs.contains(name) {
                external_inputs.push(name.clone());
            }
        }
    }
    external_inputs.sort_unstable();

    for input_name in &external_inputs {
        lines.push(format!(
            "  {} [label={}, shape=note, style=dashed, color={}]",
            quote_id(&external_input_node_id(input_name.as_str())),
            quote_label(&format!(
                "input: {}",
                display_external_input_name(input_name)
            )),
            quote_label("#6a6a6a"),
        ));
    }

    if !source_nodes.is_empty() {
        let members = source_nodes
            .iter()
            .map(|node_id| quote_id(node_id))
            .collect::<Vec<_>>()
            .join("; ");
        lines.push(format!("  {{ rank=min; {} }}", members));
    }
    if !action_nodes.is_empty() {
        let members = action_nodes
            .iter()
            .map(|node_id| quote_id(node_id))
            .collect::<Vec<_>>()
            .join("; ");
        lines.push(format!("  {{ rank=max; {} }}", members));
    }

    lines.push(String::new());

    let mut edge_lines = Vec::new();
    for edge in &expanded.edges {
        let from = endpoint_id(&edge.from);
        let to = endpoint_id(&edge.to);
        let mut line = format!("  {} -> {}", quote_id(&from), quote_id(&to));
        if show_ports {
            let label = format!(
                "{} -> {}",
                endpoint_port(&edge.from),
                endpoint_port(&edge.to)
            );
            line.push_str(&format!(" [label={}]", quote_label(&label)));
        }
        edge_lines.push(line);
    }
    edge_lines.sort_unstable();
    lines.extend(edge_lines);

    lines.push("}".to_string());
    lines.join("\n") + "\n"
}

fn endpoint_id(endpoint: &ExpandedEndpoint) -> String {
    match endpoint {
        ExpandedEndpoint::NodePort { node_id, .. } => node_id.clone(),
        ExpandedEndpoint::ExternalInput { name } => external_input_node_id(name),
    }
}

fn endpoint_port(endpoint: &ExpandedEndpoint) -> String {
    match endpoint {
        ExpandedEndpoint::NodePort { port_name, .. } => port_name.clone(),
        ExpandedEndpoint::ExternalInput { name } => display_external_input_name(name).to_string(),
    }
}

fn authoring_label(node: &ergo_runtime::cluster::ExpandedNode) -> String {
    let node_path: Vec<&str> = node
        .authoring_path
        .iter()
        .map(|(_, node_id)| node_id.as_str())
        .collect();
    if node_path.is_empty() {
        node.runtime_id.clone()
    } else {
        node_path.join("/")
    }
}

fn external_input_node_id(name: &str) -> String {
    format!("external_input_{}", name)
}

fn display_external_input_name(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

struct NodeStyle {
    shape: &'static str,
    fillcolor: &'static str,
    color: &'static str,
}

fn node_style(kind: &PrimitiveKind) -> NodeStyle {
    match kind {
        PrimitiveKind::Source => NodeStyle {
            shape: "box",
            fillcolor: "#e8f4fd",
            color: "#4a90d9",
        },
        PrimitiveKind::Compute => NodeStyle {
            shape: "ellipse",
            fillcolor: "#f2f2f2",
            color: "#777777",
        },
        PrimitiveKind::Trigger => NodeStyle {
            shape: "diamond",
            fillcolor: "#fff3e0",
            color: "#d58c2d",
        },
        PrimitiveKind::Action => NodeStyle {
            shape: "doubleoctagon",
            fillcolor: "#e8f5e9",
            color: "#2f7d32",
        },
    }
}

fn quote_label(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!("\"{}\"", escaped)
}

fn quote_id(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

#[cfg(test)]
mod tests {
    use super::{graph_to_dot_from_assets, GraphToDotFromAssetsRequest};
    use ergo_loader::{load_graph_assets_from_memory, InMemorySourceInput};

    #[test]
    fn graph_to_dot_from_assets_renders_loaded_in_memory_graph() -> Result<(), String> {
        let assets = load_graph_assets_from_memory(
            "mem/root.yaml",
            &[InMemorySourceInput {
                source_id: "mem/root.yaml".to_string(),
                source_label: "mem/root.yaml".to_string(),
                content: r#"
kind: cluster
id: memory_visual
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 3
  cmp:
    impl: gt@0.1.0
edges:
  - src.value -> cmp.a
outputs:
  out: cmp.result
"#
                .to_string(),
            }],
            &[],
        )
        .map_err(|err| err.to_string())?;

        let dot = graph_to_dot_from_assets(GraphToDotFromAssetsRequest {
            assets,
            show_ports: true,
            show_impl: false,
            show_runtime_id: false,
        })?;

        assert!(dot.contains("digraph \"memory_visual\""));
        assert!(dot.contains("src"));
        assert!(dot.contains("cmp"));
        assert!(dot.contains("value -> a"));
        Ok(())
    }
}
