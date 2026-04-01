//! report tests
//!
//! Purpose:
//! - Lock the shared fixture reporting helpers without crowding the production
//!   reporting module.
//!
//! Owns:
//! - Temp-fixture scenario tests for inspect/validate/report rendering.
//!
//! Does not own:
//! - Production report DTOs or analysis logic in `report.rs`.

use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_fixture(contents: &str) -> PathBuf {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "ergo-fixtures-report-test-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("fixture.jsonl");
    fs::write(&path, contents).expect("write fixture");
    path
}

#[test]
fn inspect_reports_counts() {
    let path = write_fixture(
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Pump\",\"payload\":{\"price\":1.23},\"semantic_kind\":\"market.tick\"}}\n",
    );

    let analysis = inspect_fixture(&path).expect("inspect should succeed");
    assert_eq!(analysis.event_count, 2);
    assert_eq!(analysis.events_with_payload, 1);
    assert_eq!(analysis.events_with_semantic_kind, 1);
    assert_eq!(analysis.event_kind_counts.get("Command"), Some(&1));
    assert_eq!(analysis.event_kind_counts.get("Pump"), Some(&1));
}

#[test]
fn validate_reports_invalid_episode() {
    let path = write_fixture("{\"kind\":\"episode_start\",\"id\":\"E1\"}\n");
    let report = validate_fixture(&path);
    assert!(!report.valid);
    assert_eq!(report.issues[0].code, "fixture.no_events");
}

#[test]
fn render_json_uses_v1_schema() {
    let path = write_fixture("{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n");
    let analysis = inspect_fixture(&path).expect("inspect should succeed");
    let stats = stats_from_analysis(&analysis);
    let json = render_inspect_json(&path, stats).expect("render json");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(parsed["schema_version"], "v1");
    assert_eq!(parsed["command"], "fixture.inspect");
    assert_eq!(parsed["stats"]["event_count"], 1);
}

#[test]
fn validate_parse_error_message_stays_path_independent() {
    let path = write_fixture(
        "{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":\"oops\"}}\n",
    );
    let report = validate_fixture(&path);

    assert!(!report.valid);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].code, "fixture.parse_error");
    assert_eq!(
        report.issues[0].message,
        "fixture parse error at line 1: payload must be a JSON object, got string"
    );
    assert!(
        !report.issues[0]
            .message
            .contains(&path.display().to_string()),
        "issue message should not duplicate fixture_path"
    );
}
