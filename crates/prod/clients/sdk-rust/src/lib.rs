//! Rust SDK over Ergo host + loader.
//!
//! The SDK is the primary product surface for building an Ergo engine
//! inside a Rust crate. It wraps the existing canonical host run and
//! replay paths without introducing a second execution model.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub use ergo_adapter::fixture::FixtureItem;
use ergo_host::{
    finalize_hosted_runner_capture, load_graph_assets_from_paths, parse_egress_config_toml,
    prepare_hosted_runner_from_paths_with_surfaces, prepare_hosted_runner_with_surfaces,
    replay_graph_from_assets_with_surfaces, replay_graph_from_paths_with_surfaces,
    run_graph_from_assets_with_surfaces_and_control,
    run_graph_from_paths_with_surfaces_and_control, validate_run_graph_from_assets_with_surfaces,
    validate_run_graph_from_paths_with_surfaces, DriverConfig, HostReplayError, HostRunError,
    HostStopHandle, HostedRunner, LivePrepOptions, PrepareHostedRunnerFromPathsRequest,
    ReplayGraphFromAssetsRequest, ReplayGraphFromPathsRequest, ReplayGraphResult, RunControl,
    RunGraphFromAssetsRequest, RunGraphFromPathsRequest, RunOutcome, RuntimeSurfaces,
};
use ergo_loader::{
    load_project, PreparedGraphAssets, ProjectError as LoaderProjectError, ResolvedProject,
    ResolvedProjectIngress, ResolvedProjectProfile,
};
use ergo_runtime::catalog::{CatalogBuilder, CoreRegistrationError};

pub use ergo_host::{
    write_capture_bundle, AdapterInput, CaptureBundle, CaptureJsonStyle, EgressChannelConfig,
    EgressConfig, EgressDispatchFailure, EgressRoute, HostedEvent, HostedStepError,
    HostedStepOutcome, InterruptedRun, InterruptionReason, RunSummary,
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
    ProjectConfig(String),
    ProjectSourceConflict,
}

impl std::fmt::Display for ErgoBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Registration(detail) => write!(f, "{detail}"),
            Self::ProjectConfig(detail) => write!(f, "{detail}"),
            Self::ProjectSourceConflict => {
                write!(
                    f,
                    "project_root and in_memory_project are mutually exclusive"
                )
            }
        }
    }
}

impl std::error::Error for ErgoBuildError {}

#[derive(Debug)]
pub enum ProjectError {
    ProjectNotConfigured,
    ProfileNotFound {
        name: String,
    },
    ConfigInvalid {
        detail: String,
    },
    ProjectLoad {
        detail: String,
    },
    UnsupportedOperation {
        operation: &'static str,
        transport: &'static str,
    },
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProjectNotConfigured => {
                write!(
                    f,
                    "a project source must be configured for project/profile operations"
                )
            }
            Self::ProfileNotFound { name } => write!(f, "project profile '{name}' does not exist"),
            Self::ConfigInvalid { detail } => write!(f, "{detail}"),
            Self::ProjectLoad { detail } => write!(f, "{detail}"),
            Self::UnsupportedOperation {
                operation,
                transport,
            } => write!(
                f,
                "operation '{operation}' is not supported for {transport} projects"
            ),
        }
    }
}

