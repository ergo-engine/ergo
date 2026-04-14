//! init_project.rs — Project scaffolding generator
//!
//! Purpose:
//! - Implements the `ergo init` command, generating project directory
//!   structure, graph YAML templates, adapter stubs, and profile
//!   configuration from user prompts.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error_format::{render_cli_error, CliErrorInfo};
use ergo_host::PROCESS_DRIVER_PROTOCOL_VERSION;

#[derive(Debug, Clone)]
struct InitOptions {
    target_dir: PathBuf,
    sdk_dependency_path: String,
    force: bool,
}

#[derive(Debug, Clone)]
struct InitSummary {
    root: PathBuf,
    sdk_dependency_path: String,
}

#[derive(Debug, Clone)]
struct ProjectNames {
    package_name: String,
    project_name: String,
}

pub fn init_command(args: &[String]) -> Result<String, String> {
    let options = parse_init_options(args)?;
    let summary = scaffold_project(&options)?;
    Ok(render_init_summary(&summary))
}

fn parse_init_options(args: &[String]) -> Result<InitOptions, String> {
    let mut target_dir: Option<PathBuf> = None;
    let mut sdk_path: Option<PathBuf> = None;
    let mut force = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--sdk-path" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.missing_option_value",
                            "--sdk-path requires a path",
                        )
                        .with_where("arg '--sdk-path'")
                        .with_fix("provide --sdk-path <path-to-ergo-sdk-rust>"),
                    )
                })?;
                sdk_path = Some(PathBuf::from(value));
                index += 2;
            }
            "-f" | "--force" => {
                force = true;
                index += 1;
            }
            other if other.starts_with('-') => {
                return Err(render_cli_error(
                    &CliErrorInfo::new(
                        "cli.invalid_option",
                        format!("unknown init option '{other}'"),
                    )
                    .with_where(format!("arg '{other}'"))
                    .with_fix("use --sdk-path <path> or --force with 'ergo init'"),
                ));
            }
            other => {
                if target_dir.is_some() {
                    return Err(render_cli_error(
                        &CliErrorInfo::new(
                            "cli.unexpected_argument",
                            format!("unexpected extra init argument '{other}'"),
                        )
                        .with_where("init command")
                        .with_fix("usage: ergo init <project-dir> [--sdk-path <path>] [--force]"),
                    ));
                }
                target_dir = Some(PathBuf::from(other));
                index += 1;
            }
        }
    }

    let target_dir = target_dir.ok_or_else(|| {
        render_cli_error(
            &CliErrorInfo::new(
                "cli.missing_required_argument",
                "init requires a target project directory",
            )
            .with_where("init command")
            .with_fix("usage: ergo init <project-dir> [--sdk-path <path>] [--force]"),
        )
    })?;

    let target_dir = absolutize_path(&target_dir, "target project directory")?;

    let sdk_dependency_path = match sdk_path {
        Some(path) => resolve_explicit_sdk_dependency_path(&target_dir, &path)?,
        None => default_sdk_dependency_path(&target_dir)?,
    };

    Ok(InitOptions {
        target_dir,
        sdk_dependency_path,
        force,
    })
}

fn scaffold_project(options: &InitOptions) -> Result<InitSummary, String> {
    ensure_target_ready(&options.target_dir, options.force)?;
    create_project_directories(&options.target_dir)?;

    let names = derive_project_names(&options.target_dir)?;
    let files = scaffold_files(&names, &options.sdk_dependency_path);

    for (relative, contents) in files {
        write_scaffold_file(&options.target_dir.join(relative), &contents)?;
    }

    Ok(InitSummary {
        root: options.target_dir.clone(),
        sdk_dependency_path: options.sdk_dependency_path.clone(),
    })
}

