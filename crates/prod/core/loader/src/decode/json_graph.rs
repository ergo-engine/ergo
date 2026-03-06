use crate::io::{LoaderDecodeError, LoaderError};
use crate::DecodedAuthoringGraph;

pub fn decode_graph_json(input: &str) -> Result<DecodedAuthoringGraph, LoaderError> {
    let _: serde_json::Value = serde_json::from_str(input).map_err(|err| {
        LoaderError::Decode(LoaderDecodeError {
            message: err.to_string(),
        })
    })?;

    Err(LoaderError::Decode(LoaderDecodeError {
        message: "typed authoring decode is not wired yet".to_string(),
    }))
}
