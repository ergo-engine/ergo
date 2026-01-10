use std::collections::HashMap;

use crate::common::Value;
use crate::runtime::ExecutionContext;
use crate::source::{
    BooleanSource, ContextNumberSource, NumberSource, SourcePrimitive, StringSource,
};

fn expect_panic<F: FnOnce() -> R + std::panic::UnwindSafe, R>(f: F) {
    assert!(std::panic::catch_unwind(f).is_err());
}

#[test]
fn number_source_requires_parameter() {
    let source = NumberSource::new();
    let ctx = ExecutionContext::default();
    let outputs = source.produce(
        &HashMap::from([(
            "value".to_string(),
            crate::source::ParameterValue::Number(3.5),
        )]),
        &ctx,
    );
    assert_eq!(outputs.get("value"), Some(&Value::Number(3.5)));

    expect_panic(|| {
        source.produce(&HashMap::new(), &ctx);
    });
}

#[test]
fn boolean_source_requires_parameter() {
    let source = BooleanSource::new();
    let ctx = ExecutionContext::default();
    let outputs = source.produce(
        &HashMap::from([(
            "value".to_string(),
            crate::source::ParameterValue::Bool(true),
        )]),
        &ctx,
    );
    assert_eq!(outputs.get("value"), Some(&Value::Bool(true)));

    expect_panic(|| {
        source.produce(&HashMap::new(), &ctx);
    });
}

#[test]
fn string_source_emits_configured_value() {
    let source = StringSource::new();
    let ctx = ExecutionContext::default();
    let outputs = source.produce(
        &HashMap::from([(
            "value".to_string(),
            crate::source::ParameterValue::String("hello".to_string()),
        )]),
        &ctx,
    );
    assert_eq!(
        outputs.get("value"),
        Some(&Value::String("hello".to_string()))
    );
}

#[test]
fn string_source_defaults_to_empty_string() {
    let source = StringSource::new();
    let ctx = ExecutionContext::default();
    let outputs = source.produce(&HashMap::new(), &ctx);
    assert_eq!(outputs.get("value"), Some(&Value::String(String::new())));
}

#[test]
fn context_number_source_reads_context_value() {
    let source = ContextNumberSource::new();
    let ctx = ExecutionContext::from_values(HashMap::from([("x".to_string(), Value::Number(9.5))]));

    let outputs = source.produce(&HashMap::new(), &ctx);
    assert_eq!(outputs.get("value"), Some(&Value::Number(9.5)));
}

#[test]
fn context_number_source_missing_key_returns_default() {
    let source = ContextNumberSource::new();
    // Context has no "x" key
    let ctx = ExecutionContext::from_values(HashMap::from([(
        "other_key".to_string(),
        Value::Number(99.0),
    )]));

    let outputs = source.produce(&HashMap::new(), &ctx);
    assert_eq!(outputs.get("value"), Some(&Value::Number(0.0)));
}

#[test]
fn context_number_source_wrong_type_returns_default() {
    let source = ContextNumberSource::new();
    // Context has "x" key but wrong type (String instead of Number)
    let ctx = ExecutionContext::from_values(HashMap::from([(
        "x".to_string(),
        Value::String("not a number".to_string()),
    )]));

    let outputs = source.produce(&HashMap::new(), &ctx);
    assert_eq!(outputs.get("value"), Some(&Value::Number(0.0)));
}
