//! usecases shared tests
//!
//! Purpose:
//! - Provide shared test infrastructure for the out-of-line `usecases` submodule suites and the
//!   facade-level contract tests.
//!
//! Owns:
//! - Common injected runtime primitives, temp-file helpers, process-driver helpers, and host test
//!   harness utilities reused by `live_prep`, `live_run`, `process_driver`, and `contract`.
//!
//! Does not own:
//! - Production host semantics, which remain owned by `usecases.rs` and its child modules.
//!
//! Safety notes:
//! - Shared graph/manfiest helpers below now emit explicit already-valid YAML;
//!   `write_temp_file(...)` no longer rewrites fixture text heuristically.

use super::live_prep::{prepare_adapter_setup, prepare_graph_runtime, replay_owned_external_kinds};
use super::live_run::{run_graph_from_assets_internal, run_graph_from_paths_internal};
use super::process_driver::{kill_host_managed_child, spawn_process_driver};
use super::*;
use crate::egress::{EgressChannelConfig, EgressRoute};
use ergo_adapter::ExternalEventKind;
use ergo_runtime::action::{
    ActionEffects, ActionKind, ActionOutcome, ActionPrimitive, ActionPrimitiveManifest,
    ActionValue, ActionValueType, Cardinality as ActionCardinality,
    ExecutionSpec as ActionExecutionSpec, InputSpec as ActionInputSpec, IntentFieldSpec,
    IntentSpec, OutputSpec as ActionOutputSpec, StateSpec as ActionStateSpec,
};
use ergo_runtime::catalog::CatalogBuilder;
use ergo_runtime::common::{Value, ValueType};
use ergo_runtime::runtime::ExecutionContext;
use ergo_runtime::source::{
    Cadence as SourceCadence, ExecutionSpec as SourceExecutionSpec, OutputSpec as SourceOutputSpec,
    SourceKind, SourcePrimitive, SourcePrimitiveManifest, SourceRequires,
    StateSpec as SourceStateSpec,
};
use serde_json::json;
use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct InjectedNumberSource {
    manifest: SourcePrimitiveManifest,
    output: f64,
    counter: Option<Arc<AtomicUsize>>,
}

struct InjectedIntentAction {
    manifest: ActionPrimitiveManifest,
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
            counter: None,
        }
    }

    fn new_observed(output: f64, counter: Arc<AtomicUsize>) -> Self {
        Self {
            counter: Some(counter),
            ..Self::new(output)
        }
    }
}

impl SourcePrimitive for InjectedNumberSource {
    fn manifest(&self) -> &SourcePrimitiveManifest {
        &self.manifest
    }

    fn produce(
        &self,
        _parameters: &HashMap<String, ergo_runtime::source::ParameterValue>,
        _ctx: &ExecutionContext,
    ) -> HashMap<String, Value> {
        if let Some(counter) = &self.counter {
            counter.fetch_add(1, Ordering::SeqCst);
        }
        HashMap::from([("value".to_string(), Value::Number(self.output))])
    }
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
        _parameters: &HashMap<String, ergo_runtime::action::ParameterValue>,
    ) -> HashMap<String, ActionValue> {
        HashMap::from([(
            "outcome".to_string(),
            ActionValue::Event(ActionOutcome::Completed),
        )])
    }
}

fn build_injected_runtime_surfaces(output: f64) -> RuntimeSurfaces {
    let mut builder = CatalogBuilder::new();
    builder.add_source(Box::new(InjectedNumberSource::new(output)));
    builder.add_action(Box::new(InjectedIntentAction::new()));
    let (registries, catalog) = builder
        .build()
        .expect("injected runtime surfaces should build");
    RuntimeSurfaces::new(registries, catalog)
}

fn build_observed_runtime_surfaces(output: f64, counter: Arc<AtomicUsize>) -> RuntimeSurfaces {
    let mut builder = CatalogBuilder::new();
    builder.add_source(Box::new(InjectedNumberSource::new_observed(
        output, counter,
    )));
    builder.add_action(Box::new(InjectedIntentAction::new()));
    let (registries, catalog) = builder
        .build()
        .expect("observed runtime surfaces should build");
    RuntimeSurfaces::new(registries, catalog)
}

