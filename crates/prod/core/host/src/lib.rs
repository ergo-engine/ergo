mod capture_enrichment;
mod demo_fixture_usecase;
mod egress;
mod error;
mod gen_docs_usecase;
mod graph_dot_usecase;
mod manifest_usecases;
#[allow(clippy::result_large_err)]
mod replay;
mod replay_error_surface;
mod runner;
#[allow(clippy::result_large_err)]
mod usecases;

pub use demo_fixture_usecase::{run_demo_fixture_from_path, RunDemoFixtureRequest};
pub use egress::{
    parse_egress_config_toml, validate_egress_config, EgressChannelConfig, EgressConfig,
    EgressProcessError, EgressRoute, EgressRuntime, EgressValidationError, EgressValidationWarning,
};
pub use error::{EgressDispatchFailure, HostedStepError};
pub use gen_docs_usecase::gen_docs_command;
pub use graph_dot_usecase::{graph_to_dot_from_paths, GraphToDotFromPathsRequest};
pub use manifest_usecases::{
    check_compose_paths, validate_manifest_path, HostManifestError, HostRuleViolation,
    ManifestSummary,
};
pub use replay::{decision_counts, replay_bundle_strict, HostedReplayError};
pub use replay_error_surface::{
    describe_adapter_required, describe_host_replay_error, describe_replay_error,
    HostErrorDescriptor,
};
pub use runner::{HostedAdapterConfig, HostedEvent, HostedRunner, HostedStepOutcome};
pub use usecases::{
    replay_graph, replay_graph_from_paths, replay_graph_from_paths_with_surfaces, run_fixture,
    run_graph, run_graph_from_paths, run_graph_from_paths_with_control,
    run_graph_from_paths_with_surfaces, run_graph_from_paths_with_surfaces_and_control,
    run_graph_with_control, scan_adapter_dependencies, validate_adapter_composition,
    AdapterDependencySummary, DriverConfig, HostReplayError, HostRunError, HostStopHandle,
    InterruptedRun, InterruptionReason, ReplayGraphFromPathsRequest, ReplayGraphRequest,
    ReplayGraphResult, RunControl, RunFixtureRequest, RunFixtureResult,
    RunGraphFromPathsRequest, RunGraphRequest, RunGraphResponse, RunOutcome, RunSummary,
    RuntimeSurfaces,
};
