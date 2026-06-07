//! Rust SDK for embedding Ergo in an application.
//!
//! `ergo-sdk-rust` is the Rust application boundary for Ergo. Most users
//! build an [`Ergo`] engine, point it at a project, then run or replay a
//! named profile from that project.
//!
//! The usual mental model is:
//!
//! 1. A **project** supplies graph assets and named profiles. It can live on
//!    disk as an `ergo.toml` project or be assembled in memory with
//!    [`InMemoryProjectSnapshot`].
//! 2. A **profile** chooses one graph, one ingress source, optional adapter
//!    and egress configuration, and optional capture settings.
//! 3. A **run** executes the selected profile through Ergo's canonical host
//!    orchestration. A **replay** checks a prior capture against the same
//!    deterministic graph path.
//!
//! Start with [`Ergo::from_project`] for a filesystem project, or with
//! [`Ergo::builder`] when you need to register custom primitives or supply an
//! in-memory project. For event-by-event control, use
//! [`Ergo::runner_for_profile`] and drive the returned [`ProfileRunner`]
//! manually.
//!
//! Errors at this boundary use SDK-branded `Ergo*` types such as
//! [`ErgoRunError`] and [`ErgoReplayError`]. Match those categories first.
//! Lower-crate diagnostic error taxonomies are not part of the SDK's normal
//! public matching surface; advanced callers reach host, adapter, supervisor,
//! loader, or runtime detail only as `&dyn Error` through the
//! [`std::error::Error::source`] chain (and `downcast_ref` against the relevant
//! Ergo crate they already depend on). Explicit authoring/configuration
//! carve-outs, such as [`EgressConfigError`] and [`EgressConfigParseError`],
//! remain direct SDK API where documented.
//!
//! # Threading model
//!
//! In v1, [`Ergo`] is a single-threaded handle. Build one handle and reuse
//! it for any number of sequential operations on the same thread. Runs and
//! replays are synchronous; calls block the calling thread until the host
//! loop completes.
//!
//! To use Ergo from multiple threads today, build one handle per thread
//! (registers the primitive catalog separately) or wrap a shared handle in
//! `std::sync::Mutex` and serialize access. The engine handle is not
//! `Send + Sync` because primitive trait objects in the runtime catalog do
//! not yet require those bounds; tightening those bounds is a kernel-level
//! decision tracked separately and is on the post-v1 roadmap.
//!
//! [`StopHandle`] is `Send + Sync` and may be shared with another thread
//! that needs to request a graceful host stop while a run is blocked in
//! [`Ergo::run_with_stop`] or [`Ergo::run_profile_with_stop`]. The stop
//! handle does not require thread-mobility of [`Ergo`] itself: the
//! supervising thread only needs to call [`StopHandle::stop`].
//!
//! # Quick start
//!
//! Run a named profile from a filesystem project:
//!
//! ```no_run
//! use ergo_sdk_rust::{Ergo, ErgoRunError, RunOutcome};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let ergo = Ergo::from_project("/path/to/my-ergo-project").build()?;
//!
//!     // Validate the project before running anything live.
//!     let summary = ergo.validate_project()?;
//!     println!("project {} v{}", summary.name, summary.version);
//!
//!     match ergo.run_profile("historical")? {
//!         RunOutcome::Completed(run) => {
//!             println!("ran {} events", run.events);
//!         }
//!         RunOutcome::Interrupted(run) => {
//!             eprintln!("interrupted: {:?}", run.reason);
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//!
//! Drive a profile event-by-event with [`ProfileRunner`]:
//!
//! ```no_run
//! use ergo_sdk_rust::{Ergo, EventTime, ExternalEventKind, HostedEvent};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let ergo = Ergo::from_project("/path/to/my-ergo-project").build()?;
//!     let mut runner = ergo.runner_for_profile("manual")?;
//!
//!     let _outcome = runner.step(HostedEvent {
//!         event_id: "evt-1".to_string(),
//!         kind: ExternalEventKind::Command,
//!         at: EventTime::default(),
//!         semantic_kind: Some("tick".to_string()),
//!         payload: Some(serde_json::json!({})),
//!     })?;
//!
//!     let bundle = runner.finish()?;
//!     println!("captured {} events", bundle.events.len());
//!     Ok(())
//! }
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Fixture item data for SDK-authored in-memory fixture ingress.
pub use ergo_adapter::fixture::FixtureItem;
/// Adapter-owned event-shape types that appear in [`HostedEvent`] and
/// [`HostedStepOutcome`] fields.
///
/// These are the same types `ergo-adapter` uses for the canonical event
/// model; they are re-exported here so that callers constructing or
/// inspecting hosted events do not need a direct dependency on
/// `ergo-adapter`.
pub use ergo_adapter::{EventTime, ExternalEventKind, RunTermination};
use ergo_host::{
    finalize_hosted_runner_capture, load_graph_assets_from_paths,
    prepare_hosted_runner_from_paths_with_surfaces, prepare_hosted_runner_with_surfaces,
    replay_graph_from_assets_with_surfaces, replay_graph_from_paths_with_surfaces,
    run_graph_from_assets_with_surfaces_and_control,
    run_graph_from_paths_with_surfaces_and_control, validate_run_graph_from_assets_with_surfaces,
    validate_run_graph_from_paths_with_surfaces, write_capture_bundle as host_write_capture_bundle,
    DriverConfig, HostRunError, HostStopHandle, HostedRunner, HostedStepError, LivePrepOptions,
    PrepareHostedRunnerFromPathsRequest, ReplayGraphFromAssetsRequest, ReplayGraphFromPathsRequest,
    ReplayGraphResult, RunControl, RunGraphFromAssetsRequest, RunGraphFromPathsRequest,
    RuntimeSurfaces,
};
use ergo_loader::{
    load_project, PreparedGraphAssets, ResolvedProject, ResolvedProjectIngress,
    ResolvedProjectProfile,
};
use ergo_runtime::catalog::CatalogBuilder;

/// Transparent host-owned authoring and outcome types kept as part of the SDK
/// surface.
///
/// These types are data users construct, inspect, or receive directly; host
/// error taxonomies are wrapped in SDK `Ergo*` errors instead.
pub use ergo_host::{
    parse_egress_config_toml, AdapterInput, CaptureBundle, CaptureJsonStyle, EgressChannelConfig,
    EgressConfig, EgressConfigBuilder, EgressConfigError, EgressConfigParseError, EgressRoute,
    HostedEvent, HostedStepOutcome, InterruptedRun, InterruptionReason, RunOutcome, RunSummary,
};
/// Runtime catalog helpers re-exported for advanced primitive registration.
pub use ergo_runtime::catalog::{build_core, build_core_catalog, core_registries};
/// Runtime execution context visible to custom primitive implementations.
pub use ergo_runtime::runtime::ExecutionContext;
/// Runtime primitive modules used when implementing custom sources, computes,
/// triggers, or actions for an [`ErgoBuilder`].
pub use ergo_runtime::{action, common, compute, source, trigger};

