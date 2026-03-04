use std::collections::HashMap;
use std::fmt;

/// Resolve a manifest name that may reference a parameter via `$` prefix.
///
/// If `name` starts with `$`, the remainder is looked up in `parameters`.
/// Only `ParameterValue::String` is accepted; other types produce an error.
/// Literal names (no `$` prefix) are returned unchanged.
pub fn resolve_manifest_name(
    name: &str,
    parameters: &HashMap<String, crate::cluster::ParameterValue>,
) -> Result<String, ManifestNameError> {
    if let Some(param_name) = name.strip_prefix('$') {
        match parameters.get(param_name) {
            Some(crate::cluster::ParameterValue::String(s)) => Ok(s.clone()),
            Some(_) => Err(ManifestNameError::WrongParameterType {
                binding: name.to_string(),
            }),
            None => Err(ManifestNameError::MissingParameter {
                binding: name.to_string(),
            }),
        }
    } else {
        Ok(name.to_string())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ManifestNameError {
    MissingParameter { binding: String },
    WrongParameterType { binding: String },
}

impl fmt::Display for ManifestNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingParameter { binding } => {
                write!(
                    f,
                    "manifest name '{}' references missing parameter",
                    binding
                )
            }
            Self::WrongParameterType { binding } => {
                write!(
                    f,
                    "manifest name '{}' references non-String parameter",
                    binding
                )
            }
        }
    }
}

impl std::error::Error for ManifestNameError {}
