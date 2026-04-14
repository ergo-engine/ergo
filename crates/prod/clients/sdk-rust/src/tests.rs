//! sdk-rust tests
//!
//! Purpose:
//! - Exercise the public Rust SDK surface from inside the crate while keeping production code free of scenario-heavy test blocks.
//!
//! Owns:
//! - Integration-shaped checks for builder/setup flows, run/replay/manual-runner behavior, and SDK error propagation contracts.
//!
//! Does not own:
//! - Host or loader semantic authority; those seams stay owned by their respective crates.
//!
//! Connects to:
//! - `lib.rs` and the public SDK types it exposes over host and loader behavior.
//!
//! Safety notes:
//! - These tests intentionally lock SDK-facing orchestration and error propagation because external Rust callers rely on those contracts directly.

use super::*;
use ergo_adapter::{EventTime, ExternalEventKind, RunTermination};
use ergo_host::PROCESS_DRIVER_PROTOCOL_VERSION;
use ergo_loader::{load_graph_assets_from_memory, InMemorySourceInput};
use ergo_runtime::action::{
    ActionEffects, ActionKind, ActionOutcome, ActionPrimitive, ActionPrimitiveManifest,
    ActionValue, ActionValueType, Cardinality as ActionCardinality,
    ExecutionSpec as ActionExecutionSpec, InputSpec as ActionInputSpec, IntentFieldSpec,
    IntentSpec, OutputSpec as ActionOutputSpec, StateSpec as ActionStateSpec,
};
use ergo_runtime::common::{Value, ValueType};
use ergo_runtime::runtime::ExecutionContext;
use ergo_runtime::source::{
    Cadence as SourceCadence, ExecutionSpec as SourceExecutionSpec, OutputSpec as SourceOutputSpec,
    SourceKind, SourcePrimitive, SourcePrimitiveManifest, SourceRequires,
    StateSpec as SourceStateSpec,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct InjectedNumberSource {
    manifest: SourcePrimitiveManifest,
    output: f64,
}

impl InjectedNumberSource {
    fn new(output: f64) -> Self {
        Self {
            manifest: SourcePrimitiveManifest {
                id: "injected_number_source".to_string(),
                version: "0.1.0".to_string(),
                kind: SourceKind::Source,
                inputs: vec![],
                outputs: vec![SourceOutputSpec {
                    name: "value".to_string(),
                    value_type: ValueType::Number,
                }],
                parameters: vec![],
                requires: SourceRequires {
                    context: Vec::new(),
                },
                execution: SourceExecutionSpec {
                    deterministic: true,
                    cadence: SourceCadence::Continuous,
                },
                state: SourceStateSpec { allowed: false },
                side_effects: false,
            },
            output,
        }
    }
}

impl SourcePrimitive for InjectedNumberSource {
    fn manifest(&self) -> &SourcePrimitiveManifest {
        &self.manifest
    }

    fn produce(
        &self,
        _parameters: &HashMap<String, source::ParameterValue>,
        _ctx: &ExecutionContext,
    ) -> HashMap<String, Value> {
        HashMap::from([("value".to_string(), Value::Number(self.output))])
    }
}

struct InjectedIntentAction {
    manifest: ActionPrimitiveManifest,
}

impl InjectedIntentAction {
    fn new() -> Self {
        Self {
            manifest: ActionPrimitiveManifest {
                id: "injected_intent_action".to_string(),
                version: "0.1.0".to_string(),
                kind: ActionKind::Action,
                inputs: vec![
                    ActionInputSpec {
                        name: "event".to_string(),
                        value_type: ActionValueType::Event,
                        required: true,
                        cardinality: ActionCardinality::Single,
                    },
                    ActionInputSpec {
                        name: "qty".to_string(),
                        value_type: ActionValueType::Number,
                        required: true,
                        cardinality: ActionCardinality::Single,
                    },
                ],
                outputs: vec![ActionOutputSpec {
                    name: "outcome".to_string(),
                    value_type: ActionValueType::Event,
                }],
                parameters: vec![],
                effects: ActionEffects {
                    writes: vec![],
                    intents: vec![IntentSpec {
                        name: "place_order".to_string(),
                        fields: vec![IntentFieldSpec {
                            name: "qty".to_string(),
                            value_type: ValueType::Number,
                            from_input: Some("qty".to_string()),
                            from_param: None,
                        }],
                        mirror_writes: vec![],
                    }],
                },
                execution: ActionExecutionSpec {
                    deterministic: true,
                    retryable: false,
                },
                state: ActionStateSpec { allowed: false },
                side_effects: true,
            },
        }
    }
}

impl ActionPrimitive for InjectedIntentAction {
    fn manifest(&self) -> &ActionPrimitiveManifest {
        &self.manifest
    }

    fn execute(
        &self,
        _inputs: &HashMap<String, ActionValue>,
        _parameters: &HashMap<String, action::ParameterValue>,
    ) -> HashMap<String, ActionValue> {
        HashMap::from([(
            "outcome".to_string(),
            ActionValue::Event(ActionOutcome::Completed),
        )])
    }
}

fn make_temp_dir(label: &str) -> PathBuf {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "ergo_sdk_rust_{label}_{}_{}_{}",
        std::process::id(),
        index,
        nanos
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_file(base: &Path, rel: &str, contents: &str) -> PathBuf {
    let path = base.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(&path, contents).expect("write file");
    path
}

fn write_intent_graph(base: &Path, graph_id: &str) -> PathBuf {
    write_file(
        base,
        "graphs/strategy.yaml",
        &format!(
            r#"
kind: cluster
id: {graph_id}
version: "0.1.0"
nodes:
  gate:
    impl: const_bool@0.1.0
    params:
      value: true
  emit:
    impl: emit_if_true@0.1.0
  qty:
    impl: injected_number_source@0.1.0
  place:
    impl: injected_intent_action@0.1.0
edges:
  - "gate.value -> emit.input"
  - "emit.event -> place.event"
  - "qty.value -> place.qty"
outputs:
  outcome: place.outcome
"#
        ),
    )
}

fn write_intent_adapter_manifest(base: &Path) -> PathBuf {
    write_file(
        base,
        "adapters/trading.yaml",
        r#"
kind: adapter
id: sdk_trading_adapter
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: last_qty
    type: Number
    required: false
    writable: true
event_kinds:
  - name: price_bar
    payload_schema:
      type: object
      properties:
        price: { type: number }
      additionalProperties: false
accepts:
  effects:
    - name: set_context
      payload_schema:
        type: object
        additionalProperties: false
    - name: place_order
      payload_schema:
        type: object
        properties:
          qty: { type: number }
        required: [qty]
        additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.price_bar
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
"#,
    )
}

fn write_process_ingress_sentinel(base: &Path, sentinel_path: &Path) -> PathBuf {
    write_file(
        base,
        "scripts/ingress.sh",
        &format!(
            r#"#!/bin/sh
printf '%s\n' started > "{sentinel}"
printf '%s\n' '{{"type":"hello","protocol":"{protocol}"}}'
printf '%s\n' '{{"type":"end"}}'
"#,
            sentinel = sentinel_path.display(),
            protocol = PROCESS_DRIVER_PROTOCOL_VERSION
        ),
    )
}

fn write_minimal_adapter_file(base: &Path) -> PathBuf {
    write_file(
        base,
        "adapters/minimal.yaml",
        r#"
kind: adapter
id: minimal_test_adapter
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys: []
event_kinds:
  - name: tick
    payload_schema:
      type: object
      additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.tick
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
"#,
    )
}

fn write_process_run_script(base: &Path) -> PathBuf {
    let hello = serde_json::to_string(&serde_json::json!({
        "type":"hello",
        "protocol":PROCESS_DRIVER_PROTOCOL_VERSION
    }))
    .expect("serialize hello frame");
    let event = serde_json::to_string(&serde_json::json!({
        "type":"event",
        "event": minimal_adapter_event("evt1"),
    }))
    .expect("serialize event frame");
    let end =
        serde_json::to_string(&serde_json::json!({"type":"end"})).expect("serialize end frame");
    write_file(
            base,
            "scripts/process_ingress.sh",
            &format!(
                "#!/bin/sh\nprintf '%s\\n' '{hello}'\nprintf '%s\\n' '{event}'\nprintf '%s\\n' '{end}'\n"
            ),
        )
}

fn write_egress_ack_script(base: &Path) -> PathBuf {
    write_file(
        base,
        "channels/egress/broker.sh",
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  case "$line" in
    *'"type":"intent"'*)
      intent_id=$(printf '%s' "$line" | sed -n 's/.*"intent_id":"\([^"]*\)".*/\1/p')
      printf '{"type":"intent_ack","intent_id":"%s","status":"accepted","acceptance":"durable"}\n' "$intent_id"
      ;;
    *'"type":"end"'*)
      exit 0
      ;;
  esac
done
"#,
    )
}

