use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod adapter_manifest_io;
mod csv_fixture;
mod error_format;
mod fixture_ops;
mod gen_docs;
mod graph_to_dot;
mod graph_yaml;
mod render;
mod validate;

use crate::adapter_manifest_io::parse_adapter_manifest;
use crate::error_format::{cli_error_from_error_info, render_cli_error, CliErrorInfo};
use ergo_adapter::fixture;
#[cfg(test)]
use ergo_adapter::EventId;
use ergo_adapter::{
    adapter_fingerprint, compile_event_binder, ensure_demo_sources_have_no_required_context,
    AdapterProvides, EventTime, GraphId, RuntimeHandle,
};
use ergo_host::{
    decision_counts, replay_bundle_strict, HostedAdapterConfig, HostedEvent, HostedReplayError,
    HostedRunner,
};
use ergo_runtime::catalog::{build_core_catalog, core_registries};
use ergo_runtime::provenance::{compute_runtime_provenance, RuntimeProvenanceScheme};
use ergo_supervisor::demo::demo_1;
use ergo_supervisor::replay::{ReplayError, StrictReplayExpectations};
use ergo_supervisor::{
    write_capture_bundle, CaptureBundle, CaptureJsonStyle, Constraints, NO_ADAPTER_PROVENANCE,
};

