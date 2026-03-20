//! Rust SDK over Ergo host + loader.
//!
//! The SDK is the primary product surface for building an Ergo engine
//! inside a Rust crate. It wraps the existing canonical host run and
//! replay paths without introducing a second execution model.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ergo_adapter::host::ensure_handler_coverage;
use ergo_adapter::{validate_adapter, AdapterManifest, AdapterProvides};
use ergo_host::{
    parse_egress_config_toml, replay_graph_from_paths_with_surfaces,
    run_graph_from_paths_with_surfaces_and_control, scan_adapter_dependencies,
    validate_adapter_composition, validate_egress_config, DriverConfig, EgressConfig,
    HostReplayError, HostRunError, HostStopHandle, ReplayGraphFromPathsRequest,
    ReplayGraphResult, RunControl, RunGraphFromPathsRequest, RunOutcome, RuntimeSurfaces,
};
use ergo_loader::{
    load_project, parse_graph_file, ProjectError as LoaderProjectError, ResolvedProject,
    ResolvedProjectIngress, ResolvedProjectProfile,
};
use ergo_runtime::catalog::{
    CatalogBuilder, CorePrimitiveCatalog, CoreRegistrationError, CoreRegistries,
};
use ergo_runtime::cluster::{
    expand, ClusterDefinition, ClusterLoader, ClusterVersionIndex, ExpandError, ExpandedGraph,
    PrimitiveCatalog, PrimitiveKind, Version, VersionTargetKind,
};
use ergo_runtime::common::ErrorInfo;

pub use ergo_host::{
    EgressChannelConfig, EgressRoute, InterruptedRun, InterruptionReason, RunSummary,
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
            registries,
            catalog,
            project_root: self.project_root,
        })
    }
}

/// Built Ergo engine handle.
///
/// The current SDK exposes `Ergo` as a one-shot handle: `run`,
/// `run_profile`, `replay`, `replay_profile`, and `validate_project`
/// consume `self`.
///
/// Build a fresh handle for each operation. A reusable engine handle is
/// planned as a future ergonomics improvement once the underlying
/// ownership model is factored cleanly for it.
pub struct Ergo {
    registries: CoreRegistries,
    catalog: CorePrimitiveCatalog,
    project_root: Option<PathBuf>,
}

impl Ergo {
    pub fn builder() -> ErgoBuilder {
        ErgoBuilder::new()
    }

    pub fn from_project(path: impl AsRef<Path>) -> ErgoBuilder {
        ErgoBuilder::new().project_root(path)
    }

    pub fn run(self, config: RunConfig) -> Result<RunOutcome, ErgoRunError> {
        self.run_with_control(config, RunControl::default())
    }

    pub fn run_with_stop(
        self,
        config: RunConfig,
        stop: StopHandle,
    ) -> Result<RunOutcome, ErgoRunError> {
        self.run_with_control(
            config,
            RunControl::new().with_stop_handle(stop.host_handle()),
        )
    }

