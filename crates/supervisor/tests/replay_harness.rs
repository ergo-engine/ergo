use std::time::Duration;

use ergo_adapter::capture::{hash_payload, ExternalEventRecord};
use ergo_adapter::{
    EventId, EventPayload, EventTime, ExternalEvent, ExternalEventKind, FaultRuntimeHandle,
    RunTermination,
};
use ergo_supervisor::replay::{replay, replay_checked, ReplayError};
use ergo_supervisor::{CaptureBundle, Constraints, Decision, EpisodeInvocationRecord};
use serde_json;

fn make_event_record(id: &str, at: Duration) -> ExternalEventRecord {
    // Use Command, not Tick — Tick now has special deferred-retry behavior
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
        capture_version: "v0".to_string(),
        graph_id: ergo_adapter::GraphId::new("g"),
        config: constraints,
        events,
        decisions: Vec::new(),
        adapter_version: None,
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

#[test]
fn no_wall_clock_usage() {
    let src = include_str!("../src/lib.rs");
    assert!(
        !src.contains("SystemTime"),
        "wall clock usage is forbidden in supervisor"
    );
}

#[test]
fn sample_bundle_deserializes() {
    let data = include_str!("data/capture_v0_sample.json");
    let bundle: CaptureBundle = serde_json::from_str(data).expect("sample bundle should parse");
    let runtime = FaultRuntimeHandle::new(RunTermination::Completed);
    let records = replay(&bundle, runtime);
    assert_eq!(records.len(), bundle.events.len());
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
