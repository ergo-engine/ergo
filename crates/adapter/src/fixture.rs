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

pub fn parse_fixture(path: &Path) -> Result<Vec<FixtureItem>, String> {
    let file = fs::File::open(path).map_err(|err| format!("read fixture: {err}"))?;
    let reader = io::BufReader::new(file);
    let mut items = Vec::new();
    let mut episode_counter = 0usize;

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
                episode_counter += 1;
                let label = id.unwrap_or_else(|| format!("E{}", episode_counter));
                items.push(FixtureItem::EpisodeStart { label });
            }
            FixtureRecord::Event { event } => {
                if let Some(payload) = event.payload.as_ref() {
                    if !payload.is_object() {
                        return Err(format!(
                            "fixture parse error at line {}: payload must be a JSON object, got {}",
                            index + 1,
                            json_type_name(payload)
                        ));
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

fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

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
            err.contains("context"),
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
