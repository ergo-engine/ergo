use std::path::PathBuf;
use std::sync::Arc;

use ergo_adapter::{
    ensure_demo_sources_have_no_required_context, fixture, AdapterProvides, GraphId, RuntimeHandle,
};
use ergo_runtime::catalog::{build_core_catalog, core_registries};
use ergo_runtime::provenance::{compute_runtime_provenance, RuntimeProvenanceScheme};
use ergo_supervisor::demo::demo_1;
use ergo_supervisor::Constraints;

use crate::usecases::{run_fixture, HostRunError, RunFixtureRequest, RunFixtureResult};
use crate::HostedRunner;

const DEMO_GRAPH_ID: &str = "demo_1";

pub struct RunDemoFixtureRequest {
    pub fixture_path: PathBuf,
    pub capture_output: Option<PathBuf>,
    pub pretty_capture: bool,
}

pub fn run_demo_fixture_from_path(
    request: RunDemoFixtureRequest,
) -> Result<RunFixtureResult, HostRunError> {
    let RunDemoFixtureRequest {
        fixture_path,
        capture_output,
        pretty_capture,
    } = request;

    let graph = Arc::new(demo_1::build_demo_1_graph());
    let catalog = Arc::new(build_core_catalog());
    let core_registries = Arc::new(
        core_registries()
            .map_err(|err| HostRunError::InvalidInput(format!("core registries: {err:?}")))?,
    );

    ensure_demo_sources_have_no_required_context(&graph, &catalog, &core_registries)
        .map_err(HostRunError::InvalidInput)?;

    let runtime = RuntimeHandle::new(
        graph.clone(),
        catalog.clone(),
        core_registries.clone(),
        AdapterProvides::default(),
    );
    let runtime_provenance = compute_runtime_provenance(
        RuntimeProvenanceScheme::Rpv1,
        DEMO_GRAPH_ID,
        graph.as_ref(),
        catalog.as_ref(),
    )
    .map_err(|err| {
        HostRunError::InvalidInput(format!("runtime provenance compute failed: {err}"))
    })?;
    let runner = HostedRunner::new(
        GraphId::new(DEMO_GRAPH_ID),
        Constraints::default(),
        runtime,
        runtime_provenance,
        None,
    )
    .map_err(|err| {
        HostRunError::StepFailed(format!("failed to initialize hosted fixture runner: {err}"))
    })?;

    let capture_output =
        capture_output.unwrap_or_else(|| fixture::fixture_output_path(&fixture_path));

    run_fixture(RunFixtureRequest {
        fixture_path,
        capture_output,
        pretty_capture,
        runner,
    })
}
