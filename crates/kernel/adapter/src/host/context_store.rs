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
