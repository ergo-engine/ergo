use std::fs;
use std::path::Path;

use ergo_adapter::AdapterManifest;

pub fn parse_adapter_manifest(path: &Path) -> Result<AdapterManifest, String> {
    let data = fs::read_to_string(path)
        .map_err(|err| format!("read adapter manifest '{}': {err}", path.display()))?;
    let value = serde_yaml::from_str::<serde_json::Value>(&data)
        .map_err(|err| format!("parse adapter manifest '{}': {err}", path.display()))?;
    serde_json::from_value::<AdapterManifest>(value)
        .map_err(|err| format!("decode adapter manifest '{}': {err}", path.display()))
}
