use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use ergo_adapter::{EventId, ExternalEvent, ExternalEventKind, GraphId, RuntimeHandle};
use ergo_runtime::catalog::{build_core_catalog, core_registries};
use ergo_supervisor::demo::demo_1;
use ergo_supervisor::replay::replay_checked;
use ergo_supervisor::{CaptureBundle, CapturingSession, Constraints, DecisionLog, DecisionLogEntry};

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
            _ => Err(usage()),
        },
        Some("replay") => {
            let path = args.next().ok_or_else(usage)?;
            ensure_no_extra_args(&mut args)?;
            replay_demo_1(&path)
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
        "  ergo replay <path>",
    ]
    .join("\n")
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
    if let Some(parent) = artifact_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create replay directory: {err}"))?;
    }

    let data = serde_json::to_string_pretty(&bundle)
        .map_err(|err| format!("serialize replay bundle: {err}"))?;
    fs::write(&artifact_path, format!("{data}\n"))
        .map_err(|err| format!("write replay artifact: {err}"))?;

    println!("replay artifact: {}", artifact_path.display());
    Ok(())
}

fn replay_demo_1(path: &str) -> Result<(), String> {
    let data = fs::read_to_string(path).map_err(|err| format!("read replay artifact: {err}"))?;
    let bundle: CaptureBundle =
        serde_json::from_str(&data).map_err(|err| format!("parse replay artifact: {err}"))?;

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

    let summary = demo_1::compute_summary(&graph, &catalog, &core_registries);
    for record in &bundle.decisions {
        println!(
            "{}",
            demo_1::format_episode_summary(record.episode_id, &record.event_id, &summary)
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
