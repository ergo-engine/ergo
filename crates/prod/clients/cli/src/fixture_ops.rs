use std::path::PathBuf;

use ergo_fixtures::report::{
    inspect_fixture, render_inspect_json, render_inspect_text, render_validate_json,
    render_validate_text, stats_from_analysis, validate_fixture,
};

use crate::error_format::{render_cli_error, CliErrorInfo};

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
    let path = options.path.as_ref().expect("path is validated");

    let analysis = inspect_fixture(path).map_err(|err| {
        render_cli_error(
            &CliErrorInfo::new("fixture.parse_failed", "failed to parse fixture")
                .with_where(format!("path '{}'", path.display()))
                .with_fix("ensure each JSONL line matches fixture schema")
                .with_detail(err),
        )
    })?;
    let stats = stats_from_analysis(&analysis);

    match options.format {
        OutputFormat::Text => Ok(render_inspect_text(path, &stats)),
        OutputFormat::Json => render_inspect_json(path, stats),
    }
}

pub fn fixture_validate_command(args: &[String]) -> Result<String, String> {
    let options = parse_fixture_report_options(
        args,
        "validate",
        "usage: ergo fixture validate <events.jsonl> [--format text|json]",
    )?;
    let path = options.path.as_ref().expect("path is validated");

    let report = validate_fixture(path);
    let output = match options.format {
        OutputFormat::Text => Ok(render_validate_text(
            path,
            report.valid,
            report.stats.as_ref(),
            &report.issues,
        )),
        OutputFormat::Json => render_validate_json(path, report.valid, report.stats, report.issues),
    }?;

    if report.valid {
        Ok(output)
    } else {
        Err(output)
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