fn ensure_target_ready(target_dir: &Path, force: bool) -> Result<(), String> {
    if target_dir.exists() {
        if !target_dir.is_dir() {
            return Err(render_cli_error(
                &CliErrorInfo::new(
                    "cli.init_target_invalid",
                    format!("'{}' exists and is not a directory", target_dir.display()),
                )
                .with_where("init target")
                .with_fix("choose a new project directory"),
            ));
        }

        let mut entries = fs::read_dir(target_dir).map_err(|err| {
            render_cli_error(
                &CliErrorInfo::new(
                    "cli.init_target_read_failed",
                    format!("failed to inspect '{}'", target_dir.display()),
                )
                .with_where("init target")
                .with_fix("verify directory permissions")
                .with_detail(err.to_string()),
            )
        })?;

        if !force
            && entries
                .next()
                .transpose()
                .map_err(|err| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.init_target_read_failed",
                            format!("failed to inspect '{}'", target_dir.display()),
                        )
                        .with_where("init target")
                        .with_fix("verify directory permissions")
                        .with_detail(err.to_string()),
                    )
                })?
                .is_some()
        {
            return Err(render_cli_error(
                &CliErrorInfo::new(
                    "cli.init_target_not_empty",
                    format!("target directory '{}' is not empty", target_dir.display()),
                )
                .with_where("init target")
                .with_fix("rerun with --force to overwrite scaffold files"),
            ));
        }

        return Ok(());
    }

    fs::create_dir_all(target_dir).map_err(|err| {
        render_cli_error(
            &CliErrorInfo::new(
                "cli.init_target_create_failed",
                format!("failed to create '{}'", target_dir.display()),
            )
            .with_where("init target")
            .with_fix("verify parent directory permissions")
            .with_detail(err.to_string()),
        )
    })
}

fn create_project_directories(root: &Path) -> Result<(), String> {
    for relative in [
        "src",
        "src/implementations",
        "graphs",
        "clusters",
        "adapters",
        "channels",
        "channels/ingress",
        "channels/egress",
        "egress",
        "fixtures",
        "captures",
    ] {
        fs::create_dir_all(root.join(relative)).map_err(|err| {
            render_cli_error(
                &CliErrorInfo::new(
                    "cli.init_scaffold_create_failed",
                    format!("failed to create '{}'", root.join(relative).display()),
                )
                .with_where("project scaffold")
                .with_fix("verify directory permissions")
                .with_detail(err.to_string()),
            )
        })?;
    }

    Ok(())
}

fn write_scaffold_file(path: &Path, contents: &str) -> Result<(), String> {
    fs::write(path, contents).map_err(|err| {
        render_cli_error(
            &CliErrorInfo::new(
                "cli.init_scaffold_write_failed",
                format!("failed to write '{}'", path.display()),
            )
            .with_where("project scaffold")
            .with_fix("verify directory permissions")
            .with_detail(err.to_string()),
        )
    })
}

fn derive_project_names(target_dir: &Path) -> Result<ProjectNames, String> {
    let raw_name = target_dir
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            std::env::current_dir().ok().and_then(|path| {
                path.file_name()
                    .and_then(|value| value.to_str())
                    .map(ToOwned::to_owned)
            })
        })
        .unwrap_or_else(|| "ergo-app".to_string());

    let package_name = sanitize_package_name(&raw_name);
    if package_name.is_empty() {
        return Err(render_cli_error(
            &CliErrorInfo::new(
                "cli.init_name_invalid",
                format!("could not derive a Cargo package name from '{raw_name}'"),
            )
            .with_where("project name")
            .with_fix("use a path with letters or numbers in the final segment"),
        ));
    }

    Ok(ProjectNames {
        project_name: package_name.clone(),
        package_name,
    })
}

fn sanitize_package_name(raw_name: &str) -> String {
    let mut normalized = String::new();
    let mut last_was_separator = false;

    for ch in raw_name.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            normalized.push('-');
            last_was_separator = true;
        }
    }

    let trimmed = normalized.trim_matches('-').to_string();
    if trimmed.is_empty() {
        return "ergo-app".to_string();
    }
    if trimmed
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_digit())
    {
        format!("ergo-{trimmed}")
    } else {
        trimmed
    }
}

