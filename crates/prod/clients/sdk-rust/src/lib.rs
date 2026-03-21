//! Rust SDK over Ergo host + loader.
//!
//! The SDK is the primary product surface for building an Ergo engine
//! inside a Rust crate. It wraps the existing canonical host run and
//! replay paths without introducing a second execution model.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ergo_host::{
    finalize_hosted_runner_capture, parse_egress_config_toml,
    prepare_hosted_runner_from_paths_with_surfaces, replay_graph_from_paths_with_surfaces,
    run_graph_from_paths_with_surfaces_and_control, validate_graph_from_paths_with_surfaces,
    DriverConfig, EgressConfig, HostReplayError, HostRunError, HostStopHandle, HostedRunner,
    PrepareHostedRunnerFromPathsRequest, ReplayGraphFromPathsRequest, ReplayGraphResult,
    RunControl, RunGraphFromPathsRequest, RunOutcome, RuntimeSurfaces,
};
use ergo_loader::{
    load_project, ProjectError as LoaderProjectError, ResolvedProject, ResolvedProjectIngress,
    ResolvedProjectProfile,
};
use ergo_runtime::catalog::{CatalogBuilder, CoreRegistrationError};

pub use ergo_host::{
    write_capture_bundle, CaptureBundle, CaptureJsonStyle, EgressChannelConfig,
    EgressDispatchFailure, EgressRoute, HostedEvent, HostedStepError, HostedStepOutcome,
    InterruptedRun, InterruptionReason, RunSummary,
};
pub use ergo_runtime::catalog::{build_core, build_core_catalog, core_registries};
pub use ergo_runtime::runtime::ExecutionContext;
pub use ergo_runtime::{action, common, compute, source, trigger};

#[derive(Debug, Clone, Default)]
pub struct StopHandle {
    handle: HostStopHandle,
}

impl StopHandle {
    pub fn new() -> Self {
        Self {
            handle: HostStopHandle::new(),
        }
    }

    pub fn stop(&self) {
        self.handle.request_stop();
    }

    fn host_handle(&self) -> HostStopHandle {
        self.handle.clone()
    }
}

#[derive(Debug)]
pub enum ErgoBuildError {
    Registration(String),
}

impl std::fmt::Display for ErgoBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Registration(detail) => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for ErgoBuildError {}

#[derive(Debug)]
pub enum ProjectError {
    ProjectRootRequired,
    ConfigInvalid { detail: String },
    Load(LoaderProjectError),
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProjectRootRequired => {
                write!(f, "project root is required for project/profile operations")
            }
            Self::ConfigInvalid { detail } => write!(f, "{detail}"),
            Self::Load(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ProjectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ProjectRootRequired | Self::ConfigInvalid { .. } => None,
            Self::Load(err) => Some(err),
        }
    }
}

impl From<LoaderProjectError> for ProjectError {
    fn from(value: LoaderProjectError) -> Self {
        Self::Load(value)
    }
}

#[derive(Debug)]
pub enum ErgoRunError {
    Project(ProjectError),
    Host(HostRunError),
}

impl std::fmt::Display for ErgoRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Project(err) => write!(f, "{err}"),
            Self::Host(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ErgoRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Project(err) => Some(err),
            Self::Host(err) => Some(err),
        }
    }
}

#[derive(Debug)]
pub enum ErgoReplayError {
    Project(ProjectError),
    Host(HostReplayError),
}

impl std::fmt::Display for ErgoReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Project(err) => write!(f, "{err}"),
            Self::Host(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ErgoReplayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Project(err) => Some(err),
            Self::Host(err) => Some(err),
        }
    }
}

#[derive(Debug)]
pub enum ErgoValidateError {
    Project(ProjectError),
    Validation { profile: String, detail: String },
}

impl std::fmt::Display for ErgoValidateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Project(err) => write!(f, "{err}"),
            Self::Validation { profile, detail } => {
                write!(f, "validation failed for profile '{profile}': {detail}")
            }
        }
    }
}

impl std::error::Error for ErgoValidateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Project(err) => Some(err),
            Self::Validation { .. } => None,
        }
    }
}

/// Manual-runner setup currently reports the same project/host error surface as
/// profile execution. Keep the alias so callers can name the runner-specific
/// result today; if the surfaces diverge later this can become a distinct type
/// without renaming the API.
pub type ErgoRunnerError = ErgoRunError;

#[derive(Debug)]
pub enum ProfileRunnerCaptureError {
    Finish(HostedStepError),
    CaptureOutputNotConfigured,
    Write {
        detail: String,
        bundle: CaptureBundle,
    },
}

impl std::fmt::Display for ProfileRunnerCaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Finish(err) => write!(f, "{err}"),
            Self::CaptureOutputNotConfigured => {
                write!(f, "profile does not declare capture_output")
            }
            Self::Write { detail, .. } => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for ProfileRunnerCaptureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Finish(err) => Some(err),
            Self::CaptureOutputNotConfigured | Self::Write { .. } => None,
        }
    }
}