const DEMO_GRAPH_ID: &str = "demo_1";

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    match args.next() {
        None => {
            print!("{}", usage());
            Ok(())
        }
        Some(command) => match command.as_str() {
            "help" => {
                let topic_parts: Vec<String> = args.collect();
                if topic_parts.is_empty() {
                    print!("{}", usage());
                    return Ok(());
                }
                let topic = topic_parts.join(" ").to_ascii_lowercase();
                if let Some(help_text) = help_topic(&topic) {
                    print!("{help_text}");
                    Ok(())
                } else {
                    Err(render_cli_error(
                        &CliErrorInfo::new(
                            "cli.unknown_help_topic",
                            format!("unknown help topic '{}'", topic_parts.join(" ")),
                        )
                        .with_where("help topic")
                        .with_fix("run 'ergo help' to list available topics"),
                    ))
                }
            }
            "fixture" => {
                let target = args.next().ok_or_else(fixture_ops::fixture_usage)?;
                match target.as_str() {
                    "run" => {
                        let path = args.next().ok_or_else(fixture_ops::fixture_usage)?;
                        let rest: Vec<String> = args.collect();
                        let run_opts = parse_run_artifact_options(&rest, "fixture run")?;
                        run_fixture(
                            Path::new(&path),
                            run_opts.capture_output.as_deref(),
                            run_opts.pretty_capture,
                        )
                        .map(|_| ())
                    }
                    "inspect" => {
                        let rest: Vec<String> = args.collect();
                        let out = fixture_ops::fixture_inspect_command(&rest)?;
                        print!("{out}");
                        Ok(())
                    }
                    "validate" => {
                        let rest: Vec<String> = args.collect();
                        let out = fixture_ops::fixture_validate_command(&rest)?;
                        print!("{out}");
                        Ok(())
                    }
                    _ => Err(render_cli_error(
                        &CliErrorInfo::new(
                            "cli.invalid_subcommand",
                            format!("unknown fixture subcommand '{target}'"),
                        )
                        .with_where("fixture subcommand")
                        .with_fix(fixture_ops::fixture_usage()),
                    )),
                }
            }
            "gen-docs" => {
                let rest: Vec<String> = args.collect();
                let out = gen_docs::gen_docs_command(&rest)?;
                print!("{out}");
                Ok(())
            }
            "validate" => {
                let rest: Vec<String> = args.collect();
                let out = validate::validate_command(&rest)?;
                print!("{out}");
                Ok(())
            }
            "csv-to-fixture" => {
                let rest: Vec<String> = args.collect();
                let out = csv_fixture::csv_to_fixture_command(&rest)?;
                print!("{out}");
                Ok(())
            }
            "check-compose" => {
                let rest: Vec<String> = args.collect();
                let out = validate::check_compose_command(&rest)?;
                print!("{out}");
                Ok(())
            }
            "graph-to-dot" => {
                let rest: Vec<String> = args.collect();
                let out = graph_to_dot::graph_to_dot_command(&rest)?;
                print!("{out}");
                Ok(())
            }
            "render" => {
                let target = args.next().ok_or_else(usage)?;
                match target.as_str() {
                    "graph" => {
                        let rest: Vec<String> = args.collect();
                        let out = render::render_graph_command(&rest)?;
                        print!("{out}");
                        Ok(())
                    }
                    _ => Err(usage()),
                }
            }
            "run" => {
                let target = args.next().ok_or_else(usage)?;
                match target.as_str() {
                    "fixture" => Err(render_cli_error(
                        &CliErrorInfo::new(
                            "cli.command_removed",
                            "'ergo run fixture' was removed in v1",
                        )
                        .with_where("command 'run fixture'")
                        .with_fix("use 'ergo fixture run <events.jsonl>'"),
                    )),
                    _ => {
                        let rest: Vec<String> = args.collect();
                        graph_yaml::run_graph_command(Path::new(&target), &rest)
                    }
                }
            }
            "replay" => {
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
            _ => Err(render_cli_error(
                &CliErrorInfo::new(
                    "cli.unknown_command",
                    format!("unknown command '{command}'"),
                )
                .with_where("command")
                .with_fix("run 'ergo help' to see the v1 command map"),
            )),
        },
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
    capture_output: Option<PathBuf>,
}

fn parse_run_artifact_options(args: &[String], target: &str) -> Result<RunArtifactOptions, String> {
    let mut options = RunArtifactOptions::default();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-p" | "--pretty-capture" => {
                options.pretty_capture = true;
                i += 1;
            }
            "-o" | "--capture" | "--capture-output" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.missing_option_value",
                            format!("{} requires a path", args[i]),
                        )
                        .with_where(format!("arg '{}'", args[i]))
                        .with_fix("provide -o <path>"),
                    )
                })?;
                options.capture_output = Some(PathBuf::from(value));
                i += 2;
            }
            other => {
                return Err(render_cli_error(
                    &CliErrorInfo::new(
                        "cli.invalid_option",
                        format!("unknown run {target} option '{other}'"),
                    )
                    .with_where(format!("arg '{other}'"))
                    .with_fix(format!(
                        "for 'ergo run {target}', use -p|--pretty-capture and -o|--capture|--capture-output"
                    )),
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
            "-g" | "--graph" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.missing_option_value",
                            format!("{} requires a path", args[i]),
                        )
                        .with_where(format!("arg '{}'", args[i]))
                        .with_fix("provide -g <graph.yaml>"),
                    )
                })?;
                options.graph_path = Some(PathBuf::from(value));
                i += 2;
            }
            "-a" | "--adapter" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.missing_option_value",
                            format!("{} requires a path", args[i]),
                        )
                        .with_where(format!("arg '{}'", args[i]))
                        .with_fix("provide -a <adapter.yaml>"),
                    )
                })?;
                options.adapter_path = Some(PathBuf::from(value));
                i += 2;
            }
            "--cluster-path" | "--search-path" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.missing_option_value",
                            format!("{} requires a path", args[i]),
                        )
                        .with_where(format!("arg '{}'", args[i]))
                        .with_fix("provide a directory path"),
                    )
                })?;
                options.cluster_paths.push(PathBuf::from(value));
                i += 2;
            }
            other => {
                return Err(render_cli_error(
                    &CliErrorInfo::new(
                        "cli.invalid_option",
                        format!("unknown replay option '{other}'"),
                    )
                    .with_where(format!("arg '{other}'"))
                    .with_fix(
                        "use -g|--graph, -a|--adapter, --cluster-path, or --search-path for replay",
                    ),
                ))
            }
        }
    }

    if options.graph_path.is_none() {
        return Err(render_cli_error(
            &CliErrorInfo::new(
                "cli.missing_required_option",
                "replay requires -g|--graph <graph.yaml>",
            )
            .with_where("replay command options")
            .with_fix("rerun with -g <graph.yaml>"),
        ));
    }

    Ok(options)
}

