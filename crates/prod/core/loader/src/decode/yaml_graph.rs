//! yaml_graph
//!
//! Purpose:
//! - Decode YAML graph authoring text or files into the loader's `DecodedAuthoringGraph` surface.
//! - Provide YAML-specific source labeling around the shared authoring-graph normalization path.
//!
//! Owns:
//! - YAML string parsing for graph authoring input.
//! - YAML file reads plus truthful path-labeled decode errors.
//!
//! Does not own:
//! - Shared raw graph normalization, identifier validation, or edge/signature parsing rules.
//! - Filesystem or in-memory source resolution, cluster search, or semantic validation.
//!
//! Connects to:
//! - `authoring_graph.rs` for the shared raw authoring model and normalization logic.
//! - `decode/mod.rs` for public decode exports and dual-format fallback selection.
//! - `io.rs` for loader decode/io error reporting.
//!
//! Safety notes:
//! - YAML decode stays on the loader transport/decode surface and must not invent kernel or host
//!   semantics.
//! - YAML and JSON must feed the same shared normalization path so both formats produce the same
//!   decoded graph shape.

use std::fs;
use std::path::Path;

use crate::decode::authoring_graph::{decode_raw_graph, RawClusterDefinition};
use crate::io::{LoaderDecodeError, LoaderError, LoaderIoError};
use crate::DecodedAuthoringGraph;

pub fn decode_graph_yaml(input: &str) -> Result<DecodedAuthoringGraph, LoaderError> {
    decode_graph_yaml_labeled(input, "<memory>")
}

pub fn decode_graph_yaml_labeled(
    input: &str,
    source_label: &str,
) -> Result<DecodedAuthoringGraph, LoaderError> {
    parse_graph_str(input, source_label)
}

pub fn parse_graph_file(path: &Path) -> Result<DecodedAuthoringGraph, LoaderError> {
    let data = fs::read_to_string(path).map_err(|err| {
        LoaderError::Io(LoaderIoError {
            path: path.to_path_buf(),
            message: format!("read graph '{}': {err}", path.display()),
        })
    })?;
    parse_graph_str(&data, &path.display().to_string())
}

pub(crate) fn parse_graph_str(
    input: &str,
    source_label: &str,
) -> Result<DecodedAuthoringGraph, LoaderError> {
    let raw: RawClusterDefinition = serde_yaml::from_str(input)
        .map_err(|err| decode_error(format!("parse YAML '{}': {err}", source_label)))?;
    decode_raw_graph(raw, source_label)
}

fn decode_error(message: String) -> LoaderError {
    LoaderError::Decode(LoaderDecodeError { message })
}
