use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::manifest::AdapterManifest;

/// Build a stable adapter provenance fingerprint.
///
/// The manifest is canonicalized by recursively sorting object keys,
/// serialized to compact JSON, then hashed with SHA-256.
pub fn fingerprint(manifest: &AdapterManifest) -> String {
    let value = serde_json::to_value(manifest).expect("adapter manifest must serialize");
    let canonical = canonicalize(value);
    let bytes = serde_json::to_vec(&canonical).expect("canonical manifest must serialize");

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let hash = hex::encode(digest);

    format!(
        "adapter:{}@{};sha256:{}",
        manifest.id, manifest.version, hash
    )
}

fn canonicalize(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort();

            let mut out = Map::new();
            for key in keys {
                if let Some(v) = map.get(&key) {
                    out.insert(key, canonicalize(v.clone()));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(canonicalize).collect()),
        _ => value,
    }
}

#[cfg(test)]
mod tests {
    use super::fingerprint;
    use crate::manifest::{AdapterManifest, CaptureSpec, ContextKeySpec, EventKindSpec};
    use serde_json::json;

    fn baseline_manifest() -> AdapterManifest {
        AdapterManifest {
            kind: "adapter".to_string(),
            id: "demo".to_string(),
            version: "1.0.0".to_string(),
            runtime_compatibility: "0.1.0".to_string(),
            context_keys: vec![ContextKeySpec {
                name: "x".to_string(),
                ty: "Number".to_string(),
                required: false,
                writable: Some(false),
                description: None,
            }],
            event_kinds: vec![EventKindSpec {
                name: "tick".to_string(),
                payload_schema: json!({
                    "type": "object",
                    "properties": {
                        "x": { "type": "number" }
                    },
                    "required": ["x"],
                    "additionalProperties": false
                }),
            }],
            accepts: None,
            capture: CaptureSpec {
                format_version: "1".to_string(),
                fields: vec![
                    "event.tick".to_string(),
                    "meta.adapter_id".to_string(),
                    "meta.adapter_version".to_string(),
                    "meta.timestamp".to_string(),
                ],
            },
        }
    }

    #[test]
    fn fingerprint_is_stable_for_equivalent_content() {
        let a = baseline_manifest();
        let b = baseline_manifest();

        assert_eq!(fingerprint(&a), fingerprint(&b));
    }

    #[test]
    fn fingerprint_changes_when_manifest_changes() {
        let mut a = baseline_manifest();
        let mut b = baseline_manifest();
        b.version = "1.0.1".to_string();

        assert_ne!(fingerprint(&a), fingerprint(&b));

        a.context_keys[0].name = "y".to_string();
        assert_ne!(fingerprint(&a), fingerprint(&b));
    }
}