fn usage() -> String {
    [
        "Ergo CLI (v1)",
        "",
        "Core runtime",
        "  ergo run <graph.yaml> -f|--fixture <events.jsonl> [-a|--adapter <adapter.yaml>] [-o|--capture|--capture-output <path>] [-p|--pretty-capture] [--cluster-path <path> ...]",
        "  ergo run <graph.yaml> -d|--direct [--cluster-path <path> ...]",
        "  ergo replay <capture.json> -g|--graph <graph.yaml> [-a|--adapter <adapter.yaml>] [--cluster-path <path> ...]",
        "",
        "Fixture operability",
        "  ergo fixture run <events.jsonl> [-o|--capture|--capture-output <path>] [-p|--pretty-capture]",
        "  ergo fixture inspect <events.jsonl> [--format text|json]",
        "  ergo fixture validate <events.jsonl> [--format text|json]",
        "",
        "Graph visualization",
        "  ergo graph-to-dot <graph.yaml> [-o out.dot|--output out.dot] [--show-ports] [--show-impl] [--show-runtime-id] [--cluster-path <path> ...]",
        "  ergo render graph <graph.yaml> [-o out.svg|--output out.svg] [--show-ports] [--show-impl] [--show-runtime-id] [--cluster-path <path> ...]",
        "",
        "Validation and tools",
        "  ergo validate <manifest.yaml> [--format json]",
        "  ergo check-compose <adapter.yaml> <source|action>.yaml [--format json]",
        "  ergo csv-to-fixture <prices.csv> <events.jsonl> [--semantic-kind <name>] [--event-kind <Pump|DataAvailable|Command>] [--episode-id <id>]",
        "  ergo gen-docs [--check]",
        "",
        "Help",
        "  ergo help",
        "  ergo help run|replay|fixture|render graph|graph-to-dot|validate|check-compose|csv-to-fixture|gen-docs",
    ]
    .join("\n")
}

fn help_topic(topic: &str) -> Option<String> {
    match topic {
        "run" => Some(
            [
                "usage:",
                "  ergo run <graph.yaml> -f|--fixture <events.jsonl> [-a|--adapter <adapter.yaml>] [-o|--capture|--capture-output <path>] [-p|--pretty-capture] [--cluster-path <path> ...]",
                "  ergo run <graph.yaml> -d|--direct [--cluster-path <path> ...]",
            ]
            .join("\n"),
        ),
        "replay" => Some(
            [
                "usage:",
                "  ergo replay <capture.json> -g|--graph <graph.yaml> [-a|--adapter <adapter.yaml>] [--cluster-path <path> ...]",
            ]
            .join("\n"),
        ),
        "fixture" | "fixture run" | "fixture inspect" | "fixture validate" => {
            Some(fixture_ops::fixture_usage())
        }
        "render" | "render graph" => Some(
            [
                "usage:",
                "  ergo render graph <graph.yaml> [-o out.svg|--output out.svg] [--show-ports] [--show-impl] [--show-runtime-id] [--cluster-path <path> ...]",
            ]
            .join("\n"),
        ),
        "graph-to-dot" => Some(
            [
                "usage:",
                "  ergo graph-to-dot <graph.yaml> [-o out.dot|--output out.dot] [--show-ports] [--show-impl] [--show-runtime-id] [--cluster-path <path> ...]",
            ]
            .join("\n"),
        ),
        "validate" => Some("usage:\n  ergo validate <manifest.yaml> [--format json]".to_string()),
        "check-compose" => Some(
            "usage:\n  ergo check-compose <adapter.yaml> <source|action>.yaml [--format json]"
                .to_string(),
        ),
        "csv-to-fixture" => Some(
            "usage:\n  ergo csv-to-fixture <prices.csv> <events.jsonl> [--semantic-kind <name>] [--event-kind <Pump|DataAvailable|Command>] [--episode-id <id>]"
                .to_string(),
        ),
        "gen-docs" => Some("usage:\n  ergo gen-docs [--check]".to_string()),
        _ => None,
    }
}

