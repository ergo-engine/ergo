use std::collections::HashMap;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use ergo_adapter::ExternalEventKind;
use serde::Serialize;

#[derive(Debug, Clone)]
struct CsvToFixtureOptions {
    input: PathBuf,
    output: PathBuf,
    semantic_kind: String,
    event_kind: ExternalEventKind,
    episode_id: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum FixtureLine {
    EpisodeStart { id: String },
    Event { event: FixtureEvent },
}

#[derive(Debug, Serialize)]
struct FixtureEvent {
    #[serde(rename = "type")]
    kind: ExternalEventKind,
    id: String,
    semantic_kind: String,
    payload: serde_json::Value,
}

pub fn csv_to_fixture_command(args: &[String]) -> Result<String, String> {
    let options = parse_csv_to_fixture_options(args)?;
    let written = convert_csv_to_fixture(&options)?;
    Ok(format!(
        "wrote {} event(s) to {}\n",
        written,
        options.output.display()
    ))
}

fn parse_csv_to_fixture_options(args: &[String]) -> Result<CsvToFixtureOptions, String> {
    if args.len() < 2 {
        return Err(
            "csv-to-fixture requires <prices.csv> <events.jsonl>; optional flags: --semantic-kind <name>, --event-kind <Pump|DataAvailable|Command>, --episode-id <id>".to_string(),
        );
    }

    let mut options = CsvToFixtureOptions {
        input: PathBuf::from(&args[0]),
        output: PathBuf::from(&args[1]),
        semantic_kind: "price_bar".to_string(),
        event_kind: ExternalEventKind::DataAvailable,
        episode_id: "E1".to_string(),
    };

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--semantic-kind" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--semantic-kind requires a value".to_string())?;
                options.semantic_kind = value.clone();
                i += 2;
            }
            "--event-kind" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--event-kind requires a value".to_string())?;
                options.event_kind = parse_event_kind(value)?;
                i += 2;
            }
            "--episode-id" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--episode-id requires a value".to_string())?;
                options.episode_id = value.clone();
                i += 2;
            }
            other => {
                return Err(format!(
                    "unknown csv-to-fixture option '{other}'. expected --semantic-kind, --event-kind, or --episode-id"
                ))
            }
        }
    }

    if options.semantic_kind.trim().is_empty() {
        return Err("--semantic-kind cannot be empty".to_string());
    }
    if options.episode_id.trim().is_empty() {
        return Err("--episode-id cannot be empty".to_string());
    }

    Ok(options)
}

fn parse_event_kind(raw: &str) -> Result<ExternalEventKind, String> {
    let normalized = raw
        .chars()
        .filter(|ch| *ch != '_' && *ch != '-')
        .collect::<String>()
        .to_ascii_lowercase();

    match normalized.as_str() {
        "pump" => Ok(ExternalEventKind::Pump),
        "dataavailable" => Ok(ExternalEventKind::DataAvailable),
        "command" => Ok(ExternalEventKind::Command),
        _ => Err(format!(
            "invalid --event-kind '{}'; expected Pump, DataAvailable, or Command",
            raw
        )),
    }
}

