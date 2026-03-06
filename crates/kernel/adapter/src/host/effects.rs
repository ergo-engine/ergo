use crate::host::ContextStore;
use crate::AdapterProvides;
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
mod tests {
    use super::*;
    use crate::provides::ContextKeyProvision;
    use ergo_runtime::common::EffectWrite;
    use std::collections::HashMap;

    fn provides_with_context(
        entries: impl IntoIterator<Item = (&'static str, &'static str, bool)>,
    ) -> AdapterProvides {
        let mut provides = AdapterProvides::default();
        let context = entries
            .into_iter()
            .map(|(key, ty, writable)| {
                (
                    key.to_string(),
                    ContextKeyProvision {
                        ty: ty.to_string(),
                        required: false,
                        writable,
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        provides.context = context;
        provides
    }

    fn set_context_effect(writes: Vec<(&str, Value)>) -> ActionEffect {
        ActionEffect {
            kind: "set_context".to_string(),
            writes: writes
                .into_iter()
                .map(|(key, value)| EffectWrite {
                    key: key.to_string(),
                    value,
                })
                .collect(),
        }
    }

    #[test]
    fn set_context_applies_declared_writable_key() {
        let handler = SetContextHandler;
        let mut store = ContextStore::new();
        let provides = provides_with_context([("ema_fast", "Number", true)]);
        let effect = set_context_effect(vec![("ema_fast", Value::Number(12.5))]);

        let applied = handler
            .apply(&effect, &mut store, &provides)
            .expect("set_context write should apply");

        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].key, "ema_fast");
        assert_eq!(store.get("ema_fast"), Some(&serde_json::json!(12.5)));
    }

    #[test]
    fn set_context_rejects_undeclared_key() {
        let handler = SetContextHandler;
        let mut store = ContextStore::new();
        let provides = provides_with_context([("ema_fast", "Number", true)]);
        let effect = set_context_effect(vec![("ema_slow", Value::Number(21.0))]);

        let err = handler
            .apply(&effect, &mut store, &provides)
            .expect_err("undeclared key must be rejected");

        assert!(matches!(
            err,
            EffectApplyError::UndeclaredKey { kind, key }
            if kind == "set_context" && key == "ema_slow"
        ));
        assert!(store.snapshot().is_empty());
    }

    #[test]
    fn set_context_rejects_non_writable_key() {
        let handler = SetContextHandler;
        let mut store = ContextStore::new();
        let provides = provides_with_context([("trend_label", "String", false)]);
        let effect = set_context_effect(vec![("trend_label", Value::String("up".to_string()))]);

        let err = handler
            .apply(&effect, &mut store, &provides)
            .expect_err("non-writable key must be rejected");

        assert!(matches!(
            err,
            EffectApplyError::NonWritableKey { kind, key }
            if kind == "set_context" && key == "trend_label"
        ));
        assert!(store.snapshot().is_empty());
    }

    #[test]
    fn set_context_rejects_type_mismatch() {
        let handler = SetContextHandler;
        let mut store = ContextStore::new();
        let provides = provides_with_context([("armed", "Bool", true)]);
        let effect = set_context_effect(vec![("armed", Value::Number(1.0))]);

        let err = handler
            .apply(&effect, &mut store, &provides)
            .expect_err("type mismatch must be rejected");

        assert!(matches!(
            err,
            EffectApplyError::TypeMismatch { kind, key, expected, got }
            if kind == "set_context" && key == "armed" && expected == "Bool" && got == "Number"
        ));
        assert!(store.snapshot().is_empty());
    }

    #[test]
    fn set_context_no_rollback_when_later_write_fails() {
        let handler = SetContextHandler;
        let mut store = ContextStore::new();
        let provides = provides_with_context([("ema_fast", "Number", true)]);
        let effect = set_context_effect(vec![
            ("ema_fast", Value::Number(10.0)),
            ("ema_slow", Value::Number(20.0)),
        ]);

        let err = handler
            .apply(&effect, &mut store, &provides)
            .expect_err("second undeclared write should fail");

        assert!(matches!(
            err,
            EffectApplyError::UndeclaredKey { kind, key }
            if kind == "set_context" && key == "ema_slow"
        ));
        // SUP-6 alignment: partial writes remain applied; no transactional rollback.
        assert_eq!(store.get("ema_fast"), Some(&serde_json::json!(10.0)));
    }
}
