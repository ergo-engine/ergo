//! error
//!
//! Purpose:
//! - Define the SDK-branded `Ergo*` error surface that wraps host, loader, and
//!   runtime taxonomies behind stable categories for crates.io callers.
//!
//! Owns:
//! - `ErgoBuildError`, `ErgoProjectConfigError`, `ErgoProjectError`,
//!   `ErgoConfigError`, `ErgoRunError`, `ErgoRunnerError`, `ErgoStepError`,
//!   `ErgoCaptureError`, `ErgoReplayError`, and `ErgoValidationError`.
//! - Mapping from host taxonomies into SDK categories per the PUB-1 plan.
//! - Parent-only accessors that expose host source detail without leaking leaf
//!   types into the SDK root surface.
//!
//! Does not own:
//! - Host orchestration semantics or step categorization rules.
//! - Capture write atomicity or runtime registration semantics.
//!
//! Connects to:
//! - `lib.rs`, which uses these types as the public return-error surface for
//!   `Ergo`, `ErgoBuilder`, and `ProfileRunner` operations.
//! - `ergo_host`, `ergo_loader`, and `ergo_runtime` whose error types are
//!   preserved as `#[source]` chain entries.
//!
//! Safety notes:
//! - Mapping from host taxonomies is exhaustive over today's variants and
//!   collapses unknown variants to an `Internal` SDK category so semver growth
//!   in host enums does not silently change SDK categorization.
//! - Parent accessors are intentionally limited to the immediate wrapped host
//!   type; nested host detail remains reachable only by walking `source()`.

use std::path::PathBuf;

use ergo_host::{
    CaptureBundle, CaptureWriteError, EgressConfigParseError, HostAdapterSetupError,
    HostReplayError, HostReplaySetupError, HostRunError, HostSetupError, HostedEventBuildError,
    HostedReplayError, HostedStepError,
};
use ergo_loader::ProjectError as LoaderProjectError;
use ergo_runtime::catalog::CoreRegistrationError;

#[derive(Debug)]
#[non_exhaustive]
/// Errors returned while building an [`Ergo`](crate::Ergo) engine handle.
pub enum ErgoBuildError {
    /// The runtime primitive catalog could not be registered.
    Registration {
        #[allow(missing_docs)]
        inner: CoreRegistrationError,
    },
    /// The configured in-memory project snapshot is invalid.
    Project {
        #[allow(missing_docs)]
        inner: ErgoProjectError,
    },
    /// The builder was configured with both filesystem and in-memory project
    /// sources.
    ProjectSourceConflict,
}

impl std::fmt::Display for ErgoBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Registration { inner } => write!(f, "primitive registration failed: {inner}"),
            Self::Project { inner } => write!(f, "{inner}"),
            Self::ProjectSourceConflict => write!(
                f,
                "project_root and in_memory_project are mutually exclusive"
            ),
        }
    }
}

impl std::error::Error for ErgoBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Registration { inner } => Some(inner),
            Self::Project { inner } => Some(inner),
            Self::ProjectSourceConflict => None,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
/// Validation errors for SDK-owned in-memory project/profile construction.
///
/// These errors happen while assembling an [`InMemoryProjectSnapshot`](crate::InMemoryProjectSnapshot),
/// before a run, replay, or validation operation resolves a profile.
pub enum ErgoProjectConfigError {
    /// The snapshot builder has no profiles.
    InMemoryProjectHasNoProfiles,
    /// A fixture-items profile has an empty source label.
    InMemoryFixtureSourceLabelEmpty {
        /// Profile name when validation happened inside a project snapshot.
        profile: Option<String>,
    },
    /// A fixture-items profile has no fixture items.
    InMemoryFixtureItemsEmpty {
        /// Profile name when validation happened inside a project snapshot.
        profile: Option<String>,
    },
    /// A process-ingress profile has an empty command vector.
    InMemoryProcessCommandEmpty {
        /// Profile name when validation happened inside a project snapshot.
        profile: Option<String>,
    },
    /// A process-ingress profile has a blank executable string.
    InMemoryProcessExecutableBlank {
        /// Profile name when validation happened inside a project snapshot.
        profile: Option<String>,
    },
}

