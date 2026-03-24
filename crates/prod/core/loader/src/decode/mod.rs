mod json_graph;
mod yaml_graph;

use crate::io::{LoaderDecodeError, LoaderError};
pub use json_graph::decode_graph_json;
pub(crate) use json_graph::decode_graph_json_labeled;
pub use yaml_graph::{decode_graph_yaml, decode_graph_yaml_labeled, parse_graph_file};
pub(crate) use yaml_graph::{
    parse_graph_str, selector_matches_version, validate_cluster_reference_id,
};

pub type DecodedAuthoringGraph = ergo_runtime::cluster::ClusterDefinition;

pub(crate) fn parse_graph_content(
    input: &str,
    source_label: &str,
) -> Result<DecodedAuthoringGraph, LoaderError> {
    match parse_graph_str(input, source_label) {
        Ok(graph) => Ok(graph),
        Err(yaml_err) => match decode_graph_json_labeled(input, source_label) {
            Ok(graph) => Ok(graph),
            Err(json_err) => {
                if is_parse_decode_error(&yaml_err) && is_parse_decode_error(&json_err) {
                    Err(LoaderError::Decode(LoaderDecodeError {
                        message: format!(
                            "parse graph '{}': expected valid YAML or JSON authoring content",
                            source_label
                        ),
                    }))
                } else if is_parse_decode_error(&yaml_err) {
                    Err(json_err)
                } else {
                    Err(yaml_err)
                }
            }
        },
    }
}

fn is_parse_decode_error(error: &LoaderError) -> bool {
    match error {
        LoaderError::Decode(inner) => {
            inner.message.starts_with("parse YAML ") || inner.message.starts_with("parse JSON ")
        }
        _ => false,
    }
}
