//! report
//!
//! Purpose:
//! - Inspect parsed fixture streams and render stable fixture inspection and
//!   validation report DTOs.
//!
//! Owns:
//! - Fixture stats/issue DTOs and the reporting helpers that derive them from
//!   parsed fixture items.
//!
//! Does not own:
//! - Fixture parsing grammar or low-level parse error semantics, which belong
//!   to `ergo_adapter::fixture`.
//!
//! Connects to:
//! - `ergo_adapter::fixture::parse_fixture(...)` for typed parsing.
//! - CLI/reporting surfaces that render validation output.
//!
//! Safety notes:
//! - `inspect_fixture(...)` preserves typed parse failures.
//! - `validate_fixture(...)` is an output/reporting seam and intentionally
//!   renders parse failures into user-facing issue strings.

use std::collections::BTreeMap;
use std::path::Path;

use ergo_adapter::fixture::{parse_fixture, FixtureItem, FixtureParseError};
use ergo_adapter::ExternalEventKind;
use serde::Serialize;

pub const FIXTURE_OUTPUT_SCHEMA_VERSION: &str = "v1";

#[derive(Debug, Clone)]
pub struct FixtureAnalysis {
    pub total_items: usize,
    pub event_count: usize,
    pub events_with_payload: usize,
    pub events_with_semantic_kind: usize,
    pub event_kind_counts: BTreeMap<String, usize>,
    pub episode_summaries: Vec<EpisodeSummaryV1>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EpisodeSummaryV1 {
    pub label: String,
    pub event_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixtureStatsV1 {
    pub total_items: usize,
    pub episode_count: usize,
    pub event_count: usize,
    pub events_with_payload: usize,
    pub events_with_semantic_kind: usize,
    pub event_kind_counts: BTreeMap<String, usize>,
    pub episodes: Vec<EpisodeSummaryV1>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixtureIssueV1 {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixtureInspectOutputV1 {
    pub schema_version: &'static str,
    pub command: &'static str,
    pub fixture_path: String,
    pub stats: FixtureStatsV1,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixtureValidateOutputV1 {
    pub schema_version: &'static str,
    pub command: &'static str,
    pub fixture_path: String,
    pub valid: bool,
    pub stats: Option<FixtureStatsV1>,
    pub issues: Vec<FixtureIssueV1>,
}

#[derive(Debug, Clone)]
pub struct FixtureValidationReport {
    pub valid: bool,
    pub stats: Option<FixtureStatsV1>,
    pub issues: Vec<FixtureIssueV1>,
}

pub fn inspect_fixture(path: &Path) -> Result<FixtureAnalysis, FixtureParseError> {
    let items = parse_fixture(path)?;
    Ok(analyze_fixture(&items))
}

pub fn validate_fixture(path: &Path) -> FixtureValidationReport {
    match parse_fixture(path) {
        Ok(items) => {
            let analysis = analyze_fixture(&items);
            let issues = validate_analysis(&analysis);
            let valid = issues.is_empty();
            FixtureValidationReport {
                valid,
                stats: Some(stats_from_analysis(&analysis)),
                issues,
            }
        }
        Err(err) => FixtureValidationReport {
            valid: false,
            stats: None,
            issues: vec![FixtureIssueV1 {
                code: "fixture.parse_error".to_string(),
                message: fixture_parse_issue_message(&err),
            }],
        },
    }
}

fn fixture_parse_issue_message(err: &FixtureParseError) -> String {
    match err {
        FixtureParseError::Open { source, .. } => format!("read fixture: {source}"),
        FixtureParseError::ReadLine { line, source, .. } => {
            format!("read fixture line {line}: {source}")
        }
        FixtureParseError::ParseLine { line, source, .. } => {
            format!("fixture parse error at line {line}: {source}")
        }
        FixtureParseError::PayloadMustBeObject { line, got, .. } => {
            format!("fixture parse error at line {line}: payload must be a JSON object, got {got}")
        }
    }
}

pub fn analyze_fixture(items: &[FixtureItem]) -> FixtureAnalysis {
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

pub fn validate_analysis(analysis: &FixtureAnalysis) -> Vec<FixtureIssueV1> {
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

pub fn stats_from_analysis(analysis: &FixtureAnalysis) -> FixtureStatsV1 {
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

pub fn render_inspect_text(path: &Path, stats: &FixtureStatsV1) -> String {
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

pub fn render_validate_text(
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

pub fn render_inspect_json(path: &Path, stats: FixtureStatsV1) -> Result<String, String> {
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

pub fn render_validate_json(
    path: &Path,
    valid: bool,
    stats: Option<FixtureStatsV1>,
    issues: Vec<FixtureIssueV1>,
) -> Result<String, String> {
    let output = FixtureValidateOutputV1 {
        schema_version: FIXTURE_OUTPUT_SCHEMA_VERSION,
        command: "fixture.validate",
        fixture_path: path.display().to_string(),
        valid,
        stats,
        issues,
    };
    serde_json::to_string_pretty(&output)
        .map(|json| format!("{json}\n"))
        .map_err(|err| format!("serialize fixture validate output: {err}"))
}

fn event_kind_name(kind: ExternalEventKind) -> String {
    match kind {
        ExternalEventKind::Pump => "Pump".to_string(),
        ExternalEventKind::DataAvailable => "DataAvailable".to_string(),
        ExternalEventKind::Command => "Command".to_string(),
    }
}

#[cfg(test)]
mod tests;
