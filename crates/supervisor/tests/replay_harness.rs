use std::time::Duration;

use ergo_adapter::capture::{hash_payload, ExternalEventRecord};
use ergo_adapter::{
    EventId, EventPayload, EventTime, ExternalEvent, ExternalEventKind, FaultRuntimeHandle,
    RunTermination,
};
use ergo_supervisor::replay::{replay, replay_checked, replay_checked_strict, ReplayError};
use ergo_supervisor::{
    CaptureBundle, Constraints, Decision, EpisodeInvocationRecord, NO_ADAPTER_PROVENANCE,
};
use serde_json;

fn make_event_record(id: &str, at: Duration) -> ExternalEventRecord {
    // Use Command, not Pump — Pump has special deferred-retry behavior
    let event = ExternalEvent::mechanical_at(
        EventId::new(id.to_string()),
        ExternalEventKind::Command,
        EventTime::from_duration(at),
    );
    ExternalEventRecord::from_event(&event)
}

fn make_payload_record(id: &str, at: Duration, payload: &[u8]) -> ExternalEventRecord {
    let event = ExternalEvent::with_payload(
        EventId::new(id.to_string()),
        ExternalEventKind::Command,
        EventTime::from_duration(at),
        EventPayload {
            data: payload.to_vec(),
        },
    );
    ExternalEventRecord::from_event(&event)
}

fn baseline_bundle(events: Vec<ExternalEventRecord>, constraints: Constraints) -> CaptureBundle {
    CaptureBundle {
        capture_version: "v1".to_string(),
        graph_id: ergo_adapter::GraphId::new("g"),
        config: constraints,
        events,
        decisions: Vec::new(),
        adapter_provenance: NO_ADAPTER_PROVENANCE.to_string(),
    }
}

fn extract(bundle: &CaptureBundle, runtime: FaultRuntimeHandle) -> Vec<EpisodeInvocationRecord> {
    replay(bundle, runtime)
}

#[test]
fn deterministic_schedule_equivalence() {
    let events = vec![
        make_event_record("e1", Duration::from_secs(0)),
        make_event_record("e2", Duration::from_secs(1)),
    ];
    let bundle = baseline_bundle(events, Constraints::default());

    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);
    let first = extract(&bundle, runtime.clone());
    let second = extract(&bundle, runtime);

    assert_eq!(first, second, "replay should be deterministic");
}

#[test]
fn concurrency_cap_determinism() {
    let events = vec![
        make_event_record("e1", Duration::from_secs(0)),
        make_event_record("e2", Duration::from_secs(0)),
        make_event_record("e3", Duration::from_secs(0)),
    ];
    let mut constraints = Constraints::default();
    constraints.max_in_flight = Some(0);

    let bundle = baseline_bundle(events, constraints);
    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);
    let first = extract(&bundle, runtime.clone());
    let second = extract(&bundle, runtime);

    assert_eq!(first, second);
    assert!(first.iter().all(|r| r.decision == Decision::Defer));
    assert!(first.iter().all(|r| r.termination.is_none()));
}

#[test]
fn rate_limit_determinism() {
    let events = vec![
        make_event_record("e1", Duration::from_secs(0)),
        make_event_record("e2", Duration::from_secs(0)),
        make_event_record("e3", Duration::from_secs(0)),
    ];
    let mut constraints = Constraints::default();
    constraints.max_per_window = Some(2);
    constraints.rate_window = Some(Duration::from_secs(10));

    let bundle = baseline_bundle(events, constraints);
    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);
    let first = extract(&bundle, runtime.clone());
    let second = extract(&bundle, runtime);

    assert_eq!(first, second);
    assert_eq!(first[2].decision, Decision::Defer);
    assert_eq!(
        first[2].schedule_at,
        Some(EventTime::from_duration(Duration::from_secs(10)))
    );
    assert!(first[2].termination.is_none());
}

#[test]
fn retry_only_on_mechanical_failures() {
    let events = vec![make_event_record("e1", Duration::from_secs(0))];
    let mut constraints = Constraints::default();
    constraints.max_retries = 1;

    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);
    runtime.push_outcomes(
        EventId::new("e1"),
        vec![
            RunTermination::Failed(ergo_adapter::ErrKind::NetworkTimeout),
            RunTermination::Completed,
        ],
    );

    let bundle = baseline_bundle(events, constraints);
    let records = extract(&bundle, runtime);
    assert_eq!(records[0].termination, Some(RunTermination::Completed));
    assert_eq!(records[0].retry_count, 1);
}