impl std::error::Error for ProjectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl From<LoaderProjectError> for ProjectError {
    fn from(value: LoaderProjectError) -> Self {
        map_loader_project_error(value)
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
                write!(f, "profile does not declare a capture file path")
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

#[derive(Debug, Clone)]
pub struct ReplayBundleConfig {
    bundle: CaptureBundle,
    graph_path: PathBuf,
    cluster_paths: Vec<PathBuf>,
    adapter: Option<AdapterInput>,
}

impl ReplayBundleConfig {
    pub fn new(bundle: CaptureBundle, graph_path: impl AsRef<Path>) -> Self {
        Self {
            bundle,
            graph_path: graph_path.as_ref().to_path_buf(),
            cluster_paths: Vec::new(),
            adapter: None,
        }
    }

    pub fn cluster_path(mut self, path: impl AsRef<Path>) -> Self {
        self.cluster_paths.push(path.as_ref().to_path_buf());
        self
    }

    pub fn adapter_path(mut self, path: impl AsRef<Path>) -> Self {
        self.adapter = Some(AdapterInput::Path(path.as_ref().to_path_buf()));
        self
    }

    pub fn adapter(mut self, adapter: AdapterInput) -> Self {
        self.adapter = Some(adapter);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSummary {
    pub root: Option<PathBuf>,
    pub name: String,
    pub version: String,
    pub profiles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileCapture {
    InMemory,
    File { path: PathBuf, pretty: bool },
}

#[derive(Debug, Clone)]
pub struct InMemoryProfileConfig {
    graph_assets: PreparedGraphAssets,
    ingress: InMemoryIngress,
    adapter: Option<AdapterInput>,
    egress_config: Option<EgressConfig>,
    capture: ProfileCapture,
    max_duration: Option<Duration>,
    max_events: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct InMemoryProjectSnapshot {
    name: String,
    version: String,
    profiles: BTreeMap<String, InMemoryProfileConfig>,
}

#[derive(Debug, Clone)]
enum InMemoryIngress {
    FixtureItems {
        items: Vec<FixtureItem>,
        source_label: String,
    },
    Process {
        command: Vec<String>,
    },
}

impl ProfileCapture {
    pub fn in_memory() -> Self {
        Self::InMemory
    }

    pub fn file(path: impl AsRef<Path>, pretty: bool) -> Self {
        Self::File {
            path: path.as_ref().to_path_buf(),
            pretty,
        }
    }
}

impl InMemoryProfileConfig {
    pub fn process<I, S>(
        graph_assets: PreparedGraphAssets,
        command: I,
    ) -> Result<Self, ProjectError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let config = Self {
            graph_assets,
            ingress: InMemoryIngress::Process {
                command: command.into_iter().map(Into::into).collect(),
            },
            adapter: None,
            egress_config: None,
            capture: ProfileCapture::InMemory,
            max_duration: None,
            max_events: None,
        };
        validate_in_memory_profile_config(None, &config)?;
        Ok(config)
    }

    pub fn fixture_items(
        graph_assets: PreparedGraphAssets,
        items: Vec<FixtureItem>,
        source_label: impl Into<String>,
    ) -> Result<Self, ProjectError> {
        let config = Self {
            graph_assets,
            ingress: InMemoryIngress::FixtureItems {
                items,
                source_label: source_label.into(),
            },
            adapter: None,
            egress_config: None,
            capture: ProfileCapture::InMemory,
            max_duration: None,
            max_events: None,
        };
        validate_in_memory_profile_config(None, &config)?;
        Ok(config)
    }

    pub fn adapter(mut self, adapter: AdapterInput) -> Self {
        self.adapter = Some(adapter);
        self
    }

    pub fn egress_config(mut self, egress_config: EgressConfig) -> Self {
        self.egress_config = Some(egress_config);
        self
    }

    pub fn capture(mut self, capture: ProfileCapture) -> Self {
        self.capture = capture;
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

#[derive(Debug, Clone)]
pub struct InMemoryProjectSnapshotBuilder {
    name: String,
    version: String,
    profiles: BTreeMap<String, InMemoryProfileConfig>,
}

impl InMemoryProjectSnapshotBuilder {
    pub fn profile(mut self, name: impl Into<String>, profile: InMemoryProfileConfig) -> Self {
        self.profiles.insert(name.into(), profile);
        self
    }

    pub fn build(self) -> Result<InMemoryProjectSnapshot, ProjectError> {
        InMemoryProjectSnapshot::from_parts(self.name, self.version, self.profiles)
    }
}

impl InMemoryProjectSnapshot {
    pub fn builder(
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> InMemoryProjectSnapshotBuilder {
        InMemoryProjectSnapshotBuilder {
            name: name.into(),
            version: version.into(),
            profiles: BTreeMap::new(),
        }
    }

    pub fn profile_names(&self) -> Vec<String> {
        self.profiles.keys().cloned().collect()
    }

    fn resolve_profile(&self, name: &str) -> Result<&InMemoryProfileConfig, ProjectError> {
        self.profiles
            .get(name)
            .ok_or_else(|| ProjectError::ProfileNotFound {
                name: name.to_string(),
            })
    }

    fn from_parts(
        name: String,
        version: String,
        profiles: BTreeMap<String, InMemoryProfileConfig>,
    ) -> Result<Self, ProjectError> {
        if profiles.is_empty() {
            return Err(ProjectError::ConfigInvalid {
                detail: "in-memory project snapshot must declare at least one profile".to_string(),
            });
        }
        for (profile_name, profile) in &profiles {
            validate_in_memory_profile_config(Some(profile_name.as_str()), profile)?;
        }
        Ok(Self {
            name,
            version,
            profiles,
        })
    }

    fn validate(&self) -> Result<(), ProjectError> {
        let _ = Self::from_parts(
            self.name.clone(),
            self.version.clone(),
            self.profiles.clone(),
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
enum ProjectSource {
    Filesystem(PathBuf),
    InMemory(InMemoryProjectSnapshot),
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CapturePlan {
    DefaultFile { pretty: bool },
    ExplicitFile { path: PathBuf, pretty: bool },
    InMemory,
}

impl CapturePlan {
    fn configured_path(&self) -> Option<&Path> {
        match self {
            Self::ExplicitFile { path, .. } => Some(path.as_path()),
            Self::DefaultFile { .. } | Self::InMemory => None,
        }
    }

    fn pretty(&self) -> bool {
        match self {
            Self::DefaultFile { pretty } | Self::ExplicitFile { pretty, .. } => *pretty,
            Self::InMemory => false,
        }
    }
}

enum RunnerSource {
    Paths(PrepareHostedRunnerFromPathsRequest),
    Assets {
        assets: PreparedGraphAssets,
        prep: LivePrepOptions,
    },
}

struct ResolvedProfilePlan {
    profile_name: String,
    runner_source: RunnerSource,
    driver: DriverConfig,
    capture: CapturePlan,
    max_duration: Option<Duration>,
    max_events: Option<u64>,
}

pub struct ErgoBuilder {
    catalog_builder: CatalogBuilder,
    project_source: ProjectSource,
    project_source_conflict: bool,
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
            project_source: ProjectSource::None,
            project_source_conflict: false,
        }
    }

    pub fn project_root(mut self, path: impl AsRef<Path>) -> Self {
        self.set_project_source(ProjectSource::Filesystem(path.as_ref().to_path_buf()));
        self
    }

    pub fn in_memory_project(mut self, snapshot: InMemoryProjectSnapshot) -> Self {
        self.set_project_source(ProjectSource::InMemory(snapshot));
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
        if self.project_source_conflict {
            return Err(ErgoBuildError::ProjectSourceConflict);
        }
        if let ProjectSource::InMemory(snapshot) = &self.project_source {
            snapshot
                .validate()
                .map_err(|err| ErgoBuildError::ProjectConfig(err.to_string()))?;
        }
        let (registries, catalog) = self
            .catalog_builder
            .build()
            .map_err(format_registration_error)?;
        Ok(Ergo {
            runtime_surfaces: RuntimeSurfaces::new(registries, catalog),
            project_source: self.project_source,
        })
    }

    fn set_project_source(&mut self, source: ProjectSource) {
        if matches!(
            (&self.project_source, &source),
            (ProjectSource::Filesystem(_), ProjectSource::InMemory(_))
                | (ProjectSource::InMemory(_), ProjectSource::Filesystem(_))
        ) {
            self.project_source_conflict = true;
        }
        self.project_source = source;
    }
}

/// Built Ergo engine handle.
///
/// One built `Ergo` handle may be reused for multiple operations on the
/// same thread. Reuse preserves the existing primitive instances behind
/// the handle under the current in-process trust model.
pub struct Ergo {
    runtime_surfaces: RuntimeSurfaces,
    project_source: ProjectSource,
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
        let plan = self
            .resolve_profile_plan(profile_name)
            .map_err(ErgoRunError::Project)?;
        self.run_profile_plan_with_control(plan, control)
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

    pub fn replay_bundle(
        &self,
        config: ReplayBundleConfig,
    ) -> Result<ReplayGraphResult, ErgoReplayError> {
        let assets = load_graph_assets_from_paths(&config.graph_path, &config.cluster_paths)
            .map_err(|err| {
                ErgoReplayError::Project(ProjectError::ConfigInvalid {
                    detail: err.to_string(),
                })
            })?;
        replay_graph_from_assets_with_surfaces(
            ReplayGraphFromAssetsRequest {
                bundle: config.bundle,
                assets,
                prep: LivePrepOptions {
                    adapter: config.adapter,
                    egress_config: None,
                },
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
        let plan = self
            .resolve_profile_plan(profile_name)
            .map_err(ErgoReplayError::Project)?;
        match plan.runner_source {
            RunnerSource::Paths(request) => self.replay(ReplayConfig {
                capture_path: capture_path.as_ref().to_path_buf(),
                graph_path: request.graph_path,
                cluster_paths: request.cluster_paths,
                adapter_path: request.adapter_path,
            }),
            RunnerSource::Assets { .. } => Err(ErgoReplayError::Project(
                ProjectError::UnsupportedOperation {
                    operation: "replay_profile",
                    transport: "in-memory",
                },
            )),
        }
    }

    pub fn replay_profile_bundle(
        &self,
        profile_name: &str,
        bundle: CaptureBundle,
    ) -> Result<ReplayGraphResult, ErgoReplayError> {
        let plan = self
            .resolve_profile_plan(profile_name)
            .map_err(ErgoReplayError::Project)?;
        self.replay_profile_bundle_from_plan(plan, bundle)
    }

    pub fn validate_project(&self) -> Result<ProjectSummary, ErgoValidateError> {
        match &self.project_source {
            ProjectSource::Filesystem(_) => {
                let project = self.load_project().map_err(ErgoValidateError::Project)?;
                let profiles = project.profile_names();
                for profile_name in &profiles {
                    let plan = Self::resolve_profile_plan_from_project(&project, profile_name)
                        .map_err(|err| ErgoValidateError::Validation {
                            profile: profile_name.clone(),
                            detail: err.to_string(),
                        })?;
                    self.validate_profile_plan(&plan).map_err(|err| {
                        ErgoValidateError::Validation {
                            profile: profile_name.clone(),
                            detail: err.to_string(),
                        }
                    })?;
                }

                Ok(ProjectSummary {
                    root: Some(project.root.clone()),
                    name: project.manifest.name,
                    version: project.manifest.version,
                    profiles,
                })
            }
            ProjectSource::InMemory(project) => {
                let profiles = project.profile_names();
                for profile_name in &profiles {
                    let plan = self.resolve_profile_plan(profile_name).map_err(|err| {
                        ErgoValidateError::Validation {
                            profile: profile_name.clone(),
                            detail: err.to_string(),
                        }
                    })?;
                    self.validate_profile_plan(&plan).map_err(|err| {
                        ErgoValidateError::Validation {
                            profile: profile_name.clone(),
                            detail: err.to_string(),
                        }
                    })?;
                }

                Ok(ProjectSummary {
                    root: None,
                    name: project.name.clone(),
                    version: project.version.clone(),
                    profiles,
                })
            }
            ProjectSource::None => Err(ErgoValidateError::Project(
                ProjectError::ProjectNotConfigured,
            )),
        }
    }

    fn load_project(&self) -> Result<ResolvedProject, ProjectError> {
        let ProjectSource::Filesystem(project_root) = &self.project_source else {
            return Err(ProjectError::ProjectNotConfigured);
        };
        load_project(project_root).map_err(ProjectError::from)
    }

    fn resolve_profile_plan(
        &self,
        profile_name: &str,
    ) -> Result<ResolvedProfilePlan, ProjectError> {
        match &self.project_source {
            ProjectSource::Filesystem(_) => {
                let project = self.load_project()?;
                Self::resolve_profile_plan_from_project(&project, profile_name)
            }
            ProjectSource::InMemory(project) => {
                let profile = project.resolve_profile(profile_name)?;
                resolve_profile_plan_from_in_memory_profile(profile_name, profile)
            }
            ProjectSource::None => Err(ProjectError::ProjectNotConfigured),
        }
    }

    fn resolve_profile_plan_from_project(
        project: &ResolvedProject,
        profile_name: &str,
    ) -> Result<ResolvedProfilePlan, ProjectError> {
        let resolved = project
            .resolve_run_profile(profile_name)
            .map_err(ProjectError::from)?;
        resolve_profile_plan_from_resolved_profile(profile_name, resolved)
    }

    fn run_profile_plan_with_control(
        &self,
        plan: ResolvedProfilePlan,
        control: RunControl,
    ) -> Result<RunOutcome, ErgoRunError> {
        let ResolvedProfilePlan {
            profile_name,
            runner_source,
            driver,
            capture,
            max_duration,
            max_events,
        } = plan;
        match runner_source {
            RunnerSource::Paths(request) => {
                let (capture_output, pretty_capture) = match capture {
                    CapturePlan::DefaultFile { pretty } => (None, pretty),
                    CapturePlan::ExplicitFile { path, pretty } => (Some(path), pretty),
                    CapturePlan::InMemory => {
                        return Err(ErgoRunError::Project(ProjectError::ConfigInvalid {
                            detail: format!(
                                "filesystem profile '{}' cannot use in-memory capture",
                                profile_name
                            ),
                        }));
                    }
                };
                run_graph_from_paths_with_surfaces_and_control(
                    RunGraphFromPathsRequest {
                        graph_path: request.graph_path,
                        cluster_paths: request.cluster_paths,
                        driver,
                        adapter_path: request.adapter_path,
                        egress_config: request.egress_config,
                        capture_output,
                        pretty_capture,
                    },
                    self.runtime_surfaces.clone(),
                    apply_profile_limits(control, max_duration, max_events),
                )
                .map_err(ErgoRunError::Host)
            }
            RunnerSource::Assets { assets, prep } => {
                run_graph_from_assets_with_surfaces_and_control(
                    RunGraphFromAssetsRequest {
                        assets,
                        prep,
                        driver,
                        capture: host_capture_policy_from_plan(&capture)
                            .map_err(ErgoRunError::Project)?,
                    },
                    self.runtime_surfaces.clone(),
                    apply_profile_limits(control, max_duration, max_events),
                )
                .map_err(ErgoRunError::Host)
            }
        }
    }

    fn validate_profile_plan(&self, plan: &ResolvedProfilePlan) -> Result<(), ProjectError> {
        match (&plan.runner_source, &plan.capture) {
            (RunnerSource::Paths(_request), CapturePlan::InMemory) => {
                Err(ProjectError::ConfigInvalid {
                    detail: format!(
                        "filesystem profile '{}' cannot use in-memory capture",
                        plan.profile_name
                    ),
                })
            }
            (RunnerSource::Paths(request), CapturePlan::DefaultFile { pretty }) => {
                validate_run_graph_from_paths_with_surfaces(
                    RunGraphFromPathsRequest {
                        graph_path: request.graph_path.clone(),
                        cluster_paths: request.cluster_paths.clone(),
                        driver: plan.driver.clone(),
                        adapter_path: request.adapter_path.clone(),
                        egress_config: request.egress_config.clone(),
                        capture_output: None,
                        pretty_capture: *pretty,
                    },
                    self.runtime_surfaces.clone(),
                )
                .map_err(|err| ProjectError::ConfigInvalid {
                    detail: err.to_string(),
                })
            }
            (RunnerSource::Paths(request), CapturePlan::ExplicitFile { path, pretty }) => {
                validate_run_graph_from_paths_with_surfaces(
                    RunGraphFromPathsRequest {
                        graph_path: request.graph_path.clone(),
                        cluster_paths: request.cluster_paths.clone(),
                        driver: plan.driver.clone(),
                        adapter_path: request.adapter_path.clone(),
                        egress_config: request.egress_config.clone(),
                        capture_output: Some(path.clone()),
                        pretty_capture: *pretty,
                    },
                    self.runtime_surfaces.clone(),
                )
                .map_err(|err| ProjectError::ConfigInvalid {
                    detail: err.to_string(),
                })
            }
            (RunnerSource::Assets { assets, prep }, capture) => {
                validate_run_graph_from_assets_with_surfaces(
                    RunGraphFromAssetsRequest {
                        assets: assets.clone(),
                        prep: prep.clone(),
                        driver: plan.driver.clone(),
                        capture: host_capture_policy_from_plan(capture)?,
                    },
                    self.runtime_surfaces.clone(),
                )
                .map_err(|err| ProjectError::ConfigInvalid {
                    detail: err.to_string(),
                })
            }
        }
    }

    fn replay_profile_bundle_from_plan(
        &self,
        plan: ResolvedProfilePlan,
        bundle: CaptureBundle,
    ) -> Result<ReplayGraphResult, ErgoReplayError> {
        match plan.runner_source {
            RunnerSource::Paths(request) => {
                let assets =
                    load_graph_assets_from_paths(&request.graph_path, &request.cluster_paths)
                        .map_err(|err| {
                            ErgoReplayError::Project(ProjectError::ConfigInvalid {
                                detail: err.to_string(),
                            })
                        })?;
                replay_graph_from_assets_with_surfaces(
                    ReplayGraphFromAssetsRequest {
                        bundle,
                        assets,
                        prep: LivePrepOptions {
                            adapter: request.adapter_path.map(AdapterInput::Path),
                            egress_config: None,
                        },
                    },
                    self.runtime_surfaces.clone(),
                )
                .map_err(ErgoReplayError::Host)
            }
            RunnerSource::Assets { assets, prep } => replay_graph_from_assets_with_surfaces(
                ReplayGraphFromAssetsRequest {
                    bundle,
                    assets,
                    prep: LivePrepOptions {
                        adapter: prep.adapter,
                        egress_config: None,
                    },
                },
                self.runtime_surfaces.clone(),
            )
            .map_err(ErgoReplayError::Host),
        }
    }

    pub fn runner_for_profile(&self, profile_name: &str) -> Result<ProfileRunner, ErgoRunnerError> {
        let plan = self
            .resolve_profile_plan(profile_name)
            .map_err(ErgoRunnerError::Project)?;
        let ResolvedProfilePlan {
            runner_source,
            capture,
            ..
        } = plan;
        match runner_source {
            RunnerSource::Paths(request) => {
                let runner = prepare_hosted_runner_from_paths_with_surfaces(
                    request,
                    self.runtime_surfaces.clone(),
                )
                .map_err(ErgoRunnerError::Host)?;
                Ok(ProfileRunner {
                    runner: Some(runner),
                    capture,
                    state: ProfileRunnerState::Active,
                    successful_steps: 0,
                })
            }
            RunnerSource::Assets { assets, prep } => {
                let runner = prepare_hosted_runner_with_surfaces(
                    assets,
                    &prep,
                    self.runtime_surfaces.clone(),
                )
                .map_err(ErgoRunnerError::Host)?;
                Ok(ProfileRunner {
                    runner: Some(runner),
                    capture,
                    state: ProfileRunnerState::Active,
                    successful_steps: 0,
                })
            }
        }
    }
}

pub struct ProfileRunner {
    runner: Option<HostedRunner>,
    capture: CapturePlan,
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
        self.capture.configured_path()
    }

    pub fn pretty_capture(&self) -> bool {
        self.capture.pretty()
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
        let capture_path = match &self.capture {
            CapturePlan::ExplicitFile { path, .. } => path.clone(),
            CapturePlan::DefaultFile { .. } | CapturePlan::InMemory => {
                return Err(ProfileRunnerCaptureError::CaptureOutputNotConfigured);
            }
        };
        let bundle = self.finish().map_err(ProfileRunnerCaptureError::Finish)?;
        let style = if self.capture.pretty() {
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

fn map_loader_project_error(err: LoaderProjectError) -> ProjectError {
    match err {
        LoaderProjectError::ProfileNotFound { name } => ProjectError::ProfileNotFound { name },
        LoaderProjectError::ProfileInvalid { name, detail } => ProjectError::ConfigInvalid {
            detail: format!("project profile '{name}' is invalid: {detail}"),
        },
        other => ProjectError::ProjectLoad {
            detail: other.to_string(),
        },
    }
}

fn invalid_in_memory_profile(
    profile_name: Option<&str>,
    detail: impl Into<String>,
) -> ProjectError {
    let detail = detail.into();
    match profile_name {
        Some(profile_name) => ProjectError::ConfigInvalid {
            detail: format!("project profile '{profile_name}' is invalid: {detail}"),
        },
        None => ProjectError::ConfigInvalid { detail },
    }
}

fn validate_in_memory_profile_config(
    profile_name: Option<&str>,
    profile: &InMemoryProfileConfig,
) -> Result<(), ProjectError> {
    match &profile.ingress {
        InMemoryIngress::FixtureItems {
            items: _,
            source_label,
        } if source_label.trim().is_empty() => {
            return Err(invalid_in_memory_profile(
                profile_name,
                "fixture ingress source_label must not be empty",
            ));
        }
        InMemoryIngress::FixtureItems { items, .. } if items.is_empty() => {
            return Err(invalid_in_memory_profile(
                profile_name,
                "fixture ingress must declare at least one item",
            ));
        }
        InMemoryIngress::Process { command } if command.is_empty() => {
            return Err(invalid_in_memory_profile(
                profile_name,
                "process ingress command must not be empty",
            ));
        }
        _ => {}
    }

    Ok(())
}

fn live_prep_options_from_in_memory_profile(profile: &InMemoryProfileConfig) -> LivePrepOptions {
    LivePrepOptions {
        adapter: profile.adapter.clone(),
        egress_config: profile.egress_config.clone(),
    }
}

fn driver_from_in_memory_ingress(ingress: &InMemoryIngress) -> Result<DriverConfig, ProjectError> {
    match ingress {
        InMemoryIngress::Process { command } => Ok(DriverConfig::Process {
            command: command.clone(),
        }),
        InMemoryIngress::FixtureItems {
            items,
            source_label,
        } => Ok(DriverConfig::FixtureItems {
            items: items.clone(),
            source_label: source_label.clone(),
        }),
    }
}

fn capture_plan_from_resolved_profile(profile: &ResolvedProjectProfile) -> CapturePlan {
    match &profile.capture_output {
        Some(path) => CapturePlan::ExplicitFile {
            path: path.clone(),
            pretty: profile.pretty_capture,
        },
        None => CapturePlan::DefaultFile {
            pretty: profile.pretty_capture,
        },
    }
}

fn capture_plan_from_in_memory_profile(profile: &InMemoryProfileConfig) -> CapturePlan {
    match &profile.capture {
        ProfileCapture::InMemory => CapturePlan::InMemory,
        ProfileCapture::File { path, pretty } => CapturePlan::ExplicitFile {
            path: path.clone(),
            pretty: *pretty,
        },
    }
}

fn prepare_host_request_from_profile(
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

fn resolve_profile_plan_from_resolved_profile(
    profile_name: &str,
    profile: ResolvedProjectProfile,
) -> Result<ResolvedProfilePlan, ProjectError> {
    let capture = capture_plan_from_resolved_profile(&profile);
    let request = prepare_host_request_from_profile(&profile)?;
    let driver = match profile.ingress {
        ResolvedProjectIngress::Fixture { path } => DriverConfig::Fixture { path },
        ResolvedProjectIngress::Process { command } => DriverConfig::Process { command },
    };
    Ok(ResolvedProfilePlan {
        profile_name: profile_name.to_string(),
        runner_source: RunnerSource::Paths(request),
        driver,
        capture,
        max_duration: profile.max_duration,
        max_events: profile.max_events,
    })
}

fn resolve_profile_plan_from_in_memory_profile(
    profile_name: &str,
    profile: &InMemoryProfileConfig,
) -> Result<ResolvedProfilePlan, ProjectError> {
    validate_in_memory_profile_config(Some(profile_name), profile)?;
    Ok(ResolvedProfilePlan {
        profile_name: profile_name.to_string(),
        runner_source: RunnerSource::Assets {
            assets: profile.graph_assets.clone(),
            prep: live_prep_options_from_in_memory_profile(profile),
        },
        driver: driver_from_in_memory_ingress(&profile.ingress)?,
        capture: capture_plan_from_in_memory_profile(profile),
        max_duration: profile.max_duration,
        max_events: profile.max_events,
    })
}

fn host_capture_policy_from_plan(
    capture: &CapturePlan,
) -> Result<ergo_host::CapturePolicy, ProjectError> {
    match capture {
        CapturePlan::InMemory => Ok(ergo_host::CapturePolicy::InMemory),
        CapturePlan::ExplicitFile { path, pretty } => Ok(ergo_host::CapturePolicy::File {
            path: path.clone(),
            pretty: *pretty,
        }),
        CapturePlan::DefaultFile { .. } => Err(ProjectError::ConfigInvalid {
            detail: "default filesystem capture cannot be applied to in-memory graph assets"
                .to_string(),
        }),
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
    apply_profile_limits(control, config.max_duration, config.max_events)
}

fn apply_profile_limits(
    control: RunControl,
    max_duration: Option<Duration>,
    max_events: Option<u64>,
) -> RunControl {
    let control = if let Some(max_duration) = max_duration {
        control.max_duration(max_duration)
    } else {
        control
    };

    if let Some(max_events) = max_events {
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
    use ergo_loader::{load_graph_assets_from_memory, InMemorySourceInput};
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

    fn write_process_run_script(base: &Path) -> PathBuf {
        let hello = serde_json::to_string(&serde_json::json!({
            "type":"hello",
            "protocol":"ergo-driver.v0"
        }))
        .expect("serialize hello frame");
        let event = serde_json::to_string(&serde_json::json!({
            "type":"event",
            "event": manual_step_event("evt1"),
        }))
        .expect("serialize event frame");
        let end =
            serde_json::to_string(&serde_json::json!({"type":"end"})).expect("serialize end frame");
        write_file(
            base,
            "scripts/process_ingress.sh",
            &format!(
                "#!/bin/sh\nprintf '%s\\n' '{hello}'\nprintf '%s\\n' '{event}'\nprintf '%s\\n' '{end}'\n"
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

    fn load_memory_graph_assets(graph_id: &str) -> PreparedGraphAssets {
        load_graph_assets_from_memory(
            "graphs/root.yaml",
            &[InMemorySourceInput {
                source_id: "graphs/root.yaml".to_string(),
                source_label: format!("{graph_id}-root"),
                content: format!(
                    r#"
kind: cluster
id: {graph_id}
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 3.0
edges: []
outputs:
  value_out: src.value
"#
                ),
            }],
            &[],
        )
        .expect("load in-memory graph assets")
    }

    fn load_memory_intent_graph_assets(graph_id: &str) -> PreparedGraphAssets {
        load_graph_assets_from_memory(
            "graphs/root.yaml",
            &[InMemorySourceInput {
                source_id: "graphs/root.yaml".to_string(),
                source_label: format!("{graph_id}-root"),
                content: format!(
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
            }],
            &[],
        )
        .expect("load in-memory intent graph assets")
    }

    fn in_memory_project(
        name: &str,
        version: &str,
        profile_name: &str,
        profile: InMemoryProfileConfig,
    ) -> InMemoryProjectSnapshot {
        InMemoryProjectSnapshot::builder(name.to_string(), version.to_string())
            .profile(profile_name.to_string(), profile)
            .build()
            .expect("in-memory project snapshot should validate")
    }

    fn in_memory_process_profile(
        graph_assets: PreparedGraphAssets,
        command: impl IntoIterator<Item = impl Into<String>>,
    ) -> InMemoryProfileConfig {
        InMemoryProfileConfig::process(graph_assets, command)
            .expect("in-memory process profile should validate")
    }

    fn in_memory_fixture_profile(graph_assets: PreparedGraphAssets) -> InMemoryProfileConfig {
        InMemoryProfileConfig::fixture_items(
            graph_assets,
            vec![
                FixtureItem::EpisodeStart {
                    label: "E1".to_string(),
                },
                FixtureItem::Event {
                    id: Some("evt1".to_string()),
                    kind: ExternalEventKind::Command,
                    payload: Some(serde_json::json!({})),
                    semantic_kind: None,
                },
            ],
            "memory-fixture",
        )
        .expect("in-memory fixture profile should validate")
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
                assert_eq!(summary.capture_path, Some(capture));
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
                assert_eq!(summary.capture_path, Some(capture));
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
                assert_eq!(interrupted.summary.capture_path, Some(capture));
            }
            other => panic!("expected interrupted host-stop outcome, got {other:?}"),
        }

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn builder_rejects_conflicting_project_sources() {
        let snapshot = in_memory_project(
            "memory-project",
            "0.1.0",
            "historical",
            in_memory_process_profile(
                load_memory_graph_assets("sdk_builder_conflict"),
                ["/bin/echo", "noop"],
            )
            .capture(ProfileCapture::file(
                "captures/historical.capture.json",
                false,
            )),
        );

        let err = match Ergo::builder()
            .project_root(".")
            .in_memory_project(snapshot)
            .build()
        {
            Ok(_) => panic!("conflicting project sources must fail build"),
            Err(err) => err,
        };
        assert!(matches!(err, ErgoBuildError::ProjectSourceConflict));
    }

    #[test]
    fn builder_allows_replacing_the_same_project_source_kind(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let first = in_memory_project(
            "memory-project",
            "0.1.0",
            "historical",
            in_memory_process_profile(
                load_memory_graph_assets("sdk_builder_replace_first"),
                ["/bin/echo", "noop"],
            ),
        );
        let second = in_memory_project(
            "memory-project",
            "0.1.0",
            "historical",
            in_memory_process_profile(
                load_memory_graph_assets("sdk_builder_replace_second"),
                ["/bin/echo", "noop"],
            ),
        );

        let _ = Ergo::builder()
            .project_root(".")
            .project_root("..")
            .build()?;
        let _ = Ergo::builder()
            .in_memory_project(first)
            .in_memory_project(second)
            .build()?;
        Ok(())
    }

    #[test]
    fn in_memory_project_snapshot_rejects_empty_process_ingress() {
        let err = InMemoryProfileConfig::process(
            load_memory_graph_assets("sdk_memory_invalid_ingress"),
            Vec::<String>::new(),
        )
        .expect_err("empty process ingress must fail");

        assert!(matches!(err, ProjectError::ConfigInvalid { .. }));
    }

    #[test]
    fn run_profile_uses_in_memory_project_process_ingress() -> Result<(), Box<dyn std::error::Error>>
    {
        let root = make_temp_dir("memory_project_run");
        let ingress_script = write_process_run_script(&root);
        let capture = root.join("captures/historical.capture.json");
        let snapshot = in_memory_project(
            "memory-project",
            "0.1.0",
            "historical",
            in_memory_process_profile(
                load_memory_graph_assets("sdk_memory_project_run"),
                ["/bin/sh", &ingress_script.display().to_string()],
            )
            .capture(ProfileCapture::file(&capture, false)),
        );

        let outcome = Ergo::builder()
            .in_memory_project(snapshot)
            .build()?
            .run_profile("historical")?;

        match outcome {
            RunOutcome::Completed(summary) => {
                assert_eq!(summary.events, 1);
                assert_eq!(summary.capture_path, Some(capture));
            }
            other => panic!("expected completed run, got {other:?}"),
        }

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn run_profile_uses_in_memory_capture_for_in_memory_project(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("memory_project_run_capture_bundle");
        let ingress_script = write_process_run_script(&root);
        let snapshot = in_memory_project(
            "memory-project",
            "0.1.0",
            "historical",
            in_memory_process_profile(
                load_memory_graph_assets("sdk_memory_project_run_bundle"),
                ["/bin/sh", &ingress_script.display().to_string()],
            ),
        );

        let outcome = Ergo::builder()
            .in_memory_project(snapshot)
            .build()?
            .run_profile("historical")?;

        match outcome {
            RunOutcome::Completed(summary) => {
                assert_eq!(summary.events, 1);
                assert_eq!(summary.capture_path, None);
                assert_eq!(summary.capture_bundle.events.len(), 1);
            }
            other => panic!("expected completed run, got {other:?}"),
        }

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn run_profile_supports_in_memory_fixture_items_ingress(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = in_memory_project(
            "memory-project",
            "0.1.0",
            "historical",
            in_memory_fixture_profile(load_memory_graph_assets("sdk_memory_fixture_run")),
        );

        let outcome = Ergo::builder()
            .in_memory_project(snapshot)
            .build()?
            .run_profile("historical")?;

        match outcome {
            RunOutcome::Completed(summary) => {
                assert_eq!(summary.events, 1);
                assert_eq!(summary.capture_path, None);
                assert_eq!(summary.capture_bundle.events.len(), 1);
            }
            other => panic!("expected completed run, got {other:?}"),
        }

        Ok(())
    }

    #[test]
    fn run_profile_supports_in_memory_fixture_items_with_file_capture(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("memory_project_fixture_file_capture");
        let capture = root.join("captures/historical.capture.json");
        let snapshot = in_memory_project(
            "memory-project",
            "0.1.0",
            "historical",
            in_memory_fixture_profile(load_memory_graph_assets("sdk_memory_fixture_file_capture"))
                .capture(ProfileCapture::file(&capture, false)),
        );

        let outcome = Ergo::builder()
            .in_memory_project(snapshot)
            .build()?
            .run_profile("historical")?;

        match outcome {
            RunOutcome::Completed(summary) => {
                assert_eq!(summary.events, 1);
                assert_eq!(summary.capture_path, Some(capture.clone()));
                assert!(capture.exists());
                let written: CaptureBundle = serde_json::from_str(&fs::read_to_string(&capture)?)?;
                assert_eq!(written.graph_id.as_str(), "sdk_memory_fixture_file_capture");
            }
            other => panic!("expected completed run, got {other:?}"),
        }

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn replay_profile_on_in_memory_project_returns_unsupported_operation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = in_memory_project(
            "memory-project",
            "0.1.0",
            "historical",
            in_memory_process_profile(
                load_memory_graph_assets("sdk_memory_replay_unsupported"),
                ["/bin/echo", "noop"],
            ),
        );
        let ergo = Ergo::builder().in_memory_project(snapshot).build()?;
        let err = ergo
            .replay_profile("historical", "capture.json")
            .expect_err("in-memory replay_profile should be unsupported");

        match err {
            ErgoReplayError::Project(ProjectError::UnsupportedOperation {
                operation,
                transport,
            }) => {
                assert_eq!(operation, "replay_profile");
                assert_eq!(transport, "in-memory");
            }
            other => panic!("unexpected in-memory replay_profile error: {other:?}"),
        }

        Ok(())
    }

    #[test]
    fn missing_profile_error_is_normalized_for_in_memory_project(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = in_memory_project(
            "memory-project",
            "0.1.0",
            "historical",
            in_memory_process_profile(
                load_memory_graph_assets("sdk_memory_missing_profile"),
                ["/bin/echo", "noop"],
            ),
        );
        let ergo = Ergo::builder().in_memory_project(snapshot).build()?;
        let err = ergo
            .run_profile("missing")
            .expect_err("missing in-memory profile should use normalized error");
        assert!(matches!(
            err,
            ErgoRunError::Project(ProjectError::ProfileNotFound { name }) if name == "missing"
        ));
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
    fn replay_profile_bundle_reuses_project_resolution() -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("project_replay_bundle");
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
id: sdk_replay_bundle_graph
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

        let outcome = Ergo::from_project(&root)
            .build()?
            .run_profile("historical")?;
        let bundle = match outcome {
            RunOutcome::Completed(summary) => summary.capture_bundle,
            other => panic!("expected completed run, got {other:?}"),
        };

        let replay = Ergo::from_project(&root)
            .build()?
            .replay_profile_bundle("historical", bundle)?;

        assert_eq!(replay.graph_id.as_str(), "sdk_replay_bundle_graph");
        assert_eq!(replay.events, 1);

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn replay_profile_bundle_supports_in_memory_projects() -> Result<(), Box<dyn std::error::Error>>
    {
        let root = make_temp_dir("memory_project_replay_bundle");
        let ingress_script = write_process_run_script(&root);
        let snapshot = in_memory_project(
            "memory-project",
            "0.1.0",
            "historical",
            in_memory_process_profile(
                load_memory_graph_assets("sdk_memory_replay_bundle"),
                ["/bin/sh", &ingress_script.display().to_string()],
            ),
        );
        let ergo = Ergo::builder().in_memory_project(snapshot).build()?;
        let outcome = ergo.run_profile("historical")?;
        let bundle = match outcome {
            RunOutcome::Completed(summary) => summary.capture_bundle,
            other => panic!("expected completed run, got {other:?}"),
        };

        let replay = ergo.replay_profile_bundle("historical", bundle)?;
        assert_eq!(replay.graph_id.as_str(), "sdk_memory_replay_bundle");
        assert_eq!(replay.events, 1);

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn replay_bundle_replays_capture_bundle_from_paths() -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("explicit_replay_bundle");
        let graph = write_file(
            &root,
            "graph.yaml",
            r#"
kind: cluster
id: sdk_explicit_replay_bundle
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
        let fixture = write_file(
            &root,
            "fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );

        let outcome = Ergo::builder()
            .build()?
            .run(RunConfig::new(&graph, IngressConfig::fixture(&fixture)))?;
        let bundle = match outcome {
            RunOutcome::Completed(summary) => summary.capture_bundle,
            other => panic!("expected completed run, got {other:?}"),
        };

        let replay = Ergo::builder()
            .build()?
            .replay_bundle(ReplayBundleConfig::new(bundle, &graph))?;
        assert_eq!(replay.graph_id.as_str(), "sdk_explicit_replay_bundle");
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
        assert_eq!(summary.root, Some(root.clone()));
        assert_eq!(summary.profiles, vec!["live".to_string()]);

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn validate_project_returns_none_root_for_in_memory_project(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("memory_project_validate");
        let snapshot = in_memory_project(
            "memory-project",
            "0.1.0",
            "historical",
            in_memory_process_profile(
                load_memory_graph_assets("sdk_memory_validate"),
                ["/bin/echo", "noop"],
            ),
        );

        let summary = Ergo::builder()
            .in_memory_project(snapshot)
            .build()?
            .validate_project()?;
        assert_eq!(summary.name, "memory-project");
        assert_eq!(summary.version, "0.1.0");
        assert_eq!(summary.root, None);
        assert_eq!(summary.profiles, vec!["historical".to_string()]);

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn validate_project_and_run_profile_agree_for_valid_in_memory_profile(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("memory_project_validate_and_run");
        let ingress_script = write_process_run_script(&root);
        let snapshot = in_memory_project(
            "memory-project",
            "0.1.0",
            "historical",
            in_memory_process_profile(
                load_memory_graph_assets("sdk_memory_validate_and_run"),
                ["/bin/sh", &ingress_script.display().to_string()],
            ),
        );
        let ergo = Ergo::builder().in_memory_project(snapshot).build()?;

        let summary = ergo.validate_project()?;
        assert_eq!(summary.profiles, vec!["historical".to_string()]);
        assert_eq!(summary.root, None);

        let outcome = ergo.run_profile("historical")?;
        match outcome {
            RunOutcome::Completed(summary) => {
                assert_eq!(summary.events, 1);
                assert_eq!(summary.capture_path, None);
                assert_eq!(summary.capture_bundle.events.len(), 1);
            }
            other => panic!("expected completed run, got {other:?}"),
        }

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn validate_project_in_memory_preserves_adapter_required_preflight(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = in_memory_project(
            "memory-project",
            "0.1.0",
            "live",
            in_memory_process_profile(
                load_memory_intent_graph_assets("sdk_memory_validate_missing_adapter"),
                ["/bin/echo", "noop"],
            ),
        );

        let err = Ergo::builder()
            .in_memory_project(snapshot)
            .add_source(InjectedNumberSource::new(4.0))
            .add_action(InjectedIntentAction::new())
            .build()?
            .validate_project()
            .expect_err("adapter-less in-memory intent profile must fail validation");

        match err {
            ErgoValidateError::Validation { profile, detail } => {
                assert_eq!(profile, "live");
                assert!(detail.contains("adapter"));
            }
            other => panic!("unexpected error: {other}"),
        }
        Ok(())
    }

    #[test]
    fn validate_project_rejects_missing_fixture_before_run_profile(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("validate_missing_fixture_driver");
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/missing.jsonl"
"#,
        );
        write_file(
            &root,
            "graphs/strategy.yaml",
            r#"
kind: cluster
id: sdk_validate_missing_fixture
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

        let ergo = Ergo::from_project(&root).build()?;
        let validate_err = ergo
            .validate_project()
            .expect_err("missing fixture should fail validation");
        match validate_err {
            ErgoValidateError::Validation { profile, detail } => {
                assert_eq!(profile, "historical");
                assert!(detail.contains("failed to parse fixture"));
            }
            other => panic!("unexpected validation error: {other}"),
        }

        let run_err = ergo
            .run_profile("historical")
            .expect_err("missing fixture should fail run_profile");
        match run_err {
            ErgoRunError::Host(HostRunError::InvalidInput(detail)) => {
                assert!(detail.contains("failed to parse fixture"));
            }
            other => panic!("unexpected run_profile error: {other:?}"),
        }

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn validate_project_rejects_missing_in_memory_process_driver_before_run_profile(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = make_temp_dir("validate_missing_process_driver");
        let missing_driver = root.join("missing-driver.sh");
        let snapshot = in_memory_project(
            "memory-project",
            "0.1.0",
            "historical",
            in_memory_process_profile(
                load_memory_graph_assets("sdk_memory_missing_driver"),
                [missing_driver.display().to_string()],
            ),
        );

        let ergo = Ergo::builder().in_memory_project(snapshot).build()?;
        let validate_err = ergo
            .validate_project()
            .expect_err("missing process driver should fail validation");
        match validate_err {
            ErgoValidateError::Validation { profile, detail } => {
                assert_eq!(profile, "historical");
                assert!(detail.contains("spawn process driver"));
            }
            other => panic!("unexpected validation error: {other}"),
        }

        let run_err = ergo
            .run_profile("historical")
            .expect_err("missing process driver should fail run_profile");
        match run_err {
            ErgoRunError::Host(HostRunError::DriverStart(detail)) => {
                assert!(detail.contains("spawn process driver"));
            }
            other => panic!("unexpected run_profile error: {other:?}"),
        }

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
                assert!(detail.contains("available cluster sources"));
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
            matches!(
                err,
                ErgoRunError::Project(ProjectError::ProfileNotFound { ref name })
                    if name == "missing"
            ),
            "unexpected missing-profile error: {err:?}"
        );

        let summary = ergo.validate_project()?;
        assert_eq!(summary.root, Some(root.clone()));
        assert_eq!(summary.profiles, vec!["historical".to_string()]);

        let outcome = ergo.run_profile("historical")?;
        let capture = root.join("captures/historical.capture.json");
        match outcome {
            RunOutcome::Completed(summary) => {
                assert_eq!(summary.capture_path, Some(capture.clone()));
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
            ErgoRunnerError::Project(ProjectError::ConfigInvalid { detail }) => {
                assert!(detail.contains("exactly one ingress source"))
            }
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

    #[test]
    fn runner_for_profile_returns_working_profile_runner_for_in_memory_project(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = in_memory_project(
            "memory-project",
            "0.1.0",
            "manual",
            in_memory_process_profile(
                load_memory_graph_assets("sdk_memory_manual_runner"),
                ["/bin/echo", "noop"],
            ),
        );

        let ergo = Ergo::builder().in_memory_project(snapshot).build()?;
        let mut runner = ergo.runner_for_profile("manual")?;
        let outcome = runner.step(manual_step_event("evt1"))?;
        assert_eq!(outcome.termination, Some(RunTermination::Completed));

        let bundle = runner.finish()?;
        assert_eq!(bundle.events.len(), 1);

        Ok(())
    }

    #[test]
    fn runner_for_profile_in_memory_without_file_capture_path_cannot_auto_write_capture(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = in_memory_project(
            "memory-project",
            "0.1.0",
            "manual",
            in_memory_process_profile(
                load_memory_graph_assets("sdk_memory_manual_capture"),
                ["/bin/echo", "noop"],
            ),
        );

        let ergo = Ergo::builder().in_memory_project(snapshot).build()?;
        let mut runner = ergo.runner_for_profile("manual")?;
        let _ = runner.step(manual_step_event("evt1"))?;
        let err = runner
            .finish_and_write_capture()
            .expect_err("in-memory runner without file capture must not auto-write");
        assert!(matches!(
            err,
            ProfileRunnerCaptureError::CaptureOutputNotConfigured
        ));

        Ok(())
    }
}