impl std::fmt::Display for ErgoProjectConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InMemoryProjectHasNoProfiles => write!(
                f,
                "in-memory project snapshot must declare at least one profile"
            ),
            Self::InMemoryFixtureSourceLabelEmpty { profile: Some(profile) } => write!(
                f,
                "in-memory project profile '{profile}' fixture ingress source_label must not be empty"
            ),
            Self::InMemoryFixtureSourceLabelEmpty { profile: None } => {
                write!(f, "fixture ingress source_label must not be empty")
            }
            Self::InMemoryFixtureItemsEmpty { profile: Some(profile) } => write!(
                f,
                "in-memory project profile '{profile}' fixture ingress must declare at least one item"
            ),
            Self::InMemoryFixtureItemsEmpty { profile: None } => {
                write!(f, "fixture ingress must declare at least one item")
            }
            Self::InMemoryProcessCommandEmpty { profile: Some(profile) } => write!(
                f,
                "in-memory project profile '{profile}' process ingress command must not be empty"
            ),
            Self::InMemoryProcessCommandEmpty { profile: None } => {
                write!(f, "process ingress command must not be empty")
            }
            Self::InMemoryProcessExecutableBlank { profile: Some(profile) } => write!(
                f,
                "in-memory project profile '{profile}' process ingress executable must not be empty"
            ),
            Self::InMemoryProcessExecutableBlank { profile: None } => {
                write!(f, "process ingress executable must not be empty")
            }
        }
    }
}

impl std::error::Error for ErgoProjectConfigError {}

#[derive(Debug)]
#[non_exhaustive]
/// Errors returned while assembling an SDK-owned in-memory project.
pub enum ErgoProjectError {
    /// The project failed construction-time profile validation.
    Config {
        #[allow(missing_docs)]
        inner: ErgoProjectConfigError,
    },
}

impl std::fmt::Display for ErgoProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config { inner } => write!(f, "{inner}"),
        }
    }
}

impl std::error::Error for ErgoProjectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config { inner } => Some(inner),
        }
    }
}

impl From<ErgoProjectConfigError> for ErgoProjectError {
    fn from(inner: ErgoProjectConfigError) -> Self {
        Self::Config { inner }
    }
}

#[derive(Debug)]
#[non_exhaustive]
/// Errors returned while resolving SDK, project, profile, or run
/// configuration.
pub enum ErgoConfigError {
    /// The operation requires a project but the engine has no project source.
    ProjectNotConfigured,
    /// The requested profile name does not exist in the configured project.
    ProfileNotFound {
        /// Requested profile name.
        name: String,
    },
    /// The filesystem project could not be discovered, read, or decoded.
    ProjectLoad {
        #[allow(missing_docs)]
        inner: LoaderProjectError,
    },
    /// An in-memory project/profile construction rule failed during operation
    /// preparation.
    ProjectConfig {
        #[allow(missing_docs)]
        inner: ErgoProjectConfigError,
    },
    /// An explicit [`RunConfig`](crate::RunConfig) process ingress command was
    /// empty.
    ExplicitRunProcessCommandEmpty,
    /// An egress TOML file could not be read.
    EgressConfigRead {
        /// Path to the egress config file.
        path: PathBuf,
        #[allow(missing_docs)]
        inner: std::io::Error,
    },
    /// An egress TOML file could not be parsed or validated.
    EgressConfigParse {
        /// Path to the egress config file.
        path: PathBuf,
        #[allow(missing_docs)]
        inner: EgressConfigParseError,
    },
    /// A filesystem-backed profile tried to request in-memory-only capture
    /// behavior.
    FilesystemProfileCannotUseInMemoryCapture {
        /// Profile that requested the unsupported capture mode.
        profile: String,
    },
    /// An in-memory graph-assets profile tried to use the filesystem default
    /// capture path.
    InMemoryAssetsCannotUseDefaultFilesystemCapture,
    /// The requested operation is not supported for the current project
    /// transport.
    UnsupportedOperation {
        /// Operation name.
        operation: &'static str,
        /// Project transport or source kind.
        transport: &'static str,
    },
    /// Replay was configured with live egress behavior, which replay never
    /// dispatches.
    LiveEgressConfigurationNotAllowed,
}

impl std::fmt::Display for ErgoConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProjectNotConfigured => write!(
                f,
                "a project source must be configured for project/profile operations"
            ),
            Self::ProfileNotFound { name } => write!(f, "project profile '{name}' does not exist"),
            Self::ProjectLoad { inner } => write!(f, "{inner}"),
            Self::ProjectConfig { inner } => write!(f, "{inner}"),
            Self::ExplicitRunProcessCommandEmpty => write!(
                f,
                "explicit run configuration is invalid: process ingress command must not be empty"
            ),
            Self::EgressConfigRead { path, inner } => write!(
                f,
                "failed to read egress config '{}': {inner}",
                path.display()
            ),
            Self::EgressConfigParse { path, inner } => write!(
                f,
                "failed to parse egress config '{}': {inner}",
                path.display()
            ),
            Self::FilesystemProfileCannotUseInMemoryCapture { profile } => write!(
                f,
                "filesystem profile '{profile}' cannot use in-memory capture"
            ),
            Self::InMemoryAssetsCannotUseDefaultFilesystemCapture => write!(
                f,
                "default filesystem capture cannot be applied to in-memory graph assets"
            ),
            Self::UnsupportedOperation {
                operation,
                transport,
            } => write!(
                f,
                "operation '{operation}' is not supported for {transport} projects"
            ),
            Self::LiveEgressConfigurationNotAllowed => {
                write!(f, "replay does not accept live egress configuration")
            }
        }
    }
}