fn scaffold_files(names: &ProjectNames, sdk_dependency_path: &str) -> Vec<(&'static str, String)> {
    vec![
        (".gitignore", gitignore_contents()),
        ("README.md", readme_contents(names)),
        (
            "Cargo.toml",
            cargo_toml_contents(names, sdk_dependency_path),
        ),
        ("ergo.toml", ergo_toml_contents(names)),
        ("src/main.rs", main_rs_contents()),
        ("src/implementations/mod.rs", implementations_mod_contents()),
        ("src/implementations/sources.rs", sources_rs_contents()),
        ("src/implementations/actions.rs", actions_rs_contents()),
        ("graphs/strategy.yaml", graph_yaml_contents()),
        ("clusters/sample_message.yaml", cluster_yaml_contents()),
        ("adapters/sample.yaml", adapter_yaml_contents()),
        ("channels/ingress/live_feed.py", ingress_channel_contents()),
        (
            "channels/egress/sample_outbox.py",
            egress_channel_contents(),
        ),
        ("egress/live.toml", egress_toml_contents()),
        ("fixtures/historical.jsonl", fixture_contents()),
        ("captures/.gitkeep", String::new()),
    ]
}

fn gitignore_contents() -> String {
    "target/\ncaptures/*.json\n".to_string()
}

fn readme_contents(names: &ProjectNames) -> String {
    format!(
        r#"# {project_name}

This project was scaffolded by `ergo init`.

It is an SDK-first Ergo application:

- `Cargo.toml` owns Rust build configuration
- `ergo.toml` owns Ergo profiles and authored-asset wiring
- `src/implementations/` is where you add custom primitives
- `graphs/`, `clusters/`, `adapters/`, and `channels/` hold the authored runtime assets

## Quick Start

```text
cargo run
cargo run -- profiles
cargo run -- doctor
cargo run -- validate
cargo run -- replay historical captures/historical.capture.json
```

## Profiles

- `historical`
  fixture-backed sample profile that writes `captures/historical.capture.json`
- `live`
  process-ingress sample profile that uses the Python 3 example channel scripts and finalizes cleanly on Ctrl-C

## First Files To Edit

- `src/implementations/actions.rs`
- `src/implementations/sources.rs`
- `graphs/strategy.yaml`
- `clusters/sample_message.yaml`
- `adapters/sample.yaml`
- `ergo.toml`

## Notes

- The scaffolded sample channels use `python3`.
- The `run` command installs a Ctrl-C handler so long-running profiles can stop cleanly and still write capture artifacts.
- The built `Ergo` handle is same-thread reusable; the sample `main.rs` still keeps one handle per command for clarity.
- Use `cargo run -- doctor` after your first edits if you want a quick project health check.
"#,
        project_name = names.project_name
    )
}

fn cargo_toml_contents(names: &ProjectNames, sdk_dependency_path: &str) -> String {
    let sdk_dependency_path = escape_toml_string(sdk_dependency_path);
    format!(
        r#"[package]
name = "{package_name}"
version = "0.1.0"
edition = "2021"

[dependencies]
# This scaffold points at a local ergo-sdk-rust checkout until the SDK
# is published outside the repository.
ergo-sdk-rust = {{ path = "{sdk_dependency_path}" }}
ctrlc = "3.4"

[workspace]
"#,
        package_name = names.package_name,
        sdk_dependency_path = sdk_dependency_path,
    )
}

fn ergo_toml_contents(names: &ProjectNames) -> String {
    format!(
        r#"name = "{project_name}"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
adapter = "adapters/sample.yaml"
fixture = "fixtures/historical.jsonl"
egress = "egress/live.toml"
capture_output = "captures/historical.capture.json"

[profiles.live]
graph = "graphs/strategy.yaml"
adapter = "adapters/sample.yaml"
egress = "egress/live.toml"
capture_output = "captures/live.capture.json"

[profiles.live.ingress]
type = "process"
command = ["python3", "channels/ingress/live_feed.py"]
"#,
        project_name = names.project_name,
    )
}

