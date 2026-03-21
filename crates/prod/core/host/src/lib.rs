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
pub use ergo_supervisor::{write_capture_bundle, CaptureBundle, CaptureJsonStyle};
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

// Canonical client-facing host seams. CLI and SDK should route product-level
// run, replay, validation, and manual-step orchestration through these exports.
pub use usecases::{
    finalize_hosted_runner_capture, prepare_hosted_runner_from_paths,
    prepare_hosted_runner_from_paths_with_surfaces, replay_graph_from_paths,
    replay_graph_from_paths_with_surfaces, run_graph_from_paths, run_graph_from_paths_with_control,
    run_graph_from_paths_with_surfaces, run_graph_from_paths_with_surfaces_and_control,
    validate_graph_from_paths, validate_graph_from_paths_with_surfaces, AdapterDependencySummary,
    DriverConfig, HostReplayError, HostRunError, HostStopHandle, InterruptedRun,
    InterruptionReason, PrepareHostedRunnerFromPathsRequest, ReplayGraphFromPathsRequest,
    ReplayGraphRequest, ReplayGraphResult, RunControl, RunGraphFromPathsRequest, RunGraphResponse,
    RunOutcome, RunSummary, RuntimeSurfaces,
};

// Lower-level host building blocks. These remain public for advanced embedded
// callers and tests, but they are not the canonical orchestration surface that
// CLI and SDK should compose themselves.
pub use usecases::{
    replay_graph, run_fixture, run_graph, run_graph_with_control, scan_adapter_dependencies,
    validate_adapter_composition, RunFixtureRequest, RunFixtureResult, RunGraphRequest,
};
