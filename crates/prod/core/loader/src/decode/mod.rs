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
            Err(json_err) => Err(select_best_graph_parse_error(
                input,
                source_label,
                yaml_err,
                json_err,
            )),
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParseDecodeErrorKind {
    Syntax,
    Data,
    Other,
}

fn select_best_graph_parse_error(
    input: &str,
    source_label: &str,
    yaml_err: LoaderError,
    json_err: LoaderError,
) -> LoaderError {
    use ParseDecodeErrorKind::{Data, Other, Syntax};

    match (
        classify_parse_decode_error(&yaml_err),
        classify_parse_decode_error(&json_err),
    ) {
        (Syntax, Syntax) => LoaderError::Decode(LoaderDecodeError {
            message: format!(
                "parse graph '{}': expected valid YAML or JSON authoring content",
                source_label
            ),
        }),
        (Syntax, _) => json_err,
        (_, Syntax) => yaml_err,
        (Data, Data) => {
            if looks_like_json_document(input) {
                json_err
            } else {
                yaml_err
            }
        }
        (Other, _) => yaml_err,
        (_, Other) => json_err,
    }
}

fn classify_parse_decode_error(error: &LoaderError) -> ParseDecodeErrorKind {
    match error {
        LoaderError::Decode(inner) => {
            if inner.message.starts_with("parse YAML ") || inner.message.starts_with("parse JSON ")
            {
                if is_structural_decode_message(&inner.message) {
                    ParseDecodeErrorKind::Data
                } else {
                    ParseDecodeErrorKind::Syntax
                }
            } else {
                ParseDecodeErrorKind::Other
            }
        }
        _ => ParseDecodeErrorKind::Other,
    }
}

fn is_structural_decode_message(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    [
        "missing field",
        "unknown field",
        "duplicate field",
        "unknown variant",
        "invalid type",
        "invalid value",
        "invalid length",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

fn looks_like_json_document(input: &str) -> bool {
    let trimmed = input.trim_start();
    trimmed.starts_with('{') || trimmed.starts_with('[')
}
