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
pub enum ErgoBuildError {
    Registration {
        #[allow(missing_docs)]
        inner: CoreRegistrationError,
    },
    Project {
        #[allow(missing_docs)]
        inner: ErgoProjectError,
    },
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
pub enum ErgoProjectConfigError {
    InMemoryProjectHasNoProfiles,
    InMemoryFixtureSourceLabelEmpty { profile: Option<String> },
    InMemoryFixtureItemsEmpty { profile: Option<String> },
    InMemoryProcessCommandEmpty { profile: Option<String> },
    InMemoryProcessExecutableBlank { profile: Option<String> },
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
pub enum ErgoProjectError {
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
pub enum ErgoConfigError {
    ProjectNotConfigured,
    ProfileNotFound {
        name: String,
    },
    ProjectLoad {
        #[allow(missing_docs)]
        inner: LoaderProjectError,
    },
    ProjectConfig {
        #[allow(missing_docs)]
        inner: ErgoProjectConfigError,
    },
    ExplicitRunProcessCommandEmpty,
    EgressConfigRead {
        path: PathBuf,
        #[allow(missing_docs)]
        inner: std::io::Error,
    },
    EgressConfigParse {
        path: PathBuf,
        #[allow(missing_docs)]
        inner: EgressConfigParseError,
    },
    FilesystemProfileCannotUseInMemoryCapture {
        profile: String,
    },
    InMemoryAssetsCannotUseDefaultFilesystemCapture,
    UnsupportedOperation {
        operation: &'static str,
        transport: &'static str,
    },
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
pub enum ErgoStepError {
    Input {
        #[allow(missing_docs)]
        inner: HostedStepError,
    },
    EventBuild {
        #[allow(missing_docs)]
        inner: HostedEventBuildError,
    },
    Binding {
        #[allow(missing_docs)]
        inner: HostedStepError,
    },
    Lifecycle {
        #[allow(missing_docs)]
        inner: HostedStepError,
    },
    EffectApply {
        #[allow(missing_docs)]
        inner: HostedStepError,
    },
    HandlerCoverage {
        #[allow(missing_docs)]
        inner: HostedStepError,
    },
    EgressValidation {
        #[allow(missing_docs)]
        inner: HostedStepError,
    },
    EgressProcess {
        #[allow(missing_docs)]
        inner: HostedStepError,
    },
    EgressDispatch {
        #[allow(missing_docs)]
        inner: HostedStepError,
    },
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
pub enum ErgoCaptureError {
    Finalize {
        #[allow(missing_docs)]
        inner: ErgoStepError,
    },
    OutputNotConfigured,
    Write {
        #[allow(missing_docs)]
        inner: CaptureWriteError,
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
pub enum ErgoRunError {
    Config {
        #[allow(missing_docs)]
        inner: ErgoConfigError,
    },
    AdapterRequired {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    GraphLoad {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    GraphPreparation {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    AdapterComposition {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    AdapterSetup {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    Ingress {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    EgressStartup {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    EgressValidation {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    EgressDispatch {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    Step {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    Internal {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
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
pub enum ErgoRunnerError {
    Config {
        #[allow(missing_docs)]
        inner: ErgoConfigError,
    },
    AdapterRequired {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    GraphLoad {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    GraphPreparation {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    AdapterComposition {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    AdapterSetup {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    EgressStartup {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
    Initialization {
        #[allow(missing_docs)]
        inner: HostRunError,
    },
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
pub enum ErgoReplayError {
    Config {
        #[allow(missing_docs)]
        inner: ErgoConfigError,
    },
    CaptureRead {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    CaptureParse {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    GraphLoad {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    GraphPreparation {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    AdapterComposition {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    AdapterSetup {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    ReplayPreflight {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    ReplayMismatch {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    ReplayOwnership {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
    Step {
        #[allow(missing_docs)]
        inner: HostReplayError,
    },
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
pub enum ErgoValidationError {
    Config {
        #[allow(missing_docs)]
        inner: ErgoConfigError,
    },
    Profile {
        profile: String,
        #[allow(missing_docs)]
        inner: ErgoConfigError,
    },
    HostValidation {
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