mod error;
use error::{
    map_host_replay_error, map_host_run_error_to_run, map_host_run_error_to_runner,
    map_hosted_step_error,
};
pub use error::{
    ErgoBuildError, ErgoCaptureError, ErgoConfigError, ErgoErrorSource, ErgoProjectConfigError,
    ErgoProjectError, ErgoReplayError, ErgoRunError, ErgoRunnerError, ErgoStepError,
    ErgoValidationError,
};

#[derive(Debug, Clone, Default)]
/// Cooperative stop handle for a synchronous SDK run.
///
/// Clone or share this handle with the thread that should request shutdown,
/// then pass it to [`Ergo::run_with_stop`] or [`Ergo::run_profile_with_stop`].
/// Calling [`StopHandle::stop`] asks the host run loop to stop at the next
/// supported boundary; it does not forcibly kill an adapter process.
pub struct StopHandle {
    handle: HostStopHandle,
}

impl StopHandle {
    /// Creates a stop handle in the non-stopped state.
    pub fn new() -> Self {
        Self {
            handle: HostStopHandle::new(),
        }
    }

    /// Requests graceful stop for any run using this handle.
    pub fn stop(&self) {
        self.handle.request_stop();
    }

    fn host_handle(&self) -> HostStopHandle {
        self.handle.clone()
    }
}

/// Internal union of the two failure modes that profile resolution and
/// validation can report before a public SDK boundary normalizes them.
///
/// `Config` carries SDK-classified configuration mistakes; `HostRun`
/// carries a host-side preflight or run failure that will be remapped
/// to the operation-specific `Ergo*` error at the boundary.
#[derive(Debug)]
enum InternalProfileError {
    Config(ErgoConfigError),
    HostRun(HostRunError),
}

impl From<ErgoConfigError> for InternalProfileError {
    fn from(value: ErgoConfigError) -> Self {
        Self::Config(value)
    }
}

impl From<HostRunError> for InternalProfileError {
    fn from(value: HostRunError) -> Self {
        Self::HostRun(value)
    }
}

impl InternalProfileError {
    fn into_run(self) -> ErgoRunError {
        match self {
            Self::Config(inner) => ErgoRunError::Config { inner },
            Self::HostRun(err) => map_host_run_error_to_run(err),
        }
    }

    fn into_runner(self) -> ErgoRunnerError {
        match self {
            Self::Config(inner) => ErgoRunnerError::Config { inner },
            Self::HostRun(err) => map_host_run_error_to_runner(err),
        }
    }

    fn into_replay(self) -> ErgoReplayError {
        match self {
            Self::Config(inner) => ErgoReplayError::Config { inner },
            Self::HostRun(_) => {
                // Profile resolution surfaces a `HostRunError` only for the
                // SDK-side production-adapter check. Replay always rebuilds
                // prep options as fixture, so this branch is reached only
                // when a future host invariant changes; report it as a
                // configuration-class failure rather than synthesising a
                // bogus `HostReplayError`.
                ErgoReplayError::Config {
                    inner: ErgoConfigError::UnsupportedOperation {
                        operation: "replay_profile_with_production_ingress",
                        transport: "in-memory",
                    },
                }
            }
        }
    }

