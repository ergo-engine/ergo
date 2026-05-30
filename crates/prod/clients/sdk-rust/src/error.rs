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
//! - The `ErgoErrorSource` opaque envelope used to attach host/loader/runtime
//!   failure detail to SDK error variants without naming internal types on the
//!   public surface.
//! - Mapping from host taxonomies into SDK categories per the PUB-1 plan.
//!
//! Does not own:
//! - Host orchestration semantics or step categorization rules.
//! - Capture write atomicity or runtime registration semantics.
//!
//! Connects to:
//! - `lib.rs`, which uses these types as the public return-error surface for
//!   `Ergo`, `ErgoBuilder`, and `ProfileRunner` operations.
//! - `ergo_host`, `ergo_loader`, and `ergo_runtime` whose error types are
//!   wrapped in `ErgoErrorSource` and reachable only as `&dyn Error` through
//!   `std::error::Error::source` and `downcast_ref`.
//!
//! Safety notes:
//! - Mapping from host taxonomies is exhaustive over today's variants and
//!   collapses unknown variants to an `Internal` SDK category so semver growth
//!   in host enums does not silently change SDK categorization.
//! - The SDK never names host error types in its public API. Host types are
//!   reachable only via the `Error::source` chain plus `downcast_ref`, which
//!   keeps the SDK free to refactor host taxonomies post-1.0 without an SDK
//!   semver break.

use std::path::PathBuf;

use ergo_host::{
    CaptureBundle, EgressConfigParseError, HostAdapterSetupError, HostReplayError,
    HostReplaySetupError, HostRunError, HostSetupError, HostedReplayError, HostedStepError,
};
use ergo_loader::ProjectError as LoaderProjectError;

/// Opaque source-error envelope attached to wrapped variants of the SDK's
/// `Ergo*` error types.
///
/// The SDK does not name host, loader, or runtime error types on its public
/// surface. Failure detail from those internal layers is wrapped in
/// `ErgoErrorSource`, which exposes only the standard [`std::error::Error`]
/// interface: a [`Display`](std::fmt::Display) message that delegates to the
/// wrapped error and a [`source`](std::error::Error::source) hop into the
/// underlying error chain.
///
/// To inspect the underlying failure for diagnostics, call
/// [`ErgoErrorSource::as_dyn_error`] (or walk the [`Error::source`] chain on
/// the parent SDK error) and `downcast_ref` against the type from the
/// relevant Ergo crate. Doing so requires depending on that crate explicitly;
/// the SDK does not re-export the inner types.
///
/// [`Error::source`]: std::error::Error::source
pub struct ErgoErrorSource {
    inner: Box<dyn std::error::Error + Send + Sync + 'static>,
}

impl ErgoErrorSource {
    pub(crate) fn new<E>(error: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            inner: Box::new(error),
        }
    }

    /// Returns the wrapped source as a `&dyn Error`.
    ///
    /// Use this for source-chain walking and for `downcast_ref` against host
    /// or loader error types when an advanced caller already depends on the
    /// relevant Ergo crate.
    pub fn as_dyn_error(&self) -> &(dyn std::error::Error + 'static) {
        &*self.inner
    }
}

impl std::fmt::Debug for ErgoErrorSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.inner, f)
    }
}

impl std::fmt::Display for ErgoErrorSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&*self.inner, f)
    }
}

impl std::error::Error for ErgoErrorSource {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }
}

#[derive(Debug)]
#[non_exhaustive]
/// Errors returned while building an [`Ergo`](crate::Ergo) engine handle.
///
/// Match the variant first to decide what to surface to the user. For
/// underlying detail, walk [`std::error::Error::source`].
pub enum ErgoBuildError {
    /// Custom or core primitive registration failed while assembling the
    /// runtime catalog.
    ///
    /// Action: review the primitives passed to
    /// [`ErgoBuilder::add_source`](crate::ErgoBuilder::add_source) and the
    /// other `add_*` methods for duplicate identifiers or invalid manifests.
    /// Source detail is reachable via [`std::error::Error::source`].
    Registration {
        /// Opaque source describing the registration failure.
        source: ErgoErrorSource,
    },
    /// The in-memory project snapshot installed on the builder failed
    /// validation.
    ///
    /// Action: inspect the inner [`ErgoProjectError`] for the specific rule
    /// that the snapshot violated.
    Project {
        /// SDK-side validation failure for the in-memory project snapshot.
        inner: ErgoProjectError,
    },
    /// Both [`ErgoBuilder::project_root`](crate::ErgoBuilder::project_root) and
    /// [`ErgoBuilder::in_memory_project`](crate::ErgoBuilder::in_memory_project)
    /// were called on the same builder.
    ///
    /// Action: choose exactly one project source for a given builder.
    ProjectSourceConflict,
}

impl std::fmt::Display for ErgoBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Registration { .. } => {
                write!(f, "ergo: primitive catalog registration failed")
            }
            Self::Project { inner } => write!(f, "ergo: in-memory project invalid: {inner}"),
            Self::ProjectSourceConflict => write!(
                f,
                "ergo: project_root and in_memory_project are mutually exclusive"
            ),
        }
    }
}