fn main_rs_contents() -> String {
    r#"use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

use ergo_sdk_rust::{Ergo, ProjectSummary, StopHandle};

mod implementations;

use implementations::{PublishSampleAction, SampleMessageSource};

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("run");

    match command {
        "run" => {
            let profile = args.get(1).map(String::as_str).unwrap_or("historical");
            let stop = StopHandle::new();
            let stop_clone = stop.clone();
            ctrlc::set_handler(move || stop_clone.stop())?;
            let outcome = build_ergo()?.run_profile_with_stop(profile, stop)?;
            println!("run profile '{profile}': {outcome:?}");
            Ok(())
        }
        "profiles" => {
            let summary = build_ergo()?.validate_project()?;
            print_profiles(&summary);
            Ok(())
        }
        "validate" => {
            let summary = build_ergo()?.validate_project()?;
            print_project_summary("validate ok", &summary);
            Ok(())
        }
        "doctor" => doctor(),
        "replay" => {
            let profile = args.get(1).map(String::as_str).unwrap_or("historical");
            let capture_path = args
                .get(2)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(format!("captures/{profile}.capture.json")));
            let replay = build_ergo()?.replay_profile(profile, &capture_path)?;
            println!(
                "replay graph_id={:?} events={} invoked={} deferred={} skipped={}",
                replay.graph_id, replay.events, replay.invoked, replay.deferred, replay.skipped
            );
            Ok(())
        }
        "help" | "--help" | "-h" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(format!("unknown command '{other}'\n\n{}", usage()).into()),
    }
}

fn build_ergo() -> Result<Ergo, Box<dyn Error>> {
    Ok(Ergo::from_project(".")
        .add_source(SampleMessageSource::new())
        .add_action(PublishSampleAction::new())
        .build()?)
}

fn doctor() -> Result<(), Box<dyn Error>> {
    for path in [
        "graphs/strategy.yaml",
        "clusters/sample_message.yaml",
        "adapters/sample.yaml",
        "channels/ingress/live_feed.py",
        "channels/egress/sample_outbox.py",
        "egress/live.toml",
        "fixtures/historical.jsonl",
    ] {
        ensure_exists(path)?;
    }

    ensure_python3_available()?;
    compile_python("channels/ingress/live_feed.py")?;
    compile_python("channels/egress/sample_outbox.py")?;

    let summary = build_ergo()?.validate_project()?;

    print_project_summary("doctor ok", &summary);
    Ok(())
}

fn print_project_summary(label: &str, summary: &ProjectSummary) {
    let root = summary
        .root
        .as_deref()
        .map(Path::display)
        .map(|display| display.to_string())
        .unwrap_or_else(|| "<in-memory>".to_string());
    println!(
        "{label} project '{}' version={} root={}",
        summary.name,
        summary.version,
        root
    );
    print_profiles(summary);
}

fn print_profiles(summary: &ProjectSummary) {
    println!("profiles:");
    for profile in &summary.profiles {
        println!("  - {profile}");
    }
}

fn ensure_exists(path: &str) -> Result<(), Box<dyn Error>> {
    if Path::new(path).exists() {
        Ok(())
    } else {
        Err(format!("doctor failed: expected '{}' to exist", path).into())
    }
}

fn ensure_python3_available() -> Result<(), Box<dyn Error>> {
    let status = Command::new("python3")
        .arg("--version")
        .status()
        .map_err(|err| format!("doctor failed: unable to run python3 --version: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("doctor failed: python3 is required for the scaffolded sample channels".into())
    }
}

fn compile_python(path: &str) -> Result<(), Box<dyn Error>> {
    let status = Command::new("python3")
        .args(["-m", "py_compile", path])
        .status()
        .map_err(|err| format!("doctor failed: unable to compile '{}': {err}", path))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("doctor failed: python3 could not compile '{}'", path).into())
    }
}

fn usage() -> &'static str {
    "usage:\n  cargo run -- [run [profile] | profiles | validate | doctor | replay [profile] [capture.json]]"
}
"#
    .to_string()
}

fn implementations_mod_contents() -> String {
    r#"pub mod actions;
pub mod sources;

pub use actions::PublishSampleAction;
pub use sources::SampleMessageSource;
"#
    .to_string()
}