fn load_bundle(path: &Path) -> Result<CaptureBundle, String> {
    let data = fs::read_to_string(path).map_err(|err| {
        render_cli_error(
            &CliErrorInfo::new(
                "replay.capture_read_failed",
                "failed to read capture artifact",
            )
            .with_where(format!("path '{}'", path.display()))
            .with_fix("verify the file path and permissions")
            .with_detail(err.to_string()),
        )
    })?;
    serde_json::from_str(&data).map_err(|err| {
        render_cli_error(
            &CliErrorInfo::new(
                "replay.capture_parse_failed",
                "failed to parse capture artifact",
            )
            .with_where(format!("path '{}'", path.display()))
            .with_fix("ensure the file is a valid Ergo capture bundle JSON")
            .with_detail(err.to_string()),
        )
    })
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
    ensure_demo_sources_have_no_required_context(&graph, &catalog, &core_registries)?;

    let runtime = RuntimeHandle::new(
        graph.clone(),
        catalog.clone(),
        core_registries.clone(),
        AdapterProvides::default(),
    );
    let runtime_provenance = compute_runtime_provenance(
        RuntimeProvenanceScheme::Rpv1,
        DEMO_GRAPH_ID,
        graph.as_ref(),
        catalog.as_ref(),
    )
    .map_err(|err| format!("runtime provenance compute failed: {err}"))?;
    let mut runner = HostedRunner::new(
        GraphId::new(DEMO_GRAPH_ID),
        Constraints::default(),
        runtime,
        runtime_provenance,
        None,
    )
    .map_err(|err| format!("failed to initialize hosted fixture runner: {err}"))?;

    let items =
        fixture::parse_fixture(path).map_err(|err| format!("failed to parse fixture: {err}"))?;
    let mut episodes: Vec<(String, usize)> = Vec::new();
    let mut current_episode: Option<usize> = None;
    let mut event_counter = 0usize;
    for item in items {
        match item {
            fixture::FixtureItem::EpisodeStart { label } => {
                episodes.push((label, 0));
                current_episode = Some(episodes.len() - 1);
            }
            fixture::FixtureItem::Event {
                id,
                kind,
                payload,
                semantic_kind,
            } => {
                if semantic_kind.is_some() {
                    return Err(format!(
                        "fixture event '{}' set semantic_kind but fixture run is adapter-independent",
                        id.as_deref().unwrap_or("<auto>")
                    ));
                }
                if current_episode.is_none() {
                    let label = format!("E{}", episodes.len() + 1);
                    episodes.push((label, 0));
                    current_episode = Some(episodes.len() - 1);
                }

                event_counter += 1;
                let event_id = id.unwrap_or_else(|| format!("fixture_evt_{}", event_counter));
                runner
                    .step(HostedEvent {
                        event_id,
                        kind,
                        at: EventTime::default(),
                        semantic_kind: None,
                        payload,
                    })
                    .map_err(|err| format!("fixture host step failed: {err}"))?;
                let episode_index = current_episode.expect("episode index set");
                episodes[episode_index].1 += 1;
            }
        }
    }

    if episodes.is_empty() {
        return Err("fixture contained no episodes".to_string());
    }
    if event_counter == 0 {
        return Err("fixture contained no events".to_string());
    }
    if let Some((label, _)) = episodes.iter().find(|(_, count)| *count == 0) {
        return Err(format!("episode '{}' has no events", label));
    }

    let bundle = runner.into_capture_bundle();
    let output_path = output_override
        .map(PathBuf::from)
        .unwrap_or_else(|| fixture::fixture_output_path(path));
    let style = if pretty_capture {
        CaptureJsonStyle::Pretty
    } else {
        CaptureJsonStyle::Compact
    };
    write_capture_bundle(&output_path, &bundle, style)?;

    for (label, count) in &episodes {
        println!("episode {label}: events={count}");
    }
    println!("capture artifact: {}", output_path.display());
    Ok(output_path)
}