impl std::error::Error for ErgoBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Registration { source } => Some(source.as_dyn_error()),
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
///
/// Today the only failure category is configuration validation; new variants
/// may be added as the in-memory project surface grows.
pub enum ErgoProjectError {
    /// In-memory project construction violated an SDK validation rule.
    ///
    /// Action: inspect the inner [`ErgoProjectConfigError`] and adjust the
    /// snapshot before installing it on
    /// [`ErgoBuilder::in_memory_project`](crate::ErgoBuilder::in_memory_project).
    Config {
        /// SDK validation failure detail.
        inner: ErgoProjectConfigError,
    },
}

impl std::fmt::Display for ErgoProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config { inner } => write!(f, "in-memory project invalid: {inner}"),
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
///
/// These categories surface before host orchestration runs. Match the variant
/// to act on the user's mistake, then walk [`std::error::Error::source`] for
/// underlying detail (loader, parser, or filesystem error).
pub enum ErgoConfigError {
    /// An operation that requires a project (validate, profile run, profile
    /// replay) was called on an [`Ergo`](crate::Ergo) handle that has no
    /// project source.
    ///
    /// Action: configure a project on the
    /// [`ErgoBuilder`](crate::ErgoBuilder) via
    /// [`ErgoBuilder::project_root`](crate::ErgoBuilder::project_root) or
    /// [`ErgoBuilder::in_memory_project`](crate::ErgoBuilder::in_memory_project).
    ProjectNotConfigured,
    /// The requested profile name does not exist in the configured project.
    ///
    /// Action: check the spelling against the project's declared profile list
    /// (filesystem `ergo.toml` or
    /// [`InMemoryProjectSnapshot`](crate::InMemoryProjectSnapshot)).
    ProfileNotFound {
        /// Requested profile name.
        name: String,
    },
    /// The filesystem project could not be discovered, read, or decoded.
    ///
    /// Action: confirm the project root contains a valid `ergo.toml` and that
    /// referenced asset paths exist; the loader detail is reachable via
    /// [`std::error::Error::source`].
    ProjectLoad {
        /// Opaque source describing the loader failure.
        source: ErgoErrorSource,
    },
    /// An in-memory project or profile construction rule failed while
    /// preparing an operation.
    ///
    /// Action: inspect the inner [`ErgoProjectConfigError`] and adjust the
    /// snapshot installed on the builder.
    ProjectConfig {
        /// SDK validation failure detail.
        inner: ErgoProjectConfigError,
    },
    /// An explicit [`RunConfig`](crate::RunConfig) was constructed with an
    /// empty process-ingress command vector.
    ///
    /// Action: provide at least one element when calling
    /// [`IngressConfig::process`](crate::IngressConfig::process).
    ExplicitRunProcessCommandEmpty,
    /// The egress TOML configuration file could not be read from disk.
    ///
    /// Action: confirm the file exists and is readable; the underlying I/O
    /// error is reachable via [`std::error::Error::source`].
    EgressConfigRead {
        /// Path to the egress config file the SDK attempted to read.
        path: PathBuf,
        /// Underlying I/O failure.
        source: std::io::Error,
    },
    /// The egress TOML configuration could not be parsed or failed structural
    /// validation.
    ///
    /// Action: review the egress TOML file against the format expected by the
    /// SDK's egress builder; the underlying parse error is preserved in
    /// `source` and re-exported as
    /// [`EgressConfigParseError`](crate::EgressConfigParseError).
    EgressConfigParse {
        /// Path to the egress config file the SDK attempted to parse.
        path: PathBuf,
        /// Parse failure detail.
        source: EgressConfigParseError,
    },
    /// A filesystem-backed profile requested in-memory-only capture behavior,
    /// which is reserved for in-memory projects.
    ///
    /// Action: choose a capture file path (or no capture) for filesystem
    /// profiles, or move the profile to an in-memory project snapshot.
    FilesystemProfileCannotUseInMemoryCapture {
        /// Profile that requested the unsupported capture mode.
        profile: String,
    },
    /// An in-memory graph-assets profile asked the SDK to use the filesystem
    /// default capture path, which has no project root to resolve against.
    ///
    /// Action: choose an explicit capture path or in-memory capture for
    /// in-memory profiles.
    InMemoryAssetsCannotUseDefaultFilesystemCapture,
    /// The requested operation is not supported for the configured project
    /// transport (for example, path-based replay against an in-memory
    /// project).
    ///
    /// Action: use the operation variant designed for the project transport
    /// (for example, [`Ergo::replay_profile_bundle`](crate::Ergo::replay_profile_bundle)
    /// for in-memory projects).
    UnsupportedOperation {
        /// Operation name the caller invoked.
        operation: &'static str,
        /// Project transport or source kind that does not support it.
        transport: &'static str,
    },
    /// A replay request specified live egress configuration; replay never
    /// dispatches live effects.
    ///
    /// Action: omit egress configuration on replay, or use a run entry point
    /// instead.
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
            Self::ProjectLoad { .. } => {
                write!(f, "project could not be loaded from disk")
            }
            Self::ProjectConfig { inner } => write!(f, "in-memory project invalid: {inner}"),
            Self::ExplicitRunProcessCommandEmpty => write!(
                f,
                "explicit run configuration is invalid: process ingress command must not be empty"
            ),
            Self::EgressConfigRead { path, .. } => {
                write!(f, "failed to read egress config '{}'", path.display())
            }
            Self::EgressConfigParse { path, .. } => {
                write!(f, "failed to parse egress config '{}'", path.display())
            }
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
            Self::ProjectLoad { source } => Some(source.as_dyn_error()),
            Self::ProjectConfig { inner } => Some(inner),
            Self::EgressConfigRead { source, .. } => Some(source),
            Self::EgressConfigParse { source, .. } => Some(source),
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
            other => Self::ProjectLoad {
                source: ErgoErrorSource::new(other),
            },
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
/// Errors returned while manually stepping a [`ProfileRunner`](crate::ProfileRunner).
///
/// Match the variant first to decide what to do next:
///
/// - [`ErgoStepError::is_recoverable`] is `true` for input/binding/event-build
///   categories where another `step` call is allowed.
/// - [`ErgoStepError::can_finish`] is `true` for egress dispatch failures
///   that block further stepping but still permit
///   [`ProfileRunner::finish`](crate::ProfileRunner::finish) to recover the
///   capture bundle.
///
/// Underlying host detail is reachable via [`std::error::Error::source`].
pub enum ErgoStepError {
    /// The supplied [`HostedEvent`](crate::HostedEvent) was structurally
    /// invalid for stepping (duplicate id, missing semantic kind, missing
    /// payload, payload not an object, or unknown semantic kind).
    ///
    /// Action: fix the event before retrying. This category is recoverable.
    Input {
        /// Opaque source describing the input failure.
        source: ErgoErrorSource,
    },
    /// The supplied [`HostedEvent`](crate::HostedEvent) could not be converted
    /// into the graph event format.
    ///
    /// Action: rebuild the event with valid fields. This category is
    /// recoverable.
    EventBuild {
        /// Opaque source describing the event-build failure.
        source: ErgoErrorSource,
    },
    /// The event was structurally valid but did not bind to any graph node.
    ///
    /// Action: confirm the event semantic kind matches a configured trigger
    /// or input handler. This category is recoverable.
    Binding {
        /// Opaque source describing the binding failure.
        source: ErgoErrorSource,
    },
    /// The runner was used outside the prepared lifecycle (for example, a
    /// step after finalization or after a non-recoverable failure).
    ///
    /// Action: abandon this runner; lifecycle violations are not recoverable.
    Lifecycle {
        /// Opaque source describing the lifecycle violation.
        source: ErgoErrorSource,
    },
    /// Runtime effect application failed while applying the step's outcome to
    /// the graph state.
    ///
    /// Action: report this as an SDK or graph bug; the runner is no longer
    /// safe to step.
    EffectApply {
        /// Opaque source describing the effect-application failure.
        source: ErgoErrorSource,
    },
    /// A produced action handler was not covered by the configured adapter or
    /// egress channels.
    ///
    /// Action: extend the adapter manifest or egress configuration to cover
    /// the missing handler.
    HandlerCoverage {
        /// Opaque source describing the coverage gap.
        source: ErgoErrorSource,
    },
    /// Egress intent validation rejected the produced effect before dispatch.
    ///
    /// Action: review the egress channel configuration against the produced
    /// intent's contract.
    EgressValidation {
        /// Opaque source describing the validation failure.
        source: ErgoErrorSource,
    },
    /// The egress process channel failed to start or stopped responding.
    ///
    /// Action: confirm the egress executable is available and that its
    /// process protocol matches the SDK's expectations.
    EgressProcess {
        /// Opaque source describing the process-channel failure.
        source: ErgoErrorSource,
    },
    /// Egress dispatch failed after the step produced an intent. The step
    /// itself completed; the failure occurred while delivering the effect.
    ///
    /// Action: this category is non-recoverable for further stepping but
    /// [`ErgoStepError::can_finish`] is `true`, so
    /// [`ProfileRunner::finish`](crate::ProfileRunner::finish) can still
    /// recover the capture bundle.
    EgressDispatch {
        /// Opaque source describing the dispatch failure.
        source: ErgoErrorSource,
    },
    /// A host or SDK invariant failed during stepping.
    ///
    /// Action: report this as a bug; do not surface the message as user
    /// configuration feedback.
    Internal {
        /// Opaque source describing the invariant failure.
        source: ErgoErrorSource,
    },
}

impl std::fmt::Display for ErgoStepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input { .. } => write!(f, "step input was invalid"),
            Self::Binding { .. } => write!(f, "step input did not bind to any graph node"),
            Self::Lifecycle { .. } => write!(f, "runner used outside its prepared lifecycle"),
            Self::EffectApply { .. } => write!(f, "runtime effect application failed"),
            Self::HandlerCoverage { .. } => {
                write!(
                    f,
                    "produced handler is not covered by configured adapter or egress"
                )
            }
            Self::EgressValidation { .. } => {
                write!(f, "egress intent validation rejected the step output")
            }
            Self::EgressProcess { .. } => write!(f, "egress process channel failed"),
            Self::EgressDispatch { .. } => {
                write!(
                    f,
                    "egress dispatch failed after the step produced an intent"
                )
            }
            Self::Internal { .. } => write!(f, "step failed an internal SDK or host invariant"),
            Self::EventBuild { .. } => write!(f, "hosted event could not be built"),
        }
    }
}

