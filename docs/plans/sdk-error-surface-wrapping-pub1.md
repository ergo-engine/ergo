---
Authority: PROJECT
Date: 2026-05-11
Author: Codex
Status: Active
Depends-On: docs/ledger/decisions/sdk-error-surface-wrapping.md
---

# SDK Error Surface Wrapping PUB-1 Plan

## Purpose

PUB-1 applies the Q-SURFACE ruling from
`docs/ledger/decisions/sdk-error-surface-wrapping.md` to the current
`ergo-sdk` public error surface.

## Branch Reconciliation / Supersession Note 2026-06-07

The original PUB-1 plan used sparse typed parent accessors as the advanced
escape hatch for lower-crate error detail. The pre-publish PUB-4 branch
supersedes that accessor prescription with the opaque `ErgoErrorSource`
source-envelope design now recorded in the Q-SURFACE decision amendment.

This plan remains authoritative for user-facing SDK categories, root
re-export disposition, and method return-type targets. Its old accessor
prescriptions are historical: lower-crate diagnostic detail is preserved
through `ErgoErrorSource`, `std::error::Error::source`, and opt-in
`downcast_ref` by callers that explicitly depend on the relevant lower
crate.

The old accessor-reachable host enum publish gate is replaced, not deleted:
before first publish, PUB-1/PUB-6 must inventory public host, adapter,
supervisor, loader, and runtime error enums reachable through SDK source
chains, `ErgoErrorSource::as_dyn_error()`, direct SDK reexports, or direct
SDK exception fields. Each extensible enum must be `#[non_exhaustive]`, or
PUB-1 must deliberately record why the enum is frozen/exhaustive for the
first published contract.

This plan produces two artifacts:

1. A classification table for every current `pub use ergo_host::{...}`
   item in `crates/prod/clients/sdk-rust/src/lib.rs`.
2. A wrapping implementation plan for every item classified as
   `wrap-type` or `wrap-function`.

This is a planning artifact only. The SDK refactor itself is later work.

## Audit User

The audit user is Sebastian writing applications against Ergo.

Each classification answers:

- What does Sebastian do with this item when writing an Ergo
  application?
- Is that use an SDK concern, or should it remain lower-layer
  vocabulary reachable only through diagnostics or advanced escape
  hatches?

## Classification Vocabulary

- `keep-type`: keep a transparent SDK root type re-export because it is
  direct authoring/config/outcome data.
- `keep-function`: keep a transparent SDK root function because it is a
  direct authoring/config helper.
- `wrap-type`: remove the root lower-layer type re-export and expose an
  SDK-branded error type or variant instead.
- `wrap-function`: replace the lower-layer function re-export with an
  SDK wrapper function that maps lower-layer errors to SDK errors.
- `hide-behind-opaque-source`: remove the root lower-layer type
  re-export; advanced callers reach it only by depending on the lower
  crate and inspecting the SDK error source chain, usually through
  `ErgoErrorSource::as_dyn_error()` and `downcast_ref`.
- `remove-function`: remove the lower-layer function re-export without a
  direct wrapper because the SDK exposes the same user question another
  way.

## Current Re-Export Inventory

Current block under audit:

```rust
pub use ergo_host::{
    is_recoverable_hosted_step_error, parse_egress_config_toml, write_capture_bundle, AdapterInput,
    CaptureBundle, CaptureJsonStyle, CaptureWriteError, EgressChannelConfig, EgressConfig,
    EgressConfigBuilder, EgressConfigError, EgressConfigParseError, EgressDispatchFailure,
    EgressRoute, HostAdapterCompositionError, HostAdapterSetupError, HostAvailableCluster,
    HostDependencyScanError, HostDriverError, HostDriverInputError, HostDriverIoError,
    HostDriverOutputError, HostDriverProtocolError, HostDriverStartError, HostExpandContext,
    HostExpandError, HostGraphPreparationError, HostReplayError, HostReplaySetupError,
    HostRunError, HostSetupError, HostedEgressValidationError, HostedEvent, HostedEventBuildError,
    HostedStepError, HostedStepOutcome, InterruptedRun, InterruptionReason, RunSummary,
};
```

This table covers all 39 items in that block.

