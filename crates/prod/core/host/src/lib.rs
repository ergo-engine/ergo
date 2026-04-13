//! ergo_host
//!
//! Purpose:
//! - Crate facade for the production host layer.
//! - Expose the canonical client-facing host seams plus the lower-level
//!   building blocks used by advanced embedded callers and tests.
//!
//! Owns:
//! - The top-level public re-export surface for canonical run, replay,
//!   validation, manual-runner preparation/finalization, and selected lower-level
//!   host building blocks.
//!
//! Does not own:
//! - Kernel execution or replay semantics from `ergo_supervisor` /
//!   `ergo_runtime`.
//! - Loader transport and asset discovery from `ergo_loader`.
//! - Adapter contracts and capture semantics from `ergo_adapter`.
//!
//! Connects to:
//! - CLI and SDK as the direct downstream consumers of the host public API.
//! - The internal host submodules that provide the concrete implementation of
//!   this facade.
//!
//! Safety notes:
//! - The `#[allow(clippy::result_large_err)]` annotations on `replay` and
//!   `usecases` are deliberate: these public seams return structured error types
//!   whose information would be lost or obscured by boxing at the crate
//!   boundary.

mod capture_enrichment;
mod demo_fixture_usecase;
mod egress;
mod error;
mod expand_diagnostics;
mod gen_docs_usecase;
mod graph_dot_usecase;
mod manifest_usecases;
mod protocol;
#[allow(clippy::result_large_err)]
mod replay;
mod replay_error_surface;
mod runner;
#[allow(clippy::result_large_err)]
mod usecases;

pub use demo_fixture_usecase::{run_demo_fixture_from_path, RunDemoFixtureRequest};
pub use egress::{
    parse_egress_config_toml, validate_egress_config, EgressChannelConfig, EgressConfig,
    EgressConfigBuilder, EgressConfigError, EgressConfigParseError, EgressProcessError,
    EgressRoute, EgressRuntime, EgressValidationError, EgressValidationWarning,
};
pub use ergo_supervisor::{
    write_capture_bundle, CaptureBundle, CaptureJsonStyle, CaptureWriteError,
};
pub use error::{
    EgressDispatchFailure, HostedEgressValidationError, HostedEventBuildError, HostedStepError,
};
pub use gen_docs_usecase::gen_docs_command;
pub use graph_dot_usecase::{
    graph_to_dot_from_assets, graph_to_dot_from_paths, GraphToDotAvailableCluster, GraphToDotError,
    GraphToDotExpansionContext, GraphToDotExpansionError, GraphToDotFromAssetsRequest,
    GraphToDotFromPathsRequest,
};
pub use manifest_usecases::{
    check_compose_paths, check_compose_text, check_compose_values, validate_manifest_path,
    validate_manifest_text, validate_manifest_value, HostManifestError, HostRuleViolation,
    ManifestSummary,
};
pub use protocol::PROCESS_DRIVER_PROTOCOL_VERSION;
pub use replay::{decision_counts, replay_bundle_strict, HostedReplayError};
pub use replay_error_surface::{
    describe_adapter_required, describe_host_replay_error, describe_production_requires_adapter,
    describe_replay_error, HostErrorCode, HostErrorDescriptor, HostRuleId,
};
pub use runner::{
    is_recoverable_hosted_step_error, HostedAdapterConfig, HostedEvent, HostedRunner,
    HostedStepOutcome,
};

// Canonical client-facing host seams. CLI and SDK should route product-level
// run, replay, validation, and manual-step orchestration through these exports.
pub use usecases::{
    finalize_hosted_runner_capture, prepare_hosted_runner_from_paths,
    prepare_hosted_runner_from_paths_with_surfaces, replay_graph_from_paths,
    replay_graph_from_paths_with_surfaces, run_graph_from_paths, run_graph_from_paths_with_control,
    run_graph_from_paths_with_surfaces, run_graph_from_paths_with_surfaces_and_control,
    validate_graph_from_paths, validate_graph_from_paths_with_surfaces,
    validate_run_graph_from_assets, validate_run_graph_from_assets_with_surfaces,
    validate_run_graph_from_paths, validate_run_graph_from_paths_with_surfaces,
    AdapterDependencySummary, DriverConfig, HostAdapterCompositionError, HostAdapterSetupError,
    HostAvailableCluster, HostDependencyScanError, HostDriverError, HostDriverInputError,
    HostDriverIoError, HostDriverOutputError, HostDriverProtocolError, HostDriverStartError,
    HostExpandContext, HostExpandError, HostGraphPreparationError, HostReplayError,
    HostReplaySetupError, HostRunError, HostSetupError, HostStopHandle, InterruptedRun,
    InterruptionReason, PrepareHostedRunnerFromPathsRequest, ReplayGraphFromAssetsRequest,
    ReplayGraphFromPathsRequest, ReplayGraphRequest, ReplayGraphResult, RunControl,
    RunGraphFromAssetsRequest, RunGraphFromPathsRequest, RunGraphResponse, RunOutcome, RunSummary,
    RuntimeSurfaces, SessionIntent,
};

// Lower-level host building blocks. These remain public for advanced embedded
// callers and tests, but they are not the canonical orchestration surface that
// CLI and SDK should compose themselves.
pub use ergo_loader::{InMemorySourceInput, PreparedGraphAssets};
pub use usecases::{
    load_graph_assets_from_memory, load_graph_assets_from_paths, prepare_hosted_runner,
    prepare_hosted_runner_with_surfaces, replay_graph, replay_graph_from_assets,
    replay_graph_from_assets_with_surfaces, run_fixture, run_graph, run_graph_from_assets,
    run_graph_from_assets_with_control, run_graph_from_assets_with_surfaces,
    run_graph_from_assets_with_surfaces_and_control, run_graph_with_control,
    scan_adapter_dependencies, validate_adapter_composition, validate_graph,
    validate_graph_with_surfaces, AdapterInput, CapturePolicy, LivePrepOptions, RunFixtureRequest,
    RunFixtureResult, RunGraphRequest,
};