fn convert_csv_to_fixture(options: &CsvToFixtureOptions) -> Result<usize, String> {
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(&options.input)
        .map_err(|err| format!("read csv '{}': {err}", options.input.display()))?;

    let headers = reader
        .headers()
        .map_err(|err| format!("read csv headers '{}': {err}", options.input.display()))?
        .iter()
        .enumerate()
        .map(|(idx, field)| (field.trim().to_ascii_lowercase(), idx))
        .collect::<HashMap<_, _>>();

    let timestamp_col = find_required_column(
        &headers,
        &["timestamp", "time", "datetime", "date"],
        "timestamp",
    )?;
    let close_col = find_required_column(&headers, &["close", "c"], "close")?;
    let open_col = find_optional_column(&headers, &["open", "o"]);
    let high_col = find_optional_column(&headers, &["high", "h"]);
    let low_col = find_optional_column(&headers, &["low", "l"]);
    let volume_col = find_optional_column(&headers, &["volume", "vol", "v"]);
    let symbol_col = find_optional_column(&headers, &["symbol", "ticker", "instrument"]);

    if let Some(parent) = options.output.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create fixture output directory '{}': {err}",
                parent.display()
            )
        })?;
    }

    let output_file = fs::File::create(&options.output)
        .map_err(|err| format!("create fixture '{}': {err}", options.output.display()))?;
    let mut writer = BufWriter::new(output_file);

    let episode = FixtureLine::EpisodeStart {
        id: options.episode_id.clone(),
    };
    write_fixture_line(&mut writer, &episode)?;

    let mut count = 0usize;
    for (row_index, row) in reader.records().enumerate() {
        let row_number = row_index + 2;
        let row = row.map_err(|err| {
            format!(
                "read csv row {} in '{}': {err}",
                row_number,
                options.input.display()
            )
        })?;

        let timestamp = read_required_string(&row, timestamp_col, "timestamp", row_number)?;
        let close = read_required_number(&row, close_col, "close", row_number)?;

        let mut payload = serde_json::Map::new();
        payload.insert(
            "timestamp".to_string(),
            serde_json::Value::String(timestamp),
        );
        payload.insert("close".to_string(), serde_json::Value::from(close));
        payload.insert("x".to_string(), serde_json::Value::from(close));

        if let Some(value) = read_optional_number(&row, open_col, "open", row_number)? {
            payload.insert("open".to_string(), serde_json::Value::from(value));
        }
        if let Some(value) = read_optional_number(&row, high_col, "high", row_number)? {
            payload.insert("high".to_string(), serde_json::Value::from(value));
        }
        if let Some(value) = read_optional_number(&row, low_col, "low", row_number)? {
            payload.insert("low".to_string(), serde_json::Value::from(value));
        }
        if let Some(value) = read_optional_number(&row, volume_col, "volume", row_number)? {
            payload.insert("volume".to_string(), serde_json::Value::from(value));
        }
        if let Some(value) = read_optional_string(&row, symbol_col) {
            payload.insert("symbol".to_string(), serde_json::Value::String(value));
        }

        let event = FixtureLine::Event {
            event: FixtureEvent {
                kind: options.event_kind,
                id: format!("csv_evt_{}", count + 1),
                semantic_kind: options.semantic_kind.clone(),
                payload: serde_json::Value::Object(payload),
            },
        };
        write_fixture_line(&mut writer, &event)?;
        count += 1;
    }

    writer
        .flush()
        .map_err(|err| format!("flush fixture '{}': {err}", options.output.display()))?;

    if count == 0 {
        return Err(format!(
            "csv '{}' has no data rows (header-only input)",
            options.input.display()
        ));
    }

    Ok(count)
}

fn write_fixture_line(writer: &mut BufWriter<fs::File>, line: &FixtureLine) -> Result<(), String> {
    let encoded = serde_json::to_string(line)
        .map_err(|err| format!("encode fixture json line failed: {err}"))?;
    writer
        .write_all(encoded.as_bytes())
        .and_then(|_| writer.write_all(b"\n"))
        .map_err(|err| format!("write fixture line failed: {err}"))
}

fn find_required_column(
    headers: &HashMap<String, usize>,
    aliases: &[&str],
    label: &str,
) -> Result<usize, String> {
    find_optional_column(headers, aliases)
        .ok_or_else(|| format!("missing required CSV column for '{label}'"))
}

fn find_optional_column(headers: &HashMap<String, usize>, aliases: &[&str]) -> Option<usize> {
    aliases
        .iter()
        .find_map(|name| headers.get(&name.to_ascii_lowercase()).copied())
}

fn read_required_string(
    row: &csv::StringRecord,
    index: usize,
    field: &str,
    row_number: usize,
) -> Result<String, String> {
    let value = row.get(index).unwrap_or("").trim();
    if value.is_empty() {
        return Err(format!(
            "csv row {} has empty required field '{}'",
            row_number, field
        ));
    }
    Ok(value.to_string())
}