fn write_temp_file(
    base: &Path,
    name: &str,
    contents: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = base.join(name);
    fs::write(&path, contents)?;
    Ok(path)
}

fn write_intent_graph(
    base: &Path,
    name: &str,
    graph_id: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    write_temp_file(base, name, &intent_graph_yaml(graph_id))
}

fn intent_graph_yaml(graph_id: &str) -> String {
    format!(
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
    )
}

fn intent_adapter_manifest_text() -> &'static str {
    r#"
kind: adapter
id: replay_intent_adapter
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
"#
}

fn number_source_graph_yaml(graph_id: &str, value: f64) -> String {
    format!(
        r#"
kind: cluster
id: {graph_id}
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: {value}
edges: []
outputs:
  value_out: src.value
"#
    )
}

fn const_number_graph_yaml(graph_id: &str, value: f64) -> String {
    format!(
        r#"
kind: cluster
id: {graph_id}
version: "0.1.0"
nodes:
  src:
    impl: const_number@0.1.0
    params:
      value: {value}
edges: []
outputs:
  value_out: src.value
"#
    )
}

fn injected_number_graph_yaml(graph_id: &str) -> String {
    format!(
        r#"
kind: cluster
id: {graph_id}
version: "0.1.0"
nodes:
  src:
    impl: injected_number_source@0.1.0
edges: []
outputs:
  value_out: src.value
"#
    )
}

fn cluster_version_miss_graph_yaml(graph_id: &str) -> String {
    format!(
        r#"
kind: cluster
id: {graph_id}
version: "0.1.0"
nodes:
  nested:
    cluster: shared_value@^2.0
edges: []
outputs:
  result: nested.value
"#
    )
}

fn shared_value_graph_yaml(version: &str, value: f64) -> String {
    format!(
        r#"
kind: cluster
id: shared_value
version: "{version}"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: {value}
edges: []
outputs:
  value: src.value
"#
    )
}

fn context_set_no_intents_graph_yaml(graph_id: &str) -> String {
    format!(
        r#"
kind: cluster
id: {graph_id}
version: "0.1.0"
nodes:
  ev:
    impl: emit_if_event_and_true@0.1.0
  disabled:
    impl: const_bool@0.1.0
    params:
      value: false
  qty:
    impl: const_number@0.1.0
    params:
      value: 1.0
  place:
    impl: context_set_number@0.1.0
    params:
      key: "last_qty"
edges:
  - "ev.event -> place.event"
  - "disabled.value -> ev.condition"
  - "qty.value -> place.value"
outputs:
  outcome: place.outcome
"#
    )
}

fn write_intent_adapter_manifest(
    base: &Path,
    name: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    write_temp_file(base, name, intent_adapter_manifest_text())
}

/// Minimal adapter manifest with one event kind (required by ADP-4) and no
/// declared context keys or accepted effects.  Provides a valid adapter
/// contract for process driver tests that exercise host/driver protocol rather
/// than adapter semantics.
fn minimal_adapter_manifest_text() -> &'static str {
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
"#
}

fn write_minimal_adapter_manifest(
    base: &Path,
    name: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    write_temp_file(base, name, minimal_adapter_manifest_text())
}

fn core_graph_yaml(graph_id: &str) -> String {
    const_number_graph_yaml(graph_id, 7.5)
}

fn load_core_in_memory_assets(
    graph_id: &str,
) -> Result<ergo_loader::PreparedGraphAssets, Box<dyn std::error::Error>> {
    Ok(load_graph_assets_from_memory(
        "graphs/root.yaml",
        &[ergo_loader::InMemorySourceInput {
            source_id: "graphs/root.yaml".to_string(),
            source_label: "root-memory".to_string(),
            content: core_graph_yaml(graph_id),
        }],
        &[],
    )?)
}

fn load_injected_in_memory_assets(
    graph_id: &str,
) -> Result<ergo_loader::PreparedGraphAssets, Box<dyn std::error::Error>> {
    Ok(load_graph_assets_from_memory(
        "graphs/root.yaml",
        &[ergo_loader::InMemorySourceInput {
            source_id: "graphs/root.yaml".to_string(),
            source_label: "root-memory".to_string(),
            content: injected_number_graph_yaml(graph_id),
        }],
        &[],
    )?)
}