fn format_replay_error(err: &ReplayError) -> String {
    match err {
        ReplayError::UnsupportedVersion { capture_version } => render_cli_error(
            &CliErrorInfo::new(
                "replay.unsupported_capture_version",
                format!("unsupported capture version '{capture_version}'"),
            )
            .with_where("capture_version")
            .with_fix("regenerate capture with a supported runtime version"),
        ),
        ReplayError::HashMismatch { event_id } => render_cli_error(
            &CliErrorInfo::new(
                "replay.hash_mismatch",
                format!("payload hash mismatch for event '{}'", event_id.as_str()),
            )
            .with_where(format!("event '{}'", event_id.as_str()))
            .with_fix("re-run canonical capture to produce an uncorrupted bundle"),
        ),
        ReplayError::InvalidPayload { event_id, detail } => render_cli_error(
            &CliErrorInfo::new(
                "replay.invalid_payload",
                format!("invalid payload for event '{}'", event_id.as_str()),
            )
            .with_where(format!("event '{}'", event_id.as_str()))
            .with_fix("re-capture with object payloads or repair the capture bundle payload bytes")
            .with_detail(detail.clone()),
        ),
        ReplayError::AdapterProvenanceMismatch { expected, got } => render_cli_error(
            &CliErrorInfo::new(
                "replay.adapter_provenance_mismatch",
                "adapter provenance mismatch",
            )
            .with_where("capture provenance vs replay adapter")
            .with_fix("replay with the adapter used to produce the capture")
            .with_detail(format!("expected: '{expected}'"))
            .with_detail(format!("got: '{got}'")),
        ),
        ReplayError::RuntimeProvenanceMismatch { expected, got } => render_cli_error(
            &CliErrorInfo::new(
                "replay.runtime_provenance_mismatch",
                "runtime provenance mismatch",
            )
            .with_where("capture provenance vs replay runtime surface")
            .with_fix("replay against the graph/runtime used to produce the capture or recapture")
            .with_detail(format!("expected: '{expected}'"))
            .with_detail(format!("got: '{got}'")),
        ),
        ReplayError::UnexpectedAdapterProvidedForNoAdapterCapture => render_cli_error(
            &CliErrorInfo::new(
                "replay.unexpected_adapter",
                "bundle provenance is 'none'; adapter must not be provided",
            )
            .with_where("replay option '--adapter'")
            .with_fix("remove --adapter and replay without adapter"),
        ),
        ReplayError::AdapterRequiredForProvenancedCapture => render_cli_error(
            &CliErrorInfo::new(
                "replay.adapter_required",
                "bundle is adapter-provenanced; adapter is required",
            )
            .with_where("replay option '--adapter'")
            .with_fix("provide --adapter <adapter.yaml> that matches capture provenance"),
        ),
        ReplayError::EffectMismatch {
            event_id,
            effect_index,
            expected,
            actual,
            detail,
        } => {
            let mut info = CliErrorInfo::new(
                "replay.effect_mismatch",
                format!(
                    "effect mismatch at index {} for event '{}': {}",
                    effect_index,
                    event_id.as_str(),
                    detail,
                ),
            )
            .with_where(format!(
                "event '{}' effect[{}]",
                event_id.as_str(),
                effect_index
            ))
            .with_fix("inspect action effect drift and regenerate capture if needed");

            if let Some(exp) = expected {
                info = info.with_detail(format!(
                    "expected: {}",
                    serde_json::to_string(&exp.effect)
                        .unwrap_or_else(|_| "<unserializable>".to_string())
                ));
            }
            if let Some(act) = actual {
                info = info.with_detail(format!(
                    "actual: {}",
                    serde_json::to_string(&act.effect)
                        .unwrap_or_else(|_| "<unserializable>".to_string())
                ));
            }

            render_cli_error(&info)
        }
    }
}