fn read_optional_string(row: &csv::StringRecord, index: Option<usize>) -> Option<String> {
    let index = index?;
    let value = row.get(index)?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn read_required_number(
    row: &csv::StringRecord,
    index: usize,
    field: &str,
    row_number: usize,
) -> Result<f64, String> {
    let raw = row.get(index).unwrap_or("").trim();
    if raw.is_empty() {
        return Err(format!(
            "csv row {} has empty required numeric field '{}'",
            row_number, field
        ));
    }
    raw.parse::<f64>().map_err(|err| {
        format!(
            "csv row {} field '{}' must be numeric, got '{}': {err}",
            row_number, field, raw
        )
    })
}

fn read_optional_number(
    row: &csv::StringRecord,
    index: Option<usize>,
    field: &str,
    row_number: usize,
) -> Result<Option<f64>, String> {
    let Some(index) = index else {
        return Ok(None);
    };
    let raw = row.get(index).unwrap_or("").trim();
    if raw.is_empty() {
        return Ok(None);
    }
    raw.parse::<f64>().map(Some).map_err(|err| {
        format!(
            "csv row {} field '{}' must be numeric when present, got '{}': {err}",
            row_number, field, raw
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ergo_adapter::fixture::{parse_fixture, FixtureItem};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_path(name: &str) -> PathBuf {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "ergo-cli-csv-fixture-test-{}-{}-{}",
            std::process::id(),
            index,
            name
        ))
    }

    #[test]
    fn converts_price_csv_into_fixture_events() {
        let csv_path = temp_path("prices.csv");
        let fixture_path = temp_path("prices.jsonl");

        let csv = "\
timestamp,open,high,low,close,volume,symbol\n\
2026-01-01T09:30:00Z,100.0,101.5,99.9,101.2,10000,AAPL\n\
2026-01-01T09:31:00Z,101.2,101.9,100.8,100.9,9000,AAPL\n";
        fs::write(&csv_path, csv).expect("write csv");

        let options = CsvToFixtureOptions {
            input: csv_path.clone(),
            output: fixture_path.clone(),
            semantic_kind: "price_bar".to_string(),
            event_kind: ExternalEventKind::DataAvailable,
            episode_id: "E1".to_string(),
        };

        let written = convert_csv_to_fixture(&options).expect("convert csv");
        assert_eq!(written, 2);

        let fixture = parse_fixture(&fixture_path).expect("parse fixture");
        assert!(matches!(
            fixture.first(),
            Some(FixtureItem::EpisodeStart { .. })
        ));

        let event = fixture
            .iter()
            .find_map(|item| match item {
                FixtureItem::Event {
                    payload: Some(payload),
                    ..
                } => Some(payload),
                _ => None,
            })
            .expect("first event payload");
        let close = event
            .get("close")
            .and_then(serde_json::Value::as_f64)
            .expect("close present");
        let x = event
            .get("x")
            .and_then(serde_json::Value::as_f64)
            .expect("x present");
        assert_eq!(close, x);

        let _ = fs::remove_file(csv_path);
        let _ = fs::remove_file(fixture_path);
    }

    #[test]
    fn rejects_csv_missing_close_column() {
        let csv_path = temp_path("missing_close.csv");
        let fixture_path = temp_path("missing_close.jsonl");

        let csv = "\
timestamp,open,high,low,volume\n\
2026-01-01T09:30:00Z,100.0,101.5,99.9,10000\n";
        fs::write(&csv_path, csv).expect("write csv");

        let options = CsvToFixtureOptions {
            input: csv_path.clone(),
            output: fixture_path,
            semantic_kind: "price_bar".to_string(),
            event_kind: ExternalEventKind::DataAvailable,
            episode_id: "E1".to_string(),
        };

        let err = convert_csv_to_fixture(&options).expect_err("missing close must fail");
        assert!(
            err.contains("missing required CSV column for 'close'"),
            "unexpected err: {err}"
        );

        let _ = fs::remove_file(csv_path);
    }
}
