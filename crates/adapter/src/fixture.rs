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
    PathBuf::from("target").join(format!("{stem}-replay.json"))
}