| Item | Classification | PUB-1 justification |
|---|---|---|
| `is_recoverable_hosted_step_error` | remove-function | Sebastian should ask `ErgoStepError::is_recoverable()` rather than call a host predicate over a host enum. |
| `parse_egress_config_toml` | keep-function | Sebastian directly authors egress TOML and needs the SDK to parse it without depending on `ergo-host`. |
| `write_capture_bundle` | wrap-function | Sebastian may write a capture bundle manually, but failures should be SDK-branded. Replace the raw host re-export with an SDK wrapper returning `ErgoCaptureError`. |
| `AdapterInput` | keep-type | Sebastian supplies adapter configuration in advanced/prepared-runner flows. This is config input, not error taxonomy. |
| `CaptureBundle` | keep-type | Sebastian reads, stores, replays, and writes capture bundles. This is core SDK data. |
| `CaptureJsonStyle` | keep-type | Sebastian chooses capture JSON formatting when writing captures. This is direct authoring/config surface. |
| `CaptureWriteError` | wrap-type | Capture write failures are user-actionable, but the raw write-stage enum should not be SDK vocabulary. Wrap as `ErgoCaptureError::Write`. |
| `EgressChannelConfig` | keep-type | Sebastian constructs egress configs programmatically. This is direct SDK authoring surface. |
| `EgressConfig` | keep-type | Sebastian constructs or inspects validated egress configs directly. |
| `EgressConfigBuilder` | keep-type | Sebastian uses the builder for programmatic egress config. |
| `EgressConfigError` | keep-type | Direct egress config construction benefits from variant-level feedback; this is authoring surface, not host orchestration vocabulary. |
| `EgressConfigParseError` | keep-type | Direct egress TOML parsing benefits from parse/config distinction; higher-level SDK config wraps it with path context. |
| `EgressDispatchFailure` | hide-behind-opaque-source | Sebastian should match SDK categories or stable `InterruptionReason`; dispatch detail remains reachable only through the SDK error source chain and opt-in downcasting. |
| `EgressRoute` | keep-type | Sebastian constructs egress routing config programmatically. |
| `HostAdapterCompositionError` | hide-behind-opaque-source | Adapter composition details remain diagnostics behind `ErgoErrorSource`; SDK users match adapter categories first. |
| `HostAdapterSetupError` | hide-behind-opaque-source | Manifest read/parse/validation details stay available through source-chain diagnostics; SDK users match adapter categories first. |
| `HostAvailableCluster` | hide-behind-opaque-source | This is nested diagnostic data on `HostExpandError`, not a root SDK type. |
| `HostDependencyScanError` | hide-behind-opaque-source | Dependency scan failures are setup diagnostics inside host orchestration. |
| `HostDriverError` | hide-behind-opaque-source | The SDK exposes one `Ingress` run category; driver phase detail stays behind source-chain diagnostics. |
| `HostDriverInputError` | hide-behind-opaque-source | Driver input detail stays behind source-chain diagnostics. |
| `HostDriverIoError` | hide-behind-opaque-source | Driver I/O detail stays behind source-chain diagnostics. |
| `HostDriverOutputError` | hide-behind-opaque-source | Driver output detail stays behind source-chain diagnostics. |
| `HostDriverProtocolError` | hide-behind-opaque-source | Driver protocol detail stays behind source-chain diagnostics. |
| `HostDriverStartError` | hide-behind-opaque-source | Driver start detail stays behind source-chain diagnostics. |
| `HostExpandContext` | hide-behind-opaque-source | This is context on `HostExpandError`, not root SDK vocabulary. |
| `HostExpandError` | hide-behind-opaque-source | Graph expansion is exposed as SDK graph-preparation failure; host expansion detail stays behind source-chain diagnostics. |
| `HostGraphPreparationError` | hide-behind-opaque-source | SDK users match `GraphPreparation`; host graph-prep phases remain source-chain detail. |
| `HostReplayError` | wrap-type | Top-level replay failures become `ErgoReplayError` variants. Preserve the host replay error as source. |
| `HostReplaySetupError` | hide-behind-opaque-source | Replay setup details are nested under `HostReplayError` and stay behind source-chain diagnostics. |
| `HostRunError` | wrap-type | Top-level run and runner-prep failures become `ErgoRunError` / `ErgoRunnerError` variants. Preserve the host run error as source except where capture write normalizes to `ErgoCaptureError`. |
| `HostSetupError` | hide-behind-opaque-source | Setup details are nested under `HostRunError` and `HostReplayError` and stay behind source-chain diagnostics. |
| `HostedEgressValidationError` | hide-behind-opaque-source | SDK users match egress validation categories; host detail stays behind source-chain diagnostics. |
| `HostedEvent` | keep-type | Sebastian constructs hosted events when manually stepping a profile runner. |
| `HostedEventBuildError` | wrap-type | Event-build failure is a step-time category. `ErgoStepError::EventBuild` carries this exact source directly. |
| `HostedStepError` | wrap-type | Manual stepping returns `ErgoStepError`; run/replay methods surface step failures through operation errors. Preserve the host step error where it is the direct source. |
| `HostedStepOutcome` | keep-type | Sebastian receives this from manual stepping. It is outcome data, not error taxonomy. |
| `InterruptedRun` | keep-type | Stable run outcome data carried by `RunOutcome::Interrupted`; keep transparent with `RunOutcome`. |
| `InterruptionReason` | keep-type | Already the collapsed, user-facing status shape. Keep stable and transparent. |
| `RunSummary` | keep-type | Stable run success data carried by `RunOutcome::Completed`; keep transparent with `RunOutcome`. |