impl std::error::Error for ErgoConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ProjectLoad { inner } => Some(inner),
            Self::ProjectConfig { inner } => Some(inner),
            Self::EgressConfigRead { inner, .. } => Some(inner),
            Self::EgressConfigParse { inner, .. } => Some(inner),
            Self::ProjectNotConfigured
            | Self::ProfileNotFound { .. }
            | Self::ExplicitRunProcessCommandEmpty
            | Self::FilesystemProfileCannotUseInMemoryCapture { .. }
            | Self::InMemoryAssetsCannotUseDefaultFilesystemCapture
            | Self::UnsupportedOperation { .. }
            | Self::LiveEgressConfigurationNotAllowed => None,
        }
    }
}

impl From<LoaderProjectError> for ErgoConfigError {
    fn from(value: LoaderProjectError) -> Self {
        match value {
            LoaderProjectError::ProfileNotFound { name } => Self::ProfileNotFound { name },
            other => Self::ProjectLoad { inner: other },
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
/// Errors returned while manually stepping a [`ProfileRunner`](crate::ProfileRunner).
///
/// Recoverable variants allow another step attempt; check
/// [`ErgoStepError::is_recoverable`]. Egress dispatch failures may still allow
/// finalization; check [`ErgoStepError::can_finish`].
pub enum ErgoStepError {
    /// The supplied hosted event was structurally invalid for stepping.
    Input {
        #[allow(missing_docs)]
        inner: HostedStepError,
    },
    /// The hosted event could not be converted into the graph event format.
    EventBuild {
        #[allow(missing_docs)]
        inner: HostedEventBuildError,
    },
    /// Event-to-graph binding failed.
    Binding {
        #[allow(missing_docs)]
        inner: HostedStepError,
    },
    /// The runner was used in an invalid lifecycle state.
    Lifecycle {
        #[allow(missing_docs)]
        inner: HostedStepError,
    },
    /// Runtime effect application failed.
    EffectApply {
        #[allow(missing_docs)]
        inner: HostedStepError,
    },
    /// The adapter or egress configuration did not cover a produced handler.
    HandlerCoverage {
        #[allow(missing_docs)]
        inner: HostedStepError,
    },
    /// Egress intent validation failed before dispatch.
    EgressValidation {
        #[allow(missing_docs)]
        inner: HostedStepError,
    },
    /// Egress process-channel startup or communication failed.
    EgressProcess {
        #[allow(missing_docs)]
        inner: HostedStepError,
    },
    /// Egress dispatch failed after the step produced an intent.
    EgressDispatch {
        #[allow(missing_docs)]
        inner: HostedStepError,
    },
    /// A host or SDK invariant failed; report this as a bug rather than user
    /// configuration feedback.
    Internal {
        #[allow(missing_docs)]
        inner: HostedStepError,
    },
}

impl std::fmt::Display for ErgoStepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input { inner }
            | Self::Binding { inner }
            | Self::Lifecycle { inner }
            | Self::EffectApply { inner }
            | Self::HandlerCoverage { inner }
            | Self::EgressValidation { inner }
            | Self::EgressProcess { inner }
            | Self::EgressDispatch { inner }
            | Self::Internal { inner } => write!(f, "{inner}"),
            Self::EventBuild { inner } => write!(f, "event build failed: {inner}"),
        }
    }
}

impl std::error::Error for ErgoStepError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Input { inner }
            | Self::Binding { inner }
            | Self::Lifecycle { inner }
            | Self::EffectApply { inner }
            | Self::HandlerCoverage { inner }
            | Self::EgressValidation { inner }
            | Self::EgressProcess { inner }
            | Self::EgressDispatch { inner }
            | Self::Internal { inner } => Some(inner),
            Self::EventBuild { inner } => Some(inner),
        }
    }
}