impl ProfileRunnerCaptureError {
    pub fn capture_bundle(&self) -> Option<&CaptureBundle> {
        match self {
            Self::Write { bundle, .. } => Some(bundle),
            Self::Finish(_) | Self::CaptureOutputNotConfigured => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProfileRunnerState {
    Active,
    FinalizableAfterDispatchFailure,
    Failed,
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngressConfig {
    Fixture { path: PathBuf },
    Process { command: Vec<String> },
}

impl IngressConfig {
    pub fn fixture(path: impl AsRef<Path>) -> Self {
        Self::Fixture {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn process<I, S>(command: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::Process {
            command: command.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunConfig {
    graph_path: PathBuf,
    cluster_paths: Vec<PathBuf>,
    ingress: IngressConfig,
    adapter_path: Option<PathBuf>,
    egress_config_path: Option<PathBuf>,
    capture_output: Option<PathBuf>,
    pretty_capture: bool,
    max_duration: Option<Duration>,
    max_events: Option<u64>,
}

impl RunConfig {
    pub fn new(graph_path: impl AsRef<Path>, ingress: IngressConfig) -> Self {
        Self {
            graph_path: graph_path.as_ref().to_path_buf(),
            cluster_paths: Vec::new(),
            ingress,
            adapter_path: None,
            egress_config_path: None,
            capture_output: None,
            pretty_capture: false,
            max_duration: None,
            max_events: None,
        }
    }

    pub fn cluster_path(mut self, path: impl AsRef<Path>) -> Self {
        self.cluster_paths.push(path.as_ref().to_path_buf());
        self
    }

    pub fn adapter(mut self, path: impl AsRef<Path>) -> Self {
        self.adapter_path = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn egress_config(mut self, path: impl AsRef<Path>) -> Self {
        self.egress_config_path = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn capture_output(mut self, path: impl AsRef<Path>) -> Self {
        self.capture_output = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn pretty_capture(mut self, enabled: bool) -> Self {
        self.pretty_capture = enabled;
        self
    }

    pub fn max_duration(mut self, max_duration: Duration) -> Self {
        self.max_duration = Some(max_duration);
        self
    }

    pub fn max_events(mut self, max_events: u64) -> Self {
        self.max_events = Some(max_events);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayConfig {
    capture_path: PathBuf,
    graph_path: PathBuf,
    cluster_paths: Vec<PathBuf>,
    adapter_path: Option<PathBuf>,
}

impl ReplayConfig {
    pub fn new(capture_path: impl AsRef<Path>, graph_path: impl AsRef<Path>) -> Self {
        Self {
            capture_path: capture_path.as_ref().to_path_buf(),
            graph_path: graph_path.as_ref().to_path_buf(),
            cluster_paths: Vec::new(),
            adapter_path: None,
        }
    }

    pub fn cluster_path(mut self, path: impl AsRef<Path>) -> Self {
        self.cluster_paths.push(path.as_ref().to_path_buf());
        self
    }

    pub fn adapter(mut self, path: impl AsRef<Path>) -> Self {
        self.adapter_path = Some(path.as_ref().to_path_buf());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSummary {
    pub root: PathBuf,
    pub name: String,
    pub version: String,
    pub profiles: Vec<String>,
}

pub struct ErgoBuilder {
    catalog_builder: CatalogBuilder,
    project_root: Option<PathBuf>,
}

impl Default for ErgoBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ErgoBuilder {
    pub fn new() -> Self {
        Self {
            catalog_builder: CatalogBuilder::new(),
            project_root: None,
        }
    }

    pub fn project_root(mut self, path: impl AsRef<Path>) -> Self {
        self.project_root = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn add_source<P>(mut self, primitive: P) -> Self
    where
        P: source::SourcePrimitive + 'static,
    {
        self.catalog_builder.add_source(Box::new(primitive));
        self
    }

    pub fn add_compute<P>(mut self, primitive: P) -> Self
    where
        P: compute::ComputePrimitive + 'static,
    {
        self.catalog_builder.add_compute(Box::new(primitive));
        self
    }

    pub fn add_trigger<P>(mut self, primitive: P) -> Self
    where
        P: trigger::TriggerPrimitive + 'static,
    {
        self.catalog_builder.add_trigger(Box::new(primitive));
        self
    }

    pub fn add_action<P>(mut self, primitive: P) -> Self
    where
        P: action::ActionPrimitive + 'static,
    {
        self.catalog_builder.add_action(Box::new(primitive));
        self
    }

    pub fn build(self) -> Result<Ergo, ErgoBuildError> {
        let (registries, catalog) = self
            .catalog_builder
            .build()
            .map_err(format_registration_error)?;
        Ok(Ergo {
            runtime_surfaces: RuntimeSurfaces::new(registries, catalog),
            project_root: self.project_root,
        })
    }
}

/// Built Ergo engine handle.
///
/// One built `Ergo` handle may be reused for multiple operations on the
/// same thread. Reuse preserves the existing primitive instances behind
/// the handle under the current in-process trust model.
pub struct Ergo {
    runtime_surfaces: RuntimeSurfaces,
    project_root: Option<PathBuf>,
}

impl Ergo {
    pub fn builder() -> ErgoBuilder {
        ErgoBuilder::new()
    }

    pub fn from_project(path: impl AsRef<Path>) -> ErgoBuilder {
        ErgoBuilder::new().project_root(path)
    }

    pub fn run(&self, config: RunConfig) -> Result<RunOutcome, ErgoRunError> {
        self.run_with_control(config, RunControl::default())
    }

    pub fn run_with_stop(
        &self,
        config: RunConfig,
        stop: StopHandle,
    ) -> Result<RunOutcome, ErgoRunError> {
        self.run_with_control(
            config,
            RunControl::new().with_stop_handle(stop.host_handle()),
        )
    }

    fn run_with_control(
        &self,
        config: RunConfig,
        control: RunControl,
    ) -> Result<RunOutcome, ErgoRunError> {
        let request = run_request_from_config(&config)
            .map_err(|detail| ProjectError::ConfigInvalid {
                detail: format!("explicit run configuration is invalid: {detail}"),
            })
            .map_err(ErgoRunError::Project)?;

        run_graph_from_paths_with_surfaces_and_control(
            request,
            self.runtime_surfaces.clone(),
            run_control_from_config(&config, control),
        )
        .map_err(ErgoRunError::Host)
    }

    pub fn run_profile(&self, profile_name: &str) -> Result<RunOutcome, ErgoRunError> {
        self.run_profile_with_control(profile_name, RunControl::default())
    }

    pub fn run_profile_with_stop(
        &self,
        profile_name: &str,
        stop: StopHandle,
    ) -> Result<RunOutcome, ErgoRunError> {
        self.run_profile_with_control(
            profile_name,
            RunControl::new().with_stop_handle(stop.host_handle()),
        )
    }

    fn run_profile_with_control(
        &self,
        profile_name: &str,
        control: RunControl,
    ) -> Result<RunOutcome, ErgoRunError> {
        let resolved = self
            .resolve_run_profile(profile_name)
            .map_err(ErgoRunError::Project)?;
        self.run_with_control(run_config_from_resolved_profile(resolved), control)
    }

    pub fn replay(&self, config: ReplayConfig) -> Result<ReplayGraphResult, ErgoReplayError> {
        replay_graph_from_paths_with_surfaces(
            ReplayGraphFromPathsRequest {
                capture_path: config.capture_path,
                graph_path: config.graph_path,
                cluster_paths: config.cluster_paths,
                adapter_path: config.adapter_path,
            },
            self.runtime_surfaces.clone(),
        )
        .map_err(ErgoReplayError::Host)
    }

    pub fn replay_profile(
        &self,
        profile_name: &str,
        capture_path: impl AsRef<Path>,
    ) -> Result<ReplayGraphResult, ErgoReplayError> {
        let resolved = self
            .resolve_run_profile(profile_name)
            .map_err(ErgoReplayError::Project)?;
        self.replay(replay_config_from_resolved_profile(resolved, capture_path))
    }

    pub fn validate_project(&self) -> Result<ProjectSummary, ErgoValidateError> {
        let project = self.load_project().map_err(ErgoValidateError::Project)?;
        let profiles = project.profile_names();
        for profile_name in &profiles {
            let resolved = Self::resolve_run_profile_from_project(&project, profile_name)
                .map_err(ErgoValidateError::Project)?;
            let request = self
                .prepare_host_request_from_profile(&resolved)
                .map_err(|err| ErgoValidateError::Validation {
                    profile: profile_name.clone(),
                    detail: err.to_string(),
                })?;
            validate_graph_from_paths_with_surfaces(request, self.runtime_surfaces.clone())
                .map_err(|err| ErgoValidateError::Validation {
                    profile: profile_name.clone(),
                    detail: err.to_string(),
                })?;
        }

        Ok(ProjectSummary {
            root: project.root.clone(),
            name: project.manifest.name,
            version: project.manifest.version,
            profiles,
        })
    }

    fn load_project(&self) -> Result<ResolvedProject, ProjectError> {
        let project_root = self
            .project_root
            .as_deref()
            .ok_or(ProjectError::ProjectRootRequired)?;
        load_project(project_root).map_err(ProjectError::from)
    }

    fn resolve_run_profile(
        &self,
        profile_name: &str,
    ) -> Result<ResolvedProjectProfile, ProjectError> {
        let project = self.load_project()?;
        Self::resolve_run_profile_from_project(&project, profile_name)
    }

    fn resolve_run_profile_from_project(
        project: &ResolvedProject,
        profile_name: &str,
    ) -> Result<ResolvedProjectProfile, ProjectError> {
        project
            .resolve_run_profile(profile_name)
            .map_err(ProjectError::from)
    }

    fn prepare_host_request_from_profile(
        &self,
        profile: &ResolvedProjectProfile,
    ) -> Result<PrepareHostedRunnerFromPathsRequest, ProjectError> {
        let egress_config = profile
            .egress_config_path
            .as_deref()
            .map(load_egress_config)
            .transpose()
            .map_err(|detail| ProjectError::ConfigInvalid { detail })?;
        Ok(PrepareHostedRunnerFromPathsRequest {
            graph_path: profile.graph_path.clone(),
            cluster_paths: profile.cluster_paths.clone(),
            adapter_path: profile.adapter_path.clone(),
            egress_config,
        })
    }

    pub fn runner_for_profile(&self, profile_name: &str) -> Result<ProfileRunner, ErgoRunnerError> {
        let resolved = self
            .resolve_run_profile(profile_name)
            .map_err(ErgoRunnerError::Project)?;
        let request = self
            .prepare_host_request_from_profile(&resolved)
            .map_err(ErgoRunnerError::Project)?;
        let runner =
            prepare_hosted_runner_from_paths_with_surfaces(request, self.runtime_surfaces.clone())
                .map_err(ErgoRunnerError::Host)?;

        Ok(ProfileRunner {
            runner: Some(runner),
            capture_output: resolved.capture_output.clone(),
            pretty_capture: resolved.pretty_capture,
            state: ProfileRunnerState::Active,
            successful_steps: 0,
        })
    }
}

pub struct ProfileRunner {
    runner: Option<HostedRunner>,
    capture_output: Option<PathBuf>,
    pretty_capture: bool,
    state: ProfileRunnerState,
    successful_steps: usize,
}

impl ProfileRunner {
    pub fn step(&mut self, event: HostedEvent) -> Result<HostedStepOutcome, HostedStepError> {
        match self.state {
            ProfileRunnerState::Active => {}
            ProfileRunnerState::FinalizableAfterDispatchFailure => {
                return Err(lifecycle_violation(
                    "profile runner must be finalized after egress dispatch failure before stepping again",
                ));
            }
            ProfileRunnerState::Failed => {
                return Err(lifecycle_violation(
                    "profile runner cannot continue after a non-finalizable step error",
                ));
            }
            ProfileRunnerState::Finished => {
                return Err(lifecycle_violation("profile runner is already finished"));
            }
        }

        let runner = self
            .runner
            .as_mut()
            .expect("active profile runner must hold hosted runner");
        match runner.step(event) {
            Ok(outcome) => {
                self.successful_steps += 1;
                Ok(outcome)
            }
            Err(HostedStepError::EgressDispatchFailure(failure)) => {
                self.state = ProfileRunnerState::FinalizableAfterDispatchFailure;
                Err(HostedStepError::EgressDispatchFailure(failure))
            }
            Err(err) => {
                if !is_recoverable_step_error(&err) {
                    self.state = ProfileRunnerState::Failed;
                }
                Err(err)
            }
        }
    }

    pub fn context_snapshot(
        &self,
    ) -> Result<&std::collections::BTreeMap<String, serde_json::Value>, HostedStepError> {
        let Some(runner) = self.runner.as_ref() else {
            return Err(lifecycle_violation("profile runner is already finished"));
        };
        Ok(runner.context_snapshot())
    }

    pub fn capture_output_path(&self) -> Option<&Path> {
        self.capture_output.as_deref()
    }

    pub fn pretty_capture(&self) -> bool {
        self.pretty_capture
    }

    pub fn finish(&mut self) -> Result<CaptureBundle, HostedStepError> {
        match self.state {
            ProfileRunnerState::Finished => {
                return Err(lifecycle_violation("profile runner is already finished"));
            }
            ProfileRunnerState::Failed => {
                return Err(lifecycle_violation(
                    "profile runner cannot finalize after a non-finalizable step error",
                ));
            }
            ProfileRunnerState::Active if self.successful_steps == 0 => {
                return Err(lifecycle_violation(
                    "profile runner cannot finalize before the first successful step",
                ));
            }
            ProfileRunnerState::Active | ProfileRunnerState::FinalizableAfterDispatchFailure => {}
        }

        self.state = ProfileRunnerState::Finished;
        let runner = self
            .runner
            .take()
            .expect("unfinished profile runner must hold hosted runner");
        finalize_hosted_runner_capture(runner, false)
    }

    pub fn finish_and_write_capture(&mut self) -> Result<CaptureBundle, ProfileRunnerCaptureError> {
        let capture_path = self
            .capture_output
            .clone()
            .ok_or(ProfileRunnerCaptureError::CaptureOutputNotConfigured)?;
        let bundle = self.finish().map_err(ProfileRunnerCaptureError::Finish)?;
        let style = if self.pretty_capture {
            CaptureJsonStyle::Pretty
        } else {
            CaptureJsonStyle::Compact
        };
        match write_capture_bundle(&capture_path, &bundle, style) {
            Ok(()) => Ok(bundle),
            Err(detail) => Err(ProfileRunnerCaptureError::Write { detail, bundle }),
        }
    }
}

fn ingress_from_resolved(ingress: ResolvedProjectIngress) -> IngressConfig {
    match ingress {
        ResolvedProjectIngress::Fixture { path } => IngressConfig::Fixture { path },
        ResolvedProjectIngress::Process { command } => IngressConfig::Process { command },
    }
}

fn run_config_from_resolved_profile(profile: ResolvedProjectProfile) -> RunConfig {
    RunConfig {
        graph_path: profile.graph_path,
        cluster_paths: profile.cluster_paths,
        ingress: ingress_from_resolved(profile.ingress),
        adapter_path: profile.adapter_path,
        egress_config_path: profile.egress_config_path,
        capture_output: profile.capture_output,
        pretty_capture: profile.pretty_capture,
        max_duration: profile.max_duration,
        max_events: profile.max_events,
    }
}

fn replay_config_from_resolved_profile(
    profile: ResolvedProjectProfile,
    capture_path: impl AsRef<Path>,
) -> ReplayConfig {
    ReplayConfig {
        capture_path: capture_path.as_ref().to_path_buf(),
        graph_path: profile.graph_path,
        cluster_paths: profile.cluster_paths,
        adapter_path: profile.adapter_path,
    }
}

fn format_registration_error(err: CoreRegistrationError) -> ErgoBuildError {
    ErgoBuildError::Registration(format!("primitive registration failed: {err:?}"))
}

fn lifecycle_violation(detail: impl Into<String>) -> HostedStepError {
    HostedStepError::LifecycleViolation {
        detail: detail.into(),
    }
}

// Caller-input validation failures happen before the host runner commits a step,
// so the manual-stepping wrapper should let callers correct and continue.
fn is_recoverable_step_error(err: &HostedStepError) -> bool {
    matches!(
        err,
        HostedStepError::DuplicateEventId { .. }
            | HostedStepError::MissingSemanticKind
            | HostedStepError::MissingPayload
            | HostedStepError::PayloadMustBeObject
            | HostedStepError::UnknownSemanticKind { .. }
            | HostedStepError::BindingError(_)
            | HostedStepError::EventBuildError(_)
    )
}

fn run_request_from_config(config: &RunConfig) -> Result<RunGraphFromPathsRequest, String> {
    Ok(RunGraphFromPathsRequest {
        graph_path: config.graph_path.clone(),
        cluster_paths: config.cluster_paths.clone(),
        driver: ingress_to_driver(&config.ingress)?,
        adapter_path: config.adapter_path.clone(),
        egress_config: match &config.egress_config_path {
            Some(path) => Some(load_egress_config(path)?),
            None => None,
        },
        capture_output: config.capture_output.clone(),
        pretty_capture: config.pretty_capture,
    })
}

fn run_control_from_config(config: &RunConfig, control: RunControl) -> RunControl {
    let control = if let Some(max_duration) = config.max_duration {
        control.max_duration(max_duration)
    } else {
        control
    };

    if let Some(max_events) = config.max_events {
        control.max_events(max_events)
    } else {
        control
    }
}

fn ingress_to_driver(ingress: &IngressConfig) -> Result<DriverConfig, String> {
    match ingress {
        IngressConfig::Fixture { path } => Ok(DriverConfig::Fixture { path: path.clone() }),
        IngressConfig::Process { command } => {
            if command.is_empty() {
                return Err("process ingress command must not be empty".to_string());
            }
            Ok(DriverConfig::Process {
                command: command.clone(),
            })
        }
    }
}

fn load_egress_config(path: &Path) -> Result<EgressConfig, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read egress config '{}': {err}", path.display()))?;
    parse_egress_config_toml(&raw)
        .map_err(|err| format!("failed to parse egress config '{}': {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ergo_adapter::{EventTime, ExternalEventKind, RunTermination};
    use ergo_runtime::action::{
        ActionEffects, ActionKind, ActionOutcome, ActionPrimitive, ActionPrimitiveManifest,
        ActionValue, ActionValueType, Cardinality as ActionCardinality,
        ExecutionSpec as ActionExecutionSpec, InputSpec as ActionInputSpec, IntentFieldSpec,
        IntentSpec, OutputSpec as ActionOutputSpec, StateSpec as ActionStateSpec,
    };
    use ergo_runtime::common::{Value, ValueType};
    use ergo_runtime::runtime::ExecutionContext;
    use ergo_runtime::source::{
        Cadence as SourceCadence, ExecutionSpec as SourceExecutionSpec,
        OutputSpec as SourceOutputSpec, SourceKind, SourcePrimitive, SourcePrimitiveManifest,
        SourceRequires, StateSpec as SourceStateSpec,
    };
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct InjectedNumberSource {
        manifest: SourcePrimitiveManifest,
        output: f64,
    }

    impl InjectedNumberSource {
        fn new(output: f64) -> Self {
            Self {
                manifest: SourcePrimitiveManifest {
                    id: "injected_number_source".to_string(),
                    version: "0.1.0".to_string(),
                    kind: SourceKind::Source,
                    inputs: vec![],
                    outputs: vec![SourceOutputSpec {
                        name: "value".to_string(),
                        value_type: ValueType::Number,
                    }],
                    parameters: vec![],
                    requires: SourceRequires {
                        context: Vec::new(),
                    },
                    execution: SourceExecutionSpec {
                        deterministic: true,
                        cadence: SourceCadence::Continuous,
                    },
                    state: SourceStateSpec { allowed: false },
                    side_effects: false,
                },
                output,
            }
        }
    }

    impl SourcePrimitive for InjectedNumberSource {
        fn manifest(&self) -> &SourcePrimitiveManifest {
            &self.manifest
        }

        fn produce(
            &self,
            _parameters: &HashMap<String, source::ParameterValue>,
            _ctx: &ExecutionContext,
        ) -> HashMap<String, Value> {
            HashMap::from([("value".to_string(), Value::Number(self.output))])
        }
    }

    struct InjectedIntentAction {
        manifest: ActionPrimitiveManifest,
    }

    impl InjectedIntentAction {
        fn new() -> Self {
            Self {
                manifest: ActionPrimitiveManifest {
                    id: "injected_intent_action".to_string(),
                    version: "0.1.0".to_string(),
                    kind: ActionKind::Action,
                    inputs: vec![
                        ActionInputSpec {
                            name: "event".to_string(),
                            value_type: ActionValueType::Event,
                            required: true,
                            cardinality: ActionCardinality::Single,
                        },
                        ActionInputSpec {
                            name: "qty".to_string(),
                            value_type: ActionValueType::Number,
                            required: true,
                            cardinality: ActionCardinality::Single,
                        },
                    ],
                    outputs: vec![ActionOutputSpec {
                        name: "outcome".to_string(),
                        value_type: ActionValueType::Event,
                    }],
                    parameters: vec![],
                    effects: ActionEffects {
                        writes: vec![],
                        intents: vec![IntentSpec {
                            name: "place_order".to_string(),
                            fields: vec![IntentFieldSpec {
                                name: "qty".to_string(),
                                value_type: ValueType::Number,
                                from_input: Some("qty".to_string()),
                                from_param: None,
                            }],
                            mirror_writes: vec![],
                        }],
                    },
                    execution: ActionExecutionSpec {
                        deterministic: true,
                        retryable: false,
                    },
                    state: ActionStateSpec { allowed: false },
                    side_effects: true,
                },
            }
        }
    }

    impl ActionPrimitive for InjectedIntentAction {
        fn manifest(&self) -> &ActionPrimitiveManifest {
            &self.manifest
        }

        fn execute(
            &self,
            _inputs: &HashMap<String, ActionValue>,
            _parameters: &HashMap<String, action::ParameterValue>,
        ) -> HashMap<String, ActionValue> {
            HashMap::from([(
                "outcome".to_string(),
                ActionValue::Event(ActionOutcome::Completed),
            )])
        }
    }

    fn make_temp_dir(label: &str) -> PathBuf {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ergo_sdk_rust_{label}_{}_{}_{}",
            std::process::id(),
            index,
            nanos
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_file(base: &Path, rel: &str, contents: &str) -> PathBuf {
        let path = base.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&path, contents).expect("write file");
        path
    }

    fn write_intent_graph(base: &Path, graph_id: &str) -> PathBuf {
        write_file(
            base,
            "graphs/strategy.yaml",
            &format!(
                r#"
kind: cluster
id: {graph_id}
version: "0.1.0"
nodes:
  gate:
    impl: const_bool@0.1.0
    params:
      value: true
  emit:
    impl: emit_if_true@0.1.0
  qty:
    impl: injected_number_source@0.1.0
  place:
    impl: injected_intent_action@0.1.0
edges:
  - "gate.value -> emit.input"
  - "emit.event -> place.event"
  - "qty.value -> place.qty"
outputs:
  outcome: place.outcome
"#
            ),
        )
    }

    fn write_intent_adapter_manifest(base: &Path) -> PathBuf {
        write_file(
            base,
            "adapters/trading.yaml",
            r#"
kind: adapter
id: sdk_trading_adapter
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: last_qty
    type: Number
    required: false
    writable: true
event_kinds:
  - name: price_bar
    payload_schema:
      type: object
      properties:
        price: { type: number }
      additionalProperties: false
accepts:
  effects:
    - name: set_context
      payload_schema:
        type: object
        additionalProperties: false
    - name: place_order
      payload_schema:
        type: object
        properties:
          qty: { type: number }
        required: [qty]
        additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.price_bar
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
"#,
        )
    }

    fn write_process_ingress_sentinel(base: &Path, sentinel_path: &Path) -> PathBuf {
        write_file(
            base,
            "scripts/ingress.sh",
            &format!(
                r#"#!/bin/sh
printf '%s\n' started > "{sentinel}"
printf '%s\n' '{{"type":"hello","protocol":"ergo-driver.v0"}}'
printf '%s\n' '{{"type":"end"}}'
"#,
                sentinel = sentinel_path.display()
            ),
        )
    }

    fn write_egress_ack_script(base: &Path) -> PathBuf {
        write_file(
            base,
            "channels/egress/broker.sh",
            r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  case "$line" in
    *'"type":"intent"'*)
      intent_id=$(printf '%s' "$line" | sed -n 's/.*"intent_id":"\([^"]*\)".*/\1/p')
      printf '{"type":"intent_ack","intent_id":"%s","status":"accepted","acceptance":"durable"}\n' "$intent_id"
      ;;
    *'"type":"end"'*)
      exit 0
      ;;
  esac
done
"#,
        )
    }

    fn write_egress_io_script(base: &Path) -> PathBuf {
        write_file(
            base,
            "channels/egress/broker.sh",
            r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  case "$line" in
    *'"type":"intent"'*)
      exit 1
      ;;
    *'"type":"end"'*)
      exit 0
      ;;
  esac
done
"#,
        )
    }

    fn write_egress_config(base: &Path, command: Vec<String>) -> PathBuf {
        write_file(
            base,
            "egress/live.toml",
            &format!(
                r#"
default_ack_timeout = "100ms"

[channels.broker]
type = "process"
command = [{command}]

[routes.place_order]
channel = "broker"
"#,
                command = command
                    .into_iter()
                    .map(|part| format!("{part:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
    }

    fn manual_step_event(event_id: &str) -> HostedEvent {
        HostedEvent {
            event_id: event_id.to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: None,
            payload: Some(serde_json::json!({})),
        }
    }

    fn adapter_bound_event(event_id: &str, price: f64) -> HostedEvent {
        HostedEvent {
            event_id: event_id.to_string(),
            kind: ExternalEventKind::Command,
            at: EventTime::default(),
            semantic_kind: Some("price_bar".to_string()),
            payload: Some(serde_json::json!({ "price": price })),
        }
    }

    #[test]
    fn explicit_run_uses_registered_custom_source() -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("explicit_run");
        let graph = write_file(
            &root,
            "graph.yaml",
            r#"
kind: cluster
id: sdk_explicit_run
version: "0.1.0"
nodes:
  src:
    impl: injected_number_source@0.1.0
edges: []
outputs:
  value_out: src.value
"#,
        );
        let fixture = write_file(
            &root,
            "fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );
        let capture = root.join("capture.json");

        let outcome = Ergo::builder()
            .add_source(InjectedNumberSource::new(7.5))
            .build()?
            .run(RunConfig::new(graph, IngressConfig::fixture(fixture)).capture_output(&capture))?;

        match outcome {
            RunOutcome::Completed(summary) => {
                assert_eq!(summary.events, 1);
                assert_eq!(summary.capture_path, capture);
            }
            other => panic!("expected completed run, got {other:?}"),
        }

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn run_with_stop_can_request_zero_event_host_stop() -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("explicit_stop");
        let graph = write_file(
            &root,
            "graph.yaml",
            r#"
kind: cluster
id: sdk_explicit_stop
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 3.0
edges: []
outputs:
  value_out: src.value
"#,
        );
        let hello = serde_json::to_string(&serde_json::json!({
            "type":"hello",
            "protocol":"ergo-driver.v0"
        }))?;
        let driver = write_file(
            &root,
            "driver.sh",
            &format!("#!/bin/sh\nprintf '%s\\n' '{hello}'\nexec sleep 5\n"),
        );
        let capture = root.join("capture.json");
        let stop = StopHandle::new();
        let stop_clone = stop.clone();
        let stopper = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            stop_clone.stop();
        });

        let err = Ergo::builder()
            .build()?
            .run_with_stop(
                RunConfig::new(
                    graph,
                    IngressConfig::process(["/bin/sh".to_string(), driver.display().to_string()]),
                )
                .capture_output(&capture),
                stop,
            )
            .expect_err("zero-event host stop should surface an SDK error");

        stopper.join().expect("stopper thread must join");
        match err {
            ErgoRunError::Host(HostRunError::StepFailed(message)) => {
                assert!(
                    message.contains("host stop requested before first committed event"),
                    "unexpected error message: {message}"
                );
            }
            other => panic!("expected host stop StepFailed error, got {other:?}"),
        }
        assert!(
            !capture.exists(),
            "zero-event stop must not write a capture artifact"
        );

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn run_profile_discovers_project_and_clusters() -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("project_run");
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
capture_output = "captures/historical.capture.json"
"#,
        );
        write_file(
            &root,
            "graphs/strategy.yaml",
            r#"
kind: cluster
id: sdk_project_graph
version: "0.1.0"
nodes:
  shared:
    cluster: shared_value@0.1.0
edges: []
outputs:
  result: shared.value
"#,
        );
        write_file(
            &root,
            "clusters/shared_value.yaml",
            r#"
kind: cluster
id: shared_value
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 3.0
edges: []
outputs:
  value: src.value
"#,
        );
        write_file(
            &root,
            "fixtures/historical.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

        let outcome = Ergo::from_project(root.join("graphs"))
            .build()?
            .run_profile("historical")?;

        let capture = root.join("captures/historical.capture.json");
        match outcome {
            RunOutcome::Completed(summary) => {
                assert_eq!(summary.events, 1);
                assert_eq!(summary.capture_path, capture);
            }
            other => panic!("expected completed run, got {other:?}"),
        }

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn run_profile_with_stop_honors_profile_max_events() -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("project_profile_bounds");
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
capture_output = "captures/historical.capture.json"
max_events = 1
"#,
        );
        write_file(
            &root,
            "graphs/strategy.yaml",
            r#"
kind: cluster
id: sdk_profile_bounds
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 3.0
edges: []
outputs:
  result: src.value
"#,
        );
        write_file(
            &root,
            "fixtures/historical.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

        let outcome = Ergo::from_project(&root)
            .build()?
            .run_profile_with_stop("historical", StopHandle::new())?;

        let capture = root.join("captures/historical.capture.json");
        match outcome {
            RunOutcome::Interrupted(interrupted) => {
                assert_eq!(interrupted.reason, InterruptionReason::HostStopRequested);
                assert_eq!(interrupted.summary.events, 1);
                assert_eq!(interrupted.summary.capture_path, capture);
            }
            other => panic!("expected interrupted host-stop outcome, got {other:?}"),
        }

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn replay_profile_reuses_project_resolution() -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("project_replay");
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
capture_output = "captures/historical.capture.json"
"#,
        );
        write_file(
            &root,
            "graphs/strategy.yaml",
            r#"
kind: cluster
id: sdk_replay_graph
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.0
edges: []
outputs:
  value_out: src.value
"#,
        );
        write_file(
            &root,
            "fixtures/historical.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

        let capture = root.join("captures/historical.capture.json");
        let _ = Ergo::from_project(&root)
            .build()?
            .run_profile("historical")?;

        let replay = Ergo::from_project(&root)
            .build()?
            .replay_profile("historical", &capture)?;

        assert_eq!(replay.graph_id.as_str(), "sdk_replay_graph");
        assert_eq!(replay.events, 1);

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn validate_project_rejects_profile_missing_required_adapter(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("validate_missing_adapter");
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
fixture = "fixtures/live.jsonl"
"#,
        );
        write_file(
            &root,
            "graphs/strategy.yaml",
            r#"
kind: cluster
id: sdk_validate_missing_adapter
version: "0.1.0"
nodes:
  gate:
    impl: const_bool@0.1.0
    params:
      value: true
  emit:
    impl: emit_if_true@0.1.0
  qty:
    impl: injected_number_source@0.1.0
  place:
    impl: injected_intent_action@0.1.0
edges:
  - "gate.value -> emit.input"
  - "emit.event -> place.event"
  - "qty.value -> place.qty"
outputs:
  outcome: place.outcome
"#,
        );
        write_file(
            &root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

        let err = Ergo::from_project(&root)
            .add_source(InjectedNumberSource::new(4.0))
            .add_action(InjectedIntentAction::new())
            .build()?
            .validate_project()
            .expect_err("adapter-less intent profile must fail validation");

        match err {
            ErgoValidateError::Validation { profile, detail } => {
                assert_eq!(profile, "live");
                assert!(detail.contains("adapter"));
            }
            other => panic!("unexpected error: {other}"),
        }

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn validate_project_accepts_adapter_and_egress_profile(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("validate_egress");
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
adapter = "adapters/trading.yaml"
fixture = "fixtures/live.jsonl"
egress = "egress/live.toml"
"#,
        );
        write_file(
            &root,
            "graphs/strategy.yaml",
            r#"
kind: cluster
id: sdk_validate_egress
version: "0.1.0"
nodes:
  gate:
    impl: const_bool@0.1.0
    params:
      value: true
  emit:
    impl: emit_if_true@0.1.0
  qty:
    impl: injected_number_source@0.1.0
  place:
    impl: injected_intent_action@0.1.0
edges:
  - "gate.value -> emit.input"
  - "emit.event -> place.event"
  - "qty.value -> place.qty"
outputs:
  outcome: place.outcome
"#,
        );
        write_file(
            &root,
            "adapters/trading.yaml",
            r#"
kind: adapter
id: sdk_trading_adapter
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: last_qty
    type: Number
    required: false
    writable: true
event_kinds:
  - name: price_bar
    payload_schema:
      type: object
      properties:
        price: { type: number }
      additionalProperties: false
accepts:
  effects:
    - name: set_context
      payload_schema:
        type: object
        additionalProperties: false
    - name: place_order
      payload_schema:
        type: object
        properties:
          qty: { type: number }
        required: [qty]
        additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.price_bar
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
"#,
        );
        write_file(
            &root,
            "egress/live.toml",
            r#"
default_ack_timeout = "5s"

[channels.broker]
type = "process"
command = ["python3", "channels/egress/broker.py"]

[routes.place_order]
channel = "broker"
ack_timeout = "10s"
"#,
        );
        write_file(
            &root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"semantic_kind\":\"price_bar\",\"payload\":{\"price\":101.25}}}\n",
        );

        let summary = Ergo::from_project(&root)
            .add_source(InjectedNumberSource::new(4.0))
            .add_action(InjectedIntentAction::new())
            .build()?
            .validate_project()?;

        assert_eq!(summary.name, "sdk-project");
        assert_eq!(summary.profiles, vec!["live".to_string()]);

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn validate_project_reports_invalid_egress_config_as_profile_validation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("validate_invalid_egress_config");
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
fixture = "fixtures/live.jsonl"
egress = "egress/live.toml"
"#,
        );
        write_file(
            &root,
            "graphs/strategy.yaml",
            r#"
kind: cluster
id: sdk_validate_invalid_egress
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 4.0
edges: []
outputs:
  result: src.value
"#,
        );
        write_file(
            &root,
            "egress/live.toml",
            r#"
default_ack_timeout = "5s"

[channels.broker]
type = "process"
command = ["oops"
"#,
        );
        write_file(
            &root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

        let err = Ergo::from_project(&root)
            .build()?
            .validate_project()
            .expect_err("invalid egress config must fail profile validation");

        match err {
            ErgoValidateError::Validation { profile, detail } => {
                assert_eq!(profile, "live");
                assert!(detail.contains("failed to parse egress config"));
            }
            other => panic!("unexpected invalid-egress validation error: {other}"),
        }

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn validate_project_surfaces_runtime_owned_cluster_version_details(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("validate_cluster_version_miss");
        fs::create_dir_all(root.join("clusters")).expect("create clusters dir");

        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
"#,
        );
        write_file(
            &root,
            "graphs/strategy.yaml",
            r#"
kind: cluster
id: sdk_cluster_version_miss
version: "0.1.0"
nodes:
  shared:
    cluster: shared_value@^2.0
edges: []
outputs:
  result: shared.value
"#,
        );
        write_file(
            &root,
            "graphs/shared_value.yaml",
            r#"
kind: cluster
id: shared_value
version: "1.5.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 4.0
edges: []
outputs:
  value: src.value
"#,
        );
        write_file(
            &root,
            "clusters/shared_value.yaml",
            r#"
kind: cluster
id: shared_value
version: "1.0.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 3.0
edges: []
outputs:
  value: src.value
"#,
        );
        write_file(
            &root,
            "fixtures/historical.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

        let err = Ergo::from_project(&root)
            .build()?
            .validate_project()
            .expect_err("version-miss cluster profile must fail validation");

        match err {
            ErgoValidateError::Validation { profile, detail } => {
                assert_eq!(profile, "historical");
                assert!(detail.contains("graph expansion failed"));
                assert!(!detail.contains("cluster discovery failed"));
                assert!(detail.contains("shared_value"));
                assert!(detail.contains("^2.0"));
                assert!(detail.contains("available: 1.0.0, 1.5.0"));
                assert!(detail.contains("available cluster files"));
                assert!(detail.contains("shared_value@1.0.0"));
                assert!(detail.contains("shared_value@1.5.0"));
            }
            other => panic!("unexpected error: {other}"),
        }

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn ergo_handle_can_run_the_same_profile_twice() -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("reuse_run_profile");
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
capture_output = "captures/historical.capture.json"
"#,
        );
        write_file(
            &root,
            "graphs/strategy.yaml",
            r#"
kind: cluster
id: sdk_reuse_run_profile
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 4.0
edges: []
outputs:
  result: src.value
"#,
        );
        write_file(
            &root,
            "fixtures/historical.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

        let ergo = Ergo::from_project(&root).build()?;
        let first = ergo.run_profile("historical")?;
        let second = ergo.run_profile("historical")?;

        match (first, second) {
            (RunOutcome::Completed(first), RunOutcome::Completed(second)) => {
                assert_eq!(first.events, 1);
                assert_eq!(second.events, 1);
                assert_eq!(first.capture_path, second.capture_path);
            }
            other => panic!("expected two completed runs, got {other:?}"),
        }

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn ergo_handle_survives_errors_and_can_validate_run_and_replay(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("reuse_validate_run_replay");
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
capture_output = "captures/historical.capture.json"
"#,
        );
        write_file(
            &root,
            "graphs/strategy.yaml",
            r#"
kind: cluster
id: sdk_reuse_validate_run_replay
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 5.0
edges: []
outputs:
  result: src.value
"#,
        );
        write_file(
            &root,
            "fixtures/historical.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

        let ergo = Ergo::from_project(&root).build()?;
        let err = ergo
            .run_profile("missing")
            .expect_err("missing profile should not consume the handle");
        assert!(
            matches!(err, ErgoRunError::Project(ProjectError::Load(_))),
            "unexpected missing-profile error: {err:?}"
        );

        let summary = ergo.validate_project()?;
        assert_eq!(summary.profiles, vec!["historical".to_string()]);

        let outcome = ergo.run_profile("historical")?;
        let capture = root.join("captures/historical.capture.json");
        match outcome {
            RunOutcome::Completed(summary) => {
                assert_eq!(summary.capture_path, capture);
                assert_eq!(summary.events, 1);
            }
            other => panic!("expected completed run, got {other:?}"),
        }

        let replay = ergo.replay_profile("historical", &capture)?;
        assert_eq!(replay.graph_id.as_str(), "sdk_reuse_validate_run_replay");
        assert_eq!(replay.events, 1);

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn runner_for_profile_supports_multiple_steps_without_launching_ingress_or_auto_writing_capture(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("manual_runner_no_ingress");
        let ingress_sentinel = root.join("ingress-started.txt");
        let ingress_script = write_process_ingress_sentinel(&root, &ingress_sentinel);
        write_file(
            &root,
            "ergo.toml",
            &format!(
                r#"
name = "sdk-project"
version = "0.1.0"

[profiles.manual]
graph = "graphs/strategy.yaml"
capture_output = "captures/manual.capture.json"
max_duration = "1ms"
max_events = 1

[profiles.manual.ingress]
type = "process"
command = ["/bin/sh", "{ingress_script}"]
"#,
                ingress_script = ingress_script.display()
            ),
        );
        write_file(
            &root,
            "graphs/strategy.yaml",
            r#"
kind: cluster
id: sdk_manual_runner_no_ingress
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 6.0
edges: []
outputs:
  result: src.value
"#,
        );

        let ergo = Ergo::from_project(&root).build()?;
        let mut runner = ergo.runner_for_profile("manual")?;
        let capture = root.join("captures/manual.capture.json");
        assert!(
            !ingress_sentinel.exists(),
            "manual runner must not launch ingress"
        );

        let first = runner.step(manual_step_event("e1"))?;
        assert_eq!(first.termination, Some(RunTermination::Completed));
        thread::sleep(Duration::from_millis(10));
        let second = runner.step(manual_step_event("e2"))?;
        assert_eq!(second.termination, Some(RunTermination::Completed));

        let bundle = runner.finish()?;
        assert_eq!(bundle.events.len(), 2);
        assert_eq!(bundle.decisions.len(), 2);
        assert!(
            !capture.exists(),
            "manual finish should return a bundle without auto-writing capture_output"
        );
        assert!(
            !ingress_sentinel.exists(),
            "manual runner must not launch ingress"
        );

        let err = runner
            .step(manual_step_event("e3"))
            .expect_err("step after finish must fail");
        assert!(
            matches!(err, HostedStepError::LifecycleViolation { .. }),
            "unexpected step-after-finish error: {err:?}"
        );

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn runner_for_profile_can_finish_and_write_capture_with_profile_settings(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("manual_runner_write_capture");
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.manual]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
capture_output = "captures/manual.capture.json"
pretty_capture = true
"#,
        );
        write_file(
            &root,
            "graphs/strategy.yaml",
            r#"
kind: cluster
id: sdk_manual_runner_write_capture
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 7.0
edges: []
outputs:
  result: src.value
"#,
        );
        write_file(
            &root,
            "fixtures/historical.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

        let ergo = Ergo::from_project(&root).build()?;
        let mut runner = ergo.runner_for_profile("manual")?;
        let capture = root.join("captures/manual.capture.json");
        let outcome = runner.step(manual_step_event("e1"))?;
        assert_eq!(outcome.termination, Some(RunTermination::Completed));
        let bundle = runner.finish_and_write_capture()?;
        let persisted: CaptureBundle = serde_json::from_str(&fs::read_to_string(&capture)?)?;

        assert!(
            capture.exists(),
            "finish_and_write_capture should write capture_output"
        );
        assert_eq!(bundle.decisions.len(), 1);
        assert_eq!(persisted.decisions.len(), 1);
        assert!(fs::read_to_string(&capture)?.contains("\n  "));

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn runner_for_profile_preserves_bundle_when_capture_write_fails(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("manual_runner_write_capture_failure");
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.manual]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
capture_output = "captures/manual.capture.json"
"#,
        );
        write_file(
            &root,
            "graphs/strategy.yaml",
            r#"
kind: cluster
id: sdk_manual_runner_write_capture_failure
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 7.0
edges: []
outputs:
  result: src.value
"#,
        );
        write_file(
            &root,
            "fixtures/historical.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );
        write_file(&root, "captures", "not-a-directory");

        let ergo = Ergo::from_project(&root).build()?;
        let mut runner = ergo.runner_for_profile("manual")?;
        let outcome = runner.step(manual_step_event("e1"))?;
        assert_eq!(outcome.termination, Some(RunTermination::Completed));

        let err = runner
            .finish_and_write_capture()
            .expect_err("capture write failure should preserve bundle in the error");

        match &err {
            ProfileRunnerCaptureError::Write { detail, bundle } => {
                assert!(detail.contains("create capture output directory"));
                assert_eq!(bundle.decisions.len(), 1);
            }
            other => panic!("unexpected write-failure error: {other}"),
        }
        assert_eq!(
            err.capture_bundle()
                .expect("write failure should expose recovered bundle")
                .decisions
                .len(),
            1
        );

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn runner_for_profile_still_requires_a_declared_ingress_source(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("manual_runner_requires_ingress");
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.manual]
graph = "graphs/strategy.yaml"
"#,
        );
        write_file(
            &root,
            "graphs/strategy.yaml",
            r#"
kind: cluster
id: sdk_manual_runner_requires_ingress
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 8.0
edges: []
outputs:
  result: src.value
"#,
        );

        let ergo = Ergo::from_project(&root).build()?;
        let err = match ergo.runner_for_profile("manual") {
            Ok(_) => panic!("profile resolution should still require ingress"),
            Err(err) => err,
        };
        match err {
            ErgoRunnerError::Project(ProjectError::Load(LoaderProjectError::ProfileInvalid {
                detail,
                ..
            })) => assert!(detail.contains("exactly one ingress source")),
            other => panic!("unexpected runner_for_profile ingress error: {other:?}"),
        }

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn runner_for_profile_preserves_adapter_required_preflight(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("manual_runner_adapter_preflight");
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
fixture = "fixtures/live.jsonl"
"#,
        );
        write_intent_graph(&root, "sdk_manual_runner_adapter_preflight");
        write_file(
            &root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

        let ergo = Ergo::from_project(&root)
            .add_source(InjectedNumberSource::new(4.0))
            .add_action(InjectedIntentAction::new())
            .build()?;
        let err = match ergo.runner_for_profile("live") {
            Ok(_) => panic!("adapter-required profile should fail before returning runner"),
            Err(err) => err,
        };
        match err {
            ErgoRunnerError::Host(HostRunError::AdapterRequired(summary)) => {
                assert!(summary.requires_adapter);
            }
            other => panic!("unexpected adapter-preflight error: {other:?}"),
        }

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn runner_for_profile_surfaces_egress_startup_failure_at_creation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("manual_runner_egress_startup");
        let missing_binary = "/definitely/missing-egress-binary".to_string();
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
adapter = "adapters/trading.yaml"
fixture = "fixtures/live.jsonl"
egress = "egress/live.toml"
"#,
        );
        write_intent_graph(&root, "sdk_manual_runner_egress_startup");
        write_intent_adapter_manifest(&root);
        write_egress_config(&root, vec![missing_binary]);
        write_file(
            &root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

        let ergo = Ergo::from_project(&root)
            .add_source(InjectedNumberSource::new(4.0))
            .add_action(InjectedIntentAction::new())
            .build()?;
        let err = match ergo.runner_for_profile("live") {
            Ok(_) => panic!("egress startup failure should surface during runner creation"),
            Err(err) => err,
        };
        assert!(
            matches!(err, ErgoRunnerError::Host(HostRunError::DriverIo(_))),
            "unexpected startup error: {err:?}"
        );

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn profile_runner_zero_step_finish_fails_but_recoverable_input_errors_do_not_poison_session(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let zero_root = make_temp_dir("manual_runner_zero_step");
        write_file(
            &zero_root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.manual]
graph = "graphs/strategy.yaml"
fixture = "fixtures/live.jsonl"
"#,
        );
        write_file(
            &zero_root,
            "graphs/strategy.yaml",
            r#"
kind: cluster
id: sdk_manual_runner_zero_step
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 9.0
edges: []
outputs:
  result: src.value
"#,
        );
        write_file(
            &zero_root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

        let ergo = Ergo::from_project(&zero_root).build()?;
        let mut zero_runner = ergo.runner_for_profile("manual")?;
        let zero_err = zero_runner
            .finish()
            .expect_err("zero-step finish must fail");
        assert!(
            matches!(zero_err, HostedStepError::LifecycleViolation { .. }),
            "unexpected zero-step finish error: {zero_err:?}"
        );
        let _ = fs::remove_dir_all(&zero_root);

        let failure_root = make_temp_dir("manual_runner_recoverable_input_error");
        write_file(
            &failure_root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
adapter = "adapters/trading.yaml"
fixture = "fixtures/live.jsonl"
egress = "egress/live.toml"
"#,
        );
        write_intent_graph(&failure_root, "sdk_manual_runner_nonfinalizable");
        write_intent_adapter_manifest(&failure_root);
        let egress_script = write_egress_ack_script(&failure_root);
        write_egress_config(
            &failure_root,
            vec!["/bin/sh".to_string(), egress_script.display().to_string()],
        );
        write_file(
            &failure_root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

        let ergo = Ergo::from_project(&failure_root)
            .add_source(InjectedNumberSource::new(4.0))
            .add_action(InjectedIntentAction::new())
            .build()?;
        let mut runner = ergo.runner_for_profile("live")?;
        let step_err = runner
            .step(HostedEvent {
                event_id: "evt_bad".to_string(),
                kind: ExternalEventKind::Command,
                at: EventTime::default(),
                semantic_kind: None,
                payload: Some(serde_json::json!({"price": 100.0})),
            })
            .expect_err("missing semantic kind should surface a recoverable input error");
        assert!(matches!(step_err, HostedStepError::MissingSemanticKind));

        let recovered = runner.step(adapter_bound_event("evt_good", 101.5))?;
        assert_eq!(recovered.termination, Some(RunTermination::Completed));

        let bundle = runner.finish()?;
        assert_eq!(bundle.events.len(), 1);
        assert_eq!(bundle.decisions.len(), 1);

        let _ = fs::remove_dir_all(failure_root);
        Ok(())
    }

    #[test]
    fn profile_runner_can_finalize_after_egress_dispatch_failure(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("manual_runner_dispatch_failure");
        let egress_script = write_egress_io_script(&root);
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
adapter = "adapters/trading.yaml"
fixture = "fixtures/live.jsonl"
egress = "egress/live.toml"
"#,
        );
        write_intent_graph(&root, "sdk_manual_runner_dispatch_failure");
        write_intent_adapter_manifest(&root);
        write_egress_config(
            &root,
            vec!["/bin/sh".to_string(), egress_script.display().to_string()],
        );
        write_file(
            &root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

        let ergo = Ergo::from_project(&root)
            .add_source(InjectedNumberSource::new(4.0))
            .add_action(InjectedIntentAction::new())
            .build()?;
        let mut runner = ergo.runner_for_profile("live")?;
        let step_err = runner
            .step(adapter_bound_event("evt1", 101.5))
            .expect_err("egress dispatch failure should interrupt manual stepping");
        assert!(
            matches!(step_err, HostedStepError::EgressDispatchFailure(_)),
            "unexpected dispatch failure: {step_err:?}"
        );

        let step_again = runner
            .step(adapter_bound_event("evt2", 101.6))
            .expect_err("runner should require finalization after dispatch failure");
        assert!(
            matches!(step_again, HostedStepError::LifecycleViolation { .. }),
            "unexpected post-dispatch step result: {step_again:?}"
        );

        let bundle = runner.finish()?;
        assert_eq!(bundle.events.len(), 1);
        assert_eq!(bundle.decisions.len(), 1);
        assert!(bundle.decisions[0].interruption.is_some());

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn profile_runner_dispatches_egress_and_finishes_with_a_bundle(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("manual_runner_egress_success");
        let egress_script = write_egress_ack_script(&root);
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
adapter = "adapters/trading.yaml"
fixture = "fixtures/live.jsonl"
egress = "egress/live.toml"
capture_output = "captures/live.capture.json"
"#,
        );
        write_intent_graph(&root, "sdk_manual_runner_egress_success");
        write_intent_adapter_manifest(&root);
        write_egress_config(
            &root,
            vec!["/bin/sh".to_string(), egress_script.display().to_string()],
        );
        write_file(
            &root,
            "fixtures/live.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

        let ergo = Ergo::from_project(&root)
            .add_source(InjectedNumberSource::new(4.0))
            .add_action(InjectedIntentAction::new())
            .build()?;
        let mut runner = ergo.runner_for_profile("live")?;
        let outcome = runner.step(adapter_bound_event("evt1", 101.5))?;
        assert_eq!(outcome.termination, Some(RunTermination::Completed));

        let bundle = runner.finish()?;
        assert_eq!(bundle.events.len(), 1);
        assert_eq!(bundle.decisions.len(), 1);
        assert_eq!(bundle.decisions[0].intent_acks.len(), 1);
        assert!(
            !root.join("captures/live.capture.json").exists(),
            "manual finish should not auto-write capture_output"
        );

        let _ = fs::remove_dir_all(root);
        Ok(())
    }
}
