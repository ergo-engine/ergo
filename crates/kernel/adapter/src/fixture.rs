//! fixture
//!
//! Purpose:
//! - Parse JSONL fixture files into the kernel-owned fixture item sequence used
//!   by host fixture ingress, fixture reporting, and stress tests.
//!
//! Owns:
//! - The typed fixture parse boundary over file open/read, per-line decode, and
//!   local payload-shape validation.
//! - Default fixture capture artifact naming.
//!
//! Does not own:
//! - Host canonical-run validation rules such as duplicate ids or adapter-bound
//!   semantic-kind requirements.
//! - CLI/report rendering of parse failures.
//!
//! Connects to:
//! - `ergo_host` fixture/process-driver ingress.
//! - `crates/shared/fixtures`, which reports and validates fixture structure.
//!
//! Safety notes:
//! - Payload validation here is intentionally local: fixture payloads must be
//!   JSON objects before they ever reach host event construction.

use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::ExternalEventKind;

#[derive(Debug, Clone)]
pub enum FixtureItem {
    EpisodeStart {
        label: String,
    },
    Event {
        id: Option<String>,
        kind: ExternalEventKind,
        payload: Option<serde_json::Value>,
        semantic_kind: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum FixtureRecord {
    EpisodeStart { id: Option<String> },
    Event { event: FixtureEvent },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureEvent {
    #[serde(rename = "type")]
    kind: ExternalEventKind,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    payload: Option<serde_json::Value>,
    #[serde(default)]
    semantic_kind: Option<String>,
}

#[derive(Debug)]
pub enum FixtureParseError {
    Open {
        path: PathBuf,
        source: io::Error,
    },
    ReadLine {
        path: PathBuf,
        line: usize,
        source: io::Error,
    },
    ParseLine {
        path: PathBuf,
        line: usize,
        source: serde_json::Error,
    },
    PayloadMustBeObject {
        path: PathBuf,
        line: usize,
        got: String,
    },
}

impl std::fmt::Display for FixtureParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open { path, source } => {
                write!(f, "read fixture '{}': {source}", path.display())
            }
            Self::ReadLine { path, line, source } => {
                write!(
                    f,
                    "read fixture line {line} in '{}': {source}",
                    path.display()
                )
            }
            Self::ParseLine { path, line, source } => {
                write!(
                    f,
                    "fixture parse error at line {line} in '{}': {source}",
                    path.display()
                )
            }
            Self::PayloadMustBeObject { path, line, got } => write!(
                f,
                "fixture parse error at line {line} in '{}': payload must be a JSON object, got {got}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for FixtureParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Open { source, .. } => Some(source),
            Self::ReadLine { source, .. } => Some(source),
            Self::ParseLine { source, .. } => Some(source),
            Self::PayloadMustBeObject { .. } => None,
        }
    }
}

pub fn parse_fixture(path: &Path) -> Result<Vec<FixtureItem>, FixtureParseError> {
    let file = fs::File::open(path).map_err(|source| FixtureParseError::Open {
        path: path.to_path_buf(),
        source,
    })?;
    let reader = io::BufReader::new(file);
    let mut items = Vec::new();
    let mut episode_counter = 0usize;

    for (index, line) in reader.lines().enumerate() {
        let line_number = index + 1;
        let line = line.map_err(|source| FixtureParseError::ReadLine {
            path: path.to_path_buf(),
            line: line_number,
            source,
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let record: FixtureRecord =
            serde_json::from_str(trimmed).map_err(|source| FixtureParseError::ParseLine {
                path: path.to_path_buf(),
                line: line_number,
                source,
            })?;

        match record {
            FixtureRecord::EpisodeStart { id } => {
                episode_counter += 1;
                let label = id.unwrap_or_else(|| format!("E{}", episode_counter));
                items.push(FixtureItem::EpisodeStart { label });
            }
            FixtureRecord::Event { event } => {
                if let Some(payload) = event.payload.as_ref() {
                    if !payload.is_object() {
                        return Err(FixtureParseError::PayloadMustBeObject {
                            path: path.to_path_buf(),
                            line: line_number,
                            got: json_type_name(payload).to_string(),
                        });
                    }
                }

                items.push(FixtureItem::Event {
                    id: event.id,
                    kind: event.kind,
                    payload: event.payload,
                    semantic_kind: event.semantic_kind,
                });
            }
        }
    }

    Ok(items)
}

pub fn fixture_output_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "fixture".into());
    PathBuf::from("target").join(format!("{stem}-capture.json"))
}

use crate::common::json_type_name;

#[cfg(test)]
mod tests {
    use super::fixture_output_path;
    use std::path::{Path, PathBuf};

    #[test]
    fn fixture_output_path_uses_capture_suffix() {
        assert_eq!(
            fixture_output_path(Path::new("fixtures/demo_1.jsonl")),
            PathBuf::from("target/demo_1-capture.json")
        );
    }

    #[test]
    fn rejects_unknown_fields_in_fixture_line() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("ergo-fixture-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"kind":"episode_start","id":"E1"}}"#).unwrap();
        writeln!(
            f,
            r#"{{"kind":"event","event":{{"type":"Command"}},"context":{{"x":2.5}}}}"#
        )
        .unwrap();
        drop(f);
        let result = super::parse_fixture(&path);
        assert!(result.is_err(), "should reject unknown field 'context'");
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("context"),
            "error should mention the unknown field: {err}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fixture_output_path_falls_back_to_fixture_when_stem_missing() {
        assert_eq!(
            fixture_output_path(Path::new("")),
            PathBuf::from("target/fixture-capture.json")
        );
    }
}