fn load_intent_in_memory_assets(
    graph_id: &str,
) -> Result<ergo_loader::PreparedGraphAssets, Box<dyn std::error::Error>> {
    Ok(load_graph_assets_from_memory(
        "graphs/root.yaml",
        &[ergo_loader::InMemorySourceInput {
            source_id: "graphs/root.yaml".to_string(),
            source_label: "root-memory".to_string(),
            content: intent_graph_yaml(graph_id),
        }],
        &[],
    )?)
}

fn write_egress_ack_script(base: &Path, name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    write_temp_file(
        base,
        name,
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  case "$line" in
    *'"type":"intent"'*)
      intent_id=$(printf '%s' "$line" | sed -n 's/.*"intent_id":"\([^"]*\)".*/\1/p')
      printf '{"type":"intent_ack","intent_id":"%s","status":"accepted","acceptance":"durable","egress_ref":"broker-ref-1"}\n' "$intent_id"
      ;;
    *'"type":"end"'*)
      exit 0
      ;;
  esac
done
"#,
    )
}

fn write_egress_protocol_script(
    base: &Path,
    name: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    write_temp_file(
        base,
        name,
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  case "$line" in
    *'"type":"intent"'*)
      printf '%s\n' '{"type":"intent_ack","intent_id":"wrong","status":"accepted","acceptance":"durable"}'
      ;;
    *'"type":"end"'*)
      exit 0
      ;;
  esac
done
"#,
    )
}

fn write_egress_io_script(base: &Path, name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    write_temp_file(
        base,
        name,
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

fn write_egress_hanging_shutdown_script(
    base: &Path,
    name: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    write_temp_file(
        base,
        name,
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  case "$line" in
    *'"type":"intent"'*)
      intent_id=$(printf '%s' "$line" | sed -n 's/.*"intent_id":"\([^"]*\)".*/\1/p')
      printf '{"type":"intent_ack","intent_id":"%s","status":"accepted","acceptance":"durable"}\n' "$intent_id"
      ;;
    *'"type":"end"'*)
      sleep 7
      exit 0
      ;;
  esac
done
"#,
    )
}

fn write_egress_end_sentinel_script(
    base: &Path,
    name: &str,
    sentinel_path: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    write_temp_file(
        base,
        name,
        &format!(
            r#"#!/bin/sh
sentinel='{sentinel}'
printf '%s\n' '{{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}}'
while IFS= read -r line; do
  case "$line" in
    *'"type":"intent"'*)
      intent_id=$(printf '%s' "$line" | sed -n 's/.*"intent_id":"\([^"]*\)".*/\1/p')
      printf '{{"type":"intent_ack","intent_id":"%s","status":"accepted","acceptance":"durable"}}\n' "$intent_id"
      ;;
    *'"type":"end"'*)
      printf '%s\n' 'saw_end' > "$sentinel"
      exit 0
      ;;
  esac
done
"#,
            sentinel = sentinel_path.display()
        ),
    )
}

fn write_egress_ack_once_then_timeout_script(
    base: &Path,
    name: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    write_temp_file(
        base,
        name,
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
acked=0
while IFS= read -r line; do
  case "$line" in
    *'"type":"intent"'*)
      if [ "$acked" = "0" ]; then
        intent_id=$(printf '%s' "$line" | sed -n 's/.*"intent_id":"\([^"]*\)".*/\1/p')
        printf '{"type":"intent_ack","intent_id":"%s","status":"accepted","acceptance":"durable"}\n' "$intent_id"
        acked=1
      fi
      ;;
    *'"type":"end"'*)
      exit 0
      ;;
  esac
done
"#,
    )
}

fn make_intent_egress_config(script_path: &Path) -> EgressConfig {
    make_intent_egress_config_with_timeout(script_path, Duration::from_millis(250))
}

fn make_intent_egress_config_with_timeout(script_path: &Path, timeout: Duration) -> EgressConfig {
    EgressConfig::builder(timeout)
        .channel(
            "broker",
            EgressChannelConfig::process(vec![
                "/bin/sh".to_string(),
                script_path.display().to_string(),
            ])
            .expect("channel config should be valid"),
        )
        .expect("channel should insert")
        .route(
            "place_order",
            EgressRoute::new("broker", None).expect("route should be valid"),
        )
        .expect("route should insert")
        .build()
        .expect("egress config should build")
}

