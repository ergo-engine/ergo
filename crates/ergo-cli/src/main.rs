use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod adapter_manifest_io;
mod csv_fixture;
mod error_format;
mod gen_docs;
mod graph_yaml;
mod validate;

use crate::adapter_manifest_io::parse_adapter_manifest;
use crate::error_format::render_error_info;
use ergo_adapter::fixture;
#[cfg(test)]
use ergo_adapter::EventPayload;
use ergo_adapter::{
    adapter_fingerprint, ensure_demo_sources_have_no_required_context, AdapterProvides, EventId,
    ExternalEvent, ExternalEventKind, GraphId, RuntimeHandle,
};
#[cfg(test)]
use ergo_runtime::action::ActionOutcome;
use ergo_runtime::catalog::{build_core_catalog, core_registries};
use ergo_supervisor::demo::demo_1;
use ergo_supervisor::fixture_runner;
use ergo_supervisor::replay::{replay_checked_strict, ReplayError};
use ergo_supervisor::{
    write_capture_bundle, CaptureBundle, CaptureJsonStyle, CapturingSession, Constraints, Decision,
    DecisionLog, DecisionLogEntry, NO_ADAPTER_PROVENANCE,
};
#[cfg(test)]
use std::collections::HashMap;

const DEMO_GRAPH_ID: &str = "demo_1";
const DEFAULT_CAPTURE_PATH: &str = "target/demo-1-capture.json";

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
        Some("csv-to-fixture") => {
            let rest: Vec<String> = args.collect();
            let out = csv_fixture::csv_to_fixture_command(&rest)?;
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
                    let rest: Vec<String> = args.collect();
                    let run_opts = parse_run_artifact_options(&rest, "demo-1")?;
                    run_demo_1(run_opts.pretty_capture, None).map(|_| ())
                }
                "fixture" => {
                    let path = args.next().ok_or_else(usage)?;
                    let rest: Vec<String> = args.collect();
                    let run_opts = parse_run_artifact_options(&rest, "fixture")?;
                    run_fixture(Path::new(&path), None, run_opts.pretty_capture).map(|_| ())
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
            replay_graph(
                Path::new(&path),
                replay_opts
                    .graph_path
                    .as_ref()
                    .expect("replay options enforce graph path"),
                &replay_opts.cluster_paths,
                replay_opts.adapter_path.as_deref(),
            )
        }
        _ => Err(usage()),
    }
}

#[derive(Debug, Default)]
struct ReplayOptions {
    graph_path: Option<PathBuf>,
    adapter_path: Option<PathBuf>,
    cluster_paths: Vec<PathBuf>,
}

#[derive(Debug, Default)]
struct RunArtifactOptions {
    pretty_capture: bool,
}

fn parse_run_artifact_options(args: &[String], target: &str) -> Result<RunArtifactOptions, String> {
    let mut options = RunArtifactOptions::default();

    for arg in args {
        match arg.as_str() {
            "--pretty-capture" => options.pretty_capture = true,
            other => {
                return Err(format!(
                    "unknown run {target} option '{other}'. expected --pretty-capture"
                ));
            }
        }
    }

    Ok(options)
}

fn parse_replay_options(args: &[String]) -> Result<ReplayOptions, String> {
    let mut options = ReplayOptions::default();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--graph" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--graph requires a path".to_string())?;
                options.graph_path = Some(PathBuf::from(value));
                i += 2;
            }
            "--adapter" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--adapter requires a path".to_string())?;
                options.adapter_path = Some(PathBuf::from(value));
                i += 2;
            }
            "--cluster-path" | "--search-path" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| format!("{} requires a path", args[i]))?;
                options.cluster_paths.push(PathBuf::from(value));
                i += 2;
            }
            other => {
                return Err(format!(
                    "unknown replay option '{other}'. expected --graph, --adapter, --cluster-path, or --search-path"
                ))
            }
        }
    }

    if options.graph_path.is_none() {
        return Err("replay requires --graph <graph.yaml>".to_string());
    }

    Ok(options)
}