fn sources_rs_contents() -> String {
    r#"use std::collections::HashMap;

use ergo_sdk_rust::{common, source, ExecutionContext};

pub struct SampleMessageSource {
    manifest: source::SourcePrimitiveManifest,
}

impl SampleMessageSource {
    pub fn new() -> Self {
        Self {
            manifest: source::SourcePrimitiveManifest {
                id: "sample_message_source".to_string(),
                version: "0.1.0".to_string(),
                kind: source::SourceKind::Source,
                inputs: vec![],
                outputs: vec![source::OutputSpec {
                    name: "message".to_string(),
                    value_type: common::ValueType::String,
                }],
                parameters: vec![source::ParameterSpec {
                    name: "value".to_string(),
                    value_type: source::ParameterValue::String(String::new()).value_type(),
                    default: Some(source::ParameterValue::String(
                        "hello-from-cluster".to_string(),
                    )),
                    bounds: None,
                }],
                requires: source::SourceRequires { context: vec![] },
                execution: source::ExecutionSpec {
                    deterministic: true,
                    cadence: source::Cadence::Continuous,
                },
                state: source::StateSpec { allowed: false },
                side_effects: false,
            },
        }
    }
}

impl Default for SampleMessageSource {
    fn default() -> Self {
        Self::new()
    }
}

impl source::SourcePrimitive for SampleMessageSource {
    fn manifest(&self) -> &source::SourcePrimitiveManifest {
        &self.manifest
    }

    fn produce(
        &self,
        parameters: &HashMap<String, source::ParameterValue>,
        _ctx: &ExecutionContext,
    ) -> HashMap<String, common::Value> {
        let value = parameters
            .get("value")
            .and_then(|parameter| match parameter {
                source::ParameterValue::String(value) => Some(value.clone()),
                _ => None,
            })
            .expect("missing required parameter 'value' for sample_message_source");

        HashMap::from([("message".to_string(), common::Value::String(value))])
    }
}
"#
    .to_string()
}

fn actions_rs_contents() -> String {
    r#"use std::collections::HashMap;

use ergo_sdk_rust::{action, common};

pub struct PublishSampleAction {
    manifest: action::ActionPrimitiveManifest,
}

impl PublishSampleAction {
    pub fn new() -> Self {
        Self {
            manifest: action::ActionPrimitiveManifest {
                id: "publish_sample_action".to_string(),
                version: "0.1.0".to_string(),
                kind: action::ActionKind::Action,
                inputs: vec![
                    action::InputSpec {
                        name: "event".to_string(),
                        value_type: action::ActionValueType::Event,
                        required: true,
                        cardinality: action::Cardinality::Single,
                    },
                    action::InputSpec {
                        name: "message".to_string(),
                        value_type: action::ActionValueType::String,
                        required: true,
                        cardinality: action::Cardinality::Single,
                    },
                ],
                outputs: vec![action::OutputSpec {
                    name: "outcome".to_string(),
                    value_type: action::ActionValueType::Event,
                }],
                parameters: vec![],
                effects: action::ActionEffects {
                    writes: vec![],
                    intents: vec![action::IntentSpec {
                        name: "publish_sample".to_string(),
                        fields: vec![action::IntentFieldSpec {
                            name: "message".to_string(),
                            value_type: common::ValueType::String,
                            from_input: Some("message".to_string()),
                            from_param: None,
                        }],
                        mirror_writes: vec![action::IntentMirrorWriteSpec {
                            name: "last_message".to_string(),
                            value_type: common::ValueType::String,
                            from_field: "message".to_string(),
                        }],
                    }],
                },
                execution: action::ExecutionSpec {
                    deterministic: true,
                    retryable: false,
                },
                state: action::StateSpec { allowed: false },
                side_effects: true,
            },
        }
    }
}

impl Default for PublishSampleAction {
    fn default() -> Self {
        Self::new()
    }
}

impl action::ActionPrimitive for PublishSampleAction {
    fn manifest(&self) -> &action::ActionPrimitiveManifest {
        &self.manifest
    }

    fn execute(
        &self,
        inputs: &HashMap<String, action::ActionValue>,
        _parameters: &HashMap<String, action::ParameterValue>,
    ) -> HashMap<String, action::ActionValue> {
        let _event = inputs
            .get("event")
            .and_then(|value| value.as_event())
            .expect("missing required event input 'event'");
        let _message = inputs
            .get("message")
            .and_then(|value| match value {
                action::ActionValue::String(value) => Some(value.as_str()),
                _ => None,
            })
            .expect("missing required string input 'message'");

        HashMap::from([(
            "outcome".to_string(),
            action::ActionValue::Event(action::ActionOutcome::Completed),
        )])
    }
}
"#
    .to_string()
}

