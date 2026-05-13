---
Authority: PROJECT
Date: 2026-05-11
Decision-Owner: Sebastian
Participants: Claude, Codex, Auggie
Status: DECIDED
Resolves: Q-SURFACE from docs/plans/crates-io-publish.md
---

# Decision: SDK Error Surface Wrapping

## Context

The SDK currently re-exports roughly 25 host error types transparently
through `pub use ergo_host::{...}`. Documented intent (per
`docs/ledger/dev-work/closed/ergo-init.md`, SDK README, `ergo init`
scaffold output, and `host/src/lib.rs` header) names `ergo-sdk-rust` as
the primary user-facing product surface. `ergo-host` remains the
canonical orchestration layer and may be used directly by in-tree
clients or advanced callers, but its detailed error taxonomy is not SDK
vocabulary.

The SDK is already behavior-thin (`SDK-CANON-1/2/3`: delegates
orchestration to host). Q-SURFACE resolves whether the SDK should also
be types-thick (wrap host types in SDK-branded equivalents) or remain
types-thin (re-export host types directly).

## Ruling

The SDK wraps non-SDK error types at the boundary. SDK consumers match
against SDK-branded error categories; lower-layer error taxonomy is
preserved underneath but is not part of the SDK public vocabulary.

## Rule 1: Collapsed Categories, Not 1:1 Mirror

The SDK exposes a small set of branded error types organized around what
users need to handle, not where in the host pipeline the error
originated. The canonical SDK error surface:

- `ErgoBuildError` -- engine construction failed (catalog
  registration, project source conflict, project validation).
- `ErgoRunError` -- running a profile failed (config resolution, host
  orchestration, dispatch).
- `ErgoReplayError` -- replaying a capture failed.
- `ErgoStepError` -- manual stepping failed (recoverable /
  non-recoverable step outcomes).
- `ErgoConfigError` -- SDK/profile/run configuration resolution
  generally; this includes filesystem project/profile lookup and
  operation configuration, but not in-memory project construction.
- `ErgoValidationError` -- `validate_project` found problems.
- `ErgoRunnerError` -- preparing a `ProfileRunner` failed.
- `ErgoProjectError` -- in-memory project assembly failed (renamed from
  `ProjectError`).
- `ErgoProjectConfigError` -- in-memory project/profile construction
  validation only (renamed from `ProjectConfigError`).

The split between `ErgoConfigError`, `ErgoProjectError`, and
`ErgoProjectConfigError` is intentional and must be preserved:

- `ErgoProjectConfigError` covers validation while constructing
  in-memory project/profile values.
- `ErgoProjectError` is reserved for public in-memory construction APIs
  that assemble those values.
- `ErgoConfigError` covers profile/run/replay configuration resolution
  once an SDK operation is being prepared.

They are not synonyms. In particular, adapter-required and
production-requires-adapter failures are host verdicts reached while
preparing a run or runner, so they belong on `ErgoRunError` or
`ErgoRunnerError`, not `ErgoConfigError`.

Each SDK error type's variants reflect user-facing categories (for
example, `ErgoRunError::AdapterComposition`,
`ErgoRunError::GraphPreparation`, `ErgoRunError::EgressDispatch`), not
host phase structure. Host types like `HostRunError`, `HostedStepError`,
`HostExpandError`, and related types are not 1:1 mirrored as first-class
SDK vocabulary.

SDK variants may include an `Internal` category when a lower layer
reports a violated host/SDK invariant rather than a user-correctable
configuration or boundary-channel failure. `Internal` is not
recoverable, should be reported as an SDK/host bug, and must not become
a lazy bucket for unmapped host variants. Mapping code must exhaust
known user-facing categories before using `Internal`.

## Rule 2: Preserve Detail Via `#[source]` + Sparse Explicit Accessors

Every SDK error variant that wraps a non-SDK error preserves the
underlying error as a `#[source] inner: XxxError` field. This rule was
prompted by host errors, but it applies to all non-SDK crate errors
reachable through SDK error variants, including loader and runtime
errors.

Explicit accessors are an advanced escape hatch, not the primary
matching model. They intentionally make the returned non-SDK type part
of the SDK method signature, so they must stay sparse and parent-only:
expose the nearest owning source object a caller can reasonably
pattern-match, not one helper per nested host leaf type.

```rust
impl ErgoRunError {
    pub fn as_host_run_error(&self) -> Option<&ergo_host::HostRunError> { ... }
}
```

