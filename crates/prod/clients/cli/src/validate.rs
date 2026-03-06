use std::path::Path;

use ergo_host::{check_compose_paths, validate_manifest_path, HostRuleViolation, ManifestSummary};

#[derive(Debug, Clone, Copy)]
enum OutputFormat {
    Text,
    Json,
}

pub fn validate_command(args: &[String]) -> Result<String, String> {
    let mut args = args.to_vec();
    let format = take_format(&mut args)?;
    if args.len() != 1 {
        return Err(validate_usage());
    }

    let path = Path::new(&args[0]);
    match validate_manifest_path(path) {
        Ok(summary) => Ok(render_success(&summary, format)),
        Err(err) => Err(render_failure(
            &[err.into_rule_violation()],
            format,
            "Manifest invalid",
        )),
    }
}

pub fn check_compose_command(args: &[String]) -> Result<String, String> {
    let mut args = args.to_vec();
    let format = take_format(&mut args)?;
    if args.len() != 2 {
        return Err(check_compose_usage());
    }

    let adapter_path = Path::new(&args[0]);
    let other_path = Path::new(&args[1]);

    match check_compose_paths(adapter_path, other_path) {
        Ok(()) => Ok(render_compose_success(format)),
        Err(err) => Err(render_failure(
            &[err.into_rule_violation()],
            format,
            "Composition invalid",
        )),
    }
}

fn take_format(args: &mut Vec<String>) -> Result<OutputFormat, String> {
    let mut format = OutputFormat::Text;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--format" {
            if i + 1 >= args.len() {
                return Err("--format requires a value".to_string());
            }
            let value = args.remove(i + 1);
            args.remove(i);
            format = match value.as_str() {
                "json" => OutputFormat::Json,
                other => {
                    return Err(format!("unsupported format '{other}', use 'json'"));
                }
            };
            continue;
        }
        i += 1;
    }
    Ok(format)
}

fn render_success(summary: &ManifestSummary, format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => format!(
            "✓ Manifest valid\n  Kind: {}\n  ID: {}\n  Version: {}\n",
            summary.kind, summary.id, summary.version
        ),
        OutputFormat::Json => {
            let value = serde_json::json!({
                "ok": true,
                "kind": summary.kind,
                "id": summary.id,
                "version": summary.version,
            });
            format!("{}\n", serde_json::to_string_pretty(&value).unwrap())
        }
    }
}

fn render_compose_success(format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => "✓ Composition valid\n".to_string(),
        OutputFormat::Json => {
            let value = serde_json::json!({ "ok": true });
            format!("{}\n", serde_json::to_string_pretty(&value).unwrap())
        }
    }
}

fn render_failure(errors: &[HostRuleViolation], format: OutputFormat, header: &str) -> String {
    match format {
        OutputFormat::Text => render_failure_text(errors, header),
        OutputFormat::Json => render_failure_json(errors),
    }
}

fn render_failure_text(errors: &[HostRuleViolation], header: &str) -> String {
    let mut out = format!("✗ {}\n\n", header);
    for err in errors {
        out.push_str(&format!("  {}  {}\n", err.rule_id, err.summary));
        if let Some(path) = &err.path {
            out.push_str(&format!("         Path: {}\n", path));
        }
        if let Some(fix) = &err.fix {
            out.push_str(&format!("         Fix: {}\n", fix));
        }
        out.push_str(&format!("         Docs: {}\n\n", err.doc_anchor));
    }
    out.trim_end().to_string()
}

fn render_failure_json(errors: &[HostRuleViolation]) -> String {
    let rendered: Vec<serde_json::Value> = errors.iter().map(rule_violation_to_json).collect();
    let value = serde_json::json!({
        "ok": false,
        "errors": rendered,
    });
    serde_json::to_string_pretty(&value).unwrap()
}

fn rule_violation_to_json(err: &HostRuleViolation) -> serde_json::Value {
    serde_json::json!({
        "rule_id": err.rule_id,
        "phase": err.phase,
        "doc_anchor": err.doc_anchor,
        "summary": err.summary,
        "path": err.path,
        "fix": err.fix,
    })
}

fn validate_usage() -> String {
    ["usage:", "  ergo validate <manifest.yaml> [--format json]"].join("\n")
}

fn check_compose_usage() -> String {
    [
        "usage:",
        "  ergo check-compose <adapter.yaml> <source|action>.yaml [--format json]",
    ]
    .join("\n")
}