fn write_egress_io_script(base: &Path) -> PathBuf {
    write_file(
        base,
        "channels/egress/broker.sh",
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  case "$line" in
    *'"type":"intent"'*)
      exit 1
      ;;
    *'"type":"end"'*)
      exit 0
      ;;
  esac
done
"#,
    )
}

fn write_egress_config(base: &Path, command: Vec<String>) -> PathBuf {
    write_file(
        base,
        "egress/live.toml",
        &format!(
            r#"
default_ack_timeout = "100ms"

[channels.broker]
type = "process"
command = [{command}]

[routes.place_order]
channel = "broker"
"#,
            command = command
                .into_iter()
                .map(|part| format!("{part:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    )
}

fn manual_step_event(event_id: &str) -> HostedEvent {
    HostedEvent {
        event_id: event_id.to_string(),
        kind: ExternalEventKind::Command,
        at: EventTime::default(),
        semantic_kind: None,
        payload: Some(serde_json::json!({})),
    }
}

/// Event with semantic kind matching the minimal adapter's `tick` event kind.
fn minimal_adapter_event(event_id: &str) -> HostedEvent {
    HostedEvent {
        event_id: event_id.to_string(),
        kind: ExternalEventKind::Command,
        at: EventTime::default(),
        semantic_kind: Some("tick".to_string()),
        payload: Some(serde_json::json!({})),
    }
}

fn load_memory_graph_assets(graph_id: &str) -> PreparedGraphAssets {
    load_graph_assets_from_memory(
        "graphs/root.yaml",
        &[InMemorySourceInput {
            source_id: "graphs/root.yaml".to_string(),
            source_label: format!("{graph_id}-root"),
            content: format!(
                r#"
kind: cluster
id: {graph_id}
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 3.0
edges: []
outputs:
  value_out: src.value
"#
            ),
        }],
        &[],
    )
    .expect("load in-memory graph assets")
}

fn load_memory_intent_graph_assets(graph_id: &str) -> PreparedGraphAssets {
    load_graph_assets_from_memory(
        "graphs/root.yaml",
        &[InMemorySourceInput {
            source_id: "graphs/root.yaml".to_string(),
            source_label: format!("{graph_id}-root"),
            content: format!(
                r#"
kind: cluster
id: {graph_id}
version: "0.1.0"
nodes:
  gate:
    impl: const_bool@0.1.0
    params:
      value: true
  emit:
    impl: emit_if_true@0.1.0
  qty:
    impl: injected_number_source@0.1.0
  place:
    impl: injected_intent_action@0.1.0
edges:
  - "gate.value -> emit.input"
  - "emit.event -> place.event"
  - "qty.value -> place.qty"
outputs:
  outcome: place.outcome
"#
            ),
        }],
        &[],
    )
    .expect("load in-memory intent graph assets")
}

fn in_memory_project(
    name: &str,
    version: &str,
    profile_name: &str,
    profile: InMemoryProfileConfig,
) -> InMemoryProjectSnapshot {
    InMemoryProjectSnapshot::builder(name.to_string(), version.to_string())
        .profile(profile_name.to_string(), profile)
        .build()
        .expect("in-memory project snapshot should validate")
}

/// Minimal adapter manifest for process driver tests.
/// Contains one event kind (`tick`) to satisfy ADP-4.
fn minimal_adapter_for_process_tests() -> AdapterInput {
    AdapterInput::Text {
        content: r#"
kind: adapter
id: minimal_test_adapter
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys: []
event_kinds:
  - name: tick
    payload_schema:
      type: object
      additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.tick
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
"#
        .to_string(),
        source_label: "inline-minimal-adapter".to_string(),
    }
}

fn in_memory_process_profile(
    graph_assets: PreparedGraphAssets,
    command: impl IntoIterator<Item = impl Into<String>>,
) -> InMemoryProfileConfig {
    InMemoryProfileConfig::process(graph_assets, command)
        .expect("in-memory process profile should validate")
        .adapter(minimal_adapter_for_process_tests())
}

fn in_memory_fixture_profile(graph_assets: PreparedGraphAssets) -> InMemoryProfileConfig {
    InMemoryProfileConfig::fixture_items(
        graph_assets,
        vec![
            FixtureItem::EpisodeStart {
                label: "E1".to_string(),
            },
            FixtureItem::Event {
                id: Some("evt1".to_string()),
                kind: ExternalEventKind::Command,
                payload: Some(serde_json::json!({})),
                semantic_kind: None,
            },
        ],
        "memory-fixture",
    )
    .expect("in-memory fixture profile should validate")
}

fn adapter_bound_event(event_id: &str, price: f64) -> HostedEvent {
    HostedEvent {
        event_id: event_id.to_string(),
        kind: ExternalEventKind::Command,
        at: EventTime::default(),
        semantic_kind: Some("price_bar".to_string()),
        payload: Some(serde_json::json!({ "price": price })),
    }
}

#[test]
fn explicit_run_uses_registered_custom_source() -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("explicit_run");
    let graph = write_file(
        &root,
        "graph.yaml",
        r#"
kind: cluster
id: sdk_explicit_run
version: "0.1.0"
nodes:
  src:
    impl: injected_number_source@0.1.0
edges: []
outputs:
  value_out: src.value
"#,
    );
    let fixture = write_file(
            &root,
            "fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );
    let capture = root.join("capture.json");

    let outcome = Ergo::builder()
        .add_source(InjectedNumberSource::new(7.5))
        .build()?
        .run(RunConfig::new(graph, IngressConfig::fixture(fixture)).capture_output(&capture))?;

    match outcome {
        RunOutcome::Completed(summary) => {
            assert_eq!(summary.events, 1);
            assert_eq!(summary.capture_path, Some(capture));
        }
        other => panic!("expected completed run, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn run_with_stop_can_request_zero_event_host_stop() -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("explicit_stop");
    let adapter_path = write_minimal_adapter_file(&root);
    let graph = write_file(
        &root,
        "graph.yaml",
        r#"
kind: cluster
id: sdk_explicit_stop
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 3.0
edges: []
outputs:
  value_out: src.value
"#,
    );
    let hello = serde_json::to_string(&serde_json::json!({
        "type":"hello",
        "protocol":PROCESS_DRIVER_PROTOCOL_VERSION
    }))?;
    let driver = write_file(
        &root,
        "driver.sh",
        &format!("#!/bin/sh\nprintf '%s\\n' '{hello}'\nexec sleep 5\n"),
    );
    let capture = root.join("capture.json");
    let stop = StopHandle::new();
    let stop_clone = stop.clone();
    let stopper = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        stop_clone.stop();
    });

    let err = Ergo::builder()
        .build()?
        .run_with_stop(
            RunConfig::new(
                graph,
                IngressConfig::process(["/bin/sh".to_string(), driver.display().to_string()]),
            )
            .adapter(&adapter_path)
            .capture_output(&capture),
            stop,
        )
        .expect_err("zero-event host stop should surface an SDK error");

    stopper.join().expect("stopper thread must join");
    match err {
        ErgoRunError::Host(HostRunError::Driver(HostDriverError::Output(
            HostDriverOutputError::StopBeforeFirstCommittedEvent,
        ))) => {}
        other => panic!("expected host stop StepFailed error, got {other:?}"),
    }
    assert!(
        !capture.exists(),
        "zero-event stop must not write a capture artifact"
    );

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn run_profile_discovers_project_and_clusters() -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("project_run");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
capture_output = "captures/historical.capture.json"
"#,
    );
    write_file(
        &root,
        "graphs/strategy.yaml",
        r#"
kind: cluster
id: sdk_project_graph
version: "0.1.0"
nodes:
  shared:
    cluster: shared_value@0.1.0
edges: []
outputs:
  result: shared.value
"#,
    );
    write_file(
        &root,
        "clusters/shared_value.yaml",
        r#"
kind: cluster
id: shared_value
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 3.0
edges: []
outputs:
  value: src.value
"#,
    );
    write_file(
            &root,
            "fixtures/historical.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

    let outcome = Ergo::from_project(root.join("graphs"))
        .build()?
        .run_profile("historical")?;

    let capture = root.join("captures/historical.capture.json");
    match outcome {
        RunOutcome::Completed(summary) => {
            assert_eq!(summary.events, 1);
            assert_eq!(summary.capture_path, Some(capture));
        }
        other => panic!("expected completed run, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn run_profile_with_stop_honors_profile_max_events() -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("project_profile_bounds");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
capture_output = "captures/historical.capture.json"
max_events = 1
"#,
    );
    write_file(
        &root,
        "graphs/strategy.yaml",
        r#"
kind: cluster
id: sdk_profile_bounds
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 3.0
edges: []
outputs:
  result: src.value
"#,
    );
    write_file(
            &root,
            "fixtures/historical.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

    let outcome = Ergo::from_project(&root)
        .build()?
        .run_profile_with_stop("historical", StopHandle::new())?;

    let capture = root.join("captures/historical.capture.json");
    match outcome {
        RunOutcome::Interrupted(interrupted) => {
            assert_eq!(interrupted.reason, InterruptionReason::HostStopRequested);
            assert_eq!(interrupted.summary.events, 1);
            assert_eq!(interrupted.summary.capture_path, Some(capture));
        }
        other => panic!("expected interrupted host-stop outcome, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn builder_rejects_conflicting_project_sources() {
    let snapshot = in_memory_project(
        "memory-project",
        "0.1.0",
        "historical",
        in_memory_process_profile(
            load_memory_graph_assets("sdk_builder_conflict"),
            ["/bin/echo", "noop"],
        )
        .capture(ProfileCapture::file(
            "captures/historical.capture.json",
            false,
        )),
    );

    let err = match Ergo::builder()
        .project_root(".")
        .in_memory_project(snapshot)
        .build()
    {
        Ok(_) => panic!("conflicting project sources must fail build"),
        Err(err) => err,
    };
    assert!(matches!(err, ErgoBuildError::ProjectSourceConflict));
}

#[test]
fn builder_allows_replacing_the_same_project_source_kind() -> Result<(), Box<dyn std::error::Error>>
{
    let first = in_memory_project(
        "memory-project",
        "0.1.0",
        "historical",
        in_memory_process_profile(
            load_memory_graph_assets("sdk_builder_replace_first"),
            ["/bin/echo", "noop"],
        ),
    );
    let second = in_memory_project(
        "memory-project",
        "0.1.0",
        "historical",
        in_memory_process_profile(
            load_memory_graph_assets("sdk_builder_replace_second"),
            ["/bin/echo", "noop"],
        ),
    );

    let _ = Ergo::builder()
        .project_root(".")
        .project_root("..")
        .build()?;
    let _ = Ergo::builder()
        .in_memory_project(first)
        .in_memory_project(second)
        .build()?;
    Ok(())
}

#[test]
fn in_memory_project_snapshot_rejects_empty_process_ingress() {
    let err = InMemoryProfileConfig::process(
        load_memory_graph_assets("sdk_memory_invalid_ingress"),
        Vec::<String>::new(),
    )
    .expect_err("empty process ingress must fail");

    assert!(matches!(
        err,
        ProjectError::Config(ProjectConfigError::InMemoryProcessCommandEmpty { profile: None })
    ));
}

#[test]
fn in_memory_project_snapshot_rejects_blank_process_executable() {
    let err = InMemoryProfileConfig::process(
        load_memory_graph_assets("sdk_memory_blank_process_ingress"),
        ["", "--flag"],
    )
    .expect_err("blank process executable must fail");

    assert!(matches!(
        err,
        ProjectError::Config(ProjectConfigError::InMemoryProcessExecutableBlank { profile: None })
    ));
    assert!(err.to_string().contains("executable must not be empty"));
}

#[test]
fn in_memory_project_snapshot_rejects_whitespace_process_executable() {
    let err = InMemoryProfileConfig::process(
        load_memory_graph_assets("sdk_memory_whitespace_process_ingress"),
        ["   ", "--flag"],
    )
    .expect_err("whitespace-only process executable must fail");

    assert!(matches!(
        err,
        ProjectError::Config(ProjectConfigError::InMemoryProcessExecutableBlank { profile: None })
    ));
    assert!(err.to_string().contains("executable must not be empty"));
}

#[test]
fn run_profile_uses_in_memory_project_process_ingress() -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("memory_project_run");
    let ingress_script = write_process_run_script(&root);
    let capture = root.join("captures/historical.capture.json");
    let snapshot = in_memory_project(
        "memory-project",
        "0.1.0",
        "historical",
        in_memory_process_profile(
            load_memory_graph_assets("sdk_memory_project_run"),
            ["/bin/sh", &ingress_script.display().to_string()],
        )
        .capture(ProfileCapture::file(&capture, false)),
    );

    let outcome = Ergo::builder()
        .in_memory_project(snapshot)
        .build()?
        .run_profile("historical")?;

    match outcome {
        RunOutcome::Completed(summary) => {
            assert_eq!(summary.events, 1);
            assert_eq!(summary.capture_path, Some(capture));
        }
        other => panic!("expected completed run, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn run_profile_uses_in_memory_capture_for_in_memory_project(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("memory_project_run_capture_bundle");
    let ingress_script = write_process_run_script(&root);
    let snapshot = in_memory_project(
        "memory-project",
        "0.1.0",
        "historical",
        in_memory_process_profile(
            load_memory_graph_assets("sdk_memory_project_run_bundle"),
            ["/bin/sh", &ingress_script.display().to_string()],
        ),
    );

    let outcome = Ergo::builder()
        .in_memory_project(snapshot)
        .build()?
        .run_profile("historical")?;

    match outcome {
        RunOutcome::Completed(summary) => {
            assert_eq!(summary.events, 1);
            assert_eq!(summary.capture_path, None);
            assert_eq!(summary.capture_bundle.events.len(), 1);
        }
        other => panic!("expected completed run, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn run_profile_supports_in_memory_fixture_items_ingress() -> Result<(), Box<dyn std::error::Error>>
{
    let snapshot = in_memory_project(
        "memory-project",
        "0.1.0",
        "historical",
        in_memory_fixture_profile(load_memory_graph_assets("sdk_memory_fixture_run")),
    );

    let outcome = Ergo::builder()
        .in_memory_project(snapshot)
        .build()?
        .run_profile("historical")?;

    match outcome {
        RunOutcome::Completed(summary) => {
            assert_eq!(summary.events, 1);
            assert_eq!(summary.capture_path, None);
            assert_eq!(summary.capture_bundle.events.len(), 1);
        }
        other => panic!("expected completed run, got {other:?}"),
    }

    Ok(())
}

#[test]
fn run_profile_supports_in_memory_fixture_items_with_file_capture(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("memory_project_fixture_file_capture");
    let capture = root.join("captures/historical.capture.json");
    let snapshot = in_memory_project(
        "memory-project",
        "0.1.0",
        "historical",
        in_memory_fixture_profile(load_memory_graph_assets("sdk_memory_fixture_file_capture"))
            .capture(ProfileCapture::file(&capture, false)),
    );

    let outcome = Ergo::builder()
        .in_memory_project(snapshot)
        .build()?
        .run_profile("historical")?;

    match outcome {
        RunOutcome::Completed(summary) => {
            assert_eq!(summary.events, 1);
            assert_eq!(summary.capture_path, Some(capture.clone()));
            assert!(capture.exists());
            let written: CaptureBundle = serde_json::from_str(&fs::read_to_string(&capture)?)?;
            assert_eq!(written.graph_id.as_str(), "sdk_memory_fixture_file_capture");
        }
        other => panic!("expected completed run, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn replay_profile_on_in_memory_project_returns_unsupported_operation(
) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = in_memory_project(
        "memory-project",
        "0.1.0",
        "historical",
        in_memory_process_profile(
            load_memory_graph_assets("sdk_memory_replay_unsupported"),
            ["/bin/echo", "noop"],
        ),
    );
    let ergo = Ergo::builder().in_memory_project(snapshot).build()?;
    let err = ergo
        .replay_profile("historical", "capture.json")
        .expect_err("in-memory replay_profile should be unsupported");

    match err {
        ErgoReplayError::Project(ProjectError::UnsupportedOperation {
            operation,
            transport,
        }) => {
            assert_eq!(operation, "replay_profile");
            assert_eq!(transport, "in-memory");
        }
        other => panic!("unexpected in-memory replay_profile error: {other:?}"),
    }

    Ok(())
}

#[test]
fn missing_profile_error_is_normalized_for_in_memory_project(
) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = in_memory_project(
        "memory-project",
        "0.1.0",
        "historical",
        in_memory_process_profile(
            load_memory_graph_assets("sdk_memory_missing_profile"),
            ["/bin/echo", "noop"],
        ),
    );
    let ergo = Ergo::builder().in_memory_project(snapshot).build()?;
    let err = ergo
        .run_profile("missing")
        .expect_err("missing in-memory profile should use normalized error");
    assert!(matches!(
        err,
        ErgoRunError::Project(ProjectError::ProfileNotFound { name }) if name == "missing"
    ));
    Ok(())
}

#[test]
fn replay_profile_reuses_project_resolution() -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("project_replay");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
capture_output = "captures/historical.capture.json"
"#,
    );
    write_file(
        &root,
        "graphs/strategy.yaml",
        r#"
kind: cluster
id: sdk_replay_graph
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.0
edges: []
outputs:
  value_out: src.value
"#,
    );
    write_file(
            &root,
            "fixtures/historical.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

    let capture = root.join("captures/historical.capture.json");
    let _ = Ergo::from_project(&root)
        .build()?
        .run_profile("historical")?;

    let replay = Ergo::from_project(&root)
        .build()?
        .replay_profile("historical", &capture)?;

    assert_eq!(replay.graph_id.as_str(), "sdk_replay_graph");
    assert_eq!(replay.events, 1);

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn replay_profile_bundle_reuses_project_resolution() -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("project_replay_bundle");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
"#,
    );
    write_file(
        &root,
        "graphs/strategy.yaml",
        r#"
kind: cluster
id: sdk_replay_bundle_graph
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.0
edges: []
outputs:
  value_out: src.value
"#,
    );
    write_file(
            &root,
            "fixtures/historical.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

    let outcome = Ergo::from_project(&root)
        .build()?
        .run_profile("historical")?;
    let bundle = match outcome {
        RunOutcome::Completed(summary) => summary.capture_bundle,
        other => panic!("expected completed run, got {other:?}"),
    };

    let replay = Ergo::from_project(&root)
        .build()?
        .replay_profile_bundle("historical", bundle)?;

    assert_eq!(replay.graph_id.as_str(), "sdk_replay_bundle_graph");
    assert_eq!(replay.events, 1);

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn replay_profile_bundle_supports_in_memory_projects() -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("memory_project_replay_bundle");
    let ingress_script = write_process_run_script(&root);
    let snapshot = in_memory_project(
        "memory-project",
        "0.1.0",
        "historical",
        in_memory_process_profile(
            load_memory_graph_assets("sdk_memory_replay_bundle"),
            ["/bin/sh", &ingress_script.display().to_string()],
        ),
    );
    let ergo = Ergo::builder().in_memory_project(snapshot).build()?;
    let outcome = ergo.run_profile("historical")?;
    let bundle = match outcome {
        RunOutcome::Completed(summary) => summary.capture_bundle,
        other => panic!("expected completed run, got {other:?}"),
    };

    let replay = ergo.replay_profile_bundle("historical", bundle)?;
    assert_eq!(replay.graph_id.as_str(), "sdk_memory_replay_bundle");
    assert_eq!(replay.events, 1);

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn replay_bundle_replays_capture_bundle_from_paths() -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("explicit_replay_bundle");
    let graph = write_file(
        &root,
        "graph.yaml",
        r#"
kind: cluster
id: sdk_explicit_replay_bundle
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.0
edges: []
outputs:
  value_out: src.value
"#,
    );
    let fixture = write_file(
            &root,
            "fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

    let outcome = Ergo::builder()
        .build()?
        .run(RunConfig::new(&graph, IngressConfig::fixture(&fixture)))?;
    let bundle = match outcome {
        RunOutcome::Completed(summary) => summary.capture_bundle,
        other => panic!("expected completed run, got {other:?}"),
    };

    let replay = Ergo::builder()
        .build()?
        .replay_bundle(ReplayBundleConfig::new(bundle, &graph))?;
    assert_eq!(replay.graph_id.as_str(), "sdk_explicit_replay_bundle");
    assert_eq!(replay.events, 1);

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn validate_project_rejects_profile_missing_required_adapter(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("validate_missing_adapter");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
fixture = "fixtures/live.jsonl"
"#,
    );
    write_file(
        &root,
        "graphs/strategy.yaml",
        r#"
kind: cluster
id: sdk_validate_missing_adapter
version: "0.1.0"
nodes:
  gate:
    impl: const_bool@0.1.0
    params:
      value: true
  emit:
    impl: emit_if_true@0.1.0
  qty:
    impl: injected_number_source@0.1.0
  place:
    impl: injected_intent_action@0.1.0
edges:
  - "gate.value -> emit.input"
  - "emit.event -> place.event"
  - "qty.value -> place.qty"
outputs:
  outcome: place.outcome
"#,
    );
    write_file(
            &root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

    let err = Ergo::from_project(&root)
        .add_source(InjectedNumberSource::new(4.0))
        .add_action(InjectedIntentAction::new())
        .build()?
        .validate_project()
        .expect_err("adapter-less intent profile must fail validation");

    match err {
        ErgoValidateError::Validation { profile, source } => {
            assert_eq!(profile, "live");
            assert!(matches!(
                source,
                ProjectError::Host(HostRunError::AdapterRequired(_))
            ));
        }
        other => panic!("unexpected error: {other}"),
    }

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn validate_project_accepts_adapter_and_egress_profile() -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("validate_egress");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
adapter = "adapters/trading.yaml"
fixture = "fixtures/live.jsonl"
egress = "egress/live.toml"
"#,
    );
    write_file(
        &root,
        "graphs/strategy.yaml",
        r#"
kind: cluster
id: sdk_validate_egress
version: "0.1.0"
nodes:
  gate:
    impl: const_bool@0.1.0
    params:
      value: true
  emit:
    impl: emit_if_true@0.1.0
  qty:
    impl: injected_number_source@0.1.0
  place:
    impl: injected_intent_action@0.1.0
edges:
  - "gate.value -> emit.input"
  - "emit.event -> place.event"
  - "qty.value -> place.qty"
outputs:
  outcome: place.outcome
"#,
    );
    write_file(
        &root,
        "adapters/trading.yaml",
        r#"
kind: adapter
id: sdk_trading_adapter
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: last_qty
    type: Number
    required: false
    writable: true
event_kinds:
  - name: price_bar
    payload_schema:
      type: object
      properties:
        price: { type: number }
      additionalProperties: false
accepts:
  effects:
    - name: set_context
      payload_schema:
        type: object
        additionalProperties: false
    - name: place_order
      payload_schema:
        type: object
        properties:
          qty: { type: number }
        required: [qty]
        additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.price_bar
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
"#,
    );
    write_file(
        &root,
        "egress/live.toml",
        r#"
default_ack_timeout = "5s"

[channels.broker]
type = "process"
command = ["python3", "channels/egress/broker.py"]

[routes.place_order]
channel = "broker"
ack_timeout = "10s"
"#,
    );
    write_file(
            &root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"semantic_kind\":\"price_bar\",\"payload\":{\"price\":101.25}}}\n",
        );

    let summary = Ergo::from_project(&root)
        .add_source(InjectedNumberSource::new(4.0))
        .add_action(InjectedIntentAction::new())
        .build()?
        .validate_project()?;

    assert_eq!(summary.name, "sdk-project");
    assert_eq!(summary.root, Some(root.clone()));
    assert_eq!(summary.profiles, vec!["live".to_string()]);

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn validate_project_returns_none_root_for_in_memory_project(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("memory_project_validate");
    let snapshot = in_memory_project(
        "memory-project",
        "0.1.0",
        "historical",
        in_memory_process_profile(
            load_memory_graph_assets("sdk_memory_validate"),
            ["/bin/echo", "noop"],
        ),
    );

    let summary = Ergo::builder()
        .in_memory_project(snapshot)
        .build()?
        .validate_project()?;
    assert_eq!(summary.name, "memory-project");
    assert_eq!(summary.version, "0.1.0");
    assert_eq!(summary.root, None);
    assert_eq!(summary.profiles, vec!["historical".to_string()]);

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn validate_project_and_run_profile_agree_for_valid_in_memory_profile(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("memory_project_validate_and_run");
    let ingress_script = write_process_run_script(&root);
    let snapshot = in_memory_project(
        "memory-project",
        "0.1.0",
        "historical",
        in_memory_process_profile(
            load_memory_graph_assets("sdk_memory_validate_and_run"),
            ["/bin/sh", &ingress_script.display().to_string()],
        ),
    );
    let ergo = Ergo::builder().in_memory_project(snapshot).build()?;

    let summary = ergo.validate_project()?;
    assert_eq!(summary.profiles, vec!["historical".to_string()]);
    assert_eq!(summary.root, None);

    let outcome = ergo.run_profile("historical")?;
    match outcome {
        RunOutcome::Completed(summary) => {
            assert_eq!(summary.events, 1);
            assert_eq!(summary.capture_path, None);
            assert_eq!(summary.capture_bundle.events.len(), 1);
        }
        other => panic!("expected completed run, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn validate_project_in_memory_preserves_adapter_required_preflight(
) -> Result<(), Box<dyn std::error::Error>> {
    // This test intentionally constructs a process profile WITHOUT an adapter
    // to verify that validation catches missing adapters.  We cannot use
    // `in_memory_process_profile` because it now provides a minimal adapter
    // by default (production closure).
    let profile = InMemoryProfileConfig::process(
        load_memory_intent_graph_assets("sdk_memory_validate_missing_adapter"),
        ["/bin/echo", "noop"],
    )
    .expect("in-memory process profile should validate");
    // Note: no `.adapter(...)` call — intentionally omitted.
    let snapshot = in_memory_project("memory-project", "0.1.0", "live", profile);

    let err = Ergo::builder()
        .in_memory_project(snapshot)
        .add_source(InjectedNumberSource::new(4.0))
        .add_action(InjectedIntentAction::new())
        .build()?
        .validate_project()
        .expect_err("adapter-less in-memory intent profile must fail validation");

    // The SDK now enforces the production closure structurally: the
    // `for_production` constructor requires a non-optional adapter, so
    // `live_prep_options_from_in_memory_profile` returns
    // `ProductionRequiresAdapter` before the host graph-dependency gate
    // ever runs.  This is the intended layered enforcement: SDK catches
    // configuration errors before host catches graph-structural errors.
    match err {
        ErgoValidateError::Validation { profile, source } => {
            assert_eq!(profile, "live");
            assert!(matches!(
                source,
                ProjectError::Host(HostRunError::ProductionRequiresAdapter)
            ));
        }
        other => panic!("unexpected error: {other}"),
    }
    Ok(())
}

#[test]
fn validate_project_rejects_missing_fixture_before_run_profile(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("validate_missing_fixture_driver");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/missing.jsonl"
"#,
    );
    write_file(
        &root,
        "graphs/strategy.yaml",
        r#"
kind: cluster
id: sdk_validate_missing_fixture
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 3.0
edges: []
outputs:
  value_out: src.value
"#,
    );

    let ergo = Ergo::from_project(&root).build()?;
    let validate_err = ergo
        .validate_project()
        .expect_err("missing fixture should fail validation");
    match validate_err {
        ErgoValidateError::Validation { profile, source } => {
            assert_eq!(profile, "historical");
            match source {
                ProjectError::Host(HostRunError::Driver(HostDriverError::Input(
                    HostDriverInputError::FixtureParse(detail),
                ))) => {
                    assert!(detail.to_string().contains("read fixture"));
                }
                other => panic!("unexpected validation source: {other:?}"),
            }
        }
        other => panic!("unexpected validation error: {other}"),
    }

    let run_err = ergo
        .run_profile("historical")
        .expect_err("missing fixture should fail run_profile");
    match run_err {
        ErgoRunError::Host(HostRunError::Driver(HostDriverError::Input(
            HostDriverInputError::FixtureParse(detail),
        ))) => {
            assert!(detail.to_string().contains("read fixture"));
        }
        other => panic!("unexpected run_profile error: {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn validate_project_rejects_missing_in_memory_process_driver_before_run_profile(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("validate_missing_process_driver");
    let missing_driver = root.join("missing-driver.sh");
    let snapshot = in_memory_project(
        "memory-project",
        "0.1.0",
        "historical",
        in_memory_process_profile(
            load_memory_graph_assets("sdk_memory_missing_driver"),
            [missing_driver.display().to_string()],
        ),
    );

    let ergo = Ergo::builder().in_memory_project(snapshot).build()?;
    let validate_err = ergo
        .validate_project()
        .expect_err("missing process driver should fail validation");
    match validate_err {
        ErgoValidateError::Validation { profile, source } => {
            assert_eq!(profile, "historical");
            assert!(matches!(
                source,
                ProjectError::Host(HostRunError::Driver(HostDriverError::Input(
                    HostDriverInputError::ProcessPathMetadata { .. },
                )))
            ));
        }
        other => panic!("unexpected validation error: {other}"),
    }

    let run_err = ergo
        .run_profile("historical")
        .expect_err("missing process driver should fail run_profile");
    match run_err {
        ErgoRunError::Host(HostRunError::Driver(HostDriverError::Input(
            HostDriverInputError::ProcessPathMetadata { .. },
        ))) => {}
        other => panic!("unexpected run_profile error: {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn validate_project_reports_invalid_egress_config_as_profile_validation(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("validate_invalid_egress_config");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
fixture = "fixtures/live.jsonl"
egress = "egress/live.toml"
"#,
    );
    write_file(
        &root,
        "graphs/strategy.yaml",
        r#"
kind: cluster
id: sdk_validate_invalid_egress
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 4.0
edges: []
outputs:
  result: src.value
"#,
    );
    write_file(
        &root,
        "egress/live.toml",
        r#"
default_ack_timeout = "5s"

[channels.broker]
type = "process"
command = ["oops"
"#,
    );
    write_file(
            &root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

    let err = Ergo::from_project(&root)
        .build()?
        .validate_project()
        .expect_err("invalid egress config must fail profile validation");

    match err {
        ErgoValidateError::Validation { profile, source } => {
            assert_eq!(profile, "live");
            match source {
                ProjectError::Config(ProjectConfigError::EgressConfigParse { path, source }) => {
                    assert!(path.ends_with("egress/live.toml"));
                    assert!(source.to_string().contains("TOML parse error"));
                }
                other => panic!("unexpected invalid-egress source: {other:?}"),
            }
        }
        other => panic!("unexpected invalid-egress validation error: {other}"),
    }

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn validate_project_surfaces_runtime_owned_cluster_version_details(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("validate_cluster_version_miss");
    fs::create_dir_all(root.join("clusters")).expect("create clusters dir");

    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
"#,
    );
    write_file(
        &root,
        "graphs/strategy.yaml",
        r#"
kind: cluster
id: sdk_cluster_version_miss
version: "0.1.0"
nodes:
  shared:
    cluster: shared_value@^2.0
edges: []
outputs:
  result: shared.value
"#,
    );
    write_file(
        &root,
        "graphs/shared_value.yaml",
        r#"
kind: cluster
id: shared_value
version: "1.5.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 4.0
edges: []
outputs:
  value: src.value
"#,
    );
    write_file(
        &root,
        "clusters/shared_value.yaml",
        r#"
kind: cluster
id: shared_value
version: "1.0.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 3.0
edges: []
outputs:
  value: src.value
"#,
    );
    write_file(
            &root,
            "fixtures/historical.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

    let err = Ergo::from_project(&root)
        .build()?
        .validate_project()
        .expect_err("version-miss cluster profile must fail validation");

    match err {
        ErgoValidateError::Validation { profile, source } => {
            assert_eq!(profile, "historical");
            let detail = source.to_string();
            assert!(detail.contains("graph expansion failed"));
            assert!(!detail.contains("cluster discovery failed"));
            assert!(detail.contains("shared_value"));
            assert!(detail.contains("^2.0"));
            assert!(detail.contains("available: 1.0.0, 1.5.0"));
            assert!(detail.contains("available cluster sources"));
            assert!(detail.contains("shared_value@1.0.0"));
            assert!(detail.contains("shared_value@1.5.0"));
        }
        other => panic!("unexpected error: {other}"),
    }

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn ergo_handle_can_run_the_same_profile_twice() -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("reuse_run_profile");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
capture_output = "captures/historical.capture.json"
"#,
    );
    write_file(
        &root,
        "graphs/strategy.yaml",
        r#"
kind: cluster
id: sdk_reuse_run_profile
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 4.0
edges: []
outputs:
  result: src.value
"#,
    );
    write_file(
            &root,
            "fixtures/historical.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

    let ergo = Ergo::from_project(&root).build()?;
    let first = ergo.run_profile("historical")?;
    let second = ergo.run_profile("historical")?;

    match (first, second) {
        (RunOutcome::Completed(first), RunOutcome::Completed(second)) => {
            assert_eq!(first.events, 1);
            assert_eq!(second.events, 1);
            assert_eq!(first.capture_path, second.capture_path);
        }
        other => panic!("expected two completed runs, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn ergo_handle_survives_errors_and_can_validate_run_and_replay(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("reuse_validate_run_replay");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
capture_output = "captures/historical.capture.json"
"#,
    );
    write_file(
        &root,
        "graphs/strategy.yaml",
        r#"
kind: cluster
id: sdk_reuse_validate_run_replay
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 5.0
edges: []
outputs:
  result: src.value
"#,
    );
    write_file(
            &root,
            "fixtures/historical.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

    let ergo = Ergo::from_project(&root).build()?;
    let err = ergo
        .run_profile("missing")
        .expect_err("missing profile should not consume the handle");
    assert!(
        matches!(
            err,
            ErgoRunError::Project(ProjectError::ProfileNotFound { ref name })
                if name == "missing"
        ),
        "unexpected missing-profile error: {err:?}"
    );

    let summary = ergo.validate_project()?;
    assert_eq!(summary.root, Some(root.clone()));
    assert_eq!(summary.profiles, vec!["historical".to_string()]);

    let outcome = ergo.run_profile("historical")?;
    let capture = root.join("captures/historical.capture.json");
    match outcome {
        RunOutcome::Completed(summary) => {
            assert_eq!(summary.capture_path, Some(capture.clone()));
            assert_eq!(summary.events, 1);
        }
        other => panic!("expected completed run, got {other:?}"),
    }

    let replay = ergo.replay_profile("historical", &capture)?;
    assert_eq!(replay.graph_id.as_str(), "sdk_reuse_validate_run_replay");
    assert_eq!(replay.events, 1);

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn runner_for_profile_supports_multiple_steps_without_launching_ingress_or_auto_writing_capture(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("manual_runner_no_ingress");
    let ingress_sentinel = root.join("ingress-started.txt");
    let ingress_script = write_process_ingress_sentinel(&root, &ingress_sentinel);
    write_minimal_adapter_file(&root);
    write_file(
        &root,
        "ergo.toml",
        &format!(
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.manual]
graph = "graphs/strategy.yaml"
adapter = "adapters/minimal.yaml"
capture_output = "captures/manual.capture.json"
max_duration = "1ms"
max_events = 1

[profiles.manual.ingress]
type = "process"
command = ["/bin/sh", "{ingress_script}"]
"#,
            ingress_script = ingress_script.display()
        ),
    );
    write_file(
        &root,
        "graphs/strategy.yaml",
        r#"
kind: cluster
id: sdk_manual_runner_no_ingress
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 6.0
edges: []
outputs:
  result: src.value
"#,
    );

    let ergo = Ergo::from_project(&root).build()?;
    let mut runner = ergo.runner_for_profile("manual")?;
    let capture = root.join("captures/manual.capture.json");
    assert!(
        !ingress_sentinel.exists(),
        "manual runner must not launch ingress"
    );

    let first = runner.step(minimal_adapter_event("e1"))?;
    assert_eq!(first.termination, Some(RunTermination::Completed));
    thread::sleep(Duration::from_millis(10));
    let second = runner.step(minimal_adapter_event("e2"))?;
    assert_eq!(second.termination, Some(RunTermination::Completed));

    let bundle = runner.finish()?;
    assert_eq!(bundle.events.len(), 2);
    assert_eq!(bundle.decisions.len(), 2);
    assert!(
        !capture.exists(),
        "manual finish should return a bundle without auto-writing capture_output"
    );
    assert!(
        !ingress_sentinel.exists(),
        "manual runner must not launch ingress"
    );

    let err = runner
        .step(minimal_adapter_event("e3"))
        .expect_err("step after finish must fail");
    assert!(
        matches!(err, HostedStepError::LifecycleViolation { .. }),
        "unexpected step-after-finish error: {err:?}"
    );

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn runner_for_profile_can_finish_and_write_capture_with_profile_settings(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("manual_runner_write_capture");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.manual]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
capture_output = "captures/manual.capture.json"
pretty_capture = true
"#,
    );
    write_file(
        &root,
        "graphs/strategy.yaml",
        r#"
kind: cluster
id: sdk_manual_runner_write_capture
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 7.0
edges: []
outputs:
  result: src.value
"#,
    );
    write_file(
            &root,
            "fixtures/historical.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

    let ergo = Ergo::from_project(&root).build()?;
    let mut runner = ergo.runner_for_profile("manual")?;
    let capture = root.join("captures/manual.capture.json");
    let outcome = runner.step(manual_step_event("e1"))?;
    assert_eq!(outcome.termination, Some(RunTermination::Completed));
    let bundle = runner.finish_and_write_capture()?;
    let persisted: CaptureBundle = serde_json::from_str(&fs::read_to_string(&capture)?)?;

    assert!(
        capture.exists(),
        "finish_and_write_capture should write capture_output"
    );
    assert_eq!(bundle.decisions.len(), 1);
    assert_eq!(persisted.decisions.len(), 1);
    assert!(fs::read_to_string(&capture)?.contains("\n  "));

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn runner_for_profile_preserves_bundle_when_capture_write_fails(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("manual_runner_write_capture_failure");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.manual]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
capture_output = "captures/manual.capture.json"
"#,
    );
    write_file(
        &root,
        "graphs/strategy.yaml",
        r#"
kind: cluster
id: sdk_manual_runner_write_capture_failure
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 7.0
edges: []
outputs:
  result: src.value
"#,
    );
    write_file(
            &root,
            "fixtures/historical.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );
    write_file(&root, "captures", "not-a-directory");

    let ergo = Ergo::from_project(&root).build()?;
    let mut runner = ergo.runner_for_profile("manual")?;
    let outcome = runner.step(manual_step_event("e1"))?;
    assert_eq!(outcome.termination, Some(RunTermination::Completed));

    let err = runner
        .finish_and_write_capture()
        .expect_err("capture write failure should preserve bundle in the error");

    match &err {
        ProfileRunnerCaptureError::Write { source, bundle } => {
            assert!(source
                .to_string()
                .contains("create capture output directory"));
            assert_eq!(bundle.decisions.len(), 1);
        }
        other => panic!("unexpected write-failure error: {other}"),
    }
    assert_eq!(
        err.capture_bundle()
            .expect("write failure should expose recovered bundle")
            .decisions
            .len(),
        1
    );

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn runner_for_profile_still_requires_a_declared_ingress_source(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("manual_runner_requires_ingress");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.manual]
graph = "graphs/strategy.yaml"
"#,
    );
    write_file(
        &root,
        "graphs/strategy.yaml",
        r#"
kind: cluster
id: sdk_manual_runner_requires_ingress
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 8.0
edges: []
outputs:
  result: src.value
"#,
    );

    let ergo = Ergo::from_project(&root).build()?;
    let err = match ergo.runner_for_profile("manual") {
        Ok(_) => panic!("profile resolution should still require ingress"),
        Err(err) => err,
    };
    match err {
        ErgoRunnerError::Project(ProjectError::Load(LoaderProjectError::ProfileInvalid {
            detail,
            ..
        })) => {
            assert!(detail.contains("exactly one ingress source"));
        }
        other => panic!("unexpected runner_for_profile ingress error: {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn runner_for_profile_preserves_adapter_required_preflight(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("manual_runner_adapter_preflight");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
fixture = "fixtures/live.jsonl"
"#,
    );
    write_intent_graph(&root, "sdk_manual_runner_adapter_preflight");
    write_file(
            &root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

    let ergo = Ergo::from_project(&root)
        .add_source(InjectedNumberSource::new(4.0))
        .add_action(InjectedIntentAction::new())
        .build()?;
    let err = match ergo.runner_for_profile("live") {
        Ok(_) => panic!("adapter-required profile should fail before returning runner"),
        Err(err) => err,
    };
    match err {
        ErgoRunnerError::Host(HostRunError::AdapterRequired(summary)) => {
            assert!(summary.requires_adapter);
        }
        other => panic!("unexpected adapter-preflight error: {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn runner_for_profile_surfaces_egress_startup_failure_at_creation(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("manual_runner_egress_startup");
    let missing_binary = "/definitely/missing-egress-binary".to_string();
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
adapter = "adapters/trading.yaml"
fixture = "fixtures/live.jsonl"
egress = "egress/live.toml"
"#,
    );
    write_intent_graph(&root, "sdk_manual_runner_egress_startup");
    write_intent_adapter_manifest(&root);
    write_egress_config(&root, vec![missing_binary]);
    write_file(
            &root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

    let ergo = Ergo::from_project(&root)
        .add_source(InjectedNumberSource::new(4.0))
        .add_action(InjectedIntentAction::new())
        .build()?;
    let err = match ergo.runner_for_profile("live") {
        Ok(_) => panic!("egress startup failure should surface during runner creation"),
        Err(err) => err,
    };
    assert!(matches!(
        err,
        ErgoRunnerError::Host(HostRunError::Setup(HostSetupError::StartEgress(_)))
    ));

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn profile_runner_zero_step_finish_fails_but_recoverable_input_errors_do_not_poison_session(
) -> Result<(), Box<dyn std::error::Error>> {
    let zero_root = make_temp_dir("manual_runner_zero_step");
    write_file(
        &zero_root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.manual]
graph = "graphs/strategy.yaml"
fixture = "fixtures/live.jsonl"
"#,
    );
    write_file(
        &zero_root,
        "graphs/strategy.yaml",
        r#"
kind: cluster
id: sdk_manual_runner_zero_step
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 9.0
edges: []
outputs:
  result: src.value
"#,
    );
    write_file(
            &zero_root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

    let ergo = Ergo::from_project(&zero_root).build()?;
    let mut zero_runner = ergo.runner_for_profile("manual")?;
    let zero_err = zero_runner
        .finish()
        .expect_err("zero-step finish must fail");
    assert!(
        matches!(zero_err, HostedStepError::LifecycleViolation { .. }),
        "unexpected zero-step finish error: {zero_err:?}"
    );
    let _ = fs::remove_dir_all(&zero_root);

    let failure_root = make_temp_dir("manual_runner_recoverable_input_error");
    write_file(
        &failure_root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
adapter = "adapters/trading.yaml"
fixture = "fixtures/live.jsonl"
egress = "egress/live.toml"
"#,
    );
    write_intent_graph(&failure_root, "sdk_manual_runner_nonfinalizable");
    write_intent_adapter_manifest(&failure_root);
    let egress_script = write_egress_ack_script(&failure_root);
    write_egress_config(
        &failure_root,
        vec!["/bin/sh".to_string(), egress_script.display().to_string()],
    );
    write_file(
            &failure_root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

    let ergo = Ergo::from_project(&failure_root)
        .add_source(InjectedNumberSource::new(4.0))
        .add_action(InjectedIntentAction::new())
        .build()?;
    let mut runner = ergo.runner_for_profile("live")?;
    let step_err = runner
        .step(HostedEvent {
            event_id: "evt_bad".to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: None,
            payload: Some(serde_json::json!({"price": 100.0})),
        })
        .expect_err("missing semantic kind should surface a recoverable input error");
    assert!(matches!(step_err, HostedStepError::MissingSemanticKind));

    let recovered = runner.step(adapter_bound_event("evt_good", 101.5))?;
    assert_eq!(recovered.termination, Some(RunTermination::Completed));

    let bundle = runner.finish()?;
    assert_eq!(bundle.events.len(), 1);
    assert_eq!(bundle.decisions.len(), 1);

    let _ = fs::remove_dir_all(failure_root);
    Ok(())
}

#[test]
fn profile_runner_can_finalize_after_egress_dispatch_failure(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("manual_runner_dispatch_failure");
    let egress_script = write_egress_io_script(&root);
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
adapter = "adapters/trading.yaml"
fixture = "fixtures/live.jsonl"
egress = "egress/live.toml"
"#,
    );
    write_intent_graph(&root, "sdk_manual_runner_dispatch_failure");
    write_intent_adapter_manifest(&root);
    write_egress_config(
        &root,
        vec!["/bin/sh".to_string(), egress_script.display().to_string()],
    );
    write_file(
            &root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

    let ergo = Ergo::from_project(&root)
        .add_source(InjectedNumberSource::new(4.0))
        .add_action(InjectedIntentAction::new())
        .build()?;
    let mut runner = ergo.runner_for_profile("live")?;
    let step_err = runner
        .step(adapter_bound_event("evt1", 101.5))
        .expect_err("egress dispatch failure should interrupt manual stepping");
    assert!(
        matches!(step_err, HostedStepError::EgressDispatchFailure(_)),
        "unexpected dispatch failure: {step_err:?}"
    );

    let step_again = runner
        .step(adapter_bound_event("evt2", 101.6))
        .expect_err("runner should require finalization after dispatch failure");
    assert!(
        matches!(step_again, HostedStepError::LifecycleViolation { .. }),
        "unexpected post-dispatch step result: {step_again:?}"
    );

    let bundle = runner.finish()?;
    assert_eq!(bundle.events.len(), 1);
    assert_eq!(bundle.decisions.len(), 1);
    assert!(bundle.decisions[0].interruption.is_some());

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn profile_runner_dispatches_egress_and_finishes_with_a_bundle(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = make_temp_dir("manual_runner_egress_success");
    let egress_script = write_egress_ack_script(&root);
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
adapter = "adapters/trading.yaml"
fixture = "fixtures/live.jsonl"
egress = "egress/live.toml"
capture_output = "captures/live.capture.json"
"#,
    );
    write_intent_graph(&root, "sdk_manual_runner_egress_success");
    write_intent_adapter_manifest(&root);
    write_egress_config(
        &root,
        vec!["/bin/sh".to_string(), egress_script.display().to_string()],
    );
    write_file(
            &root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

    let ergo = Ergo::from_project(&root)
        .add_source(InjectedNumberSource::new(4.0))
        .add_action(InjectedIntentAction::new())
        .build()?;
    let mut runner = ergo.runner_for_profile("live")?;
    let outcome = runner.step(adapter_bound_event("evt1", 101.5))?;
    assert_eq!(outcome.termination, Some(RunTermination::Completed));

    let bundle = runner.finish()?;
    assert_eq!(bundle.events.len(), 1);
    assert_eq!(bundle.decisions.len(), 1);
    assert_eq!(bundle.decisions[0].intent_acks.len(), 1);
    assert!(
        !root.join("captures/live.capture.json").exists(),
        "manual finish should not auto-write capture_output"
    );

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn runner_for_profile_returns_working_profile_runner_for_in_memory_project(
) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = in_memory_project(
        "memory-project",
        "0.1.0",
        "manual",
        in_memory_process_profile(
            load_memory_graph_assets("sdk_memory_manual_runner"),
            ["/bin/echo", "noop"],
        ),
    );

    let ergo = Ergo::builder().in_memory_project(snapshot).build()?;
    let mut runner = ergo.runner_for_profile("manual")?;
    let outcome = runner.step(minimal_adapter_event("evt1"))?;
    assert_eq!(outcome.termination, Some(RunTermination::Completed));

    let bundle = runner.finish()?;
    assert_eq!(bundle.events.len(), 1);

    Ok(())
}

#[test]
fn runner_for_profile_in_memory_without_file_capture_path_cannot_auto_write_capture(
) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = in_memory_project(
        "memory-project",
        "0.1.0",
        "manual",
        in_memory_process_profile(
            load_memory_graph_assets("sdk_memory_manual_capture"),
            ["/bin/echo", "noop"],
        ),
    );

    let ergo = Ergo::builder().in_memory_project(snapshot).build()?;
    let mut runner = ergo.runner_for_profile("manual")?;
    let _ = runner.step(minimal_adapter_event("evt1"))?;
    let err = runner
        .finish_and_write_capture()
        .expect_err("in-memory runner without file capture must not auto-write");
    assert!(matches!(
        err,
        ProfileRunnerCaptureError::CaptureOutputNotConfigured
    ));

    Ok(())
}
