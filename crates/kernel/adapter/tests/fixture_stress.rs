use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use ergo_adapter::fixture::{parse_fixture, FixtureItem};
use ergo_adapter::ExternalEventKind;
use serde_json::{json, Value};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempJsonl {
    dir: PathBuf,
    path: PathBuf,
}

impl TempJsonl {
    fn new(name: &str, contents: &str) -> Self {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "ergo-adapter-fixture-stress-{}-{}-{}",
            std::process::id(),
            name,
            index
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("fixture.jsonl");
        fs::write(&path, contents).expect("write fixture");
        Self { dir, path }
    }
}

impl Drop for TempJsonl {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn parse_ok(name: &str, contents: &str) -> Vec<FixtureItem> {
    let fixture = TempJsonl::new(name, contents);
    parse_fixture(&fixture.path).unwrap_or_else(|err| panic!("expected parse success, got: {err}"))
}

fn parse_err(name: &str, contents: &str) -> String {
    let fixture = TempJsonl::new(name, contents);
    parse_fixture(&fixture.path).expect_err("expected parse failure")
}

fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "expected error/text to contain {needle:?}\nactual:\n{haystack}"
    );
}

fn assert_event(
    item: &FixtureItem,
    expected_kind: ExternalEventKind,
    expected_id: Option<&str>,
    expected_payload: Option<Value>,
    expected_semantic_kind: Option<&str>,
) {
    match item {
        FixtureItem::Event {
            id,
            kind,
            payload,
            semantic_kind,
        } => {
            assert_eq!(*kind, expected_kind, "event kind mismatch");
            assert_eq!(id.as_deref(), expected_id, "event id mismatch");
            assert_eq!(payload, &expected_payload, "event payload mismatch");
            assert_eq!(
                semantic_kind.as_deref(),
                expected_semantic_kind,
                "semantic_kind mismatch"
            );
        }
        other => panic!("expected event item, got {other:?}"),
    }
}

fn assert_episode_start(item: &FixtureItem, expected_label: &str) {
    match item {
        FixtureItem::EpisodeStart { label } => assert_eq!(label, expected_label),
        other => panic!("expected episode_start item, got {other:?}"),
    }
}

#[test]
fn category1_unknown_fields_fail_with_field_name() {
    let cases = [
        (
            "unknown_event_line_context",
            "{\"kind\":\"event\",\"event\":{\"type\":\"Command\"},\"context\":{\"x\":1.0}}\n",
            "context",
        ),
        (
            "unknown_episode_start_extra",
            "{\"kind\":\"episode_start\",\"id\":\"E1\",\"extra\":\"junk\"}\n",
            "extra",
        ),
        (
            "unknown_inside_event_foo",
            "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"foo\":\"bar\"}}\n",
            "foo",
        ),
        (
            "multiple_unknown_fields",
            "{\"kind\":\"event\",\"event\":{\"type\":\"Command\"},\"a\":1,\"b\":2}\n",
            "a",
        ),
        (
            "unknown_null_value",
            "{\"kind\":\"event\",\"event\":{\"type\":\"Command\"},\"ghost\":null}\n",
            "ghost",
        ),
        (
            "unknown_typo_payloads",
            "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payloads\":{\"x\":1}}}\n",
            "payloads",
        ),
        (
            "unknown_nested_metadata",
            "{\"kind\":\"event\",\"event\":{\"type\":\"Command\"},\"metadata\":{\"source\":\"test\",\"version\":2}}\n",
            "metadata",
        ),
    ];

    for (name, contents, field) in cases {
        let err = parse_err(name, contents);
        assert_contains(&err, "fixture parse error at line 1");
        assert_contains(&err, field);
    }
}

#[test]
fn category2_valid_minimal_event_no_payload_parses() {
    let items = parse_ok(
        "minimal_event",
        "{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
    );
    assert_eq!(items.len(), 1);
    assert_event(&items[0], ExternalEventKind::Command, None, None, None);
}

