mod capture_enrichment;
mod demo_fixture_usecase;
mod error;
mod gen_docs_usecase;
mod graph_dot_usecase;
mod manifest_usecases;
mod replay;
mod replay_error_surface;
mod runner;
mod usecases;

pub use demo_fixture_usecase::{run_demo_fixture_from_path, RunDemoFixtureRequest};
pub use error::HostedStepError;
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
    replay_graph, replay_graph_from_paths, run_fixture, run_graph, run_graph_from_paths,
    scan_adapter_dependencies, validate_adapter_composition, AdapterDependencySummary,
    HostReplayError, HostRunError, ReplayGraphFromPathsRequest, ReplayGraphRequest,
    ReplayGraphResult, RunFixtureRequest, RunFixtureResult, RunGraphFromPathsRequest,
    RunGraphRequest, RunGraphResult,
};
