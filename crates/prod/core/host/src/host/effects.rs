//! host::effects
//!
//! Purpose:
//! - Define the host-owned effect-handler seam and the default
//!   `set_context` handler used by the canonical runner.
//!
//! Owns:
//! - `EffectHandler`, `SetContextHandler`, `AppliedWrite`, and
//!   `EffectApplyError`.
//!
//! Does not own:
//! - Runtime effect production or the accepted effect contract itself; those
//!   come from the runtime and adapter layers.
//! - Hosted-runner orchestration, capture enrichment, or egress dispatch.
//!
//! Connects to:
//! - `runner.rs`, which dispatches handler-owned effects through these types.
//! - `context_store.rs`, which stores applied writes.
//!
//! Safety notes:
//! - `SetContextHandler` validates declared key, writable, and type before
//!   mutating `ContextStore`.
//! - Partial writes are not rolled back when a later write fails.

use super::context_store::ContextStore;
use ergo_adapter::AdapterProvides;
use ergo_runtime::common::{ActionEffect, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct AppliedWrite {
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EffectApplyError {
    UnhandledEffectKind {
        kind: String,
    },
    UndeclaredKey {
        kind: String,
        key: String,
    },
    NonWritableKey {
        kind: String,
        key: String,
    },
    TypeMismatch {
        kind: String,
        key: String,
        expected: String,
        got: String,
    },
    InvalidValueConversion {
        kind: String,
        key: String,
        detail: String,
    },
}

impl std::fmt::Display for EffectApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnhandledEffectKind { kind } => {
                write!(f, "no registered effect handler for kind '{kind}'")
            }
            Self::UndeclaredKey { kind, key } => {
                write!(f, "effect '{kind}' writes undeclared context key '{key}'")
            }
            Self::NonWritableKey { kind, key } => {
                write!(f, "effect '{kind}' writes non-writable context key '{key}'")
            }
            Self::TypeMismatch {
                kind,
                key,
                expected,
                got,
            } => write!(
                f,
                "effect '{kind}' key '{key}' has wrong type: expected {expected}, got {got}"
            ),
            Self::InvalidValueConversion { kind, key, detail } => write!(
                f,
                "effect '{kind}' key '{key}' value cannot be converted: {detail}"
            ),
        }
    }
}

impl std::error::Error for EffectApplyError {}

pub trait EffectHandler: Send + Sync {
    fn kind(&self) -> &str;

    fn apply(
        &self,
        effect: &ActionEffect,
        store: &mut ContextStore,
        provides: &AdapterProvides,
    ) -> Result<Vec<AppliedWrite>, EffectApplyError>;
}

#[derive(Debug, Default)]
pub struct SetContextHandler;

impl EffectHandler for SetContextHandler {
    fn kind(&self) -> &str {
        "set_context"
    }

    fn apply(
        &self,
        effect: &ActionEffect,
        store: &mut ContextStore,
        provides: &AdapterProvides,
    ) -> Result<Vec<AppliedWrite>, EffectApplyError> {
        if effect.kind != self.kind() {
            return Err(EffectApplyError::UnhandledEffectKind {
                kind: effect.kind.clone(),
            });
        }

        let mut applied = Vec::with_capacity(effect.writes.len());
        for write in &effect.writes {
            let Some(context_spec) = provides.context.get(&write.key) else {
                return Err(EffectApplyError::UndeclaredKey {
                    kind: effect.kind.clone(),
                    key: write.key.clone(),
                });
            };

            if !context_spec.writable {
                return Err(EffectApplyError::NonWritableKey {
                    kind: effect.kind.clone(),
                    key: write.key.clone(),
                });
            }

            let got = value_type_name(&write.value).to_string();
            if context_spec.ty != got {
                return Err(EffectApplyError::TypeMismatch {
                    kind: effect.kind.clone(),
                    key: write.key.clone(),
                    expected: context_spec.ty.clone(),
                    got,
                });
            }

            let json_value = runtime_value_to_json(&write.value).ok_or_else(|| {
                EffectApplyError::InvalidValueConversion {
                    kind: effect.kind.clone(),
                    key: write.key.clone(),
                    detail: "non-finite number".to_string(),
                }
            })?;

            store.set(write.key.clone(), json_value.clone());
            applied.push(AppliedWrite {
                key: write.key.clone(),
                value: json_value,
            });
        }

        Ok(applied)
    }
}

fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Number(_) => "Number",
        Value::Series(_) => "Series",
        Value::Bool(_) => "Bool",
        Value::String(_) => "String",
    }
}

fn runtime_value_to_json(value: &Value) -> Option<serde_json::Value> {
    match value {
        Value::Number(n) => serde_json::Number::from_f64(*n).map(serde_json::Value::Number),
        Value::Series(items) => {
            let mut converted = Vec::with_capacity(items.len());
            for item in items {
                let number = serde_json::Number::from_f64(*item)?;
                converted.push(serde_json::Value::Number(number));
            }
            Some(serde_json::Value::Array(converted))
        }
        Value::Bool(b) => Some(serde_json::Value::Bool(*b)),
        Value::String(s) => Some(serde_json::Value::String(s.clone())),
    }
}

#[cfg(test)]
mod tests;