fn usage() -> String {
    [
        "usage:",
        "  ergo gen-docs [--check]",
        "  ergo validate <manifest.yaml> [--format json]",
        "  ergo csv-to-fixture <prices.csv> <events.jsonl> [--semantic-kind <name>] [--event-kind <Pump|DataAvailable|Command>] [--episode-id <id>]",
        "  ergo check-compose <adapter.yaml> <source|action>.yaml [--format json]",
        "  ergo run demo-1 [--pretty-capture]",
        "  ergo run fixture <path> [--pretty-capture]",
        "  ergo run <graph.yaml> --fixture <events.jsonl> [--adapter <adapter.yaml>] [--capture-output <path>] [--pretty-capture] [--cluster-path <path> ...]",
        "  ergo run <graph.yaml> --direct [--cluster-path <path> ...]",
        "  ergo replay <path> --graph <graph.yaml> [--adapter <adapter.yaml>] [--cluster-path <path> ...]",
    ]
    .join("\n")
}

fn load_bundle(path: &Path) -> Result<CaptureBundle, String> {
    let data = fs::read_to_string(path).map_err(|err| format!("read replay artifact: {err}"))?;
    serde_json::from_str(&data).map_err(|err| format!("parse replay artifact: {err}"))
}

fn run_demo_1(pretty_capture: bool, output_override: Option<&Path>) -> Result<PathBuf, String> {
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

    let artifact_path = output_override
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CAPTURE_PATH));
    let style = if pretty_capture {
        CaptureJsonStyle::Pretty
    } else {
        CaptureJsonStyle::Compact
    };
    write_capture_bundle(&artifact_path, &bundle, style)?;

    println!("capture artifact: {}", artifact_path.display());
    Ok(artifact_path)
}

fn run_fixture(
    path: &Path,
    output_override: Option<&Path>,
    pretty_capture: bool,
) -> Result<PathBuf, String> {
    let graph = Arc::new(demo_1::build_demo_1_graph());
    let catalog = Arc::new(build_core_catalog());
    let core_registries =
        Arc::new(core_registries().map_err(|err| format!("core registries: {err:?}"))?);

    let items =
        fixture::parse_fixture(path).map_err(|err| format!("Failed to parse fixture: {err}"))?;
    let output_path = output_override
        .map(PathBuf::from)
        .unwrap_or_else(|| fixture::fixture_output_path(path));
    let style = if pretty_capture {
        CaptureJsonStyle::Pretty
    } else {
        CaptureJsonStyle::Compact
    };
    let result = fixture_runner::run_fixture(
        items,
        graph,
        catalog,
        core_registries,
        Some(output_path),
        style,
    )?;

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

    println!("capture artifact: {}", result.artifact_path.display());
    Ok(result.artifact_path)
}

#[cfg(test)]
fn action_status(outcome: ActionOutcome) -> &'static str {
    if outcome == ActionOutcome::Skipped {
        "skipped"
    } else {
        "executed"
    }
}

#[cfg(test)]
fn trigger_status(outcome: ActionOutcome) -> &'static str {
    if outcome == ActionOutcome::Skipped {
        "not_emitted"
    } else {
        "emitted"
    }
}

#[cfg(test)]
fn context_value_from_json(payload: &serde_json::Value) -> Option<f64> {
    payload
        .as_object()
        .and_then(|object| object.get(demo_1::CONTEXT_NUMBER_KEY))
        .and_then(|value| value.as_f64())
}

#[cfg(test)]
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