## Shared Rules for Error Wrapping

All `Ergo*` errors preserve non-SDK diagnostic detail through the standard
error source chain. Host, adapter, supervisor, loader, and runtime
diagnostics that should not become SDK vocabulary use `source:
ErgoErrorSource` rather than concrete lower-crate field types.

Typed SDK parent accessors are not part of the first published SDK contract.
Do not add accessors for lower-crate parents or nested leaves such as
`HostRunError`, `HostReplayError`, `HostedStepError`,
`HostAdapterSetupError`, `HostDriverInputError`, or
`EgressDispatchFailure`. Users who need those details depend on the
relevant lower crate explicitly and downcast the source-chain detail.

Direct SDK-facing exceptions remain direct when classified as intentional
authoring/configuration surface. In this plan, `EgressConfigError` and
`EgressConfigParseError` stay public because programmatic egress config and
TOML parsing are SDK-facing authoring concerns, not hidden host
orchestration diagnostics.

Reachable lower-crate public error enum stability inventory:

- Direct SDK exception/re-export error types, including
  `EgressConfigError` and `EgressConfigParseError`.
- Error enums wrapped behind `ErgoErrorSource`, including host, adapter,
  supervisor, loader, and runtime errors reachable through SDK error
  variants, source-chain traversal, and `ErgoErrorSource::as_dyn_error()`.
- Public nested lower-crate error enums matchable after downcasting those
  parent errors.

PUB-1/PUB-6 must audit that inventory against the semver rule in
`crates-io-publish-set.md`: mark extensible public lower-crate error enums
`#[non_exhaustive]` before first publish, or explicitly record why an enum's
variant set is frozen/exhaustive for the first published contract. This is
the replacement publish gate for the old accessor-reachable host enum gate.

`Internal` means a host/SDK invariant failed. It is not recoverable, is
not user-actionable configuration feedback, and should be reported as an
SDK/host bug. Mapping code must exhaust user-facing categories before
using `Internal`.

## `ErgoBuildError`

Purpose: engine construction failed before an `Ergo` handle exists.

Proposed variants:

- `Registration { source: ErgoErrorSource }`
- `Project { #[source] inner: ErgoProjectError }`
- `ProjectSourceConflict`

Runtime registration detail is preserved through `ErgoErrorSource` and the
standard error source chain.

Affected SDK methods:

- `ErgoBuilder::build() -> Result<Ergo, ErgoBuildError>` remains, but
  `ProjectConfig(ProjectError)` becomes `Project(ErgoProjectError)`.

## `ErgoProjectConfigError`

Purpose: in-memory project/profile construction validation only.

This is the rename target for the construction-only subset of current
`ProjectConfigError`.

Proposed variants:

- `InMemoryProjectHasNoProfiles`
- `InMemoryFixtureSourceLabelEmpty { profile: Option<String> }`
- `InMemoryFixtureItemsEmpty { profile: Option<String> }`
- `InMemoryProcessCommandEmpty { profile: Option<String> }`
- `InMemoryProcessExecutableBlank { profile: Option<String> }`

Move these current `ProjectConfigError` variants to `ErgoConfigError`
because they occur during operation/profile config resolution rather
than in-memory construction:

- `ExplicitRunProcessCommandEmpty`
- `EgressConfigRead`
- `EgressConfigParse`
- `FilesystemProfileCannotUseInMemoryCapture`
- `InMemoryAssetsCannotUseDefaultFilesystemCapture`

Affected SDK methods:

- `InMemoryProfileConfig::fixture_items(...)`
- `InMemoryProfileConfig::process(...)`
- `InMemoryProjectSnapshot::build()`
- In-memory profile validation helpers

## `ErgoProjectError`

Purpose: public in-memory project assembly failed.

`ErgoProjectError` is reserved for in-memory construction APIs. It does
not carry filesystem project loading, profile/run resolution, or host
orchestration failures.

Proposed variants:

- `Config { #[source] inner: ErgoProjectConfigError }`

Affected SDK methods:

- In-memory project/profile construction APIs return `ErgoProjectError`
  where they currently return `ProjectError`.
- Profile lookup while preparing a run/replay/runner returns
  `ErgoConfigError`, not `ErgoProjectError`.

## `ErgoConfigError`

Purpose: SDK/profile/run/replay configuration resolution once an
operation is being prepared.

Proposed variants:

- `ProjectNotConfigured`
- `ProfileNotFound { name: String }`
- `ProjectLoad { source: ErgoErrorSource }`
- `ProjectConfig { #[source] inner: ErgoProjectConfigError }`
- `ExplicitRunProcessCommandEmpty`
- `EgressConfigRead { path: PathBuf, #[source] inner: std::io::Error }`
- `EgressConfigParse { path: PathBuf, #[source] inner: EgressConfigParseError }`
- `FilesystemProfileCannotUseInMemoryCapture { profile: String }`
- `InMemoryAssetsCannotUseDefaultFilesystemCapture`
- `UnsupportedOperation { operation: &'static str, transport: &'static str }`

Do not put `AdapterRequired` or `ProductionRequiresAdapter` on
`ErgoConfigError`; those are host verdicts reached while preparing a
run/runner and belong to `ErgoRunError` or `ErgoRunnerError`.

Dual-shape egress parse rule:

- Direct authoring call:
  `parse_egress_config_toml(...) -> Result<EgressConfig, EgressConfigParseError>`.
- Higher-level SDK profile/run resolution:
  `ErgoConfigError::EgressConfigParse { path, inner }`.

This is intentional. Direct parsing returns the raw parser/config error;
operation-level config adds path/context.

Affected SDK methods/helpers:

- `run_request_from_config(...)`
- `resolve_profile_plan(...)`
- `resolve_filesystem_profile_plan(...)`
- `resolve_in_memory_profile_plan(...)`
- `validate_profile_plan(...)`
- `load_project(...)`
- `load_egress_config(...)`

## `ErgoRunError`

Purpose: running a profile or explicit run config failed.

Wrap target:

- `HostRunError` as `wrap-type`.

Proposed variants:

- `Config { #[source] inner: ErgoConfigError }`
- `AdapterRequired { source: ErgoErrorSource }`
- `GraphLoad { source: ErgoErrorSource }`
- `GraphPreparation { source: ErgoErrorSource }`
- `AdapterComposition { source: ErgoErrorSource }`
- `AdapterSetup { source: ErgoErrorSource }`
- `Ingress { source: ErgoErrorSource }`
- `EgressStartup { source: ErgoErrorSource }`
- `EgressValidation { source: ErgoErrorSource }`
- `EgressDispatch { source: ErgoErrorSource }`
- `Step { source: ErgoErrorSource }`
- `Capture { #[source] inner: ErgoCaptureError }`
- `Internal { source: ErgoErrorSource }`

Mapping guidance from `HostRunError`:

- `AdapterRequired(_)` and `ProductionRequiresAdapter` map to
  `AdapterRequired`.