#[test]
fn deadline_path_determinism() {
    let events = vec![make_event_record("e1", Duration::from_secs(0))];
    let mut constraints = Constraints::default();
    constraints.deadline = Some(Duration::ZERO);

    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);
    let bundle = baseline_bundle(events, constraints);

    let records = extract(&bundle, runtime);
    assert_eq!(records[0].termination, Some(RunTermination::Aborted));
}

#[test]
fn payload_hashes_are_stable() {
    let payload = EventPayload {
        data: b"abc".to_vec(),
    };
    let record = make_payload_record("e1", Duration::from_secs(0), &payload.data);
    assert_eq!(record.payload_hash, hash_payload(&payload));
    assert!(record.validate_hash());
}

/// SUP-NOW-1: Wall-clock ban must cover entire supervisor crate.
/// Bans SystemTime::now, Instant::now, and related patterns to ensure deterministic replay.
///
/// SUP-NOW-SCAN-1: Defense-in-depth static scan. This is a cheap, deterministic check
/// that catches common wall-clock usage patterns. It does NOT catch all aliasing
/// (e.g., `use std::time::Instant::now; now()` or macro-generated code).
/// For full assurance, replay tests must exercise all code paths.
#[test]
fn no_wall_clock_usage() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let src_dir = std::path::Path::new(&manifest_dir).join("src");

    // Forbidden patterns (SUP-NOW-SCAN-1):
    // - Explicit type::now() calls
    // - Broader `::now()` catches aliased type names (e.g., `ST::now()`)
    let forbidden_patterns = [
        "SystemTime::now",
        "Instant::now",
        "::now()", // Broader catch for aliased imports like `use std::time::Instant as I; I::now()`
    ];

    for entry in std::fs::read_dir(&src_dir).expect("failed to read src directory") {
        let entry = entry.expect("failed to read directory entry");
        let path = entry.path();

        if path.extension().map_or(false, |ext| ext == "rs") {
            let contents = std::fs::read_to_string(&path).expect("failed to read source file");
            let filename = path.file_name().unwrap().to_string_lossy();

            for pattern in &forbidden_patterns {
                assert!(
                    !contents.contains(pattern),
                    "wall clock usage '{}' found in {}: forbidden by SUP-NOW-1",
                    pattern,
                    filename
                );
            }
        }
    }
}

#[test]
fn sample_bundle_deserializes() {
    let data = include_str!("data/capture_v1_sample.json");
    let bundle: CaptureBundle = serde_json::from_str(data).expect("sample bundle should parse");
    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);
    let records = replay(&bundle, runtime);
    assert_eq!(records.len(), bundle.events.len());
}

#[test]
fn legacy_adapter_version_field_fails_deserialization() {
    let legacy = r#"{
        "capture_version":"v1",
        "graph_id":"g",
        "config":{"max_in_flight":null,"max_per_window":null,"rate_window":null,"deadline":null,"max_retries":0},
        "events":[],
        "decisions":[],
        "adapter_version":"1.2.3"
    }"#;

    let err = serde_json::from_str::<CaptureBundle>(legacy).expect_err("legacy bundle should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("unknown field `adapter_version`"),
        "unexpected error: {msg}"
    );
}

#[test]
fn replay_rejects_corrupted_bundle() {
    let events = vec![make_payload_record("e_bad", Duration::from_secs(0), b"abc")];
    let mut bundle = baseline_bundle(events, Constraints::default());

    // Corrupt the payload without updating the hash to force mismatch.
    bundle.events[0].payload.data = b"def".to_vec();

    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);
    let err = replay_checked(&bundle, runtime).unwrap_err();

    assert!(matches!(
        err,
        ReplayError::HashMismatch { ref event_id } if event_id == &EventId::new("e_bad")
    ));
}

