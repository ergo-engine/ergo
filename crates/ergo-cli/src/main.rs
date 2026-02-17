use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod adapter_manifest_io;
mod error_format;
mod gen_docs;
mod graph_yaml;
mod validate;

use crate::adapter_manifest_io::parse_adapter_manifest;
use crate::error_format::render_error_info;
use ergo_adapter::fixture;
use ergo_adapter::{
    adapter_fingerprint, ensure_demo_sources_have_no_required_context, AdapterProvides, EventId,
    EventPayload, ExternalEvent, ExternalEventKind, GraphId, RuntimeHandle,
};
use ergo_runtime::action::ActionOutcome;
use ergo_runtime::catalog::{build_core_catalog, core_registries};
use ergo_supervisor::demo::demo_1;
use ergo_supervisor::fixture_runner;
use ergo_supervisor::replay::{replay_checked_strict, ReplayError};
use ergo_supervisor::{
    CaptureBundle, CapturingSession, Constraints, Decision, DecisionLog, DecisionLogEntry,
    NO_ADAPTER_PROVENANCE,
};

const DEMO_GRAPH_ID: &str = "demo_1";
const DEFAULT_REPLAY_PATH: &str = "target/demo-1-replay.json";

struct NullLog;

impl DecisionLog for NullLog {
    fn log(&self, _entry: DecisionLogEntry) {}
}

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("gen-docs") => {
            let rest: Vec<String> = args.collect();
            let out = gen_docs::gen_docs_command(&rest)?;
            print!("{out}");
            Ok(())
        }
        Some("validate") => {
            let rest: Vec<String> = args.collect();
            let out = validate::validate_command(&rest)?;
            print!("{out}");
            Ok(())
        }
        Some("check-compose") => {
            let rest: Vec<String> = args.collect();
            let out = validate::check_compose_command(&rest)?;
            print!("{out}");
            Ok(())
        }
        Some("run") => {
            let target = args.next().ok_or_else(usage)?;
            match target.as_str() {
                "demo-1" => {
                    ensure_no_extra_args(&mut args)?;
                    run_demo_1()
                }
                "fixture" => {
                    let path = args.next().ok_or_else(usage)?;
                    ensure_no_extra_args(&mut args)?;
                    run_fixture(Path::new(&path), None).map(|_| ())
                }
                _ => {
                    let rest: Vec<String> = args.collect();
                    graph_yaml::run_graph_command(Path::new(&target), &rest)
                }
            }
        }
        Some("replay") => {
            let path = args.next().ok_or_else(usage)?;
            let rest: Vec<String> = args.collect();
            let replay_opts = parse_replay_options(&rest)?;
            replay_demo_1(Path::new(&path), replay_opts.adapter_path.as_deref())
        }
        _ => Err(usage()),
    }
}

fn ensure_no_extra_args(args: &mut impl Iterator<Item = String>) -> Result<(), String> {
    if args.next().is_some() {
        return Err(usage());
    }
    Ok(())
}

#[derive(Debug, Default)]
struct ReplayOptions {
    adapter_path: Option<PathBuf>,
}

fn parse_replay_options(args: &[String]) -> Result<ReplayOptions, String> {
    let mut options = ReplayOptions::default();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--adapter" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--adapter requires a path".to_string())?;
                options.adapter_path = Some(PathBuf::from(value));
                i += 2;
            }
            other => {
                return Err(format!(
                    "unknown replay option '{other}'. expected --adapter"
                ))
            }
        }
    }

    Ok(options)
}

fn usage() -> String {
    [
        "usage:",
        "  ergo gen-docs [--check]",
        "  ergo validate <manifest.yaml> [--format json]",
        "  ergo check-compose <adapter.yaml> <source|action>.yaml [--format json]",
        "  ergo run demo-1",
        "  ergo run fixture <path>",
        "  ergo run <graph.yaml> --fixture <events.jsonl> [--adapter <adapter.yaml>] [--capture-output <path>] [--cluster-path <path> ...]",
        "  ergo run <graph.yaml> --direct [--cluster-path <path> ...]",
        "  ergo replay <path> [--adapter <adapter.yaml>]",
    ]
    .join("\n")
}

fn load_bundle(path: &Path) -> Result<CaptureBundle, String> {
    let data = fs::read_to_string(path).map_err(|err| format!("read replay artifact: {err}"))?;
    serde_json::from_str(&data).map_err(|err| format!("parse replay artifact: {err}"))
}

