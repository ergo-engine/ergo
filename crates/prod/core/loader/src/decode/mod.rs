//! decode
//!
//! Purpose:
//! - Expose the loader's graph decode surface for YAML and JSON authoring text.
//! - Define the loader-owned `DecodedAuthoringGraph` alias used by discovery and I/O.
//! - Coordinate dual-format in-memory parsing when callers provide unlabeled graph content that
//!   may be either YAML or JSON.
//!
//! Owns:
//! - Public decode entrypoint exports for graph authoring text and graph files.
//! - The shared format-selection policy for `parse_graph_content(...)`.
//! - Loader-local helpers that choose the most truthful decode error to return.
//!
//! Does not own:
//! - File discovery, source resolution, project loading, or semantic validation.
//! - Shared raw authoring normalization details; those live in `authoring_graph.rs`.
//! - Transport-specific YAML/JSON parsing details; those live in the format-specific child
//!   modules.
//!
//! Connects to:
//! - `authoring_graph.rs` for shared raw authoring normalization reused across formats.
//! - `yaml_graph.rs` and `json_graph.rs` for format-specific parsing.
//! - `discovery.rs` and `io.rs` for decoded graph handoff to later loader stages.
//!
//! Safety notes:
//! - This module stays on the loader decode surface and must not emit kernel rule violations.
//! - Dual-format fallback must preserve specific structural decode errors when possible rather
//!   than collapsing everything into a generic parse failure.

mod authoring_graph;
mod json_graph;
mod yaml_graph;

use crate::io::{LoaderDecodeError, LoaderError};
pub(crate) use authoring_graph::{selector_matches_version, validate_cluster_reference_id};
pub use json_graph::decode_graph_json;
pub(crate) use json_graph::decode_graph_json_labeled;
pub(crate) use yaml_graph::parse_graph_str;
pub use yaml_graph::{decode_graph_yaml, decode_graph_yaml_labeled, parse_graph_file};

pub type DecodedAuthoringGraph = ergo_runtime::cluster::ClusterDefinition;

pub(crate) fn parse_graph_content(
    input: &str,
    source_label: &str,
) -> Result<DecodedAuthoringGraph, LoaderError> {
    match parse_graph_str(input, source_label) {
        Ok(graph) => Ok(graph),
        // Try JSON only after YAML fails so callers can pass unlabeled in-memory content through
        // one loader decode seam without inventing a separate format selector.
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