fn graph_yaml_contents() -> String {
    r#"kind: cluster
id: sample_flow
version: "0.1.0"

nodes:
  gate:
    impl: const_bool@0.1.0
    params:
      value: true

  emit:
    impl: emit_if_true@0.1.0

  shared:
    cluster: sample_message@0.1.0

  publish:
    impl: publish_sample_action@0.1.0

edges:
  - "gate.value -> emit.input"
  - "emit.event -> publish.event"
  - "shared.message -> publish.message"

outputs:
  publish_outcome: publish.outcome
  shared_message: shared.message
"#
    .to_string()
}

fn cluster_yaml_contents() -> String {
    r#"kind: cluster
id: sample_message
version: "0.1.0"

nodes:
  src:
    impl: sample_message_source@0.1.0
    params:
      value: "hello-from-cluster"

edges: []

outputs:
  message: src.message
"#
    .to_string()
}

fn adapter_yaml_contents() -> String {
    r#"kind: adapter
id: sample_adapter
version: 1.0.0
runtime_compatibility: "0.1.0"

context_keys:
  - name: message
    type: String
    required: true
    writable: false
  - name: last_message
    type: String
    required: false
    writable: true

event_kinds:
  - name: sample_event
    payload_schema:
      type: object
      properties:
        message: { type: string }
      required: [message]
      additionalProperties: false

accepts:
  effects:
    - name: set_context
      payload_schema:
        type: object
        additionalProperties: false
    - name: publish_sample
      payload_schema:
        type: object
        properties:
          message: { type: string }
        required: [message]
        additionalProperties: false

capture:
  format_version: "1"
  fields:
    - event.sample_event
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
"#
    .to_string()
}

fn ingress_channel_contents() -> String {
    r#"#!/usr/bin/env python3
import json
import sys

frames = [
    {"type": "hello", "protocol": "__PROCESS_DRIVER_PROTOCOL_VERSION__"},
    {
        "type": "event",
        "event": {
            "event_id": "sample-live-evt-1",
            "kind": "Command",
            "at": {"secs": 0, "nanos": 0},
            "semantic_kind": "sample_event",
            "payload": {"message": "hello-from-live-ingress"},
        },
    },
    {"type": "end"},
]

for frame in frames:
    sys.stdout.write(json.dumps(frame) + "\n")
    sys.stdout.flush()
"#
    .replace(
        "__PROCESS_DRIVER_PROTOCOL_VERSION__",
        PROCESS_DRIVER_PROTOCOL_VERSION,
    )
}

fn egress_channel_contents() -> String {
    r#"#!/usr/bin/env python3
import json
import sys

ready = {
    "type": "ready",
    "protocol": "ergo-egress.v1",
    "handled_kinds": ["publish_sample"],
}
sys.stdout.write(json.dumps(ready) + "\n")
sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    frame = json.loads(line)
    if frame.get("type") == "end":
        break
    intent_id = frame.get("intent_id")
    if intent_id:
        ack = {
            "type": "intent_ack",
            "intent_id": intent_id,
            "status": "accepted",
            "acceptance": "durable",
            "egress_ref": "sample-outbox-1",
        }
        sys.stdout.write(json.dumps(ack) + "\n")
        sys.stdout.flush()
"#
    .to_string()
}

fn egress_toml_contents() -> String {
    r#"default_ack_timeout = "5s"

[channels.sample_outbox]
type = "process"
command = ["python3", "channels/egress/sample_outbox.py"]

[routes.publish_sample]
channel = "sample_outbox"
ack_timeout = "5s"
"#
    .to_string()
}

fn fixture_contents() -> String {
    "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"semantic_kind\":\"sample_event\",\"payload\":{\"message\":\"hello-from-historical\"}}}\n".to_string()
}

