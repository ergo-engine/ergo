use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ergo_adapter::{EventId, ExternalEvent, ExternalEventKind, GraphId, RuntimeHandle};
use ergo_runtime::action::ActionOutcome;
use ergo_runtime::catalog::{build_core_catalog, core_registries};
use ergo_supervisor::demo::demo_1;
use ergo_supervisor::replay::replay_checked;
use ergo_supervisor::{
    CaptureBundle, CapturingSession, Constraints, Decision, DecisionLog, DecisionLogEntry,
    EpisodeInvocationRecord,
};
use serde::Deserialize;

const DEMO_GRAPH_ID: &str = "demo_1";
const DEFAULT_REPLAY_PATH: &str = "target/demo-1-replay.json";

#[derive(Debug)]
struct EpisodeInfo {
    label: String,
    event_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum FixtureRecord {
    EpisodeStart { id: Option<String> },
    Event { event: FixtureEvent },
}

#[derive(Debug, Deserialize)]
struct FixtureEvent {
    #[serde(rename = "type")]
    kind: ExternalEventKind,
    #[serde(default)]
    id: Option<String>,
}

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

fn write_replay_artifact(path: &Path, bundle: &CaptureBundle) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create replay directory: {err}"))?;
    }

    let data = serde_json::to_string_pretty(bundle)
        .map_err(|err| format!("serialize replay bundle: {err}"))?;
    fs::write(path, format!("{data}\n")).map_err(|err| format!("write replay artifact: {err}"))?;
    Ok(())
}

fn load_bundle(path: &Path) -> Result<CaptureBundle, String> {
    let data = fs::read_to_string(path).map_err(|err| format!("read replay artifact: {err}"))?;
    serde_json::from_str(&data).map_err(|err| format!("parse replay artifact: {err}"))
}

fn fixture_output_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "fixture".into());
    PathBuf::from("target").join(format!("{stem}-replay.json"))
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

    let runtime = RuntimeHandle::new(graph.clone(), catalog.clone(), core_registries.clone());
    let mut session = CapturingSession::new(
        GraphId::new(DEMO_GRAPH_ID),
        Constraints::default(),
        NullLog,
        runtime,
    );

    let file = fs::File::open(path).map_err(|err| format!("read fixture: {err}"))?;
    let reader = io::BufReader::new(file);
    let mut episodes: Vec<EpisodeInfo> = Vec::new();
    let mut current_episode: Option<usize> = None;
    let mut event_counter = 0usize;

    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|err| format!("read fixture line {}: {err}", index + 1))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let record: FixtureRecord = serde_json::from_str(trimmed)
            .map_err(|err| format!("fixture parse error at line {}: {err}", index + 1))?;

        match record {
            FixtureRecord::EpisodeStart { id } => {
                let label = id.unwrap_or_else(|| format!("E{}", episodes.len() + 1));
                episodes.push(EpisodeInfo {
                    label,
                    event_ids: Vec::new(),
                });
                current_episode = Some(episodes.len() - 1);
            }
            FixtureRecord::Event { event } => {
                if current_episode.is_none() {
                    let label = format!("E{}", episodes.len() + 1);
                    episodes.push(EpisodeInfo {
                        label,
                        event_ids: Vec::new(),
                    });
                    current_episode = Some(episodes.len() - 1);
                }

                event_counter += 1;
                let event_id = event
                    .id
                    .unwrap_or_else(|| format!("fixture_evt_{}", event_counter));
                let external =
                    ExternalEvent::mechanical(EventId::new(event_id.clone()), event.kind);
                session.on_event(external);

                let episode_index = current_episode.expect("episode index set");
                episodes[episode_index].event_ids.push(event_id);
            }
        }
    }

    if episodes.is_empty() {
        return Err("fixture contained no episodes".to_string());
    }

    if episodes.iter().all(|episode| episode.event_ids.is_empty()) {
        return Err("fixture contained no events".to_string());
    }

    if let Some(episode) = episodes.iter().find(|episode| episode.event_ids.is_empty()) {
        return Err(format!("episode '{}' has no events", episode.label));
    }

    let bundle = session.into_bundle();
    let (action_a_outcome, action_b_outcome) = demo_1_action_outcomes();
    print_fixture_summaries(
        &episodes,
        &bundle.decisions,
        &action_a_outcome,
        &action_b_outcome,
    )?;

    let artifact_path = output_override
        .map(PathBuf::from)
        .unwrap_or_else(|| fixture_output_path(path));
    write_replay_artifact(&artifact_path, &bundle)?;
    println!("replay artifact: {}", artifact_path.display());
    Ok(artifact_path)
}

fn print_fixture_summaries(
    episodes: &[EpisodeInfo],
    decisions: &[EpisodeInvocationRecord],
    action_a_outcome: &ActionOutcome,
    action_b_outcome: &ActionOutcome,
) -> Result<(), String> {
    let mut decisions_by_event: HashMap<String, Vec<Decision>> = HashMap::new();
    for record in decisions {
        decisions_by_event
            .entry(record.event_id.as_str().to_string())
            .or_default()
            .push(record.decision);
    }

    for episode in episodes {
        let mut invoked = false;
        let mut deferred = false;

        for event_id in &episode.event_ids {
            let entries = decisions_by_event
                .get(event_id)
                .ok_or_else(|| format!("no decision for event '{event_id}'"))?;
            if entries.iter().any(|decision| *decision == Decision::Invoke) {
                invoked = true;
            }
            if entries.iter().any(|decision| *decision == Decision::Defer) {
                deferred = true;
            }
        }

        let decision = if invoked {
            Decision::Invoke
        } else if deferred {
            Decision::Defer
        } else {
            Decision::Skip
        };

        println!(
            "{}",
            format_decision_summary(
                &episode.label,
                decision,
                action_a_outcome,
                action_b_outcome
            )
        );
    }

    Ok(())
}

fn format_decision_summary(
    label: &str,
    decision: Decision,
    action_a_outcome: &ActionOutcome,
    action_b_outcome: &ActionOutcome,
) -> String {
    let decision_label = match decision {
        Decision::Invoke => "invoke",
        Decision::Defer => "defer",
        Decision::Skip => "skip",
    };

    let (trigger_a_status, trigger_b_status, action_a_status, action_b_status) = match decision {
        Decision::Invoke => (
            trigger_status(action_a_outcome),
            trigger_status(action_b_outcome),
            action_status(action_a_outcome),
            action_status(action_b_outcome),
        ),
        _ => ("deferred", "deferred", "deferred", "deferred"),
    };

    format!(
        "episode {}: decision={} TriggerA={} TriggerB={} ActionA={} ActionB={}",
        label, decision_label, trigger_a_status, trigger_b_status, action_a_status, action_b_status
    )
}

fn action_status(outcome: &ActionOutcome) -> &'static str {
    if *outcome == ActionOutcome::Skipped {
        "skipped"
    } else {
        "executed"
    }
}

fn trigger_status(outcome: &ActionOutcome) -> &'static str {
    if *outcome == ActionOutcome::Skipped {
        "not_emitted"
    } else {
        "emitted"
    }
}

fn demo_1_action_outcomes() -> (ActionOutcome, ActionOutcome) {
    (ActionOutcome::Completed, ActionOutcome::Skipped)
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

    if let Ok(records) = &replay_result {
        let (action_a_outcome, action_b_outcome) = demo_1_action_outcomes();
        for record in records {
            println!(
                "{}",
                format_decision_summary(
                    &record.episode_id.as_u64().to_string(),
                    record.decision,
                    &action_a_outcome,
                    &action_b_outcome
                )
            );
        }
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
