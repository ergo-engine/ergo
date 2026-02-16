use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AdapterManifest {
    pub kind: String,
    pub id: String,
    pub version: String,
    pub runtime_compatibility: String,
    pub context_keys: Vec<ContextKeySpec>,
    pub event_kinds: Vec<EventKindSpec>,
    pub accepts: Option<AcceptsSpec>,
    pub capture: CaptureSpec,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContextKeySpec {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub required: bool,
    pub writable: Option<bool>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventKindSpec {
    pub name: String,
    pub payload_schema: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AcceptsSpec {
    pub effects: Vec<EffectSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EffectSpec {
    pub name: String,
    pub payload_schema: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CaptureSpec {
    pub format_version: String,
    pub fields: Vec<String>,
}
