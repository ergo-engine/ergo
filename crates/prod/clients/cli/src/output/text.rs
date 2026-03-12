use std::path::Path;

use crate::graph_yaml::GraphRunCompletion;

pub fn write_line(message: &str) {
    println!("{message}");
}

pub fn usage() -> String {
    [
        "Ergo CLI (v1)",
        "",
        "Core runtime",
        "  ergo run <graph.yaml> (-f|--fixture <events.jsonl> | --driver-cmd <program> [--driver-arg <arg> ...]) [-a|--adapter <adapter.yaml>] [-o|--capture|--capture-output <path>] [-p|--pretty-capture] [--cluster-path <path> ...]",
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

pub fn help_topic(topic: &str, fixture_usage: &str) -> Option<String> {
    match topic {
        "run" => Some(
            [
                "usage:",
                "  ergo run <graph.yaml> (-f|--fixture <events.jsonl> | --driver-cmd <program> [--driver-arg <arg> ...]) [-a|--adapter <adapter.yaml>] [-o|--capture|--capture-output <path>] [-p|--pretty-capture] [--cluster-path <path> ...]",
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
            Some(fixture_usage.to_string())
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

pub fn render_fixture_run_summary(
    episode_event_counts: &[(String, usize)],
    capture_path: &Path,
) -> String {
    let mut lines = Vec::new();
    for (label, count) in episode_event_counts {
        lines.push(format!("episode {label}: events={count}"));
    }
    lines.push(format!("capture artifact: {}", capture_path.display()));
    lines.join("\n")
}

pub fn render_graph_run_summary(
    completion: GraphRunCompletion,
    episodes: usize,
    events: usize,
    invoked: usize,
    deferred: usize,
    capture_path: &Path,
) -> String {
    let status = match completion {
        GraphRunCompletion::Completed => "status=completed".to_string(),
        GraphRunCompletion::Interrupted { reason } => {
            format!("status=interrupted reason={}", reason.as_str())
        }
    };
    [
        format!(
            "{status} episodes={episodes} events={events} invoked={invoked} deferred={deferred}"
        ),
        format!("capture artifact: {}", capture_path.display()),
    ]
    .join("\n")
}

pub fn render_replay_summary(
    graph_id: &str,
    events: usize,
    invoked: usize,
    deferred: usize,
    skipped: usize,
) -> String {
    [
        format!(
            "replay graph_id={graph_id} events={events} invoked={invoked} deferred={deferred} skipped={skipped}"
        ),
        "replay identity: match".to_string(),
    ]
    .join("\n")
}