fn replay_graph(
    path: &Path,
    graph_path: &Path,
    cluster_paths: &[PathBuf],
    adapter_path: Option<&Path>,
) -> Result<(), String> {
    let bundle = load_bundle(path)?;
    let prepared = graph_yaml::prepare_graph_runtime(graph_path, cluster_paths)?;

    if bundle.graph_id.as_str() != prepared.graph_id {
        return Err(format!(
            "expected graph_id '{}', got '{}'",
            prepared.graph_id,
            bundle.graph_id.as_str()
        ));
    }

    let (adapter_provides, replay_fingerprint) = if let Some(path) = adapter_path {
        let manifest = parse_adapter_manifest(path)?;
        ergo_adapter::validate_adapter(&manifest)
            .map_err(|err| format!("adapter invalid: {}", render_error_info(&err)))?;
        let provides = AdapterProvides::from_manifest(&manifest);
        graph_yaml::validate_adapter_composition(
            &prepared.expanded,
            &prepared.catalog,
            &prepared.registries,
            &provides,
        )?;
        (provides, Some(adapter_fingerprint(&manifest)))
    } else {
        (AdapterProvides::default(), None)
    };

    let runtime = RuntimeHandle::new(
        Arc::new(prepared.expanded),
        Arc::new(prepared.catalog),
        Arc::new(prepared.registries),
        adapter_provides,
    );

    let replayed = replay_checked_strict(&bundle, runtime, replay_fingerprint.as_deref())
        .map_err(|err| format!("strict replay failed: {}", format_replay_error(&err)))?;
    let replay_matches = replayed == bundle.decisions;

    let invoke_count = replayed
        .iter()
        .filter(|record| record.decision == Decision::Invoke)
        .count();
    let defer_count = replayed
        .iter()
        .filter(|record| record.decision == Decision::Defer)
        .count();
    let skip_count = replayed
        .iter()
        .filter(|record| record.decision == Decision::Skip)
        .count();

    println!(
        "replay graph_id={} events={} invoked={} deferred={} skipped={}",
        bundle.graph_id.as_str(),
        bundle.events.len(),
        invoke_count,
        defer_count,
        skip_count
    );
    println!(
        "replay identity: {}",
        if replay_matches { "match" } else { "mismatch" }
    );

    if !replay_matches {
        return Err("replay decisions must match capture".to_string());
    }

    Ok(())
}

#[cfg(test)]
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
        let output_path = temp_dir.join("fixture-capture.json");
        let fixture_data = "\
{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n\
{\"kind\":\"episode_start\",\"id\":\"E2\"}\n\
{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n";

        fs::write(&fixture_path, fixture_data).map_err(|err| format!("write fixture: {err}"))?;

        let artifact_path = run_fixture(&fixture_path, Some(&output_path), false)?;
        assert_eq!(artifact_path, output_path);
        assert!(artifact_path.exists(), "expected capture artifact to exist");

        replay_demo_1(&artifact_path, None)?;

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn replay_graph_replays_yaml_capture() -> Result<(), String> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-cli-replay-graph-test-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

        let graph_path = temp_dir.join("graph.yaml");
        let fixture_path = temp_dir.join("fixture.jsonl");
        let capture_path = temp_dir.join("capture.json");

        let graph = r#"
kind: cluster
id: replay_basic
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#;
        let fixture = "\
{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n";

        fs::write(&graph_path, graph).map_err(|err| format!("write graph: {err}"))?;
        fs::write(&fixture_path, fixture).map_err(|err| format!("write fixture: {err}"))?;

        let run_args = vec![
            "--fixture".to_string(),
            fixture_path.to_string_lossy().to_string(),
            "--capture-output".to_string(),
            capture_path.to_string_lossy().to_string(),
        ];
        graph_yaml::run_graph_command(&graph_path, &run_args)?;

        replay_graph(&capture_path, &graph_path, &[], None)?;

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn replay_graph_rejects_graph_id_mismatch() -> Result<(), String> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-cli-replay-mismatch-test-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

        let graph_path = temp_dir.join("graph.yaml");
        let other_graph_path = temp_dir.join("other_graph.yaml");
        let fixture_path = temp_dir.join("fixture.jsonl");
        let capture_path = temp_dir.join("capture.json");

        let graph = r#"
