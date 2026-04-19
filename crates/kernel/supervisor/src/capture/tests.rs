//! capture tests
//!
//! Purpose:
//! - Lock the supervisor capture write seam and kernel-side decision-record
//!   materialization behavior.
//!
//! Owns:
//! - Scenario-heavy tests for atomic file writing and default-empty decision
//!   record effects before host enrichment.
//!
//! Does not own:
//! - Production capture/write implementation logic in `capture.rs`.
//!
//! Safety notes:
//! - These tests intentionally exercise temp-dir, overwrite, concurrency, and
//!   partial-failure cases outside the production file body.

use super::*;
use crate::Constraints;
use ergo_adapter::GraphId;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir(label: &str) -> PathBuf {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "ergo-supervisor-capture-{label}-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn sample_bundle() -> CaptureBundle {
    CaptureBundle {
        capture_version: crate::CAPTURE_FORMAT_VERSION.to_string(),
        graph_id: GraphId::new("capture_test"),
        config: Constraints::default(),
        events: Vec::new(),
        decisions: Vec::new(),
        adapter_provenance: crate::NO_ADAPTER_PROVENANCE.to_string(),
        runtime_provenance: "rpv1:sha256:test".to_string(),
        egress_provenance: None,
    }
}

#[test]
fn writes_compact_json_with_trailing_newline() {
    let dir = temp_dir("compact");
    let path = dir.join("capture.json");
    write_capture_bundle(&path, &sample_bundle(), CaptureJsonStyle::Compact)
        .expect("compact write should succeed");

    let raw = fs::read_to_string(&path).expect("read capture");
    assert!(raw.ends_with('\n'), "expected trailing newline");
    assert_eq!(
        raw.matches('\n').count(),
        1,
        "compact output should be single-line"
    );
    serde_json::from_str::<CaptureBundle>(&raw).expect("capture json should parse");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn writes_pretty_json_with_trailing_newline() {
    let dir = temp_dir("pretty");
    let path = dir.join("capture.json");
    write_capture_bundle(&path, &sample_bundle(), CaptureJsonStyle::Pretty)
        .expect("pretty write should succeed");

    let raw = fs::read_to_string(&path).expect("read capture");
    assert!(raw.ends_with('\n'), "expected trailing newline");
    assert!(
        raw.matches('\n').count() > 1,
        "pretty output should contain multiple lines"
    );
    serde_json::from_str::<CaptureBundle>(&raw).expect("capture json should parse");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn atomic_replace_overwrites_existing_file_cleanly() {
    let dir = temp_dir("replace");
    let path = dir.join("capture.json");
    fs::write(&path, "old-content\n").expect("write original file");

    write_capture_bundle(&path, &sample_bundle(), CaptureJsonStyle::Compact)
        .expect("atomic overwrite should succeed");

    let raw = fs::read_to_string(&path).expect("read capture");
    assert_ne!(raw, "old-content\n", "expected replacement");
    assert!(raw.ends_with('\n'), "expected trailing newline");
    serde_json::from_str::<CaptureBundle>(&raw).expect("capture json should parse");

    let temp_glob = format!("capture.json.{}.*.tmp", std::process::id());
    let leftovers = std::fs::read_dir(&dir)
        .expect("read temp dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| {
            let prefix = format!("capture.json.{}.", std::process::id());
            name.starts_with(&prefix) && name.ends_with(".tmp")
        })
        .collect::<Vec<_>>();
    assert!(
        leftovers.is_empty(),
        "temp files should not remain after success (pattern: {temp_glob})"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn concurrent_writes_to_same_destination_succeed() {
    let dir = temp_dir("concurrent");
    let path = dir.join("capture.json");
    let mut handles = Vec::new();

    for idx in 0..8 {
        let path = path.clone();
        handles.push(std::thread::spawn(move || {
            let mut bundle = sample_bundle();
            bundle.graph_id = GraphId::new(format!("capture_test_{idx}"));
            write_capture_bundle(&path, &bundle, CaptureJsonStyle::Compact)
        }));
    }

    for handle in handles {
        handle
            .join()
            .expect("thread panicked")
            .expect("writer should succeed");
    }

    let raw = fs::read_to_string(&path).expect("read capture");
    assert!(raw.ends_with('\n'), "expected trailing newline");
    serde_json::from_str::<CaptureBundle>(&raw).expect("capture json should parse");

    let _ = fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn failed_write_leaves_existing_destination_untouched() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir("failure");
    let path = dir.join("capture.json");
    fs::write(&path, "old-content\n").expect("write original file");

    let mut perms = fs::metadata(&dir).expect("dir metadata").permissions();
    perms.set_mode(0o555);
    fs::set_permissions(&dir, perms.clone()).expect("set dir readonly");

    let result = write_capture_bundle(&path, &sample_bundle(), CaptureJsonStyle::Compact);
    assert!(result.is_err(), "write should fail in readonly directory");

    let current = fs::read_to_string(&path).expect("read original file");
    assert_eq!(
        current, "old-content\n",
        "destination should remain unchanged"
    );

    perms.set_mode(0o755);
    fs::set_permissions(&dir, perms).expect("restore dir permissions");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn capturing_log_leaves_effects_empty_without_host_enrichment() {
    let bundle = Arc::new(Mutex::new(CaptureBundle {
        capture_version: crate::CAPTURE_FORMAT_VERSION.to_string(),
        graph_id: GraphId::new("hash_test"),
        config: Constraints::default(),
        events: Vec::new(),
        decisions: Vec::new(),
        adapter_provenance: crate::NO_ADAPTER_PROVENANCE.to_string(),
        runtime_provenance: "rpv1:sha256:test".to_string(),
        egress_provenance: None,
    }));

    let inner = crate::replay::MemoryDecisionLog::default();
    let capturing_log = CapturingDecisionLog::new(inner, Arc::clone(&bundle));

    let entry = crate::DecisionLogEntry {
        graph_id: GraphId::new("hash_test"),
        event_id: ergo_adapter::EventId::new("e1"),
        event: ergo_adapter::ExternalEvent::mechanical(
            ergo_adapter::EventId::new("e1"),
            ergo_adapter::ExternalEventKind::Command,
        ),
        decision: crate::Decision::Invoke,
        schedule_at: None,
        episode_id: crate::EpisodeId::new(0),
        deadline: None,
        termination: Some(ergo_adapter::RunTermination::Completed),
        retry_count: 0,
    };

    capturing_log.log(entry);

    let guard = bundle.lock().expect("bundle poisoned");
    assert_eq!(guard.decisions.len(), 1);
    let record = &guard.decisions[0];
    assert!(
        record.effects.is_empty(),
        "kernel capture should leave effects empty until host enrichment runs"
    );
}
