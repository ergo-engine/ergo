use std::collections::{HashMap, HashSet};

use crate::manifest::AdapterManifest;

/// What an adapter provides for composition validation.
/// Built from AdapterManifest after registration-phase validation passes.
#[derive(Debug, Clone)]
pub struct AdapterProvides {
    pub context: HashMap<String, ContextKeyProvision>,
    pub events: HashSet<String>,
    pub effects: HashSet<String>,
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

        let effects = manifest
            .accepts
            .as_ref()
            .map(|a| a.effects.iter().map(|e| e.name.clone()).collect())
            .unwrap_or_default();

        Self {
            context,
            events,
            effects,
        }
    }
}

impl Default for AdapterProvides {
    fn default() -> Self {
        Self {
            context: HashMap::new(),
            events: HashSet::new(),
            effects: HashSet::new(),
        }
    }
}
