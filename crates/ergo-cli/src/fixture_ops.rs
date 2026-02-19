use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ergo_adapter::fixture::{parse_fixture, FixtureItem};
use ergo_adapter::ExternalEventKind;
use serde::Serialize;

use crate::error_format::{render_cli_error, CliErrorInfo};

const FIXTURE_OUTPUT_SCHEMA_VERSION: &str = "v1";

#[derive(Debug, Clone, Copy, Default)]
enum OutputFormat {
    #[default]
    Text,
    Json,
}

#[derive(Debug, Default)]
struct FixtureReportOptions {
    path: Option<PathBuf>,
    format: OutputFormat,
}

#[derive(Debug, Clone)]
struct FixtureAnalysis {
    total_items: usize,
    event_count: usize,
    events_with_payload: usize,
    events_with_semantic_kind: usize,
    event_kind_counts: BTreeMap<String, usize>,
    episode_summaries: Vec<EpisodeSummaryV1>,
}

#[derive(Debug, Clone, Serialize)]
struct EpisodeSummaryV1 {
    label: String,
    event_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct FixtureStatsV1 {
    total_items: usize,
    episode_count: usize,
    event_count: usize,
    events_with_payload: usize,
    events_with_semantic_kind: usize,
    event_kind_counts: BTreeMap<String, usize>,
    episodes: Vec<EpisodeSummaryV1>,
}

#[derive(Debug, Clone, Serialize)]
struct FixtureIssueV1 {
    code: String,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct FixtureInspectOutputV1 {
    schema_version: &'static str,
    command: &'static str,
    fixture_path: String,
    stats: FixtureStatsV1,
}

#[derive(Debug, Clone, Serialize)]
struct FixtureValidateOutputV1 {
    schema_version: &'static str,
    command: &'static str,
    fixture_path: String,
    valid: bool,
    stats: Option<FixtureStatsV1>,
    issues: Vec<FixtureIssueV1>,
}

pub fn fixture_usage() -> String {
    [
        "usage:",
        "  ergo fixture run <events.jsonl> [-o|--capture|--capture-output <path>] [-p|--pretty-capture]",
        "  ergo fixture inspect <events.jsonl> [--format text|json]",
        "  ergo fixture validate <events.jsonl> [--format text|json]",
    ]
    .join("\n")
}

pub fn fixture_inspect_command(args: &[String]) -> Result<String, String> {
    let options = parse_fixture_report_options(
        args,
        "inspect",
        "usage: ergo fixture inspect <events.jsonl> [--format text|json]",
    )?;
    let items =
        parse_fixture(options.path.as_ref().expect("path is validated")).map_err(|err| {
            render_cli_error(
                &CliErrorInfo::new("fixture.parse_failed", "failed to parse fixture")
                    .with_where(format!(
                        "path '{}'",
                        options.path.as_ref().expect("path is validated").display()
                    ))
                    .with_fix("ensure each JSONL line matches fixture schema")
                    .with_detail(err),
            )
        })?;
    let analysis = analyze_fixture(&items);
    render_inspect_output(
        options.path.as_ref().expect("path is validated"),
        &analysis,
        options.format,
    )
}

pub fn fixture_validate_command(args: &[String]) -> Result<String, String> {
    let options = parse_fixture_report_options(
        args,
        "validate",
        "usage: ergo fixture validate <events.jsonl> [--format text|json]",
    )?;
    let path = options.path.as_ref().expect("path is validated");
    let items = match parse_fixture(path) {
        Ok(items) => items,
        Err(err) => {
            let issues = vec![FixtureIssueV1 {
                code: "fixture.parse_error".to_string(),
                message: err,
            }];
            let out = render_validate_output(path, false, None, &issues, options.format)?;
            return Err(out);
        }
    };

    let analysis = analyze_fixture(&items);
    let issues = validate_analysis(&analysis);
    if issues.is_empty() {
        render_validate_output(path, true, Some(&analysis), &issues, options.format)
    } else {
        let out = render_validate_output(path, false, Some(&analysis), &issues, options.format)?;
        Err(out)
    }
}

fn parse_fixture_report_options(
    args: &[String],
    command_name: &str,
    usage_text: &str,
) -> Result<FixtureReportOptions, String> {
    let mut options = FixtureReportOptions::default();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--format" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new("cli.missing_option_value", "--format requires a value")
                            .with_where("arg '--format'")
                            .with_fix("use --format text or --format json"),
                    )
                })?;
                options.format = match value.as_str() {
                    "text" => OutputFormat::Text,
                    "json" => OutputFormat::Json,
                    other => {
                        return Err(render_cli_error(
                            &CliErrorInfo::new(
                                "cli.invalid_option_value",
                                format!("unsupported --format value '{other}'"),
                            )
                            .with_where("arg '--format'")
                            .with_fix("use --format text or --format json"),
                        ))
                    }
                };
                i += 2;
            }
            other if !other.starts_with('-') && options.path.is_none() => {
                options.path = Some(PathBuf::from(other));
                i += 1;
            }
            other => {
                return Err(render_cli_error(
                    &CliErrorInfo::new(
                        "cli.invalid_option",
                        format!("unknown fixture {command_name} option '{other}'"),
                    )
                    .with_where(format!("arg '{other}'"))
                    .with_fix(usage_text),
                ))
            }
        }
    }

    if options.path.is_none() {
        return Err(render_cli_error(
            &CliErrorInfo::new(
                "cli.missing_required_option",
                format!("fixture {command_name} requires <events.jsonl>"),
            )
            .with_where(format!("fixture {command_name} command arguments"))
            .with_fix(usage_text),
        ));
    }

    Ok(options)
}

