use ergo_host::{
    describe_adapter_required, describe_host_replay_error, AdapterDependencySummary,
    HostErrorDescriptor, HostReplayError, HostRunError,
};

use crate::error_format::{render_cli_error, CliErrorInfo};
use crate::exit_codes::FAILURE;

pub fn write_stderr(message: &str) {
    eprintln!("{message}");
}

pub fn failure_code() -> i32 {
    FAILURE
}

pub fn unknown_help_topic(topic: &str) -> String {
    render_cli_error(
        &CliErrorInfo::new(
            "cli.unknown_help_topic",
            format!("unknown help topic '{topic}'"),
        )
        .with_where("help topic")
        .with_fix("run 'ergo help' to list available topics"),
    )
}

pub fn invalid_fixture_subcommand(target: &str, usage: &str) -> String {
    render_cli_error(
        &CliErrorInfo::new(
            "cli.invalid_subcommand",
            format!("unknown fixture subcommand '{target}'"),
        )
        .with_where("fixture subcommand")
        .with_fix(usage),
    )
}

pub fn removed_run_fixture() -> String {
    render_cli_error(
        &CliErrorInfo::new(
            "cli.command_removed",
            "'ergo run fixture' was removed in v1",
        )
        .with_where("command 'run fixture'")
        .with_fix("use 'ergo fixture run <events.jsonl>'"),
    )
}

pub fn unknown_command(command: &str) -> String {
    render_cli_error(
        &CliErrorInfo::new(
            "cli.unknown_command",
            format!("unknown command '{command}'"),
        )
        .with_where("command")
        .with_fix("run 'ergo help' to see the v1 command map"),
    )
}

pub(crate) fn render_host_error_descriptor(descriptor: HostErrorDescriptor) -> String {
    let mut info = CliErrorInfo::new(descriptor.code, descriptor.message);
    if let Some(rule_id) = descriptor.rule_id {
        info = info.with_rule_id(rule_id);
    }
    if let Some(where_field) = descriptor.where_field {
        info = info.with_where(where_field);
    }
    if let Some(fix) = descriptor.fix {
        info = info.with_fix(fix);
    }
    for detail in descriptor.details {
        info = info.with_detail(detail);
    }
    render_cli_error(&info)
}

fn extract_single_quoted(input: &str) -> Option<&str> {
    let mut parts = input.split('\'');
    parts.next()?;
    parts.next()
}

pub fn render_adapter_required(summary: &AdapterDependencySummary) -> String {
    render_host_error_descriptor(describe_adapter_required(summary))
}

pub fn render_host_run_error(err: HostRunError) -> String {
    match err {
        HostRunError::MissingIngressSource => render_cli_error(
            &CliErrorInfo::new(
                "cli.missing_required_option",
                "canonical run requires --fixture <events.jsonl>",
            )
            .with_rule_id("RUN-CANON-1")
            .with_where("canonical host ingress")
            .with_fix("provide --fixture <events.jsonl>"),
        ),
        HostRunError::AdapterRequired(summary) => render_adapter_required(&summary),
        HostRunError::InvalidInput(message) => {
            if message.contains("missing semantic_kind in adapter-bound canonical run") {
                let where_field = extract_single_quoted(&message)
                    .map(|id| format!("fixture event '{}'", id))
                    .unwrap_or_else(|| "fixture event".to_string());
                return render_cli_error(
                    &CliErrorInfo::new("fixture.semantic_kind_missing", message)
                        .with_where(where_field)
                        .with_fix(
                            "add semantic_kind to each fixture event when running with --adapter",
                        ),
                );
            }
            if message.contains("set semantic_kind but canonical run is not adapter-bound") {
                let where_field = extract_single_quoted(&message)
                    .map(|id| format!("fixture event '{}'", id))
                    .unwrap_or_else(|| "fixture event".to_string());
                return render_cli_error(
                    &CliErrorInfo::new("fixture.unexpected_semantic_kind", message)
                        .with_where(where_field)
                        .with_fix("remove semantic_kind or run with --adapter <adapter.yaml>"),
                );
            }
            if message.contains("appears more than once in canonical run input") {
                let where_field = extract_single_quoted(&message)
                    .map(|id| format!("fixture event '{}'", id))
                    .unwrap_or_else(|| "fixture event".to_string());
                return render_cli_error(
                    &CliErrorInfo::new("fixture.duplicate_event_id", message)
                        .with_where(where_field)
                        .with_fix(
                            "make fixture event ids unique, or omit ids to auto-generate unique fixture_evt_* ids",
                        ),
                );
            }
            message
        }
        HostRunError::StepFailed(message) => render_cli_error(
            &CliErrorInfo::new("host.step_failed", "canonical host step failed")
                .with_where("canonical host execution")
                .with_fix("inspect fixture payload/schema and host effect handlers")
                .with_detail(message),
        ),
        HostRunError::Io(message) => render_cli_error(
            &CliErrorInfo::new("capture.write_failed", "failed to write capture artifact")
                .with_where("capture output")
                .with_fix("verify capture output path and permissions")
                .with_detail(message),
        ),
    }
}

pub fn render_host_replay_error(err: &HostReplayError) -> String {
    render_host_error_descriptor(describe_host_replay_error(err))
}
