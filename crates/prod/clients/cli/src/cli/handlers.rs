use std::path::{Path, PathBuf};

use ergo_host::{
    replay_graph_from_paths as host_replay_graph_from_paths, run_demo_fixture_from_path,
    HostReplayError, ReplayGraphFromPathsRequest, RunDemoFixtureRequest,
};

#[derive(Debug, Clone)]
pub struct FixtureRunSummary {
    pub capture_path: PathBuf,
    pub episode_event_counts: Vec<(String, usize)>,
}

#[derive(Debug, Clone)]
pub struct ReplaySummary {
    pub graph_id: String,
    pub events: usize,
    pub invoked: usize,
    pub deferred: usize,
    pub skipped: usize,
}

pub fn run_fixture(
    path: &Path,
    output_override: Option<&Path>,
    pretty_capture: bool,
) -> Result<FixtureRunSummary, String> {
    let result = run_demo_fixture_from_path(RunDemoFixtureRequest {
        fixture_path: path.to_path_buf(),
        capture_output: output_override.map(PathBuf::from),
        pretty_capture,
    })
    .map_err(crate::output::errors::render_host_run_error)?;

    Ok(FixtureRunSummary {
        capture_path: result.capture_path,
        episode_event_counts: result.episode_event_counts,
    })
}

fn format_host_replay_error(err: &HostReplayError) -> String {
    crate::output::errors::render_host_replay_error(err)
}

pub fn replay_graph(
    path: &Path,
    graph_path: &Path,
    cluster_paths: &[PathBuf],
    adapter_path: Option<&Path>,
) -> Result<ReplaySummary, String> {
    let replay_result = host_replay_graph_from_paths(ReplayGraphFromPathsRequest {
        capture_path: path.to_path_buf(),
        graph_path: graph_path.to_path_buf(),
        cluster_paths: cluster_paths.to_vec(),
        adapter_path: adapter_path.map(Path::to_path_buf),
    })
    .map_err(|err| format_host_replay_error(&err))?;

    Ok(ReplaySummary {
        graph_id: replay_result.graph_id.as_str().to_string(),
        events: replay_result.events,
        invoked: replay_result.invoked,
        deferred: replay_result.deferred,
        skipped: replay_result.skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_handler_uses_host_path_api_error_surface() {
        let missing_capture =
            std::env::temp_dir().join(format!("ergo-cli-missing-capture-{}", std::process::id()));
        let missing_graph =
            std::env::temp_dir().join(format!("ergo-cli-missing-graph-{}", std::process::id()));
        let err = replay_graph(&missing_capture, &missing_graph, &[], None)
            .expect_err("missing capture should fail via host path API");
        assert!(
            err.contains("code: replay.host_setup_failed"),
            "unexpected error: {err}"
        );
        assert!(
            err.contains("failed to read capture artifact"),
            "unexpected error: {err}"
        );
    }
}