impl ErgoStepError {
    /// Returns true when the underlying host step failure is recoverable for
    /// continued stepping (input, binding, or event-build categories).
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::Input { .. } | Self::Binding { .. } | Self::EventBuild { .. }
        )
    }

    /// Returns true when finalization is allowed even though further stepping
    /// is blocked (egress dispatch failure).
    pub fn can_finish(&self) -> bool {
        matches!(self, Self::EgressDispatch { .. })
    }

    /// Parent-only escape hatch returning the underlying host step error.
    pub fn as_hosted_step_error(&self) -> Option<&HostedStepError> {
        match self {
            Self::Input { inner }
            | Self::Binding { inner }
            | Self::Lifecycle { inner }
            | Self::EffectApply { inner }
            | Self::HandlerCoverage { inner }
            | Self::EgressValidation { inner }
            | Self::EgressProcess { inner }
            | Self::EgressDispatch { inner }
            | Self::Internal { inner } => Some(inner),
            Self::EventBuild { .. } => None,
        }
    }

    /// Parent-only escape hatch returning the underlying host event-build
    /// error when the step failure happened during event construction.
    pub fn as_hosted_event_build_error(&self) -> Option<&HostedEventBuildError> {
        match self {
            Self::EventBuild { inner } => Some(inner),
            _ => None,
        }
    }
}

pub(crate) fn map_hosted_step_error(err: HostedStepError) -> ErgoStepError {
    match &err {
        HostedStepError::DuplicateEventId { .. }
        | HostedStepError::MissingSemanticKind
        | HostedStepError::MissingPayload
        | HostedStepError::PayloadMustBeObject
        | HostedStepError::UnknownSemanticKind { .. } => ErgoStepError::Input { inner: err },
        HostedStepError::Binding(_) => ErgoStepError::Binding { inner: err },
        HostedStepError::EventBuild(_) => match err {
            HostedStepError::EventBuild(inner) => ErgoStepError::EventBuild { inner },
            _ => unreachable!(),
        },
        HostedStepError::LifecycleViolation { .. } => ErgoStepError::Lifecycle { inner: err },
        HostedStepError::EffectApply(_) => ErgoStepError::EffectApply { inner: err },
        HostedStepError::HandlerCoverage(_) => ErgoStepError::HandlerCoverage { inner: err },
        HostedStepError::EgressValidation(_) => ErgoStepError::EgressValidation { inner: err },
        HostedStepError::EgressProcess(_) => ErgoStepError::EgressProcess { inner: err },
        HostedStepError::EgressDispatchFailure(_) => ErgoStepError::EgressDispatch { inner: err },
        _ => ErgoStepError::Internal { inner: err },
    }
}

#[derive(Debug)]
#[non_exhaustive]
/// Errors returned while finalizing or writing capture bundles.
pub enum ErgoCaptureError {
    /// Manual-runner finalization failed before a capture bundle was produced.
    Finalize {
        #[allow(missing_docs)]
        inner: ErgoStepError,
    },
    /// The profile has no capture file path for automatic writing.
    OutputNotConfigured,
    /// Writing a capture bundle to disk failed.
    Write {
        #[allow(missing_docs)]
        inner: CaptureWriteError,
        /// Bundle recovered after finalization when the write failed.
        bundle: Option<CaptureBundle>,
    },
}

impl std::fmt::Display for ErgoCaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Finalize { inner } => write!(f, "{inner}"),
            Self::OutputNotConfigured => {
                write!(f, "profile does not declare a capture file path")
            }
            Self::Write { inner, .. } => write!(f, "{inner}"),
        }
    }
}

impl std::error::Error for ErgoCaptureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Finalize { inner } => Some(inner),
            Self::Write { inner, .. } => Some(inner),
            Self::OutputNotConfigured => None,
        }
    }
}

impl ErgoCaptureError {
    /// Parent-only escape hatch returning the underlying host capture write
    /// error when the failure happened during artifact persistence.
    pub fn as_capture_write_error(&self) -> Option<&CaptureWriteError> {
        match self {
            Self::Write { inner, .. } => Some(inner),
            _ => None,
        }
    }

