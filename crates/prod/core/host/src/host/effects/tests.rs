use super::*;
use ergo_adapter::ContextKeyProvision;
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
        intents: vec![],
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