impl std::error::Error for ErgoStepError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Input { source }
            | Self::Binding { source }
            | Self::Lifecycle { source }
            | Self::EffectApply { source }
            | Self::HandlerCoverage { source }
            | Self::EgressValidation { source }
            | Self::EgressProcess { source }
            | Self::EgressDispatch { source }
            | Self::Internal { source }
            | Self::EventBuild { source } => Some(source.as_dyn_error()),
        }
    }
}

impl ErgoStepError {
    /// Returns `true` when the failure category permits another `step` call.
    ///
    /// Recoverable categories are [`ErgoStepError::Input`],
    /// [`ErgoStepError::Binding`], and [`ErgoStepError::EventBuild`].
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::Input { .. } | Self::Binding { .. } | Self::EventBuild { .. }
        )
    }

    /// Returns `true` when finalization is still allowed even though further
    /// stepping is blocked.
    ///
    /// Today this is true only for [`ErgoStepError::EgressDispatch`].
    pub fn can_finish(&self) -> bool {
        matches!(self, Self::EgressDispatch { .. })
    }
}

enum HostedStepCategory {
    Input,
    Binding,
    EventBuild,
    Lifecycle,
    EffectApply,
    HandlerCoverage,
    EgressValidation,
    EgressProcess,
    EgressDispatch,
    Internal,
}