fn format_host_replay_error(err: &HostedReplayError) -> String {
    match err {
        HostedReplayError::Preflight(replay_err) | HostedReplayError::Compare(replay_err) => {
            format_replay_error(replay_err)
        }
        HostedReplayError::EventRehydrate { event_id, detail } => render_cli_error(
            &CliErrorInfo::new(
                "replay.event_rehydrate_failed",
                format!("event '{}' failed rehydration during replay", event_id),
            )
            .with_where(format!("event '{}'", event_id))
            .with_fix("inspect capture payload/hash integrity and recapture if needed")
            .with_detail(detail.clone()),
        ),
        HostedReplayError::Step(step_err) => render_cli_error(
            &CliErrorInfo::new("replay.host_step_failed", "host replay step failed")
                .with_where("ergo-host replay lifecycle")
                .with_fix("inspect host lifecycle/effect handler failures and retry")
                .with_detail(step_err.to_string()),
        ),
        HostedReplayError::DecisionMismatch => render_cli_error(
            &CliErrorInfo::new(
                "replay.decision_mismatch",
                "replay decisions do not match capture decisions",
            )
            .with_where("decision stream comparison")
            .with_fix("inspect runtime/adapter drift and regenerate capture if needed"),
        ),
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
        return Err(render_cli_error(
            &CliErrorInfo::new("replay.graph_id_mismatch", "graph_id mismatch")
                .with_where(format!(
                    "capture graph_id '{}' vs replay graph '{}'",
                    bundle.graph_id.as_str(),
                    prepared.graph_id
                ))
                .with_fix("replay with --graph matching the original capture graph")
                .with_detail(format!("expected: '{}'", prepared.graph_id))
                .with_detail(format!("got: '{}'", bundle.graph_id.as_str())),
        ));
    }

    let (adapter_provides, adapter_config, replay_fingerprint) = if let Some(path) = adapter_path {
        let manifest = parse_adapter_manifest(path)?;
        ergo_adapter::validate_adapter(&manifest).map_err(|err| {
            cli_error_from_error_info(
                "adapter.invalid_manifest",
                "adapter manifest validation failed",
                format!("path '{}'", path.display()),
                &err,
            )
        })?;
        let provides = AdapterProvides::from_manifest(&manifest);
        graph_yaml::validate_adapter_composition(
            &prepared.expanded,
            &prepared.catalog,
            &prepared.registries,
            &provides,
        )?;
        let adapter_provenance = adapter_fingerprint(&manifest);
        let binder = compile_event_binder(&provides).map_err(|err| {
            render_cli_error(
                &CliErrorInfo::new(
                    "adapter.binder_compile_failed",
                    "adapter event binder compilation failed",
                )
                .with_where(format!("path '{}'", path.display()))
                .with_fix("fix adapter event schema/mapping and retry")
                .with_detail(err.to_string()),
            )
        })?;
        let adapter_config = HostedAdapterConfig {
            provides: provides.clone(),
            binder,
            adapter_provenance: adapter_provenance.clone(),
        };
        (provides, Some(adapter_config), Some(adapter_provenance))
    } else {
        (AdapterProvides::default(), None, None)
    };

    let runtime = RuntimeHandle::new(
        Arc::new(prepared.expanded),
        Arc::new(prepared.catalog),
        Arc::new(prepared.registries),
        adapter_provides,
    );
    let runner = HostedRunner::new(
        GraphId::new(bundle.graph_id.as_str().to_string()),
        bundle.config.clone(),
        runtime,
        prepared.runtime_provenance.clone(),
        adapter_config,
    )
    .map_err(|err| {
        render_cli_error(
            &CliErrorInfo::new(
                "host.runner_init_failed",
                "failed to initialize canonical host replay runner",
            )
            .with_where("canonical host replay setup")
            .with_fix("check adapter coverage and host configuration")
            .with_detail(err.to_string()),
        )
    })?;

    let expected_adapter_provenance = replay_fingerprint
        .as_deref()
        .unwrap_or(NO_ADAPTER_PROVENANCE);
    let replayed_bundle = replay_bundle_strict(
        &bundle,
        runner,
        StrictReplayExpectations {
            expected_adapter_provenance,
            expected_runtime_provenance: &prepared.runtime_provenance,
        },
    )
    .map_err(|err| format_host_replay_error(&err))?;

    let (invoke_count, defer_count, skip_count) = decision_counts(&replayed_bundle);

    println!(
        "replay graph_id={} events={} invoked={} deferred={} skipped={}",
        bundle.graph_id.as_str(),
        bundle.events.len(),
        invoke_count,
        defer_count,
        skip_count
    );
    println!("replay identity: match");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn fixture_run_creates_capture_via_host_runner() -> Result<(), String> {
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
        let raw =
            fs::read_to_string(&artifact_path).map_err(|err| format!("read capture: {err}"))?;
        let value: serde_json::Value =
            serde_json::from_str(&raw).map_err(|err| format!("parse capture json: {err}"))?;
        let decisions = value
            .get("decisions")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "capture decisions missing".to_string())?;
        assert!(
            decisions
                .iter()
                .all(|record| record.get("effects").is_some()),
            "serialized capture decisions must include effects field"
        );
        let bundle = load_bundle(&artifact_path)?;
        assert_eq!(bundle.graph_id.as_str(), DEMO_GRAPH_ID);
        assert_eq!(bundle.events.len(), 2, "two events expected");
        assert_eq!(bundle.decisions.len(), 2, "two decisions expected");

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
    fn replay_graph_uses_host_rehydrate_path() -> Result<(), String> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-cli-replay-host-rehydrate-{}-{}",
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

        let mut bundle = load_bundle(&capture_path)?;
        bundle.events[0].payload.data = br#""not-an-object""#.to_vec();
        bundle.events[0].payload_hash =
            ergo_adapter::capture::hash_payload(&bundle.events[0].payload);
        fs::write(
            &capture_path,
            serde_json::to_vec_pretty(&bundle)
                .map_err(|err| format!("serialize capture: {err}"))?,
        )
        .map_err(|err| format!("rewrite capture: {err}"))?;

        let err = replay_graph(&capture_path, &graph_path, &[], None)
            .expect_err("host replay should reject invalid rehydrated event payload");
        assert!(
            err.contains("code: replay.event_rehydrate_failed"),
            "unexpected err: {err}"
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn replay_graph_detects_effect_drift() -> Result<(), String> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-cli-replay-effect-drift-{}-{}",
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

        let mut bundle = load_bundle(&capture_path)?;
        let fake_effect = ergo_runtime::common::ActionEffect {
            kind: "set_context".to_string(),
            writes: vec![ergo_runtime::common::EffectWrite {
                key: "drifted".to_string(),
                value: ergo_runtime::common::Value::Number(42.0),
            }],
        };
        bundle.decisions[0]
            .effects
            .push(ergo_supervisor::CapturedActionEffect {
                effect_hash: ergo_supervisor::replay::hash_effect(&fake_effect),
                effect: fake_effect,
            });
        fs::write(
            &capture_path,
            serde_json::to_vec_pretty(&bundle)
                .map_err(|err| format!("serialize capture: {err}"))?,
        )
        .map_err(|err| format!("rewrite capture: {err}"))?;

        let err = replay_graph(&capture_path, &graph_path, &[], None)
            .expect_err("effect drift should fail canonical replay");
        assert!(
            err.contains("code: replay.effect_mismatch"),
            "unexpected err: {err}"
        );

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
        assert!(
            err.contains("error: graph_id mismatch"),
            "unexpected err: {err}"
        );
        assert!(
            err.contains("where: capture graph_id"),
            "unexpected err: {err}"
        );
        assert!(
            err.contains("fix: replay with --graph"),
            "unexpected err: {err}"
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn parse_replay_options_requires_graph() {
        let err = parse_replay_options(&["--adapter".to_string(), "adapter.yaml".to_string()])
            .expect_err("missing graph should fail");
        assert!(
            err.contains("replay requires -g|--graph"),
            "unexpected err: {err}"
        );
        assert!(
            err.contains("code: cli.missing_required_option")
                && err.contains("where: replay command options")
                && err.contains("fix: rerun with -g <graph.yaml>"),
            "unexpected err: {err}"
        );
    }

    #[test]
    fn parse_replay_options_accepts_short_graph_and_adapter_flags() {
        let opts = parse_replay_options(&[
            "-g".to_string(),
            "graph.yaml".to_string(),
            "-a".to_string(),
            "adapter.yaml".to_string(),
        ])
        .expect("short replay flags should parse");
        assert_eq!(
            opts.graph_path.as_ref().map(|v| v.as_path()),
            Some(Path::new("graph.yaml"))
        );
        assert_eq!(
            opts.adapter_path.as_ref().map(|v| v.as_path()),
            Some(Path::new("adapter.yaml"))
        );
    }

    #[test]
    fn parse_replay_options_keeps_long_flag_compatibility() {
        let opts = parse_replay_options(&[
            "--graph".to_string(),
            "graph.yaml".to_string(),
            "--adapter".to_string(),
            "adapter.yaml".to_string(),
        ])
        .expect("long replay flags should parse");
        assert_eq!(
            opts.graph_path.as_ref().map(|v| v.as_path()),
            Some(Path::new("graph.yaml"))
        );
        assert_eq!(
            opts.adapter_path.as_ref().map(|v| v.as_path()),
            Some(Path::new("adapter.yaml"))
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
        assert!(
            err.contains("code: cli.invalid_option")
                && err.contains("where: arg '--pretty-capture'")
                && err.contains("fix: use -g|--graph, -a|--adapter"),
            "unexpected err: {err}"
        );
    }

    #[test]
    fn parse_run_artifact_options_supports_short_flags_and_capture_alias() -> Result<(), String> {
        let opts = parse_run_artifact_options(
            &[
                "-p".to_string(),
                "-o".to_string(),
                "demo-short.json".to_string(),
            ],
            "fixture",
        )?;
        assert!(opts.pretty_capture);
        assert_eq!(
            opts.capture_output.as_ref().map(|v| v.as_path()),
            Some(Path::new("demo-short.json"))
        );

        let alias_opts = parse_run_artifact_options(
            &[
                "--capture".to_string(),
                "demo-alias.json".to_string(),
                "--pretty-capture".to_string(),
            ],
            "fixture",
        )?;
        assert!(alias_opts.pretty_capture);
        assert_eq!(
            alias_opts.capture_output.as_ref().map(|v| v.as_path()),
            Some(Path::new("demo-alias.json"))
        );

        Ok(())
    }

    #[test]
    fn parse_run_artifact_options_keeps_long_flag_compatibility() -> Result<(), String> {
        let opts = parse_run_artifact_options(
            &[
                "--capture-output".to_string(),
                "demo-long.json".to_string(),
                "--pretty-capture".to_string(),
            ],
            "fixture",
        )?;
        assert!(opts.pretty_capture);
        assert_eq!(
            opts.capture_output.as_ref().map(|v| v.as_path()),
            Some(Path::new("demo-long.json"))
        );
        Ok(())
    }

    #[test]
    fn parse_run_artifact_options_unknown_flag_is_actionable() {
        let err = parse_run_artifact_options(&["--wat".to_string()], "fixture")
            .expect_err("unknown run option should fail");
        assert!(
            err.contains("code: cli.invalid_option")
                && err.contains("where: arg '--wat'")
                && err.contains("fix: for 'ergo run fixture'"),
            "unexpected err: {err}"
        );
    }

    #[test]
    fn usage_moves_fixture_to_top_level_subcommand() {
        let help = usage();
        assert!(
            help.contains("ergo fixture run <events.jsonl>"),
            "expected fixture run in top-level help: {help}"
        );
        assert!(
            !help.contains("ergo run fixture"),
            "run fixture should be removed in v1 help: {help}"
        );
    }

    #[test]
    fn help_topic_fixture_matches_fixture_usage() {
        let topic = help_topic("fixture").expect("fixture help should exist");
        assert_eq!(topic, fixture_ops::fixture_usage());
    }

    #[test]
    fn help_topic_unknown_returns_none() {
        assert!(help_topic("does-not-exist").is_none());
    }

    #[test]
    fn format_replay_error_includes_rule_like_fields() {
        let err = format_replay_error(&ReplayError::AdapterRequiredForProvenancedCapture);
        assert!(
            err.contains("error:")
                && err.contains("code: replay.adapter_required")
                && err.contains("where:")
                && err.contains("fix:"),
            "unexpected err: {err}"
        );
    }

    #[test]
    fn format_replay_error_effect_mismatch_includes_code() {
        let err = format_replay_error(&ReplayError::EffectMismatch {
            event_id: EventId::new("e1"),
            effect_index: 0,
            expected: None,
            actual: None,
            detail: "hash differs".to_string(),
        });
        assert!(
            err.contains("code: replay.effect_mismatch"),
            "expected replay.effect_mismatch code: {err}"
        );
        assert!(
            err.contains("error:") && err.contains("where:") && err.contains("fix:"),
            "unexpected format: {err}"
        );
    }

    #[test]
    fn format_replay_error_effect_mismatch_surfaces_expected_actual() {
        use ergo_runtime::common::{ActionEffect, EffectWrite, Value};
        use ergo_supervisor::replay::hash_effect;
        use ergo_supervisor::CapturedActionEffect;

        let expected_effect = ActionEffect {
            kind: "set_context".to_string(),
            writes: vec![EffectWrite {
                key: "price".to_string(),
                value: Value::Number(42.0),
            }],
        };
        let actual_effect = ActionEffect {
            kind: "set_context".to_string(),
            writes: vec![EffectWrite {
                key: "volume".to_string(),
                value: Value::Number(99.0),
            }],
        };
        let err = format_replay_error(&ReplayError::EffectMismatch {
            event_id: EventId::new("e1"),
            effect_index: 0,
            expected: Some(CapturedActionEffect {
                effect: expected_effect,
                effect_hash: hash_effect(&ActionEffect {
                    kind: "set_context".to_string(),
                    writes: vec![EffectWrite {
                        key: "price".to_string(),
                        value: Value::Number(42.0),
                    }],
                }),
            }),
            actual: Some(CapturedActionEffect {
                effect: actual_effect,
                effect_hash: hash_effect(&ActionEffect {
                    kind: "set_context".to_string(),
                    writes: vec![EffectWrite {
                        key: "volume".to_string(),
                        value: Value::Number(99.0),
                    }],
                }),
            }),
            detail: "content mismatch".to_string(),
        });
        assert!(
            err.contains("code: replay.effect_mismatch"),
            "expected code: {err}"
        );
        assert!(
            err.contains("detail: expected:") && err.contains("\"price\""),
            "expected effect detail with key 'price': {err}"
        );
        assert!(
            err.contains("detail: actual:") && err.contains("\"volume\""),
            "actual effect detail with key 'volume': {err}"
        );
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
    fn fixture_run_short_o_overrides_output_path() -> Result<(), String> {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "ergo-cli-fixture-short-o-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

        let fixture_path = temp_dir.join("fixture.jsonl");
        let output_path = temp_dir.join("fixture-short-output.json");
        let fixture_data = "\
{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n";
        fs::write(&fixture_path, fixture_data).map_err(|err| format!("write fixture: {err}"))?;

        let opts = parse_run_artifact_options(
            &[
                "-o".to_string(),
                output_path.to_string_lossy().to_string(),
                "-p".to_string(),
            ],
            "fixture",
        )?;
        let artifact_path = run_fixture(
            &fixture_path,
            opts.capture_output.as_deref(),
            opts.pretty_capture,
        )?;
        assert_eq!(artifact_path, output_path);
        assert!(
            artifact_path.exists(),
            "expected fixture output override to exist"
        );

        let _ = fs::remove_dir_all(&temp_dir);
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
