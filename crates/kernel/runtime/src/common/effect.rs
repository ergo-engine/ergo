use serde::{Deserialize, Serialize};

use super::Value;

/// A single write effect: resolved key name + value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectWrite {
    pub key: String,
    pub value: Value,
}

/// A single intent field: resolved field name + typed value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentField {
    pub name: String,
    pub value: Value,
}

/// An external intent record emitted by an action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentRecord {
    pub kind: String,
    pub intent_id: String,
    pub fields: Vec<IntentField>,
}

/// An action effect containing a kind and write operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionEffect {
    pub kind: String,
    pub writes: Vec<EffectWrite>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intents: Vec<IntentRecord>,
}