fn classify_hosted_step_error(err: &HostedStepError) -> HostedStepCategory {
    match err {
        HostedStepError::DuplicateEventId { .. }
        | HostedStepError::MissingSemanticKind
        | HostedStepError::MissingPayload
        | HostedStepError::PayloadMustBeObject
        | HostedStepError::UnknownSemanticKind { .. } => HostedStepCategory::Input,
        HostedStepError::Binding(_) => HostedStepCategory::Binding,
        HostedStepError::EventBuild(_) => HostedStepCategory::EventBuild,
        HostedStepError::LifecycleViolation { .. } => HostedStepCategory::Lifecycle,
        HostedStepError::EffectApply(_) => HostedStepCategory::EffectApply,
        HostedStepError::HandlerCoverage(_) => HostedStepCategory::HandlerCoverage,
        HostedStepError::EgressValidation(_) => HostedStepCategory::EgressValidation,
        HostedStepError::EgressProcess(_) => HostedStepCategory::EgressProcess,
        HostedStepError::EgressDispatchFailure(_) => HostedStepCategory::EgressDispatch,
        _ => HostedStepCategory::Internal,
    }
}

pub(crate) fn map_hosted_step_error(err: HostedStepError) -> ErgoStepError {
    let category = classify_hosted_step_error(&err);
    if matches!(category, HostedStepCategory::EventBuild) {
        if let HostedStepError::EventBuild(inner) = err {
            return ErgoStepError::EventBuild {
                source: ErgoErrorSource::new(inner),
            };
        }
        unreachable!("classifier guarantees EventBuild matches the variant");
    }
    let source = ErgoErrorSource::new(err);
    match category {
        HostedStepCategory::Input => ErgoStepError::Input { source },
        HostedStepCategory::Binding => ErgoStepError::Binding { source },
        HostedStepCategory::EventBuild => unreachable!(),
        HostedStepCategory::Lifecycle => ErgoStepError::Lifecycle { source },
        HostedStepCategory::EffectApply => ErgoStepError::EffectApply { source },
        HostedStepCategory::HandlerCoverage => ErgoStepError::HandlerCoverage { source },
        HostedStepCategory::EgressValidation => ErgoStepError::EgressValidation { source },
        HostedStepCategory::EgressProcess => ErgoStepError::EgressProcess { source },
        HostedStepCategory::EgressDispatch => ErgoStepError::EgressDispatch { source },
        HostedStepCategory::Internal => ErgoStepError::Internal { source },
    }
}

#[derive(Debug)]
#[non_exhaustive]
/// Errors returned while finalizing or writing a manual-runner capture bundle.
///
/// `Finalize` and `Write` are paired with the matching SDK source detail.
/// When the disk write fails after a successful finalization, the produced
/// capture bundle is preserved on the variant via
/// [`ErgoCaptureError::capture_bundle`] so the caller can retry persistence.
pub enum ErgoCaptureError {
    /// Manual-runner finalization failed before a capture bundle was produced.
    ///
    /// Action: inspect the inner [`ErgoStepError`] for the lifecycle or
    /// session failure that prevented finalization.
    Finalize {
        /// SDK step error that prevented finalization.
        inner: ErgoStepError,
    },
    /// The profile does not declare a capture file path, so the SDK cannot
    /// write the capture bundle automatically.
    ///
    /// Action: call
    /// [`ProfileRunner::finish`](crate::ProfileRunner::finish) to recover the
    /// bundle and write it explicitly via
    /// [`write_capture_bundle`](crate::write_capture_bundle), or configure
    /// the profile with a capture path.
    OutputNotConfigured,
    /// Writing the capture bundle to disk failed after finalization.
    ///
    /// Action: confirm the capture path is writable; the underlying host
    /// capture-write detail is reachable via [`std::error::Error::source`].
    /// Retrieve the recovered bundle through
    /// [`ErgoCaptureError::capture_bundle`] to retry persistence elsewhere.
    Write {
        /// Opaque source describing the capture-write failure.
        source: ErgoErrorSource,
        /// Bundle recovered after finalization when the write failed.
        bundle: Option<CaptureBundle>,
    },
}

impl std::fmt::Display for ErgoCaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Finalize { inner } => {
                write!(f, "manual runner finalization failed: {inner}")
            }
            Self::OutputNotConfigured => {
                write!(f, "profile does not declare a capture file path")
            }
            Self::Write { .. } => write!(f, "capture bundle write to disk failed"),
        }
    }
}

impl std::error::Error for ErgoCaptureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Finalize { inner } => Some(inner),
            Self::Write { source, .. } => Some(source.as_dyn_error()),
            Self::OutputNotConfigured => None,
        }
    }
}