- `Setup(LoadGraphAssets(_))` and `Setup(DependencyScan(_))` map to
  `GraphLoad`. These are transport-level failures (file not found, YAML
  decode error, unresolved cluster reference) that occur while loading
  graph assets from disk or memory before any expansion or validation
  takes place. Keep this distinct from `GraphPreparation` so users (and
  the CLI renderer) can react differently to load-side IO/decode
  failures vs. structural graph problems.
- `Setup(GraphPreparation(_))` maps to `GraphPreparation`. This covers
  expansion and validation failures that occur after the graph has
  successfully loaded — version-mismatch on cluster references, cycle
  detection, schema violations, and similar logical/structural problems.
  The split between `GraphLoad` and `GraphPreparation` is normative;
  future passes must not reconflate them into a single variant.
- `Setup(AdapterSetup(Composition(_)))` maps to `AdapterComposition`.
- Other `Setup(AdapterSetup(_))` maps to `AdapterSetup`.
- `Setup(StartEgress(_))` maps to `EgressStartup`.
- `Setup(HostedRunnerValidation(_))` maps according to the nested
  `HostedStepError` category.
- `Setup(HostedRunnerInitialization(_))` maps to `Internal` unless
  nested detail is clearly user-actionable config.
- Any `Driver(_)` maps to `Ingress`; users who need driver input/start/
  protocol/I/O/output distinctions inspect source-chain detail after
  explicitly depending on `ergo-host`.
- `Step(EgressDispatchFailure(_))` maps to `EgressDispatch`.
- `Step(EgressValidation(_))` maps to `EgressValidation`.
- Other `Step(_)` maps to `Step` or `Internal` based on
  `ErgoStepError` category.
- `CaptureWrite(inner)` maps to
  `Capture { inner: ErgoCaptureError::Write { source, bundle: None } }`.

`ErgoRunError` exposes no typed host accessor. Host run detail is opaque
source-chain detail. Capture-write failures normalize to the SDK-owned
`ErgoCaptureError` so callers match `ErgoRunError::Capture` directly.

Affected SDK methods:

- `Ergo::run(...) -> Result<RunOutcome, ErgoRunError>`
- `Ergo::run_with_stop(...) -> Result<RunOutcome, ErgoRunError>`
- `Ergo::run_profile(...) -> Result<RunOutcome, ErgoRunError>`
- `Ergo::run_profile_with_stop(...) -> Result<RunOutcome, ErgoRunError>`
- Internal `run_profile_plan_with_control(...)`

## `ErgoRunnerError`

Purpose: preparing a `ProfileRunner` failed.

Keep `ErgoRunnerError` distinct from `ErgoRunError`. The runner-prep
surface has a substantive setup contract of more than four variants and
does not include run-time ingress streaming, step dispatch, or capture
write outcomes. Keeping it distinct prevents run-only categories from
appearing on runner preparation.

Wrap target:

- `HostRunError` as `wrap-type` when returned by
  `prepare_hosted_runner_*`.

Proposed runner-only variants:

- `Config { #[source] inner: ErgoConfigError }`
- `AdapterRequired { source: ErgoErrorSource }`
- `GraphLoad { source: ErgoErrorSource }`
- `GraphPreparation { source: ErgoErrorSource }`
- `AdapterComposition { source: ErgoErrorSource }`
- `AdapterSetup { source: ErgoErrorSource }`
- `EgressStartup { source: ErgoErrorSource }`
- `Initialization { source: ErgoErrorSource }`
- `Internal { source: ErgoErrorSource }`

`GraphLoad` and `GraphPreparation` follow the same load/prep split as
`ErgoRunError`: load-side transport failures route to `GraphLoad`, and
post-load expansion/validation failures route to `GraphPreparation`.

`ErgoRunnerError` exposes no typed host accessor. Preparation detail is
opaque source-chain detail, and runner preparation intentionally has no
capture-write category.

Affected SDK methods:

- `Ergo::runner_for_profile(...) -> Result<ProfileRunner, ErgoRunnerError>`
- Internal profile runner preparation helpers.

## `ErgoStepError`

Purpose: manual stepping failed.