kind: cluster
id: replay_basic
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#;

        let other_graph = r#"
kind: cluster
id: replay_other
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 8.0
edges: []
outputs:
  value_out: src.value
"#;

        let fixture = "\
{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n";

        fs::write(&graph_path, graph).map_err(|err| format!("write graph: {err}"))?;
        fs::write(&other_graph_path, other_graph)
            .map_err(|err| format!("write other graph: {err}"))?;
        fs::write(&fixture_path, fixture).map_err(|err| format!("write fixture: {err}"))?;

        let run_args = vec![
            "--fixture".to_string(),
            fixture_path.to_string_lossy().to_string(),
            "--capture-output".to_string(),
            capture_path.to_string_lossy().to_string(),
        ];
        graph_yaml::run_graph_command(&graph_path, &run_args)?;

        let err = replay_graph(&capture_path, &other_graph_path, &[], None)
            .expect_err("graph id mismatch should fail");
        assert!(err.contains("expected graph_id"), "unexpected err: {err}");

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn parse_replay_options_requires_graph() {
        let err = parse_replay_options(&["--adapter".to_string(), "adapter.yaml".to_string()])
            .expect_err("missing graph should fail");
        assert!(
            err.contains("replay requires --graph"),
            "unexpected err: {err}"
        );
    }

    #[test]
    fn parse_replay_options_rejects_pretty_capture_flag() {
        let err = parse_replay_options(&[
            "--graph".to_string(),
            "graph.yaml".to_string(),
            "--pretty-capture".to_string(),
        ])
        .expect_err("unknown replay flag should fail");
        assert!(
            err.contains("unknown replay option '--pretty-capture'"),
            "unexpected err: {err}"
        );
    }

    #[test]
    fn demo_run_pretty_capture_writes_multiline_json() -> Result<(), String> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-cli-demo-pretty-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;
        let output_path = temp_dir.join("demo-capture.json");

        run_demo_1(true, Some(&output_path))?;
        let raw = fs::read_to_string(&output_path).map_err(|err| format!("read output: {err}"))?;
        assert!(
            raw.matches('\n').count() > 1,
            "pretty capture should be multiline json"
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn fixture_run_pretty_capture_writes_multiline_json() -> Result<(), String> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-cli-fixture-pretty-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

        let fixture_path = temp_dir.join("fixture.jsonl");
        let output_path = temp_dir.join("fixture-capture.json");
        let fixture_data = "\
{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n";
        fs::write(&fixture_path, fixture_data).map_err(|err| format!("write fixture: {err}"))?;

        run_fixture(&fixture_path, Some(&output_path), true)?;
        let raw = fs::read_to_string(&output_path).map_err(|err| format!("read output: {err}"))?;
        assert!(
            raw.matches('\n').count() > 1,
            "pretty capture should be multiline json"
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn demo_run_default_output_path_is_capture_named() -> Result<(), String> {
        let artifact_path = run_demo_1(false, None)?;
        assert_eq!(artifact_path, PathBuf::from(DEFAULT_CAPTURE_PATH));
        assert!(
            artifact_path.ends_with("demo-1-capture.json"),
            "expected capture-named artifact path"
        );
        Ok(())
    }

    #[test]
    fn fixture_run_default_output_path_is_capture_named() -> Result<(), String> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-cli-fixture-default-path-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

        let fixture_name = format!("fixture_{index}.jsonl");
        let fixture_path = temp_dir.join(fixture_name.clone());
        let fixture_data = "\
{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n";
        fs::write(&fixture_path, fixture_data).map_err(|err| format!("write fixture: {err}"))?;

        let expected = PathBuf::from("target").join(format!("fixture_{index}-capture.json"));
        let artifact_path = run_fixture(&fixture_path, None, false)?;
        assert_eq!(artifact_path, expected);
        assert!(
            artifact_path.ends_with(format!("fixture_{index}-capture.json")),
            "expected capture-named fixture artifact path"
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
