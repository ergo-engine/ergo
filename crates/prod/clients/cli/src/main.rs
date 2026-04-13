#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::path::{Path, PathBuf};

mod cli;
mod csv_fixture;
mod error_format;
mod exit_codes;
mod fixture_ops;
mod gen_docs;
mod graph_to_dot;
mod graph_yaml;
mod init_project;
mod output;
mod render;
mod validate;

#[cfg(test)]
use ergo_adapter::EventId;
#[cfg(test)]
use ergo_supervisor::replay::ReplayError;
#[cfg(test)]
use ergo_supervisor::CaptureBundle;

#[cfg(test)]
pub(crate) use cli::args::{parse_replay_options, parse_run_artifact_options};
#[cfg(test)]
const DEMO_GRAPH_ID: &str = "demo_1";

fn main() {
    match cli::dispatch::run() {
        Ok(cli::dispatch::DispatchOutput::Text(message)) => {
            output::text::write_line(&message);
            std::process::exit(exit_codes::SUCCESS);
        }
        Ok(cli::dispatch::DispatchOutput::Json(raw_json)) => {
            output::json::write_json_str(&raw_json);
            std::process::exit(exit_codes::SUCCESS);
        }
        Err(message) => {
            output::errors::write_stderr(&message);
            std::process::exit(output::errors::failure_code());
        }
    }
}

fn usage() -> String {
    output::text::usage()
}

#[cfg(test)]
fn help_topic(topic: &str) -> Option<String> {
    output::text::help_topic(topic, &fixture_ops::fixture_usage())
}

#[cfg(test)]
fn load_bundle(path: &Path) -> Result<CaptureBundle, String> {
    let data = fs::read_to_string(path).map_err(|err| {
        format!(
            "failed to read capture artifact '{}': {err}",
            path.display()
        )
    })?;
    serde_json::from_str(&data).map_err(|err| {
        format!(
            "failed to parse capture artifact '{}': {err}",
            path.display()
        )
    })
}

#[cfg(test)]
fn run_fixture(
    path: &Path,
    output_override: Option<&Path>,
    pretty_capture: bool,
) -> Result<cli::handlers::FixtureRunSummary, String> {
    cli::handlers::run_fixture(path, output_override, pretty_capture)
}

#[cfg(test)]
fn format_replay_error(err: &ReplayError) -> String {
    output::errors::render_host_error_descriptor(ergo_host::describe_replay_error(err))
}

#[cfg(test)]
fn replay_graph(
    path: &Path,
    graph_path: &Path,
    cluster_paths: &[PathBuf],
    adapter_path: Option<&Path>,
) -> Result<cli::handlers::ReplaySummary, String> {
    cli::handlers::replay_graph(path, graph_path, cluster_paths, adapter_path)
}

#[cfg(test)]
mod tests;