Confirmed shape: one type with `is_recoverable()` and `can_finish()`.
Do not split recoverable and non-recoverable step errors into separate
public types.

Wrap targets:

- `HostedStepError` as `wrap-type`.
- `HostedEventBuildError` as `wrap-type`, carried directly by the
  `EventBuild` variant.

Proposed variants:

- `Input { source: ErgoErrorSource }`
- `EventBuild { source: ErgoErrorSource }`
- `Binding { source: ErgoErrorSource }`
- `Lifecycle { source: ErgoErrorSource }`
- `EffectApply { source: ErgoErrorSource }`
- `HandlerCoverage { source: ErgoErrorSource }`
- `EgressValidation { source: ErgoErrorSource }`
- `EgressProcess { source: ErgoErrorSource }`
- `EgressDispatch { source: ErgoErrorSource }`
- `Internal { source: ErgoErrorSource }`

Mapping guidance from `HostedStepError`:

- `DuplicateEventId`, `MissingSemanticKind`, `MissingPayload`,
  `PayloadMustBeObject`, `UnknownSemanticKind` map to `Input`.
- `Binding(_)` maps to `Binding`.
- `EventBuild(inner)` maps to `EventBuild { source }`, preserving the
  exact event-build detail behind `ErgoErrorSource`.
- `LifecycleViolation` maps to `Lifecycle` when the message describes
  SDK/manual-runner misuse; `MissingDecisionEntry` and
  `EffectsWithoutAdapter` map to `Internal`.
- `EffectApply(_)` maps to `EffectApply`.
- `HandlerCoverage(_)` maps to `HandlerCoverage`.
- `EgressValidation(_)` maps to `EgressValidation`.
- `EgressProcess(_)` maps to `EgressProcess`.
- `EgressDispatchFailure(_)` maps to `EgressDispatch`.

Methods:

```rust
impl ErgoStepError {
    pub fn is_recoverable(&self) -> bool;
    pub fn can_finish(&self) -> bool;
}
```

Hosted step and hosted event-build detail is opaque source-chain detail, not
typed SDK accessor surface.

`is_recoverable()` preserves the current host predicate result: input,
binding, and event-build failures that currently return true from
`ergo_host::is_recoverable_hosted_step_error(...)` are recoverable for
continued stepping.

`can_finish()` answers the user question directly. It returns true for
egress-dispatch failure, where further stepping is blocked but
finalization is allowed.

Affected SDK methods:

- `ProfileRunner::step(...) -> Result<HostedStepOutcome, ErgoStepError>`
- `ProfileRunner::context_snapshot(...) -> Result<&BTreeMap<_, _>, ErgoStepError>`
- `ProfileRunner::finish(...) -> Result<CaptureBundle, ErgoStepError>`
- Internal `lifecycle_violation(...) -> ErgoStepError`

## `ErgoCaptureError`

Purpose: capture finalization or capture write failed.

PUB-1 resolves the capture carve-out by giving capture failures one SDK
concept: `ErgoCaptureError`.

Wrap targets:

- `CaptureWriteError` as `wrap-type`.
- `write_capture_bundle` as `wrap-function`.

Proposed variants:

- `Finalize { #[source] inner: ErgoStepError }`
- `OutputNotConfigured`
- `Write { source: ErgoErrorSource, bundle: Option<CaptureBundle> }`

```rust
impl ErgoCaptureError {
    pub fn capture_bundle(&self) -> Option<&CaptureBundle>;
}
```

Capture-write detail is opaque source-chain detail. `capture_bundle()` is an
SDK helper for retrying persistence after a failed write, not a typed
lower-crate error accessor.

Affected SDK methods/functions:

- Replace `ProfileRunnerCaptureError` with `ErgoCaptureError`.
- `ProfileRunner::finish_and_write_capture(...) -> Result<CaptureBundle, ErgoCaptureError>`
- Replace the raw root `write_capture_bundle` re-export with an SDK
  wrapper:

```rust
pub fn write_capture_bundle(
    path: impl AsRef<std::path::Path>,
    bundle: &CaptureBundle,
    style: CaptureJsonStyle,
) -> Result<(), ErgoCaptureError>
```