fn default_sdk_dependency_path(target_dir: &Path) -> Result<String, String> {
    let workspace_root = workspace_root()?;
    if !target_dir.starts_with(&workspace_root) {
        return Err(render_cli_error(
            &CliErrorInfo::new(
                "cli.init_sdk_path_required",
                "default SDK path works only when scaffolding inside the current Ergo checkout",
            )
            .with_where("init command")
            .with_fix(
                "run 'ergo init' inside this checkout or provide --sdk-path <path-to-ergo-sdk-rust>",
            ),
        ));
    }

    let sdk_path = canonicalize_path(
        &workspace_root.join("crates/prod/clients/sdk-rust"),
        "default SDK path",
    )?;
    Ok(render_dependency_path(target_dir, &sdk_path))
}

fn resolve_explicit_sdk_dependency_path(
    target_dir: &Path,
    sdk_path: &Path,
) -> Result<String, String> {
    let sdk_path = absolutize_path(sdk_path, "sdk path")?;
    let sdk_path = canonicalize_path(&sdk_path, "sdk path")?;
    Ok(render_dependency_path(target_dir, &sdk_path))
}

fn workspace_root() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let Some(root) = manifest_dir.ancestors().nth(4) else {
        return Err(render_cli_error(
            &CliErrorInfo::new(
                "cli.init_workspace_root_invalid",
                "failed to derive the Ergo workspace root from the CLI checkout",
            )
            .with_where("init command")
            .with_fix("run ergo init from a valid Ergo checkout or provide --sdk-path explicitly"),
        ));
    };
    canonicalize_path(root, "workspace root")
}

fn canonicalize_path(path: &Path, label: &str) -> Result<PathBuf, String> {
    path.canonicalize().map_err(|err| {
        render_cli_error(
            &CliErrorInfo::new(
                "cli.init_path_invalid",
                format!("failed to resolve {label} '{}'", path.display()),
            )
            .with_where("init command")
            .with_fix("verify the path exists")
            .with_detail(err.to_string()),
        )
    })
}

fn absolutize_path(path: &Path, label: &str) -> Result<PathBuf, String> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|err| {
                render_cli_error(
                    &CliErrorInfo::new(
                        "cli.init_current_dir_unavailable",
                        format!("failed to resolve current directory for {label}"),
                    )
                    .with_where("init command")
                    .with_fix("verify the current working directory exists")
                    .with_detail(err.to_string()),
                )
            })?
            .join(path)
    };
    Ok(normalize_owned_path(&joined))
}

fn normalize_owned_path(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn render_dependency_path(target_dir: &Path, sdk_path: &Path) -> String {
    relative_path_from(target_dir, sdk_path)
        .map(|path| normalize_path(&path))
        .unwrap_or_else(|| normalize_path(sdk_path))
}

fn relative_path_from(from_dir: &Path, to_path: &Path) -> Option<PathBuf> {
    use std::path::Component;

    let from = normalize_owned_path(from_dir);
    let to = normalize_owned_path(to_path);

    let from_components: Vec<_> = from.components().collect();
    let to_components: Vec<_> = to.components().collect();

    let mut shared = 0;
    while shared < from_components.len()
        && shared < to_components.len()
        && from_components[shared] == to_components[shared]
    {
        shared += 1;
    }

    let from_prefix: Vec<_> = from_components[..shared]
        .iter()
        .copied()
        .filter(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
        .collect();
    let to_prefix: Vec<_> = to_components[..shared]
        .iter()
        .copied()
        .filter(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
        .collect();
    if from_prefix != to_prefix {
        return None;
    }

    let mut relative = PathBuf::new();
    for component in &from_components[shared..] {
        if matches!(component, Component::Normal(_)) {
            relative.push("..");
        }
    }
    for component in &to_components[shared..] {
        relative.push(component.as_os_str());
    }

    if relative.as_os_str().is_empty() {
        relative.push(".");
    }
    Some(relative)
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn render_init_summary(summary: &InitSummary) -> String {
    format!(
        "initialized Ergo SDK project at {}\nsdk dependency: {}\nchannel scripts: sample ingress/egress scripts target Python 3\ngenerated guide: {}/README.md\nnext steps:\n  cd {}\n  cargo run\n  cargo run -- profiles\n  cargo run -- doctor\n  cargo run -- validate\n  cargo run -- replay historical captures/historical.capture.json",
        summary.root.display(),
        summary.sdk_dependency_path,
        summary.root.display(),
        summary.root.display(),
    )
}

#[cfg(test)]
mod tests;
