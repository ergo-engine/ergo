mod json_graph;
mod yaml_graph;

pub use json_graph::decode_graph_json;
pub(crate) use yaml_graph::selector_matches_version;
pub use yaml_graph::{decode_graph_yaml, parse_graph_file};

pub type DecodedAuthoringGraph = ergo_runtime::cluster::ClusterDefinition;