impl ErgoCaptureError {
    /// Returns the capture bundle generated before the disk write failed.
    ///
    /// `Some` only for [`ErgoCaptureError::Write`] failures that follow a
    /// successful finalization. Always `None` for [`ErgoCaptureError::Finalize`]
    /// (no bundle was produced) and for [`ErgoCaptureError::OutputNotConfigured`]
    /// (the SDK never finalized). The direct [`write_capture_bundle`](crate::write_capture_bundle)
    /// wrapper also returns `None` because the caller already owns the bundle
    /// they passed in.
    pub fn capture_bundle(&self) -> Option<&CaptureBundle> {
        match self {
            Self::Write { bundle, .. } => bundle.as_ref(),
            Self::Finalize { .. } | Self::OutputNotConfigured => None,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
/// Errors returned while running a profile or an explicit run configuration.
///
/// Match the variant first to surface the right user-facing category. Wrapped
/// host detail is opaque on the public surface; reach it via
/// [`std::error::Error::source`] and `downcast_ref` against types from
/// [`ergo-host`](https://docs.rs/ergo-host) when an advanced caller already
/// depends on the host crate directly.
pub enum ErgoRunError {
    /// SDK-side configuration or profile resolution failed before host
    /// orchestration started.
    ///
    /// Action: inspect the inner [`ErgoConfigError`] for the specific
    /// configuration category.
    Config {
        /// SDK configuration failure detail.
        inner: ErgoConfigError,
    },
    /// The profile requires an adapter (production ingress or adapter-bound
    /// graph) but none was configured on the [`Ergo`](crate::Ergo) handle.
    ///
    /// Action: register an adapter via the
    /// [`ErgoBuilder`](crate::ErgoBuilder)'s primitive registration methods or
    /// supply an adapter manifest in the profile.
    AdapterRequired {
        /// Opaque source describing the adapter requirement.
        source: ErgoErrorSource,
    },
    /// Graph or cluster asset files referenced by the profile could not be
    /// loaded.
    ///
    /// Action: confirm the files exist and contain valid YAML/JSON; the
    /// underlying loader detail is reachable via
    /// [`std::error::Error::source`].
    GraphLoad {
        /// Opaque source describing the graph-load failure.
        source: ErgoErrorSource,
    },
    /// Graph assets loaded but graph preparation or validation rejected the
    /// resolved graph.
    ///
    /// Action: review the graph and cluster definitions against the
    /// preparation rules surfaced in the source detail.
    GraphPreparation {
        /// Opaque source describing the preparation failure.
        source: ErgoErrorSource,
    },
    /// Adapter manifest composition failed before the adapter was set up.
    ///
    /// Action: validate the adapter manifest against the SDK's adapter input
    /// rules.
    AdapterComposition {
        /// Opaque source describing the composition failure.
        source: ErgoErrorSource,
    },
    /// Adapter setup failed after composition.
    ///
    /// Action: confirm the adapter's primitive identifiers are registered on
    /// the builder; the source detail names the missing or rejected piece.
    AdapterSetup {
        /// Opaque source describing the adapter-setup failure.
        source: ErgoErrorSource,
    },
    /// Ingress startup, protocol, input, I/O, or output failed during the run.
    ///
    /// Action: inspect the source detail for the specific ingress failure
    /// (process ingress launch, fixture parse, protocol violation, etc.).
    Ingress {
        /// Opaque source describing the ingress failure.
        source: ErgoErrorSource,
    },
    /// Egress channel startup failed before any events were processed.
    ///
    /// Action: confirm egress executables are available and that the egress
    /// configuration matches the SDK's process-channel protocol.
    EgressStartup {
        /// Opaque source describing the egress startup failure.
        source: ErgoErrorSource,
    },
    /// Egress intent validation rejected an effect during the run.
    ///
    /// Action: review the egress channel configuration against the produced
    /// intent's contract.
    EgressValidation {
        /// Opaque source describing the egress validation failure.
        source: ErgoErrorSource,
    },
    /// Egress dispatch failed while delivering an effect during the run.
    ///
    /// Action: inspect the source detail for the dispatch failure; the run
    /// terminated at this event.
    EgressDispatch {
        /// Opaque source describing the dispatch failure.
        source: ErgoErrorSource,
    },
    /// Per-event graph stepping failed during the run.
    ///
    /// Action: inspect the source detail for the step failure; this is
    /// usually the most useful detail for debugging primitive logic.
    Step {
        /// Opaque source describing the step failure.
        source: ErgoErrorSource,
    },
    /// A host or SDK invariant failed during the run.
    ///
    /// Action: report as a bug; do not surface this category as user
    /// configuration feedback.
    Internal {
        /// Opaque source describing the invariant failure.
        source: ErgoErrorSource,
    },
    /// Capture finalization or capture-bundle write to disk failed.
    ///
    /// Action: inspect the inner [`ErgoCaptureError`] and recover the
    /// generated bundle, when present, via
    /// [`ErgoCaptureError::capture_bundle`].
    Capture {
        /// SDK capture failure detail.
        inner: ErgoCaptureError,
    },
}

impl std::fmt::Display for ErgoRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config { inner } => write!(f, "run configuration invalid: {inner}"),
            Self::AdapterRequired { .. } => write!(f, "profile requires an adapter"),
            Self::GraphLoad { .. } => write!(f, "graph or cluster assets could not be loaded"),
            Self::GraphPreparation { .. } => {
                write!(f, "graph preparation rejected the resolved graph")
            }
            Self::AdapterComposition { .. } => write!(f, "adapter manifest composition failed"),
            Self::AdapterSetup { .. } => write!(f, "adapter setup failed"),
            Self::Ingress { .. } => write!(f, "ingress channel failed during the run"),
            Self::EgressStartup { .. } => write!(f, "egress channel startup failed"),
            Self::EgressValidation { .. } => {
                write!(f, "egress intent validation rejected a step output")
            }
            Self::EgressDispatch { .. } => write!(f, "egress dispatch failed during the run"),
            Self::Step { .. } => write!(f, "per-event graph stepping failed"),
            Self::Internal { .. } => write!(f, "run failed an internal SDK or host invariant"),
            Self::Capture { inner } => write!(f, "capture handling failed: {inner}"),
        }
    }
}