fn write_replay_artifact(path: &Path, bundle: &CaptureBundle) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create replay directory: {err}"))?;
    }

    let data = serde_json::to_string_pretty(bundle)
        .map_err(|err| format!("serialize replay bundle: {err}"))?;
    fs::write(path, format!("{data}\n")).map_err(|err| format!("write replay artifact: {err}"))?;
    Ok(())
}

fn run_demo_1() -> Result<(), String> {
    let graph = Arc::new(demo_1::build_demo_1_graph());
    let catalog = Arc::new(build_core_catalog());
    let core_registries =
        Arc::new(core_registries().map_err(|err| format!("core registries: {err:?}"))?);

    ensure_demo_sources_have_no_required_context(&graph, &catalog, &core_registries)?;

    let runtime = RuntimeHandle::new(
        graph.clone(),
        catalog.clone(),
        core_registries.clone(),
        AdapterProvides::default(),
    );
    let mut session = CapturingSession::new_with_provenance(
        GraphId::new(DEMO_GRAPH_ID),
        Constraints::default(),
        NullLog,
        runtime,
        NO_ADAPTER_PROVENANCE.to_string(),
    );

    let events = [
        ExternalEvent::mechanical(EventId::new("demo_evt_1"), ExternalEventKind::Command),
        ExternalEvent::mechanical(EventId::new("demo_evt_2"), ExternalEventKind::Command),
    ];

    for event in events {
        session.on_event(event);
    }

    let bundle = session.into_bundle();
    let summary = demo_1::compute_summary(&graph, &catalog, &core_registries);

    for record in &bundle.decisions {
        println!(
            "{}",
            demo_1::format_episode_summary(record.episode_id, &record.event_id, &summary)
        );
    }

    let artifact_path = PathBuf::from(DEFAULT_REPLAY_PATH);
    write_replay_artifact(&artifact_path, &bundle)?;

    println!("replay artifact: {}", artifact_path.display());
    Ok(())
}

fn run_fixture(path: &Path, output_override: Option<&Path>) -> Result<PathBuf, String> {
    let graph = Arc::new(demo_1::build_demo_1_graph());
    let catalog = Arc::new(build_core_catalog());
    let core_registries =
        Arc::new(core_registries().map_err(|err| format!("core registries: {err:?}"))?);

    let items =
        fixture::parse_fixture(path).map_err(|err| format!("Failed to parse fixture: {err}"))?;
    let output_path = output_override
        .map(PathBuf::from)
        .unwrap_or_else(|| fixture::fixture_output_path(path));
    let result =
        fixture_runner::run_fixture(items, graph, catalog, core_registries, Some(output_path))?;

    for episode in &result.episodes {
        println!(
            "episode {}: decision={} TriggerA={} TriggerB={} ActionA={} ActionB={}",
            episode.label,
            episode.decision,
            episode.trigger_a,
            episode.trigger_b,
            episode.action_a,
            episode.action_b
        );
    }

    println!("replay artifact: {}", result.artifact_path.display());
    Ok(result.artifact_path)
}

fn action_status(outcome: ActionOutcome) -> &'static str {
    if outcome == ActionOutcome::Skipped {
        "skipped"
    } else {
        "executed"
    }
}

fn trigger_status(outcome: ActionOutcome) -> &'static str {
    if outcome == ActionOutcome::Skipped {
        "not_emitted"
    } else {
        "emitted"
    }
}

fn context_value_from_json(payload: &serde_json::Value) -> Option<f64> {
    payload
        .as_object()
        .and_then(|object| object.get(demo_1::CONTEXT_NUMBER_KEY))
        .and_then(|value| value.as_f64())
}

fn context_value_from_payload(payload: &EventPayload) -> Option<f64> {
    if payload.data.is_empty() {
        return None;
    }

    let parsed: serde_json::Value = serde_json::from_slice(&payload.data).ok()?;
    context_value_from_json(&parsed)
}

