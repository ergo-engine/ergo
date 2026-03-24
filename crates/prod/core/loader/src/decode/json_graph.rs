use crate::decode::yaml_graph::{decode_raw_graph, RawClusterDefinition};
use crate::io::{LoaderDecodeError, LoaderError};
use crate::DecodedAuthoringGraph;

pub fn decode_graph_json(input: &str) -> Result<DecodedAuthoringGraph, LoaderError> {
    decode_graph_json_labeled(input, "<memory>")
}

pub(crate) fn decode_graph_json_labeled(
    input: &str,
    source_label: &str,
) -> Result<DecodedAuthoringGraph, LoaderError> {
    let raw: RawClusterDefinition = serde_json::from_str(input).map_err(|err| {
        LoaderError::Decode(LoaderDecodeError {
            message: format!("parse JSON '{}': {err}", source_label),
        })
    })?;
    decode_raw_graph(raw, source_label)
}
