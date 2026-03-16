use std::collections::{HashMap, HashSet};

use crate::manifest::AdapterManifest;
use crate::provenance;

/// What an adapter provides for composition validation.
/// Built from AdapterManifest after registration-phase validation passes.
#[derive(Debug, Clone, Default)]
pub struct AdapterProvides {
    pub context: HashMap<String, ContextKeyProvision>,
    pub events: HashSet<String>,
    pub effects: HashSet<String>,
    pub effect_schemas: HashMap<String, serde_json::Value>,
    pub event_schemas: HashMap<String, serde_json::Value>,
    pub capture_format_version: String,
    pub adapter_fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct ContextKeyProvision {
    pub ty: String, // Keep as String; composition converts to ValueType
    pub required: bool,
    pub writable: bool,
}

impl AdapterProvides {
    /// Build from a validated AdapterManifest.
    pub fn from_manifest(manifest: &AdapterManifest) -> Self {
        let context = manifest
            .context_keys
            .iter()
            .map(|k| {
                (
                    k.name.clone(),
                    ContextKeyProvision {
                        ty: k.ty.clone(),
                        required: k.required,
                        writable: k.writable.unwrap_or(false),
                    },
                )
            })
            .collect();

        let events = manifest
            .event_kinds
            .iter()
            .map(|e| e.name.clone())
            .collect();
        let event_schemas = manifest
            .event_kinds
            .iter()
            .map(|e| (e.name.clone(), e.payload_schema.clone()))
            .collect();

        let effects = manifest
            .accepts
            .as_ref()
            .map(|a| a.effects.iter().map(|e| e.name.clone()).collect())
            .unwrap_or_default();
        let effect_schemas = manifest
            .accepts
            .as_ref()
            .map(|a| {
                a.effects
                    .iter()
                    .map(|effect| (effect.name.clone(), effect.payload_schema.clone()))
                    .collect()
            })
            .unwrap_or_default();

        Self {
            context,
            events,
            effects,
            effect_schemas,
            event_schemas,
            capture_format_version: manifest.capture.format_version.clone(),
            adapter_fingerprint: provenance::fingerprint(manifest),
        }
    }
}