    /// Recovers the generated capture bundle when finalization succeeded but
    /// the subsequent disk write failed. `None` for finalize failures and for
    /// the direct `write_capture_bundle` wrapper.
    pub fn capture_bundle(&self) -> Option<&CaptureBundle> {
        match self {
            Self::Write { bundle, .. } => bundle.as_ref(),
            Self::Finalize { .. } | Self::OutputNotConfigured => None,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
/// Errors returned while running a profile or explicit run configuration.
///
/// Match these SDK categories first. Use [`ErgoRunError::as_host_run_error`]
/// only when an advanced caller also depends on `ergo-host` and needs the host
/// taxonomy underneath the SDK category.
pub enum ErgoRunError {
    /// SDK-side configuration or profile resolution failed before host run
    /// orchestration.
    Config {
        #[allow(missing_docs)]
        inner: ErgoConfigError,
    },
    /// Production ingress or adapter-bound graph execution requires an
    /// adapter, but none was configured.
    AdapterRequired {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// Graph or cluster assets could not be loaded.
    GraphLoad {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// Graph assets loaded but failed preparation or validation.
    GraphPreparation {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// Adapter manifest composition failed.
    AdapterComposition {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// Adapter setup failed after composition.
    AdapterSetup {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// Ingress startup, protocol, input, I/O, or output failed.
    Ingress {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// Egress channel startup failed before events were processed.
    EgressStartup {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// Egress intent validation failed during the run.
    EgressValidation {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// Egress dispatch failed during the run.
    EgressDispatch {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// Per-event graph stepping failed.
    Step {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// A host or SDK invariant failed; report this as a bug rather than user
    /// configuration feedback.
    Internal {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// Capture finalization or writing failed.
    Capture {
        #[allow(missing_docs)]
        inner: ErgoCaptureError,
    },
}

impl std::fmt::Display for ErgoRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config { inner } => write!(f, "{inner}"),
            Self::AdapterRequired { inner }
            | Self::GraphLoad { inner }
            | Self::GraphPreparation { inner }
            | Self::AdapterComposition { inner }
            | Self::AdapterSetup { inner }
            | Self::Ingress { inner }
            | Self::EgressStartup { inner }
            | Self::EgressValidation { inner }
            | Self::EgressDispatch { inner }
            | Self::Step { inner }
            | Self::Internal { inner } => write!(f, "{inner}"),
            Self::Capture { inner } => write!(f, "{inner}"),
        }
    }
}

impl std::error::Error for ErgoRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config { inner } => Some(inner),
            Self::AdapterRequired { inner }
            | Self::GraphLoad { inner }
            | Self::GraphPreparation { inner }
            | Self::AdapterComposition { inner }
            | Self::AdapterSetup { inner }
            | Self::Ingress { inner }
            | Self::EgressStartup { inner }
            | Self::EgressValidation { inner }
            | Self::EgressDispatch { inner }
            | Self::Step { inner }
            | Self::Internal { inner } => Some(inner),
            Self::Capture { inner } => Some(inner),
        }
    }
}

impl ErgoRunError {
    /// Parent-only escape hatch returning the underlying host run error.
    /// Returns `None` for SDK configuration failures and normalized
    /// capture-write failures (the exact `CaptureWriteError` is preserved on
    /// `ErgoCaptureError`).
    pub fn as_host_run_error(&self) -> Option<&HostRunError> {
        match self {
            Self::AdapterRequired { inner }
            | Self::GraphLoad { inner }
            | Self::GraphPreparation { inner }
            | Self::AdapterComposition { inner }
            | Self::AdapterSetup { inner }
            | Self::Ingress { inner }
            | Self::EgressStartup { inner }
            | Self::EgressValidation { inner }
            | Self::EgressDispatch { inner }
            | Self::Step { inner }
            | Self::Internal { inner } => Some(inner),
            Self::Config { .. } | Self::Capture { .. } => None,
        }
    }

    /// Returns the wrapped capture error when the run failure normalized to a
    /// capture-write outcome.
    pub fn as_capture_error(&self) -> Option<&ErgoCaptureError> {
        match self {
            Self::Capture { inner } => Some(inner),
            _ => None,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
/// Errors returned while preparing a [`ProfileRunner`](crate::ProfileRunner).
///
/// This type is intentionally smaller than [`ErgoRunError`]: preparation can
/// fail while loading, preparing, or starting support channels, but it does not
/// include run-time ingress streaming, step dispatch, or capture-write
/// outcomes.
pub enum ErgoRunnerError {
    /// SDK-side configuration or profile resolution failed before runner
    /// preparation.
    Config {
        #[allow(missing_docs)]
        inner: ErgoConfigError,
    },
    /// Production manual stepping or an adapter-bound graph requires an
    /// adapter, but none was configured.
    AdapterRequired {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// Graph or cluster assets could not be loaded.
    GraphLoad {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// Graph assets loaded but failed preparation or validation.
    GraphPreparation {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// Adapter manifest composition failed.
    AdapterComposition {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// Adapter setup failed after composition.
    AdapterSetup {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// Egress channel startup failed while preparing the runner.
    EgressStartup {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// Hosted runner validation or initialization failed.
    Initialization {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    /// A host or SDK invariant failed; report this as a bug rather than user
    /// configuration feedback.
    Internal {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
}

impl std::fmt::Display for ErgoRunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config { inner } => write!(f, "{inner}"),
            Self::AdapterRequired { inner }
            | Self::GraphLoad { inner }
            | Self::GraphPreparation { inner }
            | Self::AdapterComposition { inner }
            | Self::AdapterSetup { inner }
            | Self::EgressStartup { inner }
            | Self::Initialization { inner }
            | Self::Internal { inner } => write!(f, "{inner}"),
        }
    }
}

impl std::error::Error for ErgoRunnerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config { inner } => Some(inner),
            Self::AdapterRequired { inner }
            | Self::GraphLoad { inner }
            | Self::GraphPreparation { inner }
            | Self::AdapterComposition { inner }
            | Self::AdapterSetup { inner }
            | Self::EgressStartup { inner }
            | Self::Initialization { inner }
            | Self::Internal { inner } => Some(inner),
        }
    }
}

impl ErgoRunnerError {
    /// Parent-only escape hatch returning the underlying host run error.
    /// Returns `None` for SDK configuration failures.
    pub fn as_host_run_error(&self) -> Option<&HostRunError> {
        match self {
            Self::AdapterRequired { inner }
            | Self::GraphLoad { inner }
            | Self::GraphPreparation { inner }
            | Self::AdapterComposition { inner }
            | Self::AdapterSetup { inner }
            | Self::EgressStartup { inner }
            | Self::Initialization { inner }
            | Self::Internal { inner } => Some(inner),
            Self::Config { .. } => None,
        }
    }
}

enum HostRunCategory {
    AdapterRequired,
    Ingress,
    EgressDispatch,
    EgressValidation,
    Step,
    Internal,
    GraphLoad,
    GraphPreparation,
    AdapterComposition,
    AdapterSetup,
    EgressStartup,
}

fn classify_host_run_error(err: &HostRunError) -> HostRunCategory {
    // The trailing wildcard arms in the classifiers below are required because
    // the host enums are `#[non_exhaustive]`; future variants must default to
    // `Internal` rather than break compilation. Today rustc can prove the arm
    // is unreachable, hence the targeted allow.
    #[allow(unreachable_patterns)]
    match err {
        HostRunError::AdapterRequired(_) | HostRunError::ProductionRequiresAdapter => {
            HostRunCategory::AdapterRequired
        }
        HostRunError::Driver(_) => HostRunCategory::Ingress,
        HostRunError::Step(step) => classify_host_step_in_run(step),
        HostRunError::Setup(setup) => classify_host_setup(setup),
        HostRunError::CaptureWrite(_) => HostRunCategory::Internal,
        _ => HostRunCategory::Internal,
    }
}

fn classify_host_step_in_run(step: &HostedStepError) -> HostRunCategory {
    match step {
        HostedStepError::EgressDispatchFailure(_) => HostRunCategory::EgressDispatch,
        HostedStepError::EgressValidation(_) => HostRunCategory::EgressValidation,
        HostedStepError::MissingDecisionEntry | HostedStepError::EffectsWithoutAdapter => {
            HostRunCategory::Internal
        }
        _ => HostRunCategory::Step,
    }
}

fn classify_host_setup(setup: &HostSetupError) -> HostRunCategory {
    #[allow(unreachable_patterns)]
    match setup {
        HostSetupError::LoadGraphAssets(_) | HostSetupError::DependencyScan(_) => {
            HostRunCategory::GraphLoad
        }
        HostSetupError::GraphPreparation(_) => HostRunCategory::GraphPreparation,
        HostSetupError::AdapterSetup(HostAdapterSetupError::Composition(_)) => {
            HostRunCategory::AdapterComposition
        }
        HostSetupError::AdapterSetup(_) => HostRunCategory::AdapterSetup,
        HostSetupError::StartEgress(_) => HostRunCategory::EgressStartup,
        HostSetupError::HostedRunnerValidation(step)
        | HostSetupError::HostedRunnerInitialization(step) => classify_host_step_in_run(step),
        _ => HostRunCategory::Internal,
    }
}

pub(crate) fn map_host_run_error_to_run(err: HostRunError) -> ErgoRunError {
    if let HostRunError::CaptureWrite(inner) = err {
        return ErgoRunError::Capture {
            inner: ErgoCaptureError::Write {
                inner,
                bundle: None,
            },
        };
    }
    match classify_host_run_error(&err) {
        HostRunCategory::AdapterRequired => ErgoRunError::AdapterRequired { inner: err },
        HostRunCategory::Ingress => ErgoRunError::Ingress { inner: err },
        HostRunCategory::EgressDispatch => ErgoRunError::EgressDispatch { inner: err },
        HostRunCategory::EgressValidation => ErgoRunError::EgressValidation { inner: err },
        HostRunCategory::Step => ErgoRunError::Step { inner: err },
        HostRunCategory::Internal => ErgoRunError::Internal { inner: err },
        HostRunCategory::GraphLoad => ErgoRunError::GraphLoad { inner: err },
        HostRunCategory::GraphPreparation => ErgoRunError::GraphPreparation { inner: err },
        HostRunCategory::AdapterComposition => ErgoRunError::AdapterComposition { inner: err },
        HostRunCategory::AdapterSetup => ErgoRunError::AdapterSetup { inner: err },
        HostRunCategory::EgressStartup => ErgoRunError::EgressStartup { inner: err },
    }
}

pub(crate) fn map_host_run_error_to_runner(err: HostRunError) -> ErgoRunnerError {
    match classify_host_run_error(&err) {
        HostRunCategory::AdapterRequired => ErgoRunnerError::AdapterRequired { inner: err },
        HostRunCategory::GraphLoad => ErgoRunnerError::GraphLoad { inner: err },
        HostRunCategory::GraphPreparation => ErgoRunnerError::GraphPreparation { inner: err },
        HostRunCategory::AdapterComposition => ErgoRunnerError::AdapterComposition { inner: err },
        HostRunCategory::AdapterSetup => ErgoRunnerError::AdapterSetup { inner: err },
        HostRunCategory::EgressStartup => ErgoRunnerError::EgressStartup { inner: err },
        HostRunCategory::Step
        | HostRunCategory::EgressDispatch
        | HostRunCategory::EgressValidation
        | HostRunCategory::Ingress
        | HostRunCategory::Internal => ErgoRunnerError::Initialization { inner: err },
    }
}

#[derive(Debug)]
#[non_exhaustive]
/// Errors returned while replaying a capture.
///
/// Replay failures are grouped by the user decision they point to: capture
/// availability, graph setup, adapter setup, deterministic mismatch, or
/// internal invariant failure.
pub enum ErgoReplayError {
    /// SDK-side configuration or profile resolution failed before replay.
    Config {
        #[allow(missing_docs)]
        inner: ErgoConfigError,
    },
    /// The capture file could not be read.
    CaptureRead {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    /// The capture data could not be parsed or rehydrated.
    CaptureParse {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    /// Graph or cluster assets could not be loaded.
    GraphLoad {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    /// Graph assets loaded but failed preparation or validation.
    GraphPreparation {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    /// Adapter manifest composition failed.
    AdapterComposition {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    /// Adapter setup failed after composition.
    AdapterSetup {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    /// Replay preflight rejected the capture before stepping.
    ReplayPreflight {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    /// Replay observed a deterministic mismatch.
    ReplayMismatch {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    /// The capture uses external event kinds the replay path cannot represent.
    ReplayOwnership {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    /// Per-event replay stepping failed.
    Step {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    /// A host or SDK invariant failed; report this as a bug rather than user
    /// configuration feedback.
    Internal {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
}

impl std::fmt::Display for ErgoReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config { inner } => write!(f, "{inner}"),
            Self::CaptureRead { inner }
            | Self::CaptureParse { inner }
            | Self::GraphLoad { inner }
            | Self::GraphPreparation { inner }
            | Self::AdapterComposition { inner }
            | Self::AdapterSetup { inner }
            | Self::ReplayPreflight { inner }
            | Self::ReplayMismatch { inner }
            | Self::ReplayOwnership { inner }
            | Self::Step { inner }
            | Self::Internal { inner } => write!(f, "{inner}"),
        }
    }
}

impl std::error::Error for ErgoReplayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config { inner } => Some(inner),
            Self::CaptureRead { inner }
            | Self::CaptureParse { inner }
            | Self::GraphLoad { inner }
            | Self::GraphPreparation { inner }
            | Self::AdapterComposition { inner }
            | Self::AdapterSetup { inner }
            | Self::ReplayPreflight { inner }
            | Self::ReplayMismatch { inner }
            | Self::ReplayOwnership { inner }
            | Self::Step { inner }
            | Self::Internal { inner } => Some(inner),
        }
    }
}

impl ErgoReplayError {
    /// Parent-only escape hatch returning the underlying host replay error.
    /// `None` for SDK-side configuration failures.
    pub fn as_host_replay_error(&self) -> Option<&HostReplayError> {
        match self {
            Self::Config { .. } => None,
            Self::CaptureRead { inner }
            | Self::CaptureParse { inner }
            | Self::GraphLoad { inner }
            | Self::GraphPreparation { inner }
            | Self::AdapterComposition { inner }
            | Self::AdapterSetup { inner }
            | Self::ReplayPreflight { inner }
            | Self::ReplayMismatch { inner }
            | Self::ReplayOwnership { inner }
            | Self::Step { inner }
            | Self::Internal { inner } => Some(inner),
        }
    }
}

enum HostReplayCategory {
    Config,
    CaptureRead,
    CaptureParse,
    GraphLoad,
    GraphPreparation,
    AdapterComposition,
    AdapterSetup,
    ReplayPreflight,
    ReplayMismatch,
    ReplayOwnership,
    Step,
    Internal,
}

fn classify_host_replay_error(err: &HostReplayError) -> HostReplayCategory {
    // Trailing wildcard arms below exist for `#[non_exhaustive]` forward-compat
    // on the host enums; rustc proves them unreachable today, hence the allow.
    #[allow(unreachable_patterns)]
    match err {
        HostReplayError::GraphIdMismatch { .. } => HostReplayCategory::ReplayMismatch,
        HostReplayError::ExternalKindsNotRepresentable { .. } => {
            HostReplayCategory::ReplayOwnership
        }
        HostReplayError::Setup(setup) => match setup {
            HostReplaySetupError::CaptureRead { .. } => HostReplayCategory::CaptureRead,
            HostReplaySetupError::CaptureParse { .. } => HostReplayCategory::CaptureParse,
            HostReplaySetupError::LiveEgressConfigurationNotAllowed => HostReplayCategory::Config,
            HostReplaySetupError::Setup(host_setup) => match host_setup {
                HostSetupError::LoadGraphAssets(_) | HostSetupError::DependencyScan(_) => {
                    HostReplayCategory::GraphLoad
                }
                HostSetupError::GraphPreparation(_) => HostReplayCategory::GraphPreparation,
                HostSetupError::AdapterSetup(HostAdapterSetupError::Composition(_)) => {
                    HostReplayCategory::AdapterComposition
                }
                HostSetupError::AdapterSetup(_) => HostReplayCategory::AdapterSetup,
                _ => HostReplayCategory::Internal,
            },
            _ => HostReplayCategory::Internal,
        },
        HostReplayError::Hosted(hosted) => match hosted {
            HostedReplayError::Preflight(_) => HostReplayCategory::ReplayPreflight,
            HostedReplayError::EventRehydrate { .. } => HostReplayCategory::CaptureParse,
            HostedReplayError::Step(_) => HostReplayCategory::Step,
            HostedReplayError::Compare(_) | HostedReplayError::DecisionMismatch => {
                HostReplayCategory::ReplayMismatch
            }
        },
        _ => HostReplayCategory::Internal,
    }
}

pub(crate) fn map_host_replay_error(err: HostReplayError) -> ErgoReplayError {
    match classify_host_replay_error(&err) {
        HostReplayCategory::Config => ErgoReplayError::Config {
            inner: ErgoConfigError::LiveEgressConfigurationNotAllowed,
        },
        HostReplayCategory::CaptureRead => ErgoReplayError::CaptureRead { inner: err },
        HostReplayCategory::CaptureParse => ErgoReplayError::CaptureParse { inner: err },
        HostReplayCategory::GraphLoad => ErgoReplayError::GraphLoad { inner: err },
        HostReplayCategory::GraphPreparation => ErgoReplayError::GraphPreparation { inner: err },
        HostReplayCategory::AdapterComposition => {
            ErgoReplayError::AdapterComposition { inner: err }
        }
        HostReplayCategory::AdapterSetup => ErgoReplayError::AdapterSetup { inner: err },
        HostReplayCategory::ReplayPreflight => ErgoReplayError::ReplayPreflight { inner: err },
        HostReplayCategory::ReplayMismatch => ErgoReplayError::ReplayMismatch { inner: err },
        HostReplayCategory::ReplayOwnership => ErgoReplayError::ReplayOwnership { inner: err },
        HostReplayCategory::Step => ErgoReplayError::Step { inner: err },
        HostReplayCategory::Internal => ErgoReplayError::Internal { inner: err },
    }
}

#[derive(Debug)]
#[non_exhaustive]
/// Errors returned while validating a configured project.
pub enum ErgoValidationError {
    /// Project-level SDK configuration prevented validation from starting.
    Config {
        #[allow(missing_docs)]
        inner: ErgoConfigError,
    },
    /// A specific profile failed SDK configuration resolution.
    Profile {
        /// Profile being validated.
        profile: String,
        #[allow(missing_docs)]
        inner: ErgoConfigError,
    },
    /// A specific profile reached host preflight and failed validation there.
    HostValidation {
        /// Profile being validated.
        profile: String,
        #[allow(missing_docs)]
        inner: HostRunError,
    },
}

impl std::fmt::Display for ErgoValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config { inner } => write!(f, "{inner}"),
            Self::Profile { profile, inner } => {
                write!(f, "profile '{profile}' configuration invalid: {inner}")
            }
            Self::HostValidation { profile, inner } => {
                write!(f, "profile '{profile}' validation failed: {inner}")
            }
        }
    }
}

impl std::error::Error for ErgoValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config { inner } | Self::Profile { inner, .. } => Some(inner),
            Self::HostValidation { inner, .. } => Some(inner),
        }
    }
}

impl ErgoValidationError {
    /// Parent-only escape hatch returning the underlying host run error when
    /// validation reached host preflight.
    pub fn as_host_run_error(&self) -> Option<&HostRunError> {
        match self {
            Self::HostValidation { inner, .. } => Some(inner),
            Self::Config { .. } | Self::Profile { .. } => None,
        }
    }
}
