//! fixture_ops
//!
//! Purpose:
//! - Implement the CLI fixture inspect/validate command handlers and their
//!   output-format routing.
//!
//! Owns:
//! - CLI argument validation and CLI error rendering for fixture report
//!   commands.
//!
//! Does not own:
//! - Fixture parsing or report/stat derivation semantics, which belong to
//!   `ergo_fixtures`.
//!
//! Connects to:
//! - `ergo_fixtures::report` for typed inspection/validation helpers.
//! - CLI error formatting/output helpers.
//!
//! Safety notes:
//! - This is an output boundary, so typed fixture parse errors are rendered to
//!   stable CLI strings here instead of being flattened upstream.

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
                .with_fix("ensure each JSONL line matches fixture schema")
                .with_detail(err.to_string()),
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
mod tests;
