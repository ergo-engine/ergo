use std::path::PathBuf;

use ergo_fixtures::csv::{convert_csv_to_fixture, parse_event_kind, CsvToFixtureOptions};

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

    let mut options =
        CsvToFixtureOptions::with_defaults(PathBuf::from(&args[0]), PathBuf::from(&args[1]));

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

#[cfg(test)]
mod tests {
    use super::*;
    use ergo_adapter::fixture::{parse_fixture, FixtureItem};
    use std::fs;
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
            event_kind: ergo_adapter::ExternalEventKind::DataAvailable,
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
            event_kind: ergo_adapter::ExternalEventKind::DataAvailable,
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