impl std::error::Error for ErgoRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config { inner } => Some(inner),
            Self::AdapterRequired { source }
            | Self::GraphLoad { source }
            | Self::GraphPreparation { source }
            | Self::AdapterComposition { source }
            | Self::AdapterSetup { source }
            | Self::Ingress { source }
            | Self::EgressStartup { source }
            | Self::EgressValidation { source }
            | Self::EgressDispatch { source }
            | Self::Step { source }
            | Self::Internal { source } => Some(source.as_dyn_error()),
            Self::Capture { inner } => Some(inner),
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
/// Errors returned while preparing a [`ProfileRunner`](crate::ProfileRunner).
///
/// Preparation runs the same setup pipeline as a full run but stops before
/// ingress streaming. It can fail while loading and preparing graph assets,
/// composing adapters, starting egress, or initializing the hosted runner;
/// it never reports run-time stepping, dispatch, or capture-write outcomes.
///
/// Wrapped host detail is opaque on the public surface and reachable via
/// [`std::error::Error::source`].
pub enum ErgoRunnerError {
    /// SDK-side configuration or profile resolution failed before runner
    /// preparation.
    ///
    /// Action: inspect the inner [`ErgoConfigError`] for the specific
    /// configuration category.
    Config {
        /// SDK configuration failure detail.
        inner: ErgoConfigError,
    },
    /// The profile requires an adapter (production stepping or adapter-bound
    /// graph) but none was configured.
    ///
    /// Action: register an adapter on the [`ErgoBuilder`](crate::ErgoBuilder)
    /// or supply an adapter manifest in the profile.
    AdapterRequired {
        /// Opaque source describing the adapter requirement.
        source: ErgoErrorSource,
    },
    /// Graph or cluster asset files referenced by the profile could not be
    /// loaded.
    ///
    /// Action: confirm the files exist and contain valid YAML/JSON.
    GraphLoad {
        /// Opaque source describing the graph-load failure.
        source: ErgoErrorSource,
    },
    /// Graph assets loaded but graph preparation or validation rejected the
    /// resolved graph.
    ///
    /// Action: review the graph and cluster definitions against the rules
    /// surfaced in the source detail.
    GraphPreparation {
        /// Opaque source describing the preparation failure.
        source: ErgoErrorSource,
    },
    /// Adapter manifest composition failed before adapter setup.
    ///
    /// Action: validate the adapter manifest against the SDK's adapter input
    /// rules.
    AdapterComposition {
        /// Opaque source describing the composition failure.
        source: ErgoErrorSource,
    },
    /// Adapter setup failed after composition.
    ///
    /// Action: confirm the adapter's primitive identifiers are registered on
    /// the builder.
    AdapterSetup {
        /// Opaque source describing the adapter-setup failure.
        source: ErgoErrorSource,
    },
    /// Egress channel startup failed while preparing the runner.
    ///
    /// Action: confirm egress executables are available and that the egress
    /// configuration matches the SDK's process-channel protocol.
    EgressStartup {
        /// Opaque source describing the egress startup failure.
        source: ErgoErrorSource,
    },
    /// Hosted runner validation or initialization failed before stepping
    /// could begin.
    ///
    /// Action: inspect the source detail for the rejected validation rule.
    Initialization {
        /// Opaque source describing the initialization failure.
        source: ErgoErrorSource,
    },
    /// A host or SDK invariant failed during preparation.
    ///
    /// Action: report as a bug; do not surface this category as user
    /// configuration feedback.
    Internal {
        /// Opaque source describing the invariant failure.
        source: ErgoErrorSource,
    },
}

impl std::fmt::Display for ErgoRunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config { inner } => write!(f, "runner configuration invalid: {inner}"),
            Self::AdapterRequired { .. } => write!(f, "profile requires an adapter"),
            Self::GraphLoad { .. } => write!(f, "graph or cluster assets could not be loaded"),
            Self::GraphPreparation { .. } => {
                write!(f, "graph preparation rejected the resolved graph")
            }
            Self::AdapterComposition { .. } => write!(f, "adapter manifest composition failed"),
            Self::AdapterSetup { .. } => write!(f, "adapter setup failed"),
            Self::EgressStartup { .. } => write!(f, "egress channel startup failed"),
            Self::Initialization { .. } => write!(f, "hosted runner initialization failed"),
            Self::Internal { .. } => {
                write!(
                    f,
                    "runner preparation failed an internal SDK or host invariant"
                )
            }
        }
    }
}