#[test]
fn category2_valid_event_with_payload_parses() {
    let items = parse_ok(
        "event_with_payload",
        "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":{\"x\":2.5}}}\n",
    );
    assert_eq!(items.len(), 1);
    assert_event(
        &items[0],
        ExternalEventKind::Command,
        None,
        Some(json!({"x": 2.5})),
        None,
    );
}

#[test]
fn category2_valid_event_with_all_optional_fields_parses() {
    let items = parse_ok(
        "event_all_optional",
        "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"id\":\"evt_1\",\"payload\":{\"x\":1},\"semantic_kind\":\"price_tick\"}}\n",
    );
    assert_eq!(items.len(), 1);
    assert_event(
        &items[0],
        ExternalEventKind::Command,
        Some("evt_1"),
        Some(json!({"x": 1})),
        Some("price_tick"),
    );
}

#[test]
fn category2_valid_episode_start_with_id_parses() {
    let items = parse_ok(
        "episode_start_with_id",
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n",
    );
    assert_eq!(items.len(), 1);
    assert_episode_start(&items[0], "E1");
}

#[test]
fn category2_valid_episode_start_without_id_autolabels_on_parse() {
    let items = parse_ok("episode_start_without_id", "{\"kind\":\"episode_start\"}\n");
    assert_eq!(items.len(), 1);
    assert_episode_start(&items[0], "E1");
}

#[test]
fn category2_valid_all_three_event_kinds_parse() {
    let items = parse_ok(
        "all_event_kinds",
        "{\"kind\":\"event\",\"event\":{\"type\":\"Pump\"}}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"DataAvailable\"}}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
    );
    assert_eq!(items.len(), 3);
    assert_event(&items[0], ExternalEventKind::Pump, None, None, None);
    assert_event(
        &items[1],
        ExternalEventKind::DataAvailable,
        None,
        None,
        None,
    );
    assert_event(&items[2], ExternalEventKind::Command, None, None, None);
}

#[test]
fn category2_valid_empty_payload_object_parses() {
    let items = parse_ok(
        "empty_payload",
        "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":{}}}\n",
    );
    assert_eq!(items.len(), 1);
    assert_event(
        &items[0],
        ExternalEventKind::Command,
        None,
        Some(json!({})),
        None,
    );
}