    fn into_validation(self, profile: &str) -> ErgoValidationError {
        match self {
            Self::Config(inner) => ErgoValidationError::Profile {
                profile: profile.to_string(),
                inner,
            },
            Self::HostRun(inner) => ErgoValidationError::HostValidation {
                profile: profile.to_string(),
                source: ErgoErrorSource::new(inner),
            },
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
/// Ingress source for an explicit, path-backed [`RunConfig`].
///
/// Use fixture ingress for deterministic historical input and process ingress
/// for a live adapter channel that speaks Ergo's process-driver protocol.
pub enum IngressConfig {
    /// Read fixture events from a JSONL fixture file.
    Fixture {
        /// Path to the fixture JSONL file.
        path: PathBuf,
    },
    /// Launch a process ingress command.
    Process {
        /// Executable and arguments for the process ingress channel.
        command: Vec<String>,
    },
}

impl IngressConfig {
    /// Creates fixture ingress from a JSONL fixture path.
    pub fn fixture(path: impl AsRef<Path>) -> Self {
        Self::Fixture {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Creates process ingress from an executable plus arguments.
    ///
    /// The command must not be empty when the run is executed.
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
/// Explicit run configuration for running graph files without an `ergo.toml`
/// profile.
///
/// Use [`Ergo::run`] for one-off execution when your application already knows
/// the graph path and ingress source. Use [`Ergo::run_profile`] when those
/// choices should come from project profile configuration.
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
    /// Starts a run configuration with one graph path and one ingress source.
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

    /// Adds a cluster search path used while resolving the graph.
    pub fn cluster_path(mut self, path: impl AsRef<Path>) -> Self {
        self.cluster_paths.push(path.as_ref().to_path_buf());
        self
    }

    /// Adds an adapter manifest path for adapter-bound graphs or production
    /// process ingress.
    pub fn adapter(mut self, path: impl AsRef<Path>) -> Self {
        self.adapter_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Adds an egress TOML configuration path for live effect dispatch.
    pub fn egress_config(mut self, path: impl AsRef<Path>) -> Self {
        self.egress_config_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Writes the produced capture bundle to this path after a successful run.
    pub fn capture_output(mut self, path: impl AsRef<Path>) -> Self {
        self.capture_output = Some(path.as_ref().to_path_buf());
        self
    }

    /// Chooses pretty JSON formatting for capture output.
    pub fn pretty_capture(mut self, enabled: bool) -> Self {
        self.pretty_capture = enabled;
        self
    }

    /// Stops the run after the configured maximum wall-clock duration.
    pub fn max_duration(mut self, max_duration: Duration) -> Self {
        self.max_duration = Some(max_duration);
        self
    }

    /// Stops the run after the configured maximum number of processed events.
    pub fn max_events(mut self, max_events: u64) -> Self {
        self.max_events = Some(max_events);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Replay configuration for reading a capture bundle from disk and replaying it
/// against graph files.
pub struct ReplayConfig {
    capture_path: PathBuf,
    graph_path: PathBuf,
    cluster_paths: Vec<PathBuf>,
    adapter_path: Option<PathBuf>,
}

impl ReplayConfig {
    /// Creates replay configuration from a capture path and graph path.
    pub fn new(capture_path: impl AsRef<Path>, graph_path: impl AsRef<Path>) -> Self {
        Self {
            capture_path: capture_path.as_ref().to_path_buf(),
            graph_path: graph_path.as_ref().to_path_buf(),
            cluster_paths: Vec::new(),
            adapter_path: None,
        }
    }

    /// Adds a cluster search path used while resolving the graph for replay.
    pub fn cluster_path(mut self, path: impl AsRef<Path>) -> Self {
        self.cluster_paths.push(path.as_ref().to_path_buf());
        self
    }

    /// Adds an adapter manifest path used to interpret captured external
    /// events.
    pub fn adapter(mut self, path: impl AsRef<Path>) -> Self {
        self.adapter_path = Some(path.as_ref().to_path_buf());
        self
    }
}

#[derive(Debug, Clone)]
/// Replay configuration for an already-loaded [`CaptureBundle`].
///
/// This is useful when the application stores captures outside the local
/// filesystem or has just received a capture from [`ProfileRunner::finish`].
pub struct ReplayBundleConfig {
    bundle: CaptureBundle,
    graph_path: PathBuf,
    cluster_paths: Vec<PathBuf>,
    adapter: Option<AdapterInput>,
}

impl ReplayBundleConfig {
    /// Creates replay configuration from an in-memory capture bundle and graph
    /// path.
    pub fn new(bundle: CaptureBundle, graph_path: impl AsRef<Path>) -> Self {
        Self {
            bundle,
            graph_path: graph_path.as_ref().to_path_buf(),
            cluster_paths: Vec::new(),
            adapter: None,
        }
    }

    /// Adds a cluster search path used while resolving the replay graph.
    pub fn cluster_path(mut self, path: impl AsRef<Path>) -> Self {
        self.cluster_paths.push(path.as_ref().to_path_buf());
        self
    }

    /// Adds an adapter manifest path used to interpret captured external
    /// events.
    pub fn adapter_path(mut self, path: impl AsRef<Path>) -> Self {
        self.adapter = Some(AdapterInput::Path(path.as_ref().to_path_buf()));
        self
    }

    /// Adds an adapter manifest from any supported SDK adapter input.
    pub fn adapter(mut self, adapter: AdapterInput) -> Self {
        self.adapter = Some(adapter);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Summary returned by [`Ergo::validate_project`].
pub struct ProjectSummary {
    /// Project root for filesystem projects, or `None` for in-memory projects.
    pub root: Option<PathBuf>,
    /// Project name from `ergo.toml` or the in-memory snapshot.
    pub name: String,
    /// Project version from `ergo.toml` or the in-memory snapshot.
    pub version: String,
    /// Profile names validated in deterministic map order.
    pub profiles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Capture policy for an in-memory profile.
///
/// Filesystem profiles use their `ergo.toml` capture settings. This type is
/// for SDK-owned in-memory profiles where the application chooses whether the
/// resulting capture stays in memory or is also written to disk.
pub enum ProfileCapture {
    /// Keep the capture bundle in memory only.
    InMemory,
    /// Write the capture to a file, optionally using pretty JSON formatting.
    File {
        /// Output path for the capture JSON.
        path: PathBuf,
        /// Whether to write pretty JSON instead of compact JSON.
        pretty: bool,
    },
}

#[derive(Debug, Clone)]
/// One profile inside an [`InMemoryProjectSnapshot`].
///
/// The profile owns prepared graph assets, ingress configuration, optional
/// adapter and egress configuration, capture policy, and optional stop limits.
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
/// SDK-owned project snapshot for applications that do not want an `ergo.toml`
/// project on disk.
///
/// A snapshot is immutable after build and can be installed on an
/// [`ErgoBuilder`] with [`ErgoBuilder::in_memory_project`].
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
    /// Keeps profile capture data in memory only.
    pub fn in_memory() -> Self {
        Self::InMemory
    }

    /// Writes profile capture data to `path`.
    pub fn file(path: impl AsRef<Path>, pretty: bool) -> Self {
        Self::File {
            path: path.as_ref().to_path_buf(),
            pretty,
        }
    }
}

impl InMemoryProfileConfig {
    /// Creates an in-memory profile that launches a process ingress command.
    ///
    /// Production process ingress requires an adapter before the profile can
    /// run. Add it with [`InMemoryProfileConfig::adapter`].
    pub fn process<I, S>(
        graph_assets: PreparedGraphAssets,
        command: I,
    ) -> Result<Self, ErgoProjectConfigError>
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

    /// Creates an in-memory profile from fixture items.
    ///
    /// `source_label` identifies the synthetic source used for the fixture
    /// stream. The item list and label must both be non-empty.
    pub fn fixture_items(
        graph_assets: PreparedGraphAssets,
        items: Vec<FixtureItem>,
        source_label: impl Into<String>,
    ) -> Result<Self, ErgoProjectConfigError> {
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

    /// Adds an adapter manifest for adapter-bound graphs or production
    /// process ingress.
    pub fn adapter(mut self, adapter: AdapterInput) -> Self {
        self.adapter = Some(adapter);
        self
    }

    /// Adds egress configuration for live effect dispatch.
    pub fn egress_config(mut self, egress_config: EgressConfig) -> Self {
        self.egress_config = Some(egress_config);
        self
    }

    /// Sets how captures from this profile are retained or written.
    pub fn capture(mut self, capture: ProfileCapture) -> Self {
        self.capture = capture;
        self
    }

    /// Stops runs of this profile after the configured wall-clock duration.
    pub fn max_duration(mut self, max_duration: Duration) -> Self {
        self.max_duration = Some(max_duration);
        self
    }

    /// Stops runs of this profile after the configured number of processed
    /// events.
    pub fn max_events(mut self, max_events: u64) -> Self {
        self.max_events = Some(max_events);
        self
    }
}

#[derive(Debug, Clone)]
/// Builder for [`InMemoryProjectSnapshot`].
pub struct InMemoryProjectSnapshotBuilder {
    name: String,
    version: String,
    profiles: BTreeMap<String, InMemoryProfileConfig>,
}

impl InMemoryProjectSnapshotBuilder {
    /// Adds or replaces a named in-memory profile.
    pub fn profile(mut self, name: impl Into<String>, profile: InMemoryProfileConfig) -> Self {
        self.profiles.insert(name.into(), profile);
        self
    }

    /// Validates and builds the snapshot.
    ///
    /// Fails when the project has no profiles or when any profile contains
    /// invalid in-memory construction data.
    pub fn build(self) -> Result<InMemoryProjectSnapshot, ErgoProjectConfigError> {
        InMemoryProjectSnapshot::from_parts(self.name, self.version, self.profiles)
    }
}

impl InMemoryProjectSnapshot {
    /// Starts an in-memory project snapshot builder.
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

    /// Returns the profile names available in this snapshot.
    pub fn profile_names(&self) -> Vec<String> {
        self.profiles.keys().cloned().collect()
    }

    fn resolve_profile(&self, name: &str) -> Result<&InMemoryProfileConfig, ErgoConfigError> {
        self.profiles
            .get(name)
            .ok_or_else(|| ErgoConfigError::ProfileNotFound {
                name: name.to_string(),
            })
    }

    fn from_parts(
        name: String,
        version: String,
        profiles: BTreeMap<String, InMemoryProfileConfig>,
    ) -> Result<Self, ErgoProjectConfigError> {
        if profiles.is_empty() {
            return Err(ErgoProjectConfigError::InMemoryProjectHasNoProfiles);
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

    fn validate(&self) -> Result<(), ErgoProjectConfigError> {
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

/// Builder for an [`Ergo`] engine handle.
///
/// Use the builder when you need custom primitives, an in-memory project, or a
/// filesystem project discovered from a path.
///
/// ```
/// # fn main() -> Result<(), ergo_sdk_rust::ErgoBuildError> {
/// let ergo = ergo_sdk_rust::Ergo::builder().build()?;
/// # let _ = ergo;
/// # Ok(())
/// # }
/// ```
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
    /// Creates a builder with the core Ergo primitive catalog and no project
    /// source.
    pub fn new() -> Self {
        Self {
            catalog_builder: CatalogBuilder::new(),
            project_source: ProjectSource::None,
            project_source_conflict: false,
        }
    }

    /// Uses a filesystem project discovered from `path`.
    ///
    /// `path` may point at the project root, at `ergo.toml`, or at a child
    /// path below the root. Project resolution happens when an operation needs
    /// the project, so this method does not read the filesystem immediately.
    pub fn project_root(mut self, path: impl AsRef<Path>) -> Self {
        self.set_project_source(ProjectSource::Filesystem(path.as_ref().to_path_buf()));
        self
    }

    /// Uses an SDK-owned in-memory project snapshot.
    ///
    /// This is the hermetic alternative to `ergo.toml` for applications that
    /// assemble project assets from memory or another storage layer.
    pub fn in_memory_project(mut self, snapshot: InMemoryProjectSnapshot) -> Self {
        self.set_project_source(ProjectSource::InMemory(snapshot));
        self
    }

    /// Registers a custom source primitive in addition to Ergo's core
    /// primitives.
    pub fn add_source<P>(mut self, primitive: P) -> Self
    where
        P: source::SourcePrimitive + 'static,
    {
        self.catalog_builder.add_source(Box::new(primitive));
        self
    }

    /// Registers a custom compute primitive in addition to Ergo's core
    /// primitives.
    pub fn add_compute<P>(mut self, primitive: P) -> Self
    where
        P: compute::ComputePrimitive + 'static,
    {
        self.catalog_builder.add_compute(Box::new(primitive));
        self
    }

    /// Registers a custom trigger primitive in addition to Ergo's core
    /// primitives.
    pub fn add_trigger<P>(mut self, primitive: P) -> Self
    where
        P: trigger::TriggerPrimitive + 'static,
    {
        self.catalog_builder.add_trigger(Box::new(primitive));
        self
    }

    /// Registers a custom action primitive in addition to Ergo's core
    /// primitives.
    pub fn add_action<P>(mut self, primitive: P) -> Self
    where
        P: action::ActionPrimitive + 'static,
    {
        self.catalog_builder.add_action(Box::new(primitive));
        self
    }

    /// Builds the reusable [`Ergo`] engine handle.
    ///
    /// Fails if primitive registration fails, if the configured in-memory
    /// project is invalid, or if both a filesystem project and in-memory
    /// project were configured on the same builder.
    pub fn build(self) -> Result<Ergo, ErgoBuildError> {
        if self.project_source_conflict {
            return Err(ErgoBuildError::ProjectSourceConflict);
        }
        if let ProjectSource::InMemory(snapshot) = &self.project_source {
            snapshot
                .validate()
                .map_err(|inner| ErgoBuildError::Project {
                    inner: ErgoProjectError::Config { inner },
                })?;
        }
        let (registries, catalog) =
            self.catalog_builder
                .build()
                .map_err(|inner| ErgoBuildError::Registration {
                    source: ErgoErrorSource::new(inner),
                })?;
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
///
/// `Ergo` is **not** `Send + Sync` in v1. Build one handle per thread, or
/// wrap a shared handle in `std::sync::Mutex` to serialize access. See the
/// crate-level *Threading model* section for the rationale and roadmap.
pub struct Ergo {
    runtime_surfaces: RuntimeSurfaces,
    project_source: ProjectSource,
}

// Compile-time guarantee that `StopHandle` remains thread-mobile. The
// supervising-thread stop pattern documented on `StopHandle` requires this
// even though `Ergo` itself is single-threaded today.
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<StopHandle>();
};

impl Ergo {
    /// Starts an [`ErgoBuilder`].
    ///
    /// ```
    /// # fn main() -> Result<(), ergo_sdk_rust::ErgoBuildError> {
    /// let ergo = ergo_sdk_rust::Ergo::builder().build()?;
    /// # let _ = ergo;
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder() -> ErgoBuilder {
        ErgoBuilder::new()
    }

    /// Starts an [`ErgoBuilder`] for a filesystem project.
    ///
    /// Project discovery walks upward from the supplied path until it finds an
    /// `ergo.toml`.
    ///
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let ergo = ergo_sdk_rust::Ergo::from_project("/path/to/my-ergo-project").build()?;
    /// let summary = ergo.validate_project()?;
    /// # let _ = summary;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_project(path: impl AsRef<Path>) -> ErgoBuilder {
        ErgoBuilder::new().project_root(path)
    }

    /// Runs an explicit graph configuration without resolving an `ergo.toml`
    /// profile.
    ///
    /// Use this when your application already has graph paths and ingress
    /// configuration. Profile-based applications usually call
    /// [`Ergo::run_profile`] instead.
    pub fn run(&self, config: RunConfig) -> Result<RunOutcome, ErgoRunError> {
        self.run_with_control(config, RunControl::default())
    }

    /// Runs an explicit graph configuration with a cooperative stop handle.
    ///
    /// The stop handle requests a graceful host stop; the returned
    /// [`RunOutcome`] records whether the run completed or was interrupted.
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
        let request =
            run_request_from_config(&config).map_err(|inner| ErgoRunError::Config { inner })?;

        run_graph_from_paths_with_surfaces_and_control(
            request,
            self.runtime_surfaces.clone(),
            run_control_from_config(&config, control),
        )
        .map_err(map_host_run_error_to_run)
    }

    /// Runs a named project profile.
    ///
    /// The profile supplies graph, ingress, adapter, egress, capture, and run
    /// limit configuration. Filesystem projects resolve profiles from
    /// `ergo.toml`; in-memory projects resolve them from
    /// [`InMemoryProjectSnapshot`].
    ///
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let ergo = ergo_sdk_rust::Ergo::from_project("/path/to/my-ergo-project").build()?;
    /// let outcome = ergo.run_profile("historical")?;
    /// # let _ = outcome;
    /// # Ok(())
    /// # }
    /// ```
    pub fn run_profile(&self, profile_name: &str) -> Result<RunOutcome, ErgoRunError> {
        self.run_profile_with_control(profile_name, RunControl::default())
    }

    /// Runs a named project profile with a cooperative stop handle.
    ///
    /// Use this for services or UIs that need to request graceful shutdown
    /// while the run is blocked in the host loop.
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
            .map_err(InternalProfileError::into_run)?;
        self.run_profile_plan_with_control(plan, control)
    }

    /// Replays a capture file against explicit graph paths.
    ///
    /// Replay is strict: the capture graph identity and event stream must
    /// match what the graph expects.
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
        .map_err(map_host_replay_error)
    }

    /// Replays an already-loaded capture bundle against explicit graph paths.
    ///
    /// Use this when your application stores captures outside the local
    /// filesystem or received a bundle from manual runner finalization.
    ///
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let bundle: ergo_sdk_rust::CaptureBundle = unimplemented!();
    /// let ergo = ergo_sdk_rust::Ergo::builder().build()?;
    /// let replay = ergo.replay_bundle(
    ///     ergo_sdk_rust::ReplayBundleConfig::new(bundle, "graphs/strategy.yaml"),
    /// )?;
    /// # let _ = replay;
    /// # Ok(())
    /// # }
    /// ```
    pub fn replay_bundle(
        &self,
        config: ReplayBundleConfig,
    ) -> Result<ReplayGraphResult, ErgoReplayError> {
        let assets = load_graph_assets_from_paths(&config.graph_path, &config.cluster_paths)
            .map_err(InternalProfileError::HostRun)
            .map_err(InternalProfileError::into_replay)?;
        replay_graph_from_assets_with_surfaces(
            ReplayGraphFromAssetsRequest {
                bundle: config.bundle,
                assets,
                prep: LivePrepOptions::for_fixture(config.adapter, None),
            },
            self.runtime_surfaces.clone(),
        )
        .map_err(map_host_replay_error)
    }

    /// Replays a capture file using the graph and adapter settings from a
    /// named filesystem project profile.
    ///
    /// In-memory profiles do not support path-based `replay_profile`; use
    /// [`Ergo::replay_profile_bundle`] for in-memory projects.
    ///
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let ergo = ergo_sdk_rust::Ergo::from_project("/path/to/my-ergo-project").build()?;
    /// let replay = ergo.replay_profile("historical", "captures/historical.capture.json")?;
    /// # let _ = replay;
    /// # Ok(())
    /// # }
    /// ```
    pub fn replay_profile(
        &self,
        profile_name: &str,
        capture_path: impl AsRef<Path>,
    ) -> Result<ReplayGraphResult, ErgoReplayError> {
        let plan = self
            .resolve_profile_plan(profile_name)
            .map_err(InternalProfileError::into_replay)?;
        match plan.runner_source {
            RunnerSource::Paths(request) => self.replay(ReplayConfig {
                capture_path: capture_path.as_ref().to_path_buf(),
                graph_path: request.graph_path,
                cluster_paths: request.cluster_paths,
                adapter_path: request.adapter_path,
            }),
            RunnerSource::Assets { .. } => Err(ErgoReplayError::Config {
                inner: ErgoConfigError::UnsupportedOperation {
                    operation: "replay_profile",
                    transport: "in-memory",
                },
            }),
        }
    }

    /// Replays an already-loaded capture bundle using a named project profile.
    ///
    /// This is the replay entry point that works for both filesystem and
    /// in-memory projects.
    pub fn replay_profile_bundle(
        &self,
        profile_name: &str,
        bundle: CaptureBundle,
    ) -> Result<ReplayGraphResult, ErgoReplayError> {
        let plan = self
            .resolve_profile_plan(profile_name)
            .map_err(InternalProfileError::into_replay)?;
        self.replay_profile_bundle_from_plan(plan, bundle)
    }

    /// Validates the configured project and all of its profiles without
    /// running ingress.
    ///
    /// The result summarizes the project name, version, and profile list.
    /// Failure is categorized as [`ErgoValidationError`] so callers can
    /// distinguish SDK configuration mistakes from host preflight failures.
    pub fn validate_project(&self) -> Result<ProjectSummary, ErgoValidationError> {
        match &self.project_source {
            ProjectSource::Filesystem(_) => {
                let project = self
                    .load_project()
                    .map_err(|inner| ErgoValidationError::Config { inner })?;
                let profiles = project.profile_names();
                for profile_name in &profiles {
                    let plan = Self::resolve_profile_plan_from_project(&project, profile_name)
                        .map_err(|err| err.into_validation(profile_name))?;
                    self.validate_profile_plan(&plan)
                        .map_err(|err| err.into_validation(profile_name))?;
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
                    let plan = self
                        .resolve_profile_plan(profile_name)
                        .map_err(|err| err.into_validation(profile_name))?;
                    self.validate_profile_plan(&plan)
                        .map_err(|err| err.into_validation(profile_name))?;
                }

                Ok(ProjectSummary {
                    root: None,
                    name: project.name.clone(),
                    version: project.version.clone(),
                    profiles,
                })
            }
            ProjectSource::None => Err(ErgoValidationError::Config {
                inner: ErgoConfigError::ProjectNotConfigured,
            }),
        }
    }

    fn load_project(&self) -> Result<ResolvedProject, ErgoConfigError> {
        let ProjectSource::Filesystem(project_root) = &self.project_source else {
            return Err(ErgoConfigError::ProjectNotConfigured);
        };
        load_project(project_root).map_err(ErgoConfigError::from)
    }

    fn resolve_profile_plan(
        &self,
        profile_name: &str,
    ) -> Result<ResolvedProfilePlan, InternalProfileError> {
        match &self.project_source {
            ProjectSource::Filesystem(_) => {
                let project = self.load_project()?;
                Self::resolve_profile_plan_from_project(&project, profile_name)
            }
            ProjectSource::InMemory(project) => {
                let profile = project.resolve_profile(profile_name)?;
                resolve_profile_plan_from_in_memory_profile(profile_name, profile)
            }
            ProjectSource::None => Err(InternalProfileError::Config(
                ErgoConfigError::ProjectNotConfigured,
            )),
        }
    }

    fn resolve_profile_plan_from_project(
        project: &ResolvedProject,
        profile_name: &str,
    ) -> Result<ResolvedProfilePlan, InternalProfileError> {
        let resolved = project
            .resolve_run_profile(profile_name)
            .map_err(ErgoConfigError::from)?;
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
                        return Err(ErgoRunError::Config {
                            inner: ErgoConfigError::FilesystemProfileCannotUseInMemoryCapture {
                                profile: profile_name,
                            },
                        });
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
                .map_err(map_host_run_error_to_run)
            }
            RunnerSource::Assets { assets, prep } => {
                let host_capture = host_capture_policy_from_plan(&capture)
                    .map_err(|inner| ErgoRunError::Config { inner })?;
                run_graph_from_assets_with_surfaces_and_control(
                    RunGraphFromAssetsRequest {
                        assets,
                        prep,
                        driver,
                        capture: host_capture,
                    },
                    self.runtime_surfaces.clone(),
                    apply_profile_limits(control, max_duration, max_events),
                )
                .map_err(map_host_run_error_to_run)
            }
        }
    }

    fn validate_profile_plan(
        &self,
        plan: &ResolvedProfilePlan,
    ) -> Result<(), InternalProfileError> {
        match (&plan.runner_source, &plan.capture) {
            (RunnerSource::Paths(_request), CapturePlan::InMemory) => {
                Err(InternalProfileError::Config(
                    ErgoConfigError::FilesystemProfileCannotUseInMemoryCapture {
                        profile: plan.profile_name.clone(),
                    },
                ))
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
                .map_err(InternalProfileError::HostRun)
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
                .map_err(InternalProfileError::HostRun)
            }
            (RunnerSource::Assets { assets, prep }, capture) => {
                let host_capture =
                    host_capture_policy_from_plan(capture).map_err(InternalProfileError::Config)?;
                validate_run_graph_from_assets_with_surfaces(
                    RunGraphFromAssetsRequest {
                        assets: assets.clone(),
                        prep: prep.clone(),
                        driver: plan.driver.clone(),
                        capture: host_capture,
                    },
                    self.runtime_surfaces.clone(),
                )
                .map_err(InternalProfileError::HostRun)
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
                        .map_err(InternalProfileError::HostRun)
                        .map_err(InternalProfileError::into_replay)?;
                replay_graph_from_assets_with_surfaces(
                    ReplayGraphFromAssetsRequest {
                        bundle,
                        assets,
                        prep: LivePrepOptions::for_fixture(
                            request.adapter_path.map(AdapterInput::Path),
                            None,
                        ),
                    },
                    self.runtime_surfaces.clone(),
                )
                .map_err(map_host_replay_error)
            }
            RunnerSource::Assets { assets, prep } => replay_graph_from_assets_with_surfaces(
                ReplayGraphFromAssetsRequest {
                    bundle,
                    assets,
                    prep: LivePrepOptions::for_fixture(prep.adapter, None),
                },
                self.runtime_surfaces.clone(),
            )
            .map_err(map_host_replay_error),
        }
    }

    /// Prepares a manual runner for a named profile.
    ///
    /// The returned [`ProfileRunner`] lets the application feed
    /// [`HostedEvent`] values one at a time, inspect context, and finalize the
    /// capture bundle. Preparing the runner resolves the profile and validates
    /// setup, but does not launch process ingress.
    ///
    /// ```no_run
    /// use ergo_sdk_rust::{Ergo, EventTime, ExternalEventKind, HostedEvent};
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let ergo = Ergo::from_project("/path/to/my-ergo-project").build()?;
    ///     let mut runner = ergo.runner_for_profile("manual")?;
    ///
    ///     let _outcome = runner.step(HostedEvent {
    ///         event_id: "evt-1".to_string(),
    ///         kind: ExternalEventKind::Command,
    ///         at: EventTime::default(),
    ///         semantic_kind: Some("tick".to_string()),
    ///         payload: Some(serde_json::json!({})),
    ///     })?;
    ///
    ///     let capture = runner.finish()?;
    ///     let _ = capture;
    ///     Ok(())
    /// }
    /// ```
    pub fn runner_for_profile(&self, profile_name: &str) -> Result<ProfileRunner, ErgoRunnerError> {
        let plan = self
            .resolve_profile_plan(profile_name)
            .map_err(InternalProfileError::into_runner)?;
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
                .map_err(map_host_run_error_to_runner)?;
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
                .map_err(map_host_run_error_to_runner)?;
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

/// Manual profile runner for applications that own event delivery.
///
/// A `ProfileRunner` is prepared from a normal project profile, then driven
/// with [`ProfileRunner::step`]. Finish it exactly once with
/// [`ProfileRunner::finish`] or [`ProfileRunner::finish_and_write_capture`]
/// to recover the capture bundle.
///
/// # Lifecycle
///
/// A runner moves through four internal states. Callers interact with them
/// only through the return values of `step`, `finish`, and
/// `finish_and_write_capture`; transitions are driven entirely by those
/// return values.
///
/// 1. **Active** — the initial state after [`Ergo::runner_for_profile`].
///    [`ProfileRunner::step`] accepts events. Recoverable input or binding
///    errors (see [`ErgoStepError::is_recoverable`]) leave the runner in
///    `Active`.
/// 2. **Finalizable-after-dispatch-failure** — entered when
///    [`ProfileRunner::step`] returns an [`ErgoStepError`] whose
///    [`ErgoStepError::can_finish`] is `true` (typically an egress dispatch
///    failure mid-step). Further `step` calls fail with a lifecycle error;
///    the only legal next call is `finish` or `finish_and_write_capture`.
/// 3. **Failed** — entered when [`ProfileRunner::step`] returns a
///    non-recoverable, non-finalizable error. Both `step` and `finish` then
///    fail with a lifecycle error. The runner cannot be revived; drop it.
/// 4. **Finished** — entered after a successful `finish` or
///    `finish_and_write_capture`. Any further call returns a lifecycle
///    [`ErgoStepError`].
///
/// `finish` additionally requires at least one successful step before it
/// will produce a capture bundle; calling `finish` from `Active` with zero
/// successful steps returns a lifecycle error rather than transitioning.
///
/// ```no_run
/// use ergo_sdk_rust::{Ergo, EventTime, ExternalEventKind, HostedEvent};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let ergo = Ergo::from_project("/path/to/my-ergo-project").build()?;
///     let mut runner = ergo.runner_for_profile("manual")?;
///
///     let _step = runner.step(HostedEvent {
///         event_id: "evt-1".to_string(),
///         kind: ExternalEventKind::Command,
///         at: EventTime::default(),
///         semantic_kind: Some("tick".to_string()),
///         payload: Some(serde_json::json!({})),
///     })?;
///
///     let capture = runner.finish()?;
///     let _ = capture;
///     Ok(())
/// }
/// ```
pub struct ProfileRunner {
    runner: Option<HostedRunner>,
    capture: CapturePlan,
    state: ProfileRunnerState,
    successful_steps: usize,
}

impl ProfileRunner {
    /// Applies one hosted event to the prepared profile.
    ///
    /// Recoverable input and binding failures leave the runner available for a
    /// later `step` call. Non-recoverable failures block further stepping.
    /// Egress dispatch failures may still permit finalization; check
    /// [`ErgoStepError::can_finish`].
    pub fn step(&mut self, event: HostedEvent) -> Result<HostedStepOutcome, ErgoStepError> {
        match self.state {
            ProfileRunnerState::Active => {}
            ProfileRunnerState::FinalizableAfterDispatchFailure => {
                return Err(map_hosted_step_error(lifecycle_violation(
                    "profile runner must be finalized after egress dispatch failure before stepping again",
                )));
            }
            ProfileRunnerState::Failed => {
                return Err(map_hosted_step_error(lifecycle_violation(
                    "profile runner cannot continue after a non-finalizable step error",
                )));
            }
            ProfileRunnerState::Finished => {
                return Err(map_hosted_step_error(lifecycle_violation(
                    "profile runner is already finished",
                )));
            }
        }

        let runner = self.runner.as_mut().ok_or_else(|| {
            map_hosted_step_error(lifecycle_violation(
                "internal: active profile runner must hold hosted runner",
            ))
        })?;
        match runner.step(event) {
            Ok(outcome) => {
                self.successful_steps += 1;
                Ok(outcome)
            }
            Err(err) => {
                let mapped = map_hosted_step_error(err);
                if mapped.can_finish() {
                    self.state = ProfileRunnerState::FinalizableAfterDispatchFailure;
                } else if !mapped.is_recoverable() {
                    self.state = ProfileRunnerState::Failed;
                }
                Err(mapped)
            }
        }
    }

    /// Borrows the current adapter context snapshot.
    ///
    /// The snapshot is available until the runner is finished. After
    /// finalization, this returns a lifecycle [`ErgoStepError`].
    pub fn context_snapshot(
        &self,
    ) -> Result<&std::collections::BTreeMap<String, serde_json::Value>, ErgoStepError> {
        let Some(runner) = self.runner.as_ref() else {
            return Err(map_hosted_step_error(lifecycle_violation(
                "profile runner is already finished",
            )));
        };
        Ok(runner.context_snapshot())
    }

    /// Returns the configured capture output path, if this profile writes one.
    pub fn capture_output_path(&self) -> Option<&Path> {
        self.capture.configured_path()
    }

    /// Returns whether the configured capture output should be pretty JSON.
    pub fn pretty_capture(&self) -> bool {
        self.capture.pretty()
    }

    /// Finalizes the runner and returns the capture bundle without writing it.
    ///
    /// The runner must have completed at least one successful step. After this
    /// call, the runner is finished and cannot step or finish again.
    pub fn finish(&mut self) -> Result<CaptureBundle, ErgoStepError> {
        match self.state {
            ProfileRunnerState::Finished => {
                return Err(map_hosted_step_error(lifecycle_violation(
                    "profile runner is already finished",
                )));
            }
            ProfileRunnerState::Failed => {
                return Err(map_hosted_step_error(lifecycle_violation(
                    "profile runner cannot finalize after a non-finalizable step error",
                )));
            }
            ProfileRunnerState::Active if self.successful_steps == 0 => {
                return Err(map_hosted_step_error(lifecycle_violation(
                    "profile runner cannot finalize before the first successful step",
                )));
            }
            ProfileRunnerState::Active | ProfileRunnerState::FinalizableAfterDispatchFailure => {}
        }

        self.state = ProfileRunnerState::Finished;
        let runner = self.runner.take().ok_or_else(|| {
            map_hosted_step_error(lifecycle_violation(
                "internal: unfinished profile runner must hold hosted runner",
            ))
        })?;
        finalize_hosted_runner_capture(runner, false).map_err(map_hosted_step_error)
    }

    /// Finalizes the runner, writes the capture to the profile's configured
    /// capture file, and returns the same bundle.
    ///
    /// If finalization succeeds but the filesystem write fails, the returned
    /// [`ErgoCaptureError`] may still expose the generated bundle through
    /// [`ErgoCaptureError::capture_bundle`].
    pub fn finish_and_write_capture(&mut self) -> Result<CaptureBundle, ErgoCaptureError> {
        let capture_path = match &self.capture {
            CapturePlan::ExplicitFile { path, .. } => path.clone(),
            CapturePlan::DefaultFile { .. } | CapturePlan::InMemory => {
                return Err(ErgoCaptureError::OutputNotConfigured);
            }
        };
        let bundle = self
            .finish()
            .map_err(|inner| ErgoCaptureError::Finalize { inner })?;
        let style = if self.capture.pretty() {
            CaptureJsonStyle::Pretty
        } else {
            CaptureJsonStyle::Compact
        };
        match host_write_capture_bundle(&capture_path, &bundle, style) {
            Ok(()) => Ok(bundle),
            Err(inner) => Err(ErgoCaptureError::Write {
                source: ErgoErrorSource::new(inner),
                bundle: Some(bundle),
            }),
        }
    }
}

/// Writes a capture bundle to disk through the SDK error surface.
///
/// Returns [`ErgoCaptureError::Write`] on filesystem failure with no recovered
/// bundle attached, because the caller already owns the bundle they passed in.
pub fn write_capture_bundle(
    path: impl AsRef<Path>,
    bundle: &CaptureBundle,
    style: CaptureJsonStyle,
) -> Result<(), ErgoCaptureError> {
    host_write_capture_bundle(path.as_ref(), bundle, style).map_err(|inner| {
        ErgoCaptureError::Write {
            source: ErgoErrorSource::new(inner),
            bundle: None,
        }
    })
}

fn profile_name_option(profile_name: Option<&str>) -> Option<String> {
    profile_name.map(ToOwned::to_owned)
}

fn validate_in_memory_profile_config(
    profile_name: Option<&str>,
    profile: &InMemoryProfileConfig,
) -> Result<(), ErgoProjectConfigError> {
    match &profile.ingress {
        InMemoryIngress::FixtureItems {
            items: _,
            source_label,
        } if source_label.trim().is_empty() => {
            return Err(ErgoProjectConfigError::InMemoryFixtureSourceLabelEmpty {
                profile: profile_name_option(profile_name),
            });
        }
        InMemoryIngress::FixtureItems { items, .. } if items.is_empty() => {
            return Err(ErgoProjectConfigError::InMemoryFixtureItemsEmpty {
                profile: profile_name_option(profile_name),
            });
        }
        InMemoryIngress::Process { command } if command.is_empty() => {
            return Err(ErgoProjectConfigError::InMemoryProcessCommandEmpty {
                profile: profile_name_option(profile_name),
            });
        }
        InMemoryIngress::Process { command }
            if command
                .first()
                .map(|program| program.trim().is_empty())
                .unwrap_or(false) =>
        {
            return Err(ErgoProjectConfigError::InMemoryProcessExecutableBlank {
                profile: profile_name_option(profile_name),
            });
        }
        _ => {}
    }

    Ok(())
}

fn live_prep_options_from_in_memory_profile(
    profile: &InMemoryProfileConfig,
) -> Result<LivePrepOptions, InternalProfileError> {
    match &profile.ingress {
        InMemoryIngress::FixtureItems { .. } => Ok(LivePrepOptions::for_fixture(
            profile.adapter.clone(),
            profile.egress_config.clone(),
        )),
        InMemoryIngress::Process { .. } => match profile.adapter.clone() {
            Some(adapter) => Ok(LivePrepOptions::for_production(
                adapter,
                profile.egress_config.clone(),
            )),
            None => Err(InternalProfileError::HostRun(
                ergo_host::HostRunError::ProductionRequiresAdapter,
            )),
        },
    }
}

fn driver_from_in_memory_ingress(ingress: &InMemoryIngress) -> DriverConfig {
    match ingress {
        InMemoryIngress::Process { command } => DriverConfig::Process {
            command: command.clone(),
        },
        InMemoryIngress::FixtureItems {
            items,
            source_label,
        } => DriverConfig::FixtureItems {
            items: items.clone(),
            source_label: source_label.clone(),
        },
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
) -> Result<PrepareHostedRunnerFromPathsRequest, InternalProfileError> {
    let egress_config = profile
        .egress_config_path
        .as_deref()
        .map(load_egress_config)
        .transpose()
        .map_err(InternalProfileError::Config)?;
    match &profile.ingress {
        ResolvedProjectIngress::Fixture { .. } => {
            Ok(PrepareHostedRunnerFromPathsRequest::for_fixture(
                profile.graph_path.clone(),
                profile.cluster_paths.clone(),
                profile.adapter_path.clone(),
                egress_config,
            ))
        }
        ResolvedProjectIngress::Process { .. } => match &profile.adapter_path {
            Some(adapter_path) => Ok(PrepareHostedRunnerFromPathsRequest::for_production(
                profile.graph_path.clone(),
                profile.cluster_paths.clone(),
                adapter_path.clone(),
                egress_config,
            )),
            None => Err(InternalProfileError::HostRun(
                ergo_host::HostRunError::ProductionRequiresAdapter,
            )),
        },
    }
}

fn resolve_profile_plan_from_resolved_profile(
    profile_name: &str,
    profile: ResolvedProjectProfile,
) -> Result<ResolvedProfilePlan, InternalProfileError> {
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
) -> Result<ResolvedProfilePlan, InternalProfileError> {
    validate_in_memory_profile_config(Some(profile_name), profile)
        .map_err(|inner| InternalProfileError::Config(ErgoConfigError::ProjectConfig { inner }))?;
    Ok(ResolvedProfilePlan {
        profile_name: profile_name.to_string(),
        runner_source: RunnerSource::Assets {
            assets: profile.graph_assets.clone(),
            prep: live_prep_options_from_in_memory_profile(profile)?,
        },
        driver: driver_from_in_memory_ingress(&profile.ingress),
        capture: capture_plan_from_in_memory_profile(profile),
        max_duration: profile.max_duration,
        max_events: profile.max_events,
    })
}

fn host_capture_policy_from_plan(
    capture: &CapturePlan,
) -> Result<ergo_host::CapturePolicy, ErgoConfigError> {
    match capture {
        CapturePlan::InMemory => Ok(ergo_host::CapturePolicy::InMemory),
        CapturePlan::ExplicitFile { path, pretty } => Ok(ergo_host::CapturePolicy::File {
            path: path.clone(),
            pretty: *pretty,
        }),
        CapturePlan::DefaultFile { .. } => {
            Err(ErgoConfigError::InMemoryAssetsCannotUseDefaultFilesystemCapture)
        }
    }
}

fn lifecycle_violation(detail: impl Into<String>) -> HostedStepError {
    HostedStepError::LifecycleViolation {
        detail: detail.into(),
    }
}

fn run_request_from_config(
    config: &RunConfig,
) -> Result<RunGraphFromPathsRequest, ErgoConfigError> {
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

fn ingress_to_driver(ingress: &IngressConfig) -> Result<DriverConfig, ErgoConfigError> {
    match ingress {
        IngressConfig::Fixture { path } => Ok(DriverConfig::Fixture { path: path.clone() }),
        IngressConfig::Process { command } => {
            if command.is_empty() {
                return Err(ErgoConfigError::ExplicitRunProcessCommandEmpty);
            }
            Ok(DriverConfig::Process {
                command: command.clone(),
            })
        }
    }
}

fn load_egress_config(path: &Path) -> Result<EgressConfig, ErgoConfigError> {
    let raw = fs::read_to_string(path).map_err(|source| ErgoConfigError::EgressConfigRead {
        path: path.to_path_buf(),
        source,
    })?;
    parse_egress_config_toml(&raw).map_err(|source| ErgoConfigError::EgressConfigParse {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests;
