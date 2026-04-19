//! host::context_store
//!
//! Purpose:
//! - Hold the host-owned mutable context map that effect handlers write and
//!   hosted-event binding reads.
//!
//! Owns:
//! - `ContextStore` and its simple set/get/snapshot accessors.
//!
//! Does not own:
//! - Context schema or writability rules; handlers validate those against
//!   adapter declarations before mutating the store.
//!
//! Connects to:
//! - `runner.rs`, which merges incoming context over the store snapshot.
//! - `effects.rs`, which mutates the store for handler-owned effect kinds.
//!
//! Safety notes:
//! - The store preserves deterministic ordering via `BTreeMap`.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct ContextStore {
    values: BTreeMap<String, serde_json::Value>,
}

impl ContextStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> &BTreeMap<String, serde_json::Value> {
        &self.values
    }

    pub fn set(&mut self, key: String, value: serde_json::Value) {
        self.values.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.values.get(key)
    }
}
