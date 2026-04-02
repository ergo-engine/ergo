//! output::errors
//!
//! Purpose:
//! - Render CLI-facing text diagnostics for host- and command-owned failures.
//!
//! Owns:
//! - The stable CLI error-code/message/fix mapping for command failures that
//!   already crossed into the CLI boundary.
//!
//! Does not own:
//! - Host error semantics or replay descriptor shaping; `ergo_host` owns those.
//! - Generic error rendering primitives; `error_format.rs` owns those helpers.
//!
//! Connects to:
//! - CLI command handlers, which route host and command failures through these
//!   renderers before writing stderr.
//!
//! Safety notes:
//! - This file is the final product-facing text boundary for CLI failures, so
//!   codes and fix guidance are downstream-significant even when the underlying
//!   host errors are fully typed.

use ergo_host::{
    describe_adapter_required, describe_host_replay_error, AdapterDependencySummary,
    HostDriverError, HostDriverInputError, HostDriverOutputError, HostErrorDescriptor,
    HostReplayError, HostRunError, HostSetupError,
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

pub fn render_adapter_required(summary: &AdapterDependencySummary) -> String {
    render_host_error_descriptor(describe_adapter_required(summary))
}

pub fn render_host_run_error(err: HostRunError) -> String {
    match err {
        HostRunError::AdapterRequired(summary) => render_adapter_required(&summary),
        HostRunError::Setup(err) => {
            let (code, message, where_field, fix) = match &err {
                HostSetupError::LoadGraphAssets(_) => (
                    "graph.load_failed",
                    "failed to load graph assets",
                    "graph/cluster inputs",
                    "verify graph, cluster, and project paths",
                ),
                HostSetupError::DependencyScan(_) => (
                    "host.dependency_scan_failed",
                    "failed to scan graph adapter dependencies",
                    "expanded graph dependency scan",
                    "inspect graph/runtime registration consistency",
                ),
                HostSetupError::GraphPreparation(_) => (
                    "graph.prepare_failed",
                    "failed to prepare graph runtime",
                    "graph preparation",
                    "inspect graph expansion and runtime provenance inputs",
                ),
                HostSetupError::AdapterSetup(_) => (
                    "adapter.setup_failed",
                    "failed to prepare adapter setup",
                    "adapter setup",
                    "verify adapter manifest, composition, and binder requirements",
                ),
                HostSetupError::HostedRunnerValidation(_) => (
                    "host.configuration_invalid",
                    "host configuration validation failed",
                    "canonical host configuration",
                    "inspect adapter, egress, and graph ownership expectations",
                ),
                HostSetupError::HostedRunnerInitialization(_) => (
                    "host.runner_init_failed",
                    "failed to initialize canonical host runner",
                    "host runner setup",
                    "inspect host runner configuration and replay/run invariants",
                ),
                HostSetupError::StartEgress(_) => (
                    "egress.start_failed",
                    "failed to start egress channels",
                    "egress startup",
                    "verify egress command/path and startup protocol",
                ),
            };
            render_cli_error(
                &CliErrorInfo::new(code, message)
                    .with_where(where_field)
                    .with_fix(fix)
                    .with_detail(err.to_string()),
            )
        }
        HostRunError::Driver(err) => match err {
            HostDriverError::Input(input) => match input {
                HostDriverInputError::MissingSemanticKind { ref event_id } => render_cli_error(
                    &CliErrorInfo::new(
                        "fixture.semantic_kind_missing",
                        input.to_string(),
                    )
                    .with_where(format!("fixture event '{}'", event_id))
                    .with_fix(
                        "add semantic_kind to each fixture event when running with --adapter",
                    ),
                ),
                HostDriverInputError::UnexpectedSemanticKind { ref event_id } => render_cli_error(
                    &CliErrorInfo::new(
                        "fixture.unexpected_semantic_kind",
                        input.to_string(),
                    )
                    .with_where(format!("fixture event '{}'", event_id))
                    .with_fix("remove semantic_kind or run with --adapter <adapter.yaml>"),
                ),
                HostDriverInputError::DuplicateEventId { ref event_id } => render_cli_error(
                    &CliErrorInfo::new("fixture.duplicate_event_id", input.to_string())
                        .with_where(format!("fixture event '{}'", event_id))
                        .with_fix(
                            "make fixture event ids unique, or omit ids to auto-generate unique fixture_evt_* ids",
                        ),
                ),
                _ => render_cli_error(
                    &CliErrorInfo::new("driver.input_invalid", "driver input is invalid")
                        .with_where("canonical host ingress")
                        .with_fix("repair the fixture or driver configuration")
                        .with_detail(input.to_string()),
                ),
            },
            HostDriverError::Start(err) => render_cli_error(
                &CliErrorInfo::new(
                    "driver.start_failed",
                    "failed to start canonical run driver",
                )
                .with_where("canonical host ingress")
                .with_fix("verify the driver command/path and protocol startup")
                .with_detail(err.to_string()),
            ),
            HostDriverError::Protocol(err) => render_cli_error(
                &CliErrorInfo::new("driver.protocol_invalid", "driver protocol is invalid")
                    .with_where("canonical host ingress")
                    .with_fix("send hello first, then event lines, then end")
                    .with_detail(err.to_string()),
            ),
            HostDriverError::Io(err) => render_cli_error(
                &CliErrorInfo::new("driver.io_failed", "driver I/O failed")
                    .with_where("canonical host ingress")
                    .with_fix("inspect driver stdout/stderr and process lifecycle")
                    .with_detail(err.to_string()),
            ),
            HostDriverError::Output(output) => {
                let (code, fix) = match &output {
                    HostDriverOutputError::StopBeforeFirstCommittedEvent => (
                        "driver.output_incomplete",
                        "let the driver commit at least one event before stopping",
                    ),
                    HostDriverOutputError::ProducedNoEpisodes
                    | HostDriverOutputError::ProducedNoEvents
                    | HostDriverOutputError::EpisodeWithoutEvents { .. } => (
                        "driver.output_invalid",
                        "repair the driver or fixture so it produces canonical episode/event output",
                    ),
                    HostDriverOutputError::UnexpectedInterruptedOutcome
                    | HostDriverOutputError::MissingCapturePath => (
                        "driver.output_contract_failed",
                        "inspect host run finalization and capture policy handling",
                    ),
                };
                render_cli_error(
                    &CliErrorInfo::new(code, "driver output is invalid")
                        .with_where("canonical host ingress")
                        .with_fix(fix)
                        .with_detail(output.to_string()),
                )
            }
        },
        HostRunError::Step(message) => render_cli_error(
            &CliErrorInfo::new("host.step_failed", "canonical host step failed")
                .with_where("canonical host execution")
                .with_fix("inspect ingress payload/schema and host effect handlers")
                .with_detail(message.to_string()),
        ),
        HostRunError::CaptureWrite(message) => render_cli_error(
            &CliErrorInfo::new("capture.write_failed", "failed to write capture artifact")
                .with_where("capture output")
                .with_fix("verify capture output path and permissions")
                .with_detail(message.to_string()),
        ),
    }
}

pub fn render_host_replay_error(err: &HostReplayError) -> String {
    render_host_error_descriptor(describe_host_replay_error(err))
}