impl std::error::Error for ErgoRunnerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config { inner } => Some(inner),
            Self::AdapterRequired { source }
            | Self::GraphLoad { source }
            | Self::GraphPreparation { source }
            | Self::AdapterComposition { source }
            | Self::AdapterSetup { source }
            | Self::EgressStartup { source }
            | Self::Initialization { source }
            | Self::Internal { source } => Some(source.as_dyn_error()),
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
                source: ErgoErrorSource::new(inner),
                bundle: None,
            },
        };
    }
    let category = classify_host_run_error(&err);
    let source = ErgoErrorSource::new(err);
    match category {
        HostRunCategory::AdapterRequired => ErgoRunError::AdapterRequired { source },
        HostRunCategory::Ingress => ErgoRunError::Ingress { source },
        HostRunCategory::EgressDispatch => ErgoRunError::EgressDispatch { source },
        HostRunCategory::EgressValidation => ErgoRunError::EgressValidation { source },
        HostRunCategory::Step => ErgoRunError::Step { source },
        HostRunCategory::Internal => ErgoRunError::Internal { source },
        HostRunCategory::GraphLoad => ErgoRunError::GraphLoad { source },
        HostRunCategory::GraphPreparation => ErgoRunError::GraphPreparation { source },
        HostRunCategory::AdapterComposition => ErgoRunError::AdapterComposition { source },
        HostRunCategory::AdapterSetup => ErgoRunError::AdapterSetup { source },
        HostRunCategory::EgressStartup => ErgoRunError::EgressStartup { source },
    }
}

pub(crate) fn map_host_run_error_to_runner(err: HostRunError) -> ErgoRunnerError {
    let category = classify_host_run_error(&err);
    let source = ErgoErrorSource::new(err);
    match category {
        HostRunCategory::AdapterRequired => ErgoRunnerError::AdapterRequired { source },
        HostRunCategory::GraphLoad => ErgoRunnerError::GraphLoad { source },
        HostRunCategory::GraphPreparation => ErgoRunnerError::GraphPreparation { source },
        HostRunCategory::AdapterComposition => ErgoRunnerError::AdapterComposition { source },
        HostRunCategory::AdapterSetup => ErgoRunnerError::AdapterSetup { source },
        HostRunCategory::EgressStartup => ErgoRunnerError::EgressStartup { source },
        HostRunCategory::Step
        | HostRunCategory::EgressDispatch
        | HostRunCategory::EgressValidation
        | HostRunCategory::Ingress
        | HostRunCategory::Internal => ErgoRunnerError::Initialization { source },
    }
}

#[derive(Debug)]
#[non_exhaustive]
/// Errors returned while replaying a capture.
///
/// Replay failures are grouped by the user decision they point to: capture
/// availability, graph setup, adapter setup, deterministic mismatch, or
/// internal invariant failure. Wrapped host detail is opaque on the public
/// surface and reachable via [`std::error::Error::source`].
pub enum ErgoReplayError {
    /// SDK-side configuration or profile resolution failed before replay
    /// could start.
    ///
    /// Action: inspect the inner [`ErgoConfigError`] for the specific
    /// configuration category. Live egress configurations are reported here
    /// because replay forbids live egress channels.
    Config {
        /// SDK configuration failure detail.
        inner: ErgoConfigError,
    },
    /// The capture file could not be opened or read.
    ///
    /// Action: confirm the capture path exists and is readable.
    CaptureRead {
        /// Opaque source describing the read failure.
        source: ErgoErrorSource,
    },
    /// The capture data could not be parsed or rehydrated into events.
    ///
    /// Action: confirm the capture file was produced by a compatible Ergo
    /// version and has not been truncated or corrupted.
    CaptureParse {
        /// Opaque source describing the parse failure.
        source: ErgoErrorSource,
    },
    /// Graph or cluster asset files referenced by the capture could not be
    /// loaded.
    ///
    /// Action: confirm the graph and cluster files referenced by the capture
    /// still exist on disk.
    GraphLoad {
        /// Opaque source describing the graph-load failure.
        source: ErgoErrorSource,
    },
    /// Graph assets loaded but graph preparation rejected the resolved
    /// graph for replay.
    ///
    /// Action: confirm the graph definition has not drifted from the version
    /// the capture was produced against.
    GraphPreparation {
        /// Opaque source describing the preparation failure.
        source: ErgoErrorSource,
    },
    /// Adapter manifest composition failed before replay started.
    ///
    /// Action: validate the adapter manifest the capture references.
    AdapterComposition {
        /// Opaque source describing the composition failure.
        source: ErgoErrorSource,
    },
    /// Adapter setup failed after composition.
    ///
    /// Action: confirm the adapter primitives the capture references are
    /// registered on the builder.
    AdapterSetup {
        /// Opaque source describing the adapter-setup failure.
        source: ErgoErrorSource,
    },
    /// Replay preflight rejected the capture before stepping started.
    ///
    /// Action: inspect the source detail for the specific preflight rule
    /// that the capture violated.
    ReplayPreflight {
        /// Opaque source describing the preflight failure.
        source: ErgoErrorSource,
    },
    /// Replay observed a deterministic mismatch between the capture and the
    /// re-executed graph.
    ///
    /// Action: this is the diagnostic surface for non-determinism; inspect
    /// the source detail for the diverging decision.
    ReplayMismatch {
        /// Opaque source describing the mismatch.
        source: ErgoErrorSource,
    },
    /// The capture uses external event kinds that the replay path cannot
    /// represent against the current graph.
    ///
    /// Action: confirm the graph's external event kinds match the capture.
    ReplayOwnership {
        /// Opaque source describing the ownership mismatch.
        source: ErgoErrorSource,
    },
    /// Per-event replay stepping failed.
    ///
    /// Action: inspect the source detail for the step failure.
    Step {
        /// Opaque source describing the step failure.
        source: ErgoErrorSource,
    },
    /// A host or SDK invariant failed during replay.
    ///
    /// Action: report as a bug; do not surface this category as user
    /// configuration feedback.
    Internal {
        /// Opaque source describing the invariant failure.
        source: ErgoErrorSource,
    },
}