fn write_process_driver_script(
    base: &Path,
    name: &str,
    lines: &[String],
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let script = format!(
        "#!/bin/sh\ncat <<'__ERGO_DRIVER__'\n{}\n__ERGO_DRIVER__\n",
        lines.join("\n")
    );
    write_temp_file(base, name, &script)
}

fn write_process_driver_program(
    base: &Path,
    name: &str,
    body: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    write_temp_file(base, name, body)
}

fn hosted_event(event_id: &str) -> HostedEvent {
    HostedEvent {
        event_id: event_id.to_string(),
        kind: ExternalEventKind::Command,
        at: EventTime::default(),
        semantic_kind: None,
        payload: Some(json!({})),
    }
}

/// Hosted event with a semantic kind matching the minimal adapter's declared
/// event kind (`"tick"`).  Used by process driver tests that are now
/// adapter-bound.
fn hosted_event_with_semantic_kind(event_id: &str) -> HostedEvent {
    HostedEvent {
        event_id: event_id.to_string(),
        kind: ExternalEventKind::Command,
        at: EventTime::default(),
        semantic_kind: Some("tick".to_string()),
        payload: Some(json!({})),
    }
}

fn expect_completed(outcome: RunGraphResponse) -> Result<RunSummary, Box<dyn std::error::Error>> {
    match outcome? {
        RunOutcome::Completed(summary) => Ok(summary),
        RunOutcome::Interrupted(interrupted) => Err(format!(
            "expected completed run, got interrupted({})",
            interrupted.reason
        )
        .into()),
    }
}

fn expect_assets_completed(
    outcome: RunGraphResponse,
) -> Result<RunSummary, Box<dyn std::error::Error>> {
    expect_completed(outcome)
}

fn short_test_process_driver_policy() -> ProcessDriverPolicy {
    ProcessDriverPolicy {
        // Keep the test policy fast, but leave enough headroom for loaded CI hosts
        // so protocol-frame startup is deterministic instead of scheduler-sensitive.
        startup_grace: Duration::from_millis(200),
        termination_grace: Duration::from_millis(100),
        poll_interval: Duration::from_millis(5),
        event_recv_timeout: Duration::from_millis(25),
    }
}

fn wait_for_path(path: &Path, timeout: Duration) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.exists() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(5));
    }
    Err(format!("timed out waiting for '{}'", path.display()).into())
}

fn current_process_group_id() -> Result<u32, Box<dyn std::error::Error>> {
    let output = Command::new("ps")
        .args(["-o", "pgid=", "-p", &std::process::id().to_string()])
        .output()?;
    if !output.status.success() {
        return Err(format!("run ps for parent pgid exited with {}", output.status).into());
    }
    let raw = String::from_utf8(output.stdout)?;
    Ok(raw.trim().parse::<u32>()?)
}

fn send_sigint_to_process_group(pgid: u32) -> Result<(), Box<dyn std::error::Error>> {
    // SAFETY: Tests only call this with a process-group id obtained from `ps` for a child group
    // they created, and `killpg` is the required libc API for delivering SIGINT to that group.
    let rc = unsafe { libc::killpg(pgid as libc::pid_t, libc::SIGINT) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().into())
    }
}

fn run_graph_from_paths_with_process_policy(
    request: RunGraphFromPathsRequest,
    process_policy: ProcessDriverPolicy,
) -> RunGraphResponse {
    run_graph_from_paths_internal(request, None, process_policy, RunControl::default())
}

fn run_graph_from_paths_with_process_policy_and_control(
    request: RunGraphFromPathsRequest,
    process_policy: ProcessDriverPolicy,
    control: RunControl,
) -> RunGraphResponse {
    run_graph_from_paths_internal(request, None, process_policy, control)
}

fn run_graph_from_assets_with_process_policy(
    request: RunGraphFromAssetsRequest,
    process_policy: ProcessDriverPolicy,
) -> RunGraphResponse {
    run_graph_from_assets_internal(request, None, process_policy, RunControl::default())
}

mod contract;
mod live_prep;
mod live_run;
mod process_driver;
