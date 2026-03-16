use std::collections::{HashMap, HashSet};

use ergo_adapter::event_binding::{
    bind_semantic_event_with_binder, compile_event_binder, EventBindingError,
};
use ergo_adapter::provides::{AdapterProvides, ContextKeyProvision};
use ergo_adapter::{EventId, EventTime, ExternalEventKind};
use serde_json::json;

fn provision(ty: &str) -> ContextKeyProvision {
    ContextKeyProvision {
        ty: ty.to_string(),
        required: true,
        writable: false,
    }
}

fn make_adapter_provides(
    context: HashMap<String, ContextKeyProvision>,
    event_schemas: HashMap<String, serde_json::Value>,
) -> AdapterProvides {
    AdapterProvides {
        context,
        events: event_schemas.keys().cloned().collect::<HashSet<_>>(),
        effects: HashSet::new(),
        effect_schemas: HashMap::new(),
        event_schemas,
        capture_format_version: "1".to_string(),
        adapter_fingerprint: "adapter:test@1.0.0;sha256:test".to_string(),
    }
}

fn price_tick_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "price": {"type": "number"},
            "symbol": {"type": "string"}
        },
        "required": ["price", "symbol"],
        "additionalProperties": false
    })
}

#[test]
fn unknown_semantic_kind_rejected() {
    let provides = make_adapter_provides(
        HashMap::from([
            ("price".to_string(), provision("Number")),
            ("symbol".to_string(), provision("String")),
        ]),
        HashMap::from([("PriceTick".to_string(), price_tick_schema())]),
    );
    let binder = compile_event_binder(&provides).expect("binder should compile");

    let err = bind_semantic_event_with_binder(
        &binder,
        EventId::new("e1"),
        ExternalEventKind::Command,
        EventTime::default(),
        "UnknownKind",
        json!({"price": 1.0, "symbol": "BTC"}),
    )
    .unwrap_err();

    assert!(matches!(err, EventBindingError::UnknownSemanticKind { .. }));
}

#[test]
fn invalid_schema_rejected() {
    let provides = make_adapter_provides(
        HashMap::from([("price".to_string(), provision("Number"))]),
        HashMap::from([("Broken".to_string(), json!({"type": 7}))]),
    );

    let err = compile_event_binder(&provides).unwrap_err();

    assert!(matches!(err, EventBindingError::InvalidSchema { .. }));
}

#[test]
fn payload_schema_mismatch_rejected() {
    let provides = make_adapter_provides(
        HashMap::from([
            ("price".to_string(), provision("Number")),
            ("symbol".to_string(), provision("String")),
        ]),
        HashMap::from([("PriceTick".to_string(), price_tick_schema())]),
    );
    let binder = compile_event_binder(&provides).expect("binder should compile");

    let err = bind_semantic_event_with_binder(
        &binder,
        EventId::new("e1"),
        ExternalEventKind::Command,
        EventTime::default(),
        "PriceTick",
        json!({"price": "not-number", "symbol": "BTC"}),
    )
    .unwrap_err();

    assert!(matches!(
        err,
        EventBindingError::PayloadSchemaMismatch { .. }
    ));
}

#[test]
fn payload_must_be_object_rejected() {
    let provides = make_adapter_provides(
        HashMap::new(),
        HashMap::from([("Scalar".to_string(), json!({"type": "number"}))]),
    );
    let binder = compile_event_binder(&provides).expect("binder should compile");

    let err = bind_semantic_event_with_binder(
        &binder,
        EventId::new("e1"),
        ExternalEventKind::Command,
        EventTime::default(),
        "Scalar",
        json!(12.3),
    )
    .unwrap_err();

    assert!(matches!(err, EventBindingError::PayloadMustBeObject { .. }));
}

#[test]
fn missing_context_provision_rejected() {
    let provides = make_adapter_provides(
        HashMap::new(),
        HashMap::from([(
            "PriceOnly".to_string(),
            json!({
                "type": "object",
                "properties": {"price": {"type": "number"}},
                "required": ["price"],
                "additionalProperties": false
            }),
        )]),
    );

    let err = compile_event_binder(&provides).unwrap_err();

    assert!(matches!(
        err,
        EventBindingError::MissingContextProvision { .. }
    ));
}

#[test]
fn context_type_mismatch_rejected() {
    let provides = make_adapter_provides(
        HashMap::from([("price".to_string(), provision("String"))]),
        HashMap::from([(
            "PriceOnly".to_string(),
            json!({
                "type": "object",
                "properties": {"price": {"type": "number"}},
                "required": ["price"],
                "additionalProperties": false
            }),
        )]),
    );

    let err = compile_event_binder(&provides).unwrap_err();

    assert!(matches!(err, EventBindingError::ContextTypeMismatch { .. }));
}

#[test]
fn unsupported_field_type_rejected() {
    let provides = make_adapter_provides(
        HashMap::from([("nested".to_string(), provision("String"))]),
        HashMap::from([(
            "Nested".to_string(),
            json!({
                "type": "object",
                "properties": {"nested": {"type": "object"}},
                "required": ["nested"],
                "additionalProperties": false
            }),
        )]),
    );

    let err = compile_event_binder(&provides).unwrap_err();

    assert!(matches!(
        err,
        EventBindingError::UnsupportedFieldType { .. }
    ));
}

#[test]
fn series_mapping_success() {
    let provides = make_adapter_provides(
        HashMap::from([("samples".to_string(), provision("Series"))]),
        HashMap::from([(
            "SeriesTick".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "samples": {
                        "type": "array",
                        "items": {"type": "number"}
                    }
                },
                "required": ["samples"],
                "additionalProperties": false
            }),
        )]),
    );
    let binder = compile_event_binder(&provides).expect("binder should compile");

    let event = bind_semantic_event_with_binder(
        &binder,
        EventId::new("e1"),
        ExternalEventKind::Command,
        EventTime::default(),
        "SeriesTick",
        json!({"samples": [1.0, 2.5, 3.75]}),
    )
    .expect("series payload should bind");

    let payload: serde_json::Value =
        serde_json::from_slice(&event.payload().data).expect("payload bytes should decode");
    assert_eq!(payload["samples"], json!([1.0, 2.5, 3.75]));
}

#[test]
fn valid_payload_emits_expected_payload() {
    let provides = make_adapter_provides(
        HashMap::from([
            ("price".to_string(), provision("Number")),
            ("symbol".to_string(), provision("String")),
        ]),
        HashMap::from([("PriceTick".to_string(), price_tick_schema())]),
    );
    let binder = compile_event_binder(&provides).expect("binder should compile");

    let event = bind_semantic_event_with_binder(
        &binder,
        EventId::new("e1"),
        ExternalEventKind::Command,
        EventTime::default(),
        "PriceTick",
        json!({"price": 12.3, "symbol": "BTC"}),
    )
    .expect("binding should succeed");

    assert_eq!(event.kind(), ExternalEventKind::Command);
    assert_eq!(event.event_id().as_str(), "e1");
    let payload: serde_json::Value =
        serde_json::from_slice(&event.payload().data).expect("payload bytes should decode");
    assert_eq!(payload["price"], json!(12.3));
    assert_eq!(payload["symbol"], json!("BTC"));
}
