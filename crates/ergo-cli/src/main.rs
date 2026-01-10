use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ergo_adapter::fixture;
use ergo_adapter::{
    EventId, EventPayload, ExternalEvent, ExternalEventKind, GraphId, RuntimeHandle,
};
use ergo_runtime::action::ActionOutcome;
use ergo_runtime::catalog::{build_core_catalog, core_registries};
use ergo_supervisor::demo::demo_1;
use ergo_supervisor::fixture_runner;
use ergo_supervisor::replay::replay_checked;
use ergo_supervisor::{
    CaptureBundle, CapturingSession, Constraints, Decision, DecisionLog, DecisionLogEntry,
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
        Some("run") => match args.next().as_deref() {
            Some("demo-1") => {
                ensure_no_extra_args(&mut args)?;
                run_demo_1()
            }
            Some("fixture") => {
                let path = args.next().ok_or_else(usage)?;
                ensure_no_extra_args(&mut args)?;
                run_fixture(Path::new(&path), None).map(|_| ())
            }
            _ => Err(usage()),
        },
        Some("replay") => {
            let path = args.next().ok_or_else(usage)?;
            ensure_no_extra_args(&mut args)?;
            replay_demo_1(Path::new(&path))
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

fn usage() -> String {
    [
        "usage:",
        "  ergo run demo-1",
        "  ergo run fixture <path>",
        "  ergo replay <path>",
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

    let runtime = RuntimeHandle::new(graph.clone(), catalog.clone(), core_registries.clone());
    let mut session = CapturingSession::new(
        GraphId::new(DEMO_GRAPH_ID),
        Constraints::default(),
        NullLog,
        runtime,
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

fn action_status(invoked: bool, outcome: ActionOutcome) -> &'static str {
    if !invoked {
        return "deferred";
    }

    if outcome == ActionOutcome::Skipped {
        "skipped"
    } else {
        "executed"
    }
}

fn trigger_status(invoked: bool, outcome: ActionOutcome) -> &'static str {
    if !invoked {
        return "deferred";
    }

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

fn replay_demo_1(path: &Path) -> Result<(), String> {
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

    let runtime = RuntimeHandle::new(graph.clone(), catalog.clone(), core_registries.clone());
    let replay_result = replay_checked(&bundle, runtime);
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
        let context_value = context_by_event
            .get(record.event_id.as_str())
            .copied()
            .flatten();
        let summary = demo_1::summary_for_context_value(context_value);
        let action_a_status = action_status(invoked, summary.action_a_outcome.clone());
        let action_b_status = action_status(invoked, summary.action_b_outcome.clone());
        let trigger_a_status = trigger_status(invoked, summary.action_a_outcome.clone());
        let trigger_b_status = trigger_status(invoked, summary.action_b_outcome.clone());
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
        eprintln!("replay error: {err:?}");
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

        replay_demo_1(&artifact_path)?;

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
