use crate::errors::InvalidAdapter;
use crate::manifest::AdapterManifest;
use crate::provides::AdapterProvides;
use crate::validate::validate_adapter;

/// Register an adapter by validating its manifest and returning what it provides.
///
/// This is the earliest rejection point for invalid adapters.
/// Validation happens before any runtime state is modified.
pub fn register(manifest: &AdapterManifest) -> Result<AdapterProvides, InvalidAdapter> {
    validate_adapter(manifest)?;
    Ok(AdapterProvides::from_manifest(manifest))
}