Users who need structured access to host taxonomy depend on `ergo-host`
explicitly and pattern-match through the parent accessor. The SDK does
not re-export host error types from its root, and SDK tutorials should
not treat host accessors as the normal application path.

Loader and runtime errors follow the same rule: they may appear as
`#[source]` fields inside `Ergo*` errors, but they should not be
casually promoted into root SDK vocabulary.

## Rule 3: `Ergo*` Prefix Naming Convention

All SDK-boundary error types use the `Ergo*` prefix. This contrasts with
host's `Host*` prefix, giving visual separation between layers. `Sdk*`
is rejected as feeling like implementation taxonomy. Unprefixed names
(`RunError`, `StepError`) are rejected due to collision risk in user
code.

## Starting Keep-Set for PUB-1 Classification

The following items are the starting keep-set for PUB-1 audit. This
list is not an exemption from classification. PUB-1 may still wrap items
on this list if a branded SDK error gives a cleaner boundary without
harming direct authoring use.

Authoring/config types (likely permanent transparent re-exports):

- `EgressConfig`, `EgressConfigBuilder`, `EgressChannelConfig`,
  `EgressRoute`, `EgressConfigError`, `EgressConfigParseError`
- `AdapterInput`
- `CaptureBundle`, `CaptureJsonStyle`
- `HostedEvent`
- `FixtureItem` (already re-exported from `ergo-adapter`, used in
  `InMemoryProfileConfig::fixture_items`)
- `InterruptionReason` (user-facing status shape, kept stable)
- `RunOutcome`, `ReplayGraphResult` (return types)
- `DriverConfig`, `RunControl`, `LivePrepOptions`
- `PreparedGraphAssets`, `InMemorySourceInput`

Provisional carve-outs (PUB-1 should re-examine):

- `CaptureWriteError` -- candidate for wrapping as a variant of
  `ErgoRunError` or a new `ErgoCaptureError`.
- `HostedEventBuildError` -- candidate for wrapping as a variant of
  `ErgoStepError`.

The PUB-1 audit classifies any host re-export not on the keep-set as:
wrap-type, wrap-function, hide behind parent accessor, keep as direct
authoring surface, or remove from SDK surface entirely.

## What This Does Not Change

- SDK behavioral delegation (`SDK-CANON-1/2/3`) -- unchanged.
- Host's internal structure -- unchanged.
- CLI's direct use of `ergo-host` -- unchanged. CLI is in-tree;
  LAYER-3 governs in-tree client behavior.
- The `ergo init` scaffold's `Cargo.toml` (only `ergo-sdk-rust`) --
  already correct for normal SDK use. Applications that opt into
  host-aware accessor matching add `ergo-host` explicitly.

## Implementation Guidance

1. Audit (PUB-1) classifies every current `pub use ergo_host::{...}` as:
   wrap-type / wrap-function / hide-behind-parent-accessor /
   keep-as-carve-out / remove. The starting keep-set above is the
   input; PUB-1 produces the final classification table.
2. New `ErgoXxxError` types are defined in
   `crates/prod/clients/sdk-rust/src/error.rs` (new module).
3. Each SDK method's return type changes from `Result<T, HostXxxError>`
   to `Result<T, ErgoXxxError>`.
4. Conversion from host errors to SDK errors happens at the SDK method
   boundary (typically via `From` impls or explicit `.map_err(...)`).
5. SDK internal tests that currently match on host error variants are
   rewritten to match on SDK error variants. Where a test specifically
   needs host detail, it uses the nearest parent accessor such as
   `as_host_run_error()` or `as_hosted_step_error()`.
6. Workspace `cargo test` passes before commit.

## Follow-Ups for PUB-1 and Implementation

- Variant granularity within each `ErgoXxxError` -- refined during
  implementation per the user-facing-category principle, not
  pre-specified here.
- PUB-1 decides whether `ErgoStepError` stays one type or splits. The
  recommended shape is one type with `is_recoverable()` and a
  user-question helper such as `can_finish()`.
- PUB-1 decides final disposition of provisional carve-outs
  (`CaptureWriteError`, `HostedEventBuildError`).
- PUB-1 decides whether `ErgoRunnerError` stays aliased to
  `ErgoRunError` or becomes a distinct type by concrete variant shape:
  if the runner-only surface is one or two variants, alias it; if it has
  a substantive four-plus-variant setup contract, keep it distinct and
  document the difference from run execution.
