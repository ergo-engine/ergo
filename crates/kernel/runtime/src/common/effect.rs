use serde::{Deserialize, Serialize};

use super::Value;

/// A single write effect: resolved key name + value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectWrite {
    pub key: String,
    pub value: Value,
}

/// An action effect containing a kind and write operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionEffect {
    pub kind: String,
    pub writes: Vec<EffectWrite>,
}