fn analyze_fixture(items: &[FixtureItem]) -> FixtureAnalysis {
    let mut event_count = 0usize;
    let mut events_with_payload = 0usize;
    let mut events_with_semantic_kind = 0usize;
    let mut event_kind_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut episode_summaries: Vec<EpisodeSummaryV1> = Vec::new();
    let mut current_episode: Option<usize> = None;

    for item in items {
        match item {
            FixtureItem::EpisodeStart { label } => {
                episode_summaries.push(EpisodeSummaryV1 {
                    label: label.clone(),
                    event_count: 0,
                });
                current_episode = Some(episode_summaries.len() - 1);
            }
            FixtureItem::Event {
                kind,
                payload,
                semantic_kind,
                ..
            } => {
                if current_episode.is_none() {
                    let label = format!("E{}", episode_summaries.len() + 1);
                    episode_summaries.push(EpisodeSummaryV1 {
                        label,
                        event_count: 0,
                    });
                    current_episode = Some(episode_summaries.len() - 1);
                }

                let episode_index = current_episode.expect("episode index is set");
                episode_summaries[episode_index].event_count += 1;
                event_count += 1;
                if payload.is_some() {
                    events_with_payload += 1;
                }
                if semantic_kind
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty())
                {
                    events_with_semantic_kind += 1;
                }
                *event_kind_counts.entry(event_kind_name(*kind)).or_default() += 1;
            }
        }
    }

    FixtureAnalysis {
        total_items: items.len(),
        event_count,
        events_with_payload,
        events_with_semantic_kind,
        event_kind_counts,
        episode_summaries,
    }
}

fn validate_analysis(analysis: &FixtureAnalysis) -> Vec<FixtureIssueV1> {
    let mut issues = Vec::new();

    if analysis.event_count == 0 {
        issues.push(FixtureIssueV1 {
            code: "fixture.no_events".to_string(),
            message: "fixture contained no events".to_string(),
        });
    }

    for episode in &analysis.episode_summaries {
        if episode.event_count == 0 {
            issues.push(FixtureIssueV1 {
                code: "fixture.episode_without_events".to_string(),
                message: format!("episode '{}' has no events", episode.label),
            });
        }
    }

    issues
}

fn render_inspect_output(
    path: &Path,
    analysis: &FixtureAnalysis,
    format: OutputFormat,
) -> Result<String, String> {
    let stats = stats_from_analysis(analysis);
    match format {
        OutputFormat::Text => Ok(render_inspect_text(path, &stats)),
        OutputFormat::Json => {
            let output = FixtureInspectOutputV1 {
                schema_version: FIXTURE_OUTPUT_SCHEMA_VERSION,
                command: "fixture.inspect",
                fixture_path: path.display().to_string(),
                stats,
            };
            serde_json::to_string_pretty(&output)
                .map(|json| format!("{json}\n"))
                .map_err(|err| format!("serialize fixture inspect output: {err}"))
        }
    }
}

fn render_validate_output(
    path: &Path,
    valid: bool,
    analysis: Option<&FixtureAnalysis>,
    issues: &[FixtureIssueV1],
    format: OutputFormat,
) -> Result<String, String> {
    let stats = analysis.map(stats_from_analysis);
    match format {
        OutputFormat::Text => Ok(render_validate_text(path, valid, stats.as_ref(), issues)),
        OutputFormat::Json => {
            let output = FixtureValidateOutputV1 {
                schema_version: FIXTURE_OUTPUT_SCHEMA_VERSION,
                command: "fixture.validate",
                fixture_path: path.display().to_string(),
                valid,
                stats,
                issues: issues.to_vec(),
            };
            serde_json::to_string_pretty(&output)
                .map(|json| format!("{json}\n"))
                .map_err(|err| format!("serialize fixture validate output: {err}"))
        }
    }
}

fn render_inspect_text(path: &Path, stats: &FixtureStatsV1) -> String {
    let mut lines = vec![
        "fixture inspect".to_string(),
        format!("path: {}", path.display()),
        format!("total_items: {}", stats.total_items),
        format!("episode_count: {}", stats.episode_count),
        format!("event_count: {}", stats.event_count),
        format!("events_with_payload: {}", stats.events_with_payload),
        format!(
            "events_with_semantic_kind: {}",
            stats.events_with_semantic_kind
        ),
    ];

    lines.push("event_kind_counts:".to_string());
    if stats.event_kind_counts.is_empty() {
        lines.push("  - (none)".to_string());
    } else {
        for (kind, count) in &stats.event_kind_counts {
            lines.push(format!("  - {kind}: {count}"));
        }
    }

    lines.push("episodes:".to_string());
    if stats.episodes.is_empty() {
        lines.push("  - (none)".to_string());
    } else {
        for episode in &stats.episodes {
            lines.push(format!("  - {}: {}", episode.label, episode.event_count));
        }
    }

    lines.push(String::new());
    lines.join("\n")
}