`Ergo::run*` maps host capture-write failures to
`ErgoRunError::Capture { inner: ErgoCaptureError::Write { ... } }`.
The direct wrapper uses `bundle: None`; `finish_and_write_capture(...)`
uses `bundle: Some(bundle)` so callers can recover the generated bundle.

## `ErgoReplayError`

Purpose: replaying a capture failed.

Wrap target:

- `HostReplayError` as `wrap-type`.

Proposed variants:

- `Config { #[source] inner: ErgoConfigError }`
- `CaptureRead { source: ErgoErrorSource }`
- `CaptureParse { source: ErgoErrorSource }`
- `GraphLoad { source: ErgoErrorSource }`
- `GraphPreparation { source: ErgoErrorSource }`
- `AdapterComposition { source: ErgoErrorSource }`
- `AdapterSetup { source: ErgoErrorSource }`
- `ReplayPreflight { source: ErgoErrorSource }`
- `ReplayMismatch { source: ErgoErrorSource }`
- `ReplayOwnership { source: ErgoErrorSource }`
- `Step { source: ErgoErrorSource }`
- `Internal { source: ErgoErrorSource }`

Mapping guidance from `HostReplayError`:

- `Setup(CaptureRead { .. })` maps to `CaptureRead`.
- `Setup(CaptureParse { .. })` maps to `CaptureParse`.
- `Setup(LiveEgressConfigurationNotAllowed)` maps to `Config`.
- `Setup(Setup(LoadGraphAssets(_)))` and
  `Setup(Setup(DependencyScan(_)))` map to `GraphLoad`. Same load/prep
  split as `ErgoRunError`: transport-level failures (file not found,
  YAML decode, unresolved cluster reference) belong here.
- `Setup(Setup(GraphPreparation(_)))` maps to `GraphPreparation`. This
  is reserved for post-load expansion and validation failures and must
  not absorb load-side errors.
- `Setup(Setup(AdapterSetup(Composition(_))))` maps to
  `AdapterComposition`.
- Other `Setup(Setup(AdapterSetup(_)))` maps to `AdapterSetup`.
- `GraphIdMismatch` maps to `ReplayMismatch`.
- `ExternalKindsNotRepresentable` maps to `ReplayOwnership`.
- `Hosted(Preflight(_))` maps to `ReplayPreflight`.
- `Hosted(EventRehydrate { .. })` maps to `CaptureParse`.
- `Hosted(Step(_))` maps to `Step`.
- `Hosted(Compare(_))` and `Hosted(DecisionMismatch)` map to
  `ReplayMismatch`.
- Any invariant breach maps to `Internal`.

`ErgoReplayError` exposes no typed host replay accessor. Replay detail is
opaque source-chain detail behind `ErgoErrorSource`.

Affected SDK methods:

- `Ergo::replay(...) -> Result<ReplayGraphResult, ErgoReplayError>`
- `Ergo::replay_bundle(...) -> Result<ReplayGraphResult, ErgoReplayError>`
- `Ergo::replay_profile(...) -> Result<ReplayGraphResult, ErgoReplayError>`
- `Ergo::replay_profile_bundle(...) -> Result<ReplayGraphResult, ErgoReplayError>`

## `ErgoValidationError`

Purpose: `validate_project` found problems.

This is the rename target for current `ErgoValidateError`.

Proposed variants:

- `Config { #[source] inner: ErgoConfigError }`
- `Profile { profile: String, #[source] inner: ErgoConfigError }`
- `HostValidation { profile: String, source: ErgoErrorSource }`

`ErgoValidationError` exposes no typed host run accessor. Host validation
detail remains source-chain detail so validation callers match SDK profile
and host-validation categories first.

Affected SDK methods:

- `Ergo::validate_project(...) -> Result<ProjectSummary, ErgoValidationError>`

## Adjacent Transparent Surfaces Outside the 39-Item Block

The decision record's keep-set includes items outside the audited
`pub use ergo_host::{...}` block. PUB-1 disposition:

- `PreparedGraphAssets`: keep transparent and make the root SDK export
  explicit. It is already used in public in-memory profile construction
  signatures.
- `RunOutcome`: keep transparent and make the root SDK export explicit.
  It is the primary run result data shape.
- `ReplayGraphResult`: keep transparent and make the root SDK export
  explicit. It is the primary replay result data shape.