impl std::fmt::Display for ErgoReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config { inner } => write!(f, "replay configuration invalid: {inner}"),
            Self::CaptureRead { .. } => write!(f, "capture file could not be read"),
            Self::CaptureParse { .. } => write!(f, "capture data could not be parsed"),
            Self::GraphLoad { .. } => write!(f, "graph or cluster assets could not be loaded"),
            Self::GraphPreparation { .. } => {
                write!(
                    f,
                    "graph preparation rejected the resolved graph for replay"
                )
            }
            Self::AdapterComposition { .. } => write!(f, "adapter manifest composition failed"),
            Self::AdapterSetup { .. } => write!(f, "adapter setup failed"),
            Self::ReplayPreflight { .. } => write!(f, "replay preflight rejected the capture"),
            Self::ReplayMismatch { .. } => {
                write!(f, "replay observed a deterministic mismatch")
            }
            Self::ReplayOwnership { .. } => {
                write!(
                    f,
                    "capture uses external event kinds the replay path cannot represent"
                )
            }
            Self::Step { .. } => write!(f, "per-event replay stepping failed"),
            Self::Internal { .. } => {
                write!(f, "replay failed an internal SDK or host invariant")
            }
        }
    }
}

impl std::error::Error for ErgoReplayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config { inner } => Some(inner),
            Self::CaptureRead { source }
            | Self::CaptureParse { source }
            | Self::GraphLoad { source }
            | Self::GraphPreparation { source }
            | Self::AdapterComposition { source }
            | Self::AdapterSetup { source }
            | Self::ReplayPreflight { source }
            | Self::ReplayMismatch { source }
            | Self::ReplayOwnership { source }
            | Self::Step { source }
            | Self::Internal { source } => Some(source.as_dyn_error()),
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
    let category = classify_host_replay_error(&err);
    if matches!(category, HostReplayCategory::Config) {
        return ErgoReplayError::Config {
            inner: ErgoConfigError::LiveEgressConfigurationNotAllowed,
        };
    }
    let source = ErgoErrorSource::new(err);
    match category {
        HostReplayCategory::Config => unreachable!("handled above"),
        HostReplayCategory::CaptureRead => ErgoReplayError::CaptureRead { source },
        HostReplayCategory::CaptureParse => ErgoReplayError::CaptureParse { source },
        HostReplayCategory::GraphLoad => ErgoReplayError::GraphLoad { source },
        HostReplayCategory::GraphPreparation => ErgoReplayError::GraphPreparation { source },
        HostReplayCategory::AdapterComposition => ErgoReplayError::AdapterComposition { source },
        HostReplayCategory::AdapterSetup => ErgoReplayError::AdapterSetup { source },
        HostReplayCategory::ReplayPreflight => ErgoReplayError::ReplayPreflight { source },
        HostReplayCategory::ReplayMismatch => ErgoReplayError::ReplayMismatch { source },
        HostReplayCategory::ReplayOwnership => ErgoReplayError::ReplayOwnership { source },
        HostReplayCategory::Step => ErgoReplayError::Step { source },
        HostReplayCategory::Internal => ErgoReplayError::Internal { source },
    }
}

#[derive(Debug)]
#[non_exhaustive]
/// Errors returned while validating a configured project.
///
/// Project validation walks each profile and returns the first failure it
/// encounters. The variant identifies which stage rejected the profile.
pub enum ErgoValidationError {
    /// Project-level SDK configuration prevented validation from starting.
    ///
    /// Action: inspect the inner [`ErgoConfigError`] for the specific
    /// configuration category.
    Config {
        /// SDK configuration failure detail.
        inner: ErgoConfigError,
    },
    /// A specific profile failed SDK configuration resolution before reaching
    /// host preflight.
    ///
    /// Action: the inner [`ErgoConfigError`] identifies the specific category
    /// that rejected this profile.
    Profile {
        /// Name of the profile being validated.
        profile: String,
        /// SDK configuration failure detail for this profile.
        inner: ErgoConfigError,
    },
    /// A specific profile reached host preflight and failed validation there.
    ///
    /// Action: the source detail identifies the rule the profile violated;
    /// reach it via [`std::error::Error::source`].
    HostValidation {
        /// Name of the profile being validated.
        profile: String,
        /// Opaque source describing the host preflight failure.
        source: ErgoErrorSource,
    },
}

impl std::fmt::Display for ErgoValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config { inner } => write!(f, "project configuration invalid: {inner}"),
            Self::Profile { profile, inner } => {
                write!(f, "profile '{profile}' configuration invalid: {inner}")
            }
            Self::HostValidation { profile, .. } => {
                write!(f, "profile '{profile}' failed host validation")
            }
        }
    }
}

impl std::error::Error for ErgoValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config { inner } | Self::Profile { inner, .. } => Some(inner),
            Self::HostValidation { source, .. } => Some(source.as_dyn_error()),
        }
    }
}