#[test]
fn replay_rejects_unknown_version() {
    let events = vec![make_event_record("e_version", Duration::from_secs(0))];
    let mut bundle = baseline_bundle(events, Constraints::default());
    bundle.capture_version = "v999".to_string();

    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);
    let err = replay_checked(&bundle, runtime).unwrap_err();

    assert!(matches!(
        err,
        ReplayError::UnsupportedVersion { ref capture_version } if capture_version == "v999"
    ));
}

#[test]
fn strict_replay_requires_adapter_for_provenanced_capture() {
    let events = vec![make_event_record("e1", Duration::from_secs(0))];
    let mut bundle = baseline_bundle(events, Constraints::default());
    bundle.adapter_provenance = "adapter:oanda@1.0.0;sha256:abc".to_string();

    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);
    let err = replay_checked_strict(&bundle, runtime, None).unwrap_err();
    assert!(matches!(
        err,
        ReplayError::AdapterRequiredForProvenancedCapture
    ));
}

#[test]
fn strict_replay_rejects_provenance_mismatch() {
    let events = vec![make_event_record("e1", Duration::from_secs(0))];
    let mut bundle = baseline_bundle(events, Constraints::default());
    bundle.adapter_provenance = "adapter:oanda@1.0.0;sha256:abc".to_string();

    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);
    let err = replay_checked_strict(&bundle, runtime, Some("adapter:oanda@1.0.0;sha256:def"))
        .unwrap_err();
    assert!(matches!(err, ReplayError::AdapterProvenanceMismatch { .. }));
}

#[test]
fn strict_replay_accepts_matching_provenance() {
    let events = vec![make_event_record("e1", Duration::from_secs(0))];
    let mut bundle = baseline_bundle(events, Constraints::default());
    bundle.adapter_provenance = "adapter:oanda@1.0.0;sha256:abc".to_string();

    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);
    let result = replay_checked_strict(&bundle, runtime, Some("adapter:oanda@1.0.0;sha256:abc"));
    assert!(result.is_ok());
}

#[test]
fn strict_replay_rejects_adapter_for_no_adapter_capture() {
    let events = vec![make_event_record("e1", Duration::from_secs(0))];
    let bundle = baseline_bundle(events, Constraints::default());

    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);
    let err = replay_checked_strict(&bundle, runtime, Some("adapter:oanda@1.0.0;sha256:abc"))
        .unwrap_err();
    assert!(matches!(
        err,
        ReplayError::UnexpectedAdapterProvidedForNoAdapterCapture
    ));
}

#[test]
fn strict_replay_accepts_none_provenance_without_adapter() {
    let events = vec![make_event_record("e1", Duration::from_secs(0))];
    let bundle = baseline_bundle(events, Constraints::default());

    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);
    let result = replay_checked_strict(&bundle, runtime, None);
    assert!(result.is_ok());
}

/// REP-1b: Point-of-use hash verification catches mid-stream corruption.
#[test]
fn replay_rejects_mid_stream_corruption() {
    // Build a bundle with 2 events: first is valid, second is corrupted
    let record1 = make_payload_record("evt-1", Duration::from_secs(0), b"first");
    let mut record2 = make_payload_record("evt-2", Duration::from_secs(1), b"second");

    // Corrupt event2's payload but leave hash unchanged
    record2.payload.data = b"corrupted".to_vec();

    let bundle = baseline_bundle(vec![record1, record2], Constraints::default());

    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);
    let result = replay_checked(&bundle, runtime);

    assert!(matches!(
        result,
        Err(ReplayError::HashMismatch { ref event_id }) if event_id == &EventId::new("evt-2")
    ));
}

/// RENAME-TICK-1: Old captures with "Tick" deserialize to Pump via serde alias.
/// This ensures backward compatibility for captures created before the rename.
#[test]
fn legacy_tick_deserializes_to_pump() {
    // JSON with legacy "Tick" value (simulating old capture format)
    let legacy_json = r#"{
        "event_id": "legacy-tick-event",
        "event_time": { "secs": 0, "nanos": 0 },
        "kind": "Tick",
        "payload": [],
        "payload_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    }"#;

    let record: ExternalEventRecord =
        serde_json::from_str(legacy_json).expect("legacy Tick should deserialize");

    assert_eq!(
        record.kind,
        ExternalEventKind::Pump,
        "legacy 'Tick' must deserialize to Pump variant"
    );
}
