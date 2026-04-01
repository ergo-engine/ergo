//! fixture_ops tests
//!
//! Purpose:
//! - Lock the CLI fixture inspect/validate handlers and their rendered output
//!   contracts.
//!
//! Owns:
//! - Temp-fixture scenario tests for fixture report command behavior.
//!
//! Does not own:
//! - Production CLI handler logic in `fixture_ops.rs`.

use super::*;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_fixture(contents: &str) -> PathBuf {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "ergo-cli-fixture-ops-test-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("fixture.jsonl");
    fs::write(&path, contents).expect("write fixture");
    path
}

#[test]
fn inspect_text_reports_counts() {
    let path = write_fixture(
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Pump\",\"payload\":{\"price\":1.23},\"semantic_kind\":\"market.tick\"}}\n",
    );
    let out = fixture_inspect_command(&[path.to_string_lossy().to_string()])
        .expect("inspect should succeed");
    assert!(out.contains("fixture inspect"), "out: {out}");
    assert!(out.contains("episode_count: 1"), "out: {out}");
    assert!(out.contains("event_count: 2"), "out: {out}");
    assert!(out.contains("events_with_payload: 1"), "out: {out}");
    assert!(out.contains("events_with_semantic_kind: 1"), "out: {out}");
    assert!(out.contains("Command: 1"), "out: {out}");
    assert!(out.contains("Pump: 1"), "out: {out}");
}

#[test]
fn inspect_json_uses_v1_schema() {
    let path = write_fixture("{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n");
    let out = fixture_inspect_command(&[
        path.to_string_lossy().to_string(),
        "--format".to_string(),
        "json".to_string(),
    ])
    .expect("inspect json should succeed");
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid json");
    assert_eq!(parsed["schema_version"], "v1");
    assert_eq!(parsed["command"], "fixture.inspect");
    assert_eq!(parsed["stats"]["event_count"], 1);
}

#[test]
fn validate_json_reports_invalid_episode() {
    let path = write_fixture("{\"kind\":\"episode_start\",\"id\":\"E1\"}\n");
    let err = fixture_validate_command(&[
        path.to_string_lossy().to_string(),
        "--format".to_string(),
        "json".to_string(),
    ])
    .expect_err("validate should fail");
    let parsed: serde_json::Value = serde_json::from_str(&err).expect("valid json");
    assert_eq!(parsed["schema_version"], "v1");
    assert_eq!(parsed["command"], "fixture.validate");
    assert_eq!(parsed["valid"], false);
    assert_eq!(parsed["issues"][0]["code"], "fixture.no_events");
}

#[test]
fn validate_text_succeeds_for_event_only_fixture() {
    let path = write_fixture("{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n");
    let out = fixture_validate_command(&[path.to_string_lossy().to_string()])
        .expect("validate should pass");
    assert!(out.contains("fixture valid"), "out: {out}");
    assert!(out.contains("issues: (none)"), "out: {out}");
}

#[test]
fn inspect_requires_fixture_path() {
    let err = fixture_inspect_command(&[]).expect_err("missing path should fail");
    assert!(
        err.contains("fixture inspect requires <events.jsonl>"),
        "unexpected err: {err}"
    );
}