fn format_replay_error(err: &ReplayError) -> String {
    match err {
        ReplayError::UnsupportedVersion { capture_version } => {
            format!("unsupported capture version '{capture_version}'")
        }
        ReplayError::HashMismatch { event_id } => {
            format!("payload hash mismatch for event '{}'", event_id.as_str())
        }
        ReplayError::AdapterProvenanceMismatch { expected, got } => {
            format!("adapter provenance mismatch: expected '{expected}', got '{got}'")
        }
        ReplayError::UnexpectedAdapterProvidedForNoAdapterCapture => {
            "bundle provenance is 'none'; do not pass --adapter".to_string()
        }
        ReplayError::AdapterRequiredForProvenancedCapture => {
            "bundle is adapter-provenanced; provide --adapter <adapter.yaml>".to_string()
        }
    }
}

fn replay_demo_1(path: &Path, adapter_path: Option<&Path>) -> Result<(), String> {
    let bundle = load_bundle(path)?;

    if bundle.graph_id.as_str() != DEMO_GRAPH_ID {
        return Err(format!(
            "expected graph_id '{DEMO_GRAPH_ID}', got '{}'",
            bundle.graph_id.as_str()
        ));
    }

    let graph = Arc::new(demo_1::build_demo_1_graph());
    let catalog = Arc::new(build_core_catalog());
    let core_registries =
        Arc::new(core_registries().map_err(|err| format!("core registries: {err:?}"))?);

    let (adapter_provides, replay_fingerprint) = if let Some(path) = adapter_path {
        let manifest = parse_adapter_manifest(path)?;
        ergo_adapter::validate_adapter(&manifest)
            .map_err(|err| format!("adapter invalid: {}", render_error_info(&err)))?;
        (
            AdapterProvides::from_manifest(&manifest),
            Some(adapter_fingerprint(&manifest)),
        )
    } else {
        (AdapterProvides::default(), None)
    };

    let runtime = RuntimeHandle::new(
        graph.clone(),
        catalog.clone(),
        core_registries.clone(),
        adapter_provides,
    );
    let replay_result = replay_checked_strict(&bundle, runtime, replay_fingerprint.as_deref());
    let replay_matches = match &replay_result {
        Ok(records) => records == &bundle.decisions,
        Err(_) => false,
    };

    let mut context_by_event: HashMap<String, Option<f64>> = HashMap::new();
    for record in &bundle.events {
        context_by_event.insert(
            record.event_id.as_str().to_string(),
            context_value_from_payload(&record.payload),
        );
    }

    for record in &bundle.decisions {
        let invoked = record.decision == Decision::Invoke;
        let decision_label = match record.decision {
            Decision::Invoke => "invoke",
            Decision::Defer => "defer",
            Decision::Skip => "skip",
        };
        let (trigger_a_status, trigger_b_status, action_a_status, action_b_status) = if invoked {
            let context_value = context_by_event
                .get(record.event_id.as_str())
                .copied()
                .flatten();
            let summary = demo_1::summary_for_context_value(context_value);
            (
                trigger_status(summary.action_a_outcome.clone()),
                trigger_status(summary.action_b_outcome.clone()),
                action_status(summary.action_a_outcome.clone()),
                action_status(summary.action_b_outcome),
            )
        } else {
            ("deferred", "deferred", "deferred", "deferred")
        };
        let label = format!("E{}", record.episode_id.as_u64() + 1);

        println!(
            "episode {}: decision={} TriggerA={} TriggerB={} ActionA={} ActionB={}",
            label,
            decision_label,
            trigger_a_status,
            trigger_b_status,
            action_a_status,
            action_b_status
        );
    }

    if let Err(err) = replay_result {
        return Err(format!(
            "strict replay failed: {}",
            format_replay_error(&err)
        ));
    }

    println!("{}", demo_1::format_replay_identity(replay_matches));
    if !replay_matches {
        return Err("replay decisions must match capture".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn fixture_run_creates_replay_and_replays_ok() -> Result<(), String> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-cli-fixture-test-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

        let fixture_path = temp_dir.join("fixture.jsonl");
        let output_path = temp_dir.join("fixture-replay.json");
        let fixture_data = "\
{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n\
{\"kind\":\"episode_start\",\"id\":\"E2\"}\n\
{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n";

        fs::write(&fixture_path, fixture_data).map_err(|err| format!("write fixture: {err}"))?;

        let artifact_path = run_fixture(&fixture_path, Some(&output_path))?;
        assert_eq!(artifact_path, output_path);
        assert!(artifact_path.exists(), "expected replay artifact to exist");

        replay_demo_1(&artifact_path, None)?;

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