    fn run_with_control(
        self,
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
            RuntimeSurfaces::new(self.registries, self.catalog),
            run_control_from_config(&config, control),
        )
        .map_err(ErgoRunError::Host)
    }

    pub fn run_profile(self, profile_name: &str) -> Result<RunOutcome, ErgoRunError> {
        self.run_profile_with_control(profile_name, RunControl::default())
    }

    pub fn run_profile_with_stop(
        self,
        profile_name: &str,
        stop: StopHandle,
    ) -> Result<RunOutcome, ErgoRunError> {
        self.run_profile_with_control(
            profile_name,
            RunControl::new().with_stop_handle(stop.host_handle()),
        )
    }

    fn run_profile_with_control(
        self,
        profile_name: &str,
        control: RunControl,
    ) -> Result<RunOutcome, ErgoRunError> {
        let project = self.load_project().map_err(ErgoRunError::Project)?;
        let resolved = project
            .resolve_run_profile(profile_name)
            .map_err(ProjectError::from)
            .map_err(ErgoRunError::Project)?;
        self.run_with_control(run_config_from_resolved_profile(resolved), control)
    }

    pub fn replay(self, config: ReplayConfig) -> Result<ReplayGraphResult, ErgoReplayError> {
        replay_graph_from_paths_with_surfaces(
            ReplayGraphFromPathsRequest {
                capture_path: config.capture_path,
                graph_path: config.graph_path,
                cluster_paths: config.cluster_paths,
                adapter_path: config.adapter_path,
            },
            RuntimeSurfaces::new(self.registries, self.catalog),
        )
        .map_err(ErgoReplayError::Host)
    }

    pub fn replay_profile(
        self,
        profile_name: &str,
        capture_path: impl AsRef<Path>,
    ) -> Result<ReplayGraphResult, ErgoReplayError> {
        let project = self.load_project().map_err(ErgoReplayError::Project)?;
        let resolved = project
            .resolve_run_profile(profile_name)
            .map_err(ProjectError::from)
            .map_err(ErgoReplayError::Project)?;
        self.replay(replay_config_from_resolved_profile(resolved, capture_path))
    }

    pub fn validate_project(self) -> Result<ProjectSummary, ErgoValidateError> {
        let project = self.load_project().map_err(ErgoValidateError::Project)?;
        for profile_name in project.profile_names() {
            let resolved = project
                .resolve_run_profile(&profile_name)
                .map_err(ProjectError::from)
                .map_err(ErgoValidateError::Project)?;
            validate_profile(&profile_name, &resolved, &self.catalog, &self.registries)?;
        }

        let profiles = project.profile_names();
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

fn summarize_expand_error(
    err: &ExpandError,
    cluster_sources: &HashMap<(String, Version), PathBuf>,
) -> String {
    let base = format!("{} ({})", err.summary(), err.rule_id());
    match err {
        ExpandError::UnsatisfiedVersionConstraint {
            target_kind: VersionTargetKind::Cluster,
            id,
            available_versions,
            ..
        } => {
            let available = available_versions
                .iter()
                .filter_map(|version| {
                    cluster_sources
                        .get(&(id.clone(), version.clone()))
                        .map(|path| format!("- {}@{} at {}", id, version, path.display()))
                })
                .collect::<Vec<_>>();
            if available.is_empty() {
                base
            } else {
                format!(
                    "{}\navailable cluster files:\n{}",
                    base,
                    available.join("\n")
                )
            }
        }
        _ => base,
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

#[derive(Clone)]
struct PreloadedClusterLoader {
    clusters: HashMap<(String, Version), ClusterDefinition>,
}

impl PreloadedClusterLoader {
    fn new(clusters: HashMap<(String, Version), ClusterDefinition>) -> Self {
        Self { clusters }
    }
}

impl ClusterLoader for PreloadedClusterLoader {
    fn load(&self, id: &str, version: &Version) -> Option<ClusterDefinition> {
        self.clusters
            .get(&(id.to_string(), version.clone()))
            .cloned()
    }
}

impl ClusterVersionIndex for PreloadedClusterLoader {
    fn available_versions(&self, id: &str) -> Vec<Version> {
        let mut versions = self
            .clusters
            .keys()
            .filter_map(|(candidate_id, version)| (candidate_id == id).then(|| version.clone()))
            .collect::<Vec<_>>();
        versions.sort();
        versions
    }
}

fn format_registration_error(err: CoreRegistrationError) -> ErgoBuildError {
    ErgoBuildError::Registration(format!("primitive registration failed: {err:?}"))
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

fn parse_adapter_manifest(path: &Path) -> Result<AdapterManifest, String> {
    let data = fs::read_to_string(path).map_err(|err| {
        format!(
            "failed to read adapter manifest '{}': {err}",
            path.display()
        )
    })?;
    let value = serde_yaml::from_str::<serde_json::Value>(&data).map_err(|err| {
        format!(
            "failed to parse adapter manifest '{}': {err}",
            path.display()
        )
    })?;
    serde_json::from_value::<AdapterManifest>(value).map_err(|err| {
        format!(
            "failed to decode adapter manifest '{}': {err}",
            path.display()
        )
    })
}

fn validate_profile(
    profile_name: &str,
    profile: &ResolvedProjectProfile,
    catalog: &CorePrimitiveCatalog,
    registries: &CoreRegistries,
) -> Result<(), ErgoValidateError> {
    let root =
        parse_graph_file(&profile.graph_path).map_err(|err| ErgoValidateError::Validation {
            profile: profile_name.to_string(),
            detail: format!("graph parse failed: {err}"),
        })?;
    let discovery = ergo_loader::discovery::discover_cluster_tree(
        &profile.graph_path,
        &root,
        &profile.cluster_paths,
    )
    .map_err(|err| ErgoValidateError::Validation {
        profile: profile_name.to_string(),
        detail: format!("cluster discovery failed: {err}"),
    })?;
    let cluster_sources = discovery.cluster_sources;
    let clusters = discovery.clusters;
    let loader = PreloadedClusterLoader::new(clusters);
    let expanded =
        expand(&root, &loader, catalog).map_err(|err| ErgoValidateError::Validation {
            profile: profile_name.to_string(),
            detail: format!(
                "graph expansion failed: {}",
                summarize_expand_error(&err, &cluster_sources)
            ),
        })?;

    let dependency_summary =
        scan_adapter_dependencies(&expanded, catalog, registries).map_err(|detail| {
            ErgoValidateError::Validation {
                profile: profile_name.to_string(),
                detail: format!("adapter dependency scan failed: {detail}"),
            }
        })?;

    let (adapter_provides, adapter_bound) = match &profile.adapter_path {
        Some(path) => {
            let manifest =
                parse_adapter_manifest(path).map_err(|detail| ErgoValidateError::Validation {
                    profile: profile_name.to_string(),
                    detail,
                })?;
            validate_adapter(&manifest).map_err(|err| ErgoValidateError::Validation {
                profile: profile_name.to_string(),
                detail: format!(
                    "adapter validation failed: {} ({})",
                    err.summary(),
                    err.rule_id()
                ),
            })?;
            let provides = AdapterProvides::from_manifest(&manifest);
            validate_adapter_composition(&expanded, catalog, registries, &provides).map_err(
                |detail| ErgoValidateError::Validation {
                    profile: profile_name.to_string(),
                    detail: format!("adapter composition failed: {detail}"),
                },
            )?;
            (provides, true)
        }
        None => {
            if dependency_summary.requires_adapter {
                return Err(ErgoValidateError::Validation {
                    profile: profile_name.to_string(),
                    detail:
                        "graph requires adapter capabilities but profile does not declare adapter"
                            .to_string(),
                });
            }
            (AdapterProvides::default(), false)
        }
    };

    if profile.egress_config_path.is_some() && !adapter_bound {
        return Err(ErgoValidateError::Validation {
            profile: profile_name.to_string(),
            detail: "egress configuration requires adapter-bound mode".to_string(),
        });
    }

    let emittable_effect_kinds = graph_emittable_effect_kinds(&expanded, catalog, registries);
    let handler_kinds = BTreeSet::from(["set_context".to_string()]);

    if let Some(path) = &profile.egress_config_path {
        let config = load_egress_config(path).map_err(|detail| ErgoValidateError::Validation {
            profile: profile_name.to_string(),
            detail,
        })?;
        let _warnings = validate_egress_config(
            &config,
            &adapter_provides,
            &emittable_effect_kinds,
            &handler_kinds,
        )
        .map_err(|detail| ErgoValidateError::Validation {
            profile: profile_name.to_string(),
            detail: format!("egress validation failed: {detail}"),
        })?;
    } else if adapter_bound {
        ensure_handler_coverage(
            &adapter_provides,
            &emittable_effect_kinds,
            &handler_kinds,
            &HashSet::new(),
        )
        .map_err(|err| ErgoValidateError::Validation {
            profile: profile_name.to_string(),
            detail: format!("handler coverage failed: {err}"),
        })?;
    }

    Ok(())
}

fn graph_emittable_effect_kinds(
    expanded: &ExpandedGraph,
    catalog: &CorePrimitiveCatalog,
    registries: &CoreRegistries,
) -> HashSet<String> {
    let mut kinds = HashSet::new();

    for node in expanded.nodes.values() {
        let Some(meta) = catalog.get(&node.implementation.impl_id, &node.implementation.version)
        else {
            continue;
        };
        if meta.kind != PrimitiveKind::Action {
            continue;
        }
        let Some(action) = registries.actions.get(&node.implementation.impl_id) else {
            continue;
        };

        let emits_set_context = !action.manifest().effects.writes.is_empty()
            || action
                .manifest()
                .effects
                .intents
                .iter()
                .any(|intent| !intent.mirror_writes.is_empty());
        if emits_set_context {
            kinds.insert("set_context".to_string());
        }
        for intent in &action.manifest().effects.intents {
            kinds.insert(intent.name.clone());
        }
    }

    kinds
}

#[cfg(test)]
mod tests {
    use super::*;
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
                    IngressConfig::process([
                        "/bin/sh".to_string(),
                        driver.display().to_string(),
                    ]),
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
    fn run_profile_with_stop_honors_profile_max_events() -> Result<(), Box<dyn std::error::Error>>
    {
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
}
