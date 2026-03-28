//! json_graph
//!
//! Purpose:
//! - Decode JSON graph authoring text into the loader's `DecodedAuthoringGraph` surface.
//! - Reuse the shared raw-graph normalization path so JSON and YAML authoring produce the same
//!   loader-owned cluster shape.
//!
//! Owns:
//! - Direct JSON string parsing for graph authoring input.
//! - JSON-specific decode error labeling for memory-backed or caller-labeled sources.
//!
//! Does not own:
//! - File I/O, dual-format YAML-vs-JSON fallback selection, or semantic validation.
//! - The raw graph normalization rules themselves; those live in `authoring_graph.rs`.
//!
//! Connects to:
//! - `decode/mod.rs` for public decode entrypoints and dual-format parse selection.
//! - `authoring_graph.rs` for the shared raw graph decode path.
//!
//! Safety notes:
//! - JSON decode failures stay on the loader decode surface and must not become semantic errors.
//! - Successful JSON decode must flow through the same raw graph normalization as YAML input.

use crate::decode::authoring_graph::{decode_raw_graph, RawClusterDefinition};
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