fn render_validate_text(
    path: &Path,
    valid: bool,
    stats: Option<&FixtureStatsV1>,
    issues: &[FixtureIssueV1],
) -> String {
    let mut lines = vec![
        if valid {
            "fixture valid".to_string()
        } else {
            "fixture invalid".to_string()
        },
        format!("path: {}", path.display()),
    ];

    if let Some(stats) = stats {
        lines.push(format!("episode_count: {}", stats.episode_count));
        lines.push(format!("event_count: {}", stats.event_count));
    }

    if issues.is_empty() {
        lines.push("issues: (none)".to_string());
    } else {
        lines.push("issues:".to_string());
        for issue in issues {
            lines.push(format!("  - [{}] {}", issue.code, issue.message));
        }
    }

    lines.push(String::new());
    lines.join("\n")
}

fn stats_from_analysis(analysis: &FixtureAnalysis) -> FixtureStatsV1 {
    FixtureStatsV1 {
        total_items: analysis.total_items,
        episode_count: analysis.episode_summaries.len(),
        event_count: analysis.event_count,
        events_with_payload: analysis.events_with_payload,
        events_with_semantic_kind: analysis.events_with_semantic_kind,
        event_kind_counts: analysis.event_kind_counts.clone(),
        episodes: analysis.episode_summaries.clone(),
    }
}

fn event_kind_name(kind: ExternalEventKind) -> String {
    match kind {
        ExternalEventKind::Pump => "Pump".to_string(),
        ExternalEventKind::DataAvailable => "DataAvailable".to_string(),
        ExternalEventKind::Command => "Command".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn write_fixture(contents: &str) -> PathBuf {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "ergo-cli-fixture-ops-test-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("fixture.jsonl");
        fs::write(&path, contents).expect("write fixture");
        path
    }

    #[test]
    fn inspect_text_reports_counts() {
        let path = write_fixture(
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
             {\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n\
             {\"kind\":\"event\",\"event\":{\"type\":\"Pump\",\"payload\":{\"price\":1.23},\"semantic_kind\":\"market.tick\"}}\n",
        );
        let out = fixture_inspect_command(&[path.to_string_lossy().to_string()])
            .expect("inspect should succeed");
        assert!(out.contains("fixture inspect"), "out: {out}");
        assert!(out.contains("episode_count: 1"), "out: {out}");
        assert!(out.contains("event_count: 2"), "out: {out}");
        assert!(out.contains("events_with_payload: 1"), "out: {out}");
        assert!(out.contains("events_with_semantic_kind: 1"), "out: {out}");
        assert!(out.contains("Command: 1"), "out: {out}");
        assert!(out.contains("Pump: 1"), "out: {out}");
    }

    #[test]
    fn inspect_json_uses_v1_schema() {
        let path = write_fixture("{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n");
        let out = fixture_inspect_command(&[
            path.to_string_lossy().to_string(),
            "--format".to_string(),
            "json".to_string(),
        ])
        .expect("inspect json should succeed");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        assert_eq!(parsed["schema_version"], "v1");
        assert_eq!(parsed["command"], "fixture.inspect");
        assert_eq!(parsed["stats"]["event_count"], 1);
    }

    #[test]
    fn validate_json_reports_invalid_episode() {
        let path = write_fixture("{\"kind\":\"episode_start\",\"id\":\"E1\"}\n");
        let err = fixture_validate_command(&[
            path.to_string_lossy().to_string(),
            "--format".to_string(),
            "json".to_string(),
        ])
        .expect_err("validate should fail");
        let parsed: serde_json::Value = serde_json::from_str(&err).expect("valid json");
        assert_eq!(parsed["schema_version"], "v1");
        assert_eq!(parsed["command"], "fixture.validate");
        assert_eq!(parsed["valid"], false);
        assert_eq!(parsed["issues"][0]["code"], "fixture.no_events");
    }

    #[test]
    fn validate_text_succeeds_for_event_only_fixture() {
        let path = write_fixture("{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n");
        let out = fixture_validate_command(&[path.to_string_lossy().to_string()])
            .expect("validate should pass");
        assert!(out.contains("fixture valid"), "out: {out}");
        assert!(out.contains("issues: (none)"), "out: {out}");
    }

    #[test]
    fn inspect_requires_fixture_path() {
        let err = fixture_inspect_command(&[]).expect_err("missing path should fail");
        assert!(
            err.contains("fixture inspect requires <events.jsonl>"),
            "unexpected err: {err}"
        );
    }
}