- `InMemorySourceInput`: do not add an unused root export during this
  error-surface refactor. It remains an approved future transparent
  keep-set item when the SDK exposes an API that takes or returns it.
- `FixtureItem`: already transparently re-exported from `ergo_adapter`;
  keep as-is.

## Opaque Source and Stability Inventory Summary

The following current root exports disappear from the SDK root and are
reachable only through source-chain diagnostics after the caller explicitly
depends on the relevant lower crate:

- `EgressDispatchFailure` through hosted step or host run detail behind
  `ErgoErrorSource`.
- `HostAdapterCompositionError`, `HostAdapterSetupError`,
  `HostAvailableCluster`, `HostDependencyScanError`,
  `HostExpandContext`, `HostExpandError`, `HostGraphPreparationError`,
  and `HostSetupError` through host run or replay detail behind
  `ErgoErrorSource`.
- `HostDriverError` and its leaf types through host run detail behind
  `ErgoErrorSource`.
- `HostReplaySetupError` through host replay detail behind
  `ErgoErrorSource`.
- `HostedEgressValidationError` through hosted step or host run/replay
  detail behind `ErgoErrorSource`.

Do not add typed SDK helpers for those parents or leaves. The replacement
publish gate is the lower-crate public error enum stability inventory:
public error enums reachable through SDK source chains, direct reexports, or
direct exception fields must be `#[non_exhaustive]` if extensible or
explicitly recorded as frozen/exhaustive for the first published contract.

## Method Return-Type Change Summary

Public SDK methods should land with these return types:

| Method/function | Current return error | Planned return error |
|---|---|---|
| `ErgoBuilder::build` | `ErgoBuildError` | `ErgoBuildError` with `ErgoProjectError` variant |
| `InMemoryProfileConfig::*` builders | `ProjectError` | `ErgoProjectError` |
| `InMemoryProjectSnapshot::build` | `ProjectError` | `ErgoProjectError` |
| `Ergo::run*` | `ErgoRunError` | `ErgoRunError` with collapsed variants |
| `Ergo::replay*` | `ErgoReplayError` | `ErgoReplayError` with collapsed variants |
| `Ergo::validate_project` | `ErgoValidateError` | `ErgoValidationError` |
| `Ergo::runner_for_profile` | `ErgoRunnerError` alias | distinct `ErgoRunnerError` |
| `ProfileRunner::step` | `HostedStepError` | `ErgoStepError` |
| `ProfileRunner::context_snapshot` | `HostedStepError` | `ErgoStepError` |
| `ProfileRunner::finish` | `HostedStepError` | `ErgoStepError` |
| `ProfileRunner::finish_and_write_capture` | `ProfileRunnerCaptureError` | `ErgoCaptureError` |
| `write_capture_bundle` root export | `CaptureWriteError` | SDK wrapper returning `ErgoCaptureError` |

## PUB-1 Completion Criteria

PUB-1 is complete when:

- Every item in the current SDK `pub use ergo_host::{...}` block has one
  of: `wrap-type`, `wrap-function`, `hide-behind-opaque-source`,
  `keep-type`, `keep-function`, or `remove-function`.
- Every `wrap-type` item has a target `ErgoXxxError`, proposed variant
  names, opaque source preservation policy, and affected SDK methods.
- Every `wrap-function` item has a replacement SDK function and mapped
  SDK error.
- Provisional carve-outs are resolved:
  - `CaptureWriteError` is owned by `ErgoCaptureError`; `ErgoRunError`
    wraps capture failures through `ErgoCaptureError`.
  - `HostedEventBuildError` is carried directly by
    `ErgoStepError::EventBuild`.
- `ErgoStepError` is one public type with `is_recoverable()` and
  `can_finish()`.
- Adjacent transparent keep-set items outside the audited block have a
  stated post-refactor disposition.
- Reachable lower-crate public error enums have a completed direct,
  source-chain, and nested inventory covering host, adapter, supervisor,
  loader, and runtime surfaces. Required `#[non_exhaustive]` annotations or
  explicit frozen/exhaustive exceptions have landed or been recorded as part
  of the publish gate.
- No SDK refactor code or CLI changes are included in this planning
  pass.