#[test]
fn category2_valid_payload_with_nested_objects_parses() {
    let items = parse_ok(
        "nested_payload",
        "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":{\"ohlcv\":{\"open\":100,\"high\":101,\"low\":99,\"close\":100.5}}}}\n",
    );
    assert_eq!(items.len(), 1);
    assert_event(
        &items[0],
        ExternalEventKind::Command,
        None,
        Some(json!({"ohlcv": {"open": 100, "high": 101, "low": 99, "close": 100.5}})),
        None,
    );
}

#[test]
fn category2_valid_payload_with_arrays_parses() {
    let items = parse_ok(
        "array_payload",
        "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":{\"values\":[1,2,3]}}}\n",
    );
    assert_eq!(items.len(), 1);
    assert_event(
        &items[0],
        ExternalEventKind::Command,
        None,
        Some(json!({"values": [1, 2, 3]})),
        None,
    );
}

#[test]
fn category2_valid_payload_with_string_values_parses() {
    let items = parse_ok(
        "string_payload_values",
        "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":{\"symbol\":\"AAPL\",\"x\":42.0}}}\n",
    );
    assert_eq!(items.len(), 1);
    assert_event(
        &items[0],
        ExternalEventKind::Command,
        None,
        Some(json!({"symbol": "AAPL", "x": 42.0})),
        None,
    );
}

#[test]
fn category2_valid_multi_episode_mixed_fixture_parses() {
    let items = parse_ok(
        "multi_episode_mixed",
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Pump\",\"payload\":{\"x\":1.0}}}\n\
         {\"kind\":\"episode_start\",\"id\":\"E2\"}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"DataAvailable\"}}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n\
         {\"kind\":\"episode_start\",\"id\":\"E3\"}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":{\"x\":2.0}}}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Pump\"}}\n",
    );

    assert_eq!(items.len(), 9);
    assert_episode_start(&items[0], "E1");
    assert_event(&items[1], ExternalEventKind::Command, None, None, None);
    assert_event(
        &items[2],
        ExternalEventKind::Pump,
        None,
        Some(json!({"x": 1.0})),
        None,
    );
    assert_episode_start(&items[3], "E2");
    assert_event(
        &items[4],
        ExternalEventKind::DataAvailable,
        None,
        None,
        None,
    );
    assert_event(&items[5], ExternalEventKind::Command, None, None, None);
    assert_episode_start(&items[6], "E3");
    assert_event(
        &items[7],
        ExternalEventKind::Command,
        None,
        Some(json!({"x": 2.0})),
        None,
    );
    assert_event(&items[8], ExternalEventKind::Pump, None, None, None);
}

#[test]
fn category2_valid_legacy_tick_alias_parses_as_pump() {
    let items = parse_ok(
        "legacy_tick_alias",
        "{\"kind\":\"event\",\"event\":{\"type\":\"Tick\"}}\n",
    );
    assert_eq!(items.len(), 1);
    assert_event(&items[0], ExternalEventKind::Pump, None, None, None);
}

#[test]
fn category3_whitespace_only_file_currently_parses_to_empty_items() {
    let items = parse_ok("whitespace_only", "   \n\t\n\n");
    assert!(
        items.is_empty(),
        "expected empty parsed fixture, got {items:?}"
    );
}

#[test]
fn category3_completely_invalid_json_fails_with_parse_error() {
    let err = parse_err("invalid_json", "{not json at all}\n");
    assert_contains(&err, "fixture parse error at line 1");
    assert_contains(&err, "key must be a string");
}

#[test]
fn category3_valid_json_wrong_shape_fails() {
    let err = parse_err("wrong_shape", "{\"hello\":\"world\"}\n");
    assert_contains(&err, "fixture parse error at line 1");
    assert_contains(&err, "kind");
}

#[test]
fn category3_missing_required_type_field_fails() {
    let err = parse_err("missing_type", "{\"kind\":\"event\",\"event\":{}}\n");
    assert_contains(&err, "fixture parse error at line 1");
    assert_contains(&err, "type");
}

#[test]
fn category3_unknown_event_type_fails() {
    let err = parse_err(
        "unknown_event_type",
        "{\"kind\":\"event\",\"event\":{\"type\":\"FooBar\"}}\n",
    );
    assert_contains(&err, "fixture parse error at line 1");
    assert_contains(&err, "FooBar");
}

#[test]
fn category3_wrong_type_for_id_field_fails() {
    let err = parse_err(
        "wrong_id_type",
        "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"id\":12345}}\n",
    );
    assert_contains(&err, "fixture parse error at line 1");
    assert_contains(&err, "id");
    assert_contains(&err, "string");
}

#[test]
fn category3_string_payload_field_must_be_object() {
    let err = parse_err(
        "payload_string_rejected",
        "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":\"not an object\"}}\n",
    );
    assert_contains(&err, "fixture parse error at line 1");
    assert_contains(&err, "payload must be a JSON object");
    assert_contains(&err, "string");
}

#[test]
fn category3_non_object_payload_field_types_are_rejected() {
    let cases = [
        (
            "payload_array_rejected",
            "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":[1,2,3]}}\n",
            "array",
        ),
        (
            "payload_number_rejected",
            "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":42}}\n",
            "number",
        ),
        (
            "payload_boolean_rejected",
            "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":true}}\n",
            "boolean",
        ),
    ];

    for (name, fixture, ty) in cases {
        let err = parse_err(name, fixture);
        assert_contains(&err, "payload must be a JSON object");
        assert_contains(&err, ty);
    }
}

#[test]
fn category2_null_payload_field_is_treated_as_no_payload() {
    let items = parse_ok(
        "payload_null_means_none",
        "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":null}}\n",
    );
    assert_eq!(items.len(), 1);
    assert_event(&items[0], ExternalEventKind::Command, None, None, None);
}

#[test]
fn category3_kind_field_misspelled_fails() {
    let err = parse_err(
        "kind_misspelled",
        "{\"kind\":\"evnt\",\"event\":{\"type\":\"Command\"}}\n",
    );
    assert_contains(&err, "fixture parse error at line 1");
    assert_contains(&err, "evnt");
}

#[test]
fn category3_duplicate_keys_in_json_fail() {
    let err = parse_err(
        "duplicate_keys",
        "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"type\":\"Pump\"}}\n",
    );
    assert_contains(&err, "fixture parse error at line 1");
    assert_contains(&err, "duplicate field");
    assert_contains(&err, "type");
}

#[test]
fn category3_event_with_no_event_object_fails() {
    let err = parse_err("missing_event_object", "{\"kind\":\"event\"}\n");
    assert_contains(&err, "fixture parse error at line 1");
    assert_contains(&err, "event");
}

#[test]
fn category3_null_event_object_fails() {
    let err = parse_err("null_event_object", "{\"kind\":\"event\",\"event\":null}\n");
    assert_contains(&err, "fixture parse error at line 1");
    assert_contains(&err, "null");
}

#[test]
fn category4_boundary_large_payload_1000_numeric_keys_parses() {
    let mut payload = serde_json::Map::new();
    for i in 0..1000usize {
        payload.insert(format!("k{i}"), json!(i as f64));
    }
    let line = json!({
        "kind": "event",
        "event": {
            "type": "Command",
            "payload": Value::Object(payload.clone())
        }
    });
    let items = parse_ok(
        "large_payload_1000",
        &(serde_json::to_string(&line).expect("serialize line") + "\n"),
    );
    assert_eq!(items.len(), 1);
    assert_event(
        &items[0],
        ExternalEventKind::Command,
        None,
        Some(Value::Object(payload)),
        None,
    );
}

#[test]
fn category4_boundary_unicode_payload_keys_and_values_parse() {
    let items = parse_ok(
        "unicode_payload",
        "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":{\"日本語\":42,\"name\":\"José\"}}}\n",
    );
    assert_eq!(items.len(), 1);
    assert_event(
        &items[0],
        ExternalEventKind::Command,
        None,
        Some(json!({"日本語": 42, "name": "José"})),
        None,
    );
}

#[test]
fn category4_boundary_payload_with_boolean_null_and_numeric_mixed_types_parses() {
    let items = parse_ok(
        "mixed_payload_scalars",
        "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":{\"flag\":true,\"missing\":null,\"n\":4.56,\"i\":2}}}\n",
    );
    assert_eq!(items.len(), 1);
    assert_event(
        &items[0],
        ExternalEventKind::Command,
        None,
        Some(json!({"flag": true, "missing": null, "n": 4.56, "i": 2})),
        None,
    );
}

#[test]
fn category4_boundary_100_events_in_single_episode_parses_and_preserves_order() {
    let mut lines = vec!["{\"kind\":\"episode_start\",\"id\":\"E1\"}".to_string()];
    for i in 0..100usize {
        lines.push(format!(
            "{{\"kind\":\"event\",\"event\":{{\"type\":\"Command\",\"id\":\"evt_{i}\",\"payload\":{{\"x\":{i}}}}}}}"
        ));
    }
    let contents = lines.join("\n") + "\n";
    let items = parse_ok("hundred_events", &contents);

    assert_eq!(items.len(), 101);
    assert_episode_start(&items[0], "E1");
    assert_event(
        &items[1],
        ExternalEventKind::Command,
        Some("evt_0"),
        Some(json!({"x": 0})),
        None,
    );
    assert_event(
        &items[100],
        ExternalEventKind::Command,
        Some("evt_99"),
        Some(json!({"x": 99})),
        None,
    );
}

#[test]
fn category4_boundary_blank_lines_between_valid_lines_are_skipped() {
    let items = parse_ok(
        "blank_lines_skipped",
        "\n  \n{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n \t \n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Pump\"}}\n\n",
    );
    assert_eq!(items.len(), 3);
    assert_episode_start(&items[0], "E1");
    assert_event(&items[1], ExternalEventKind::Command, None, None, None);
    assert_event(&items[2], ExternalEventKind::Pump, None, None, None);
}
