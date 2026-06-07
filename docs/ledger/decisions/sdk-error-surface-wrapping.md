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
scaffold output, and `host/src/lib.rs` header) names the Rust SDK (then
planned for publication as `ergo-sdk-rust`) as the primary user-facing
product surface. `ergo-host` remains the
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

## Amendment 2026-06-07: Opaque Source Envelope Supersedes Typed Accessors

The original Q-SURFACE ruling allowed sparse typed parent accessors as an
advanced escape hatch. During the pre-publish PUB-4 hardening pass, that
accessor design was superseded by an SDK-owned opaque source envelope:
`ErgoErrorSource`.

This amendment is historical and prospective: it records that the earlier
typed-accessor design existed, and that the first published SDK contract no
longer uses it. Typed SDK helpers that name lower-crate taxonomies in their
method signatures are not canonical. Lower-layer diagnostic detail remains
available through the standard error source chain and opt-in downcasting by
callers that explicitly depend on the lower crate.

The supersession applies to lower-crate diagnostic errors from host,
adapter, supervisor, loader, and runtime surfaces. It does not erase direct
SDK-facing authoring/configuration carve-outs such as `EgressConfigError` or
`EgressConfigParseError`, which remain SDK vocabulary when PUB-1 classifies
them as intentional public authoring surface.

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

## Rule 2: Preserve Detail Via Opaque Source Envelope + Source Chain

Every SDK error variant that wraps lower-crate diagnostic detail preserves
that detail without naming the lower-crate type on the SDK public surface.
The standard wrapper for host, adapter, supervisor, loader, and runtime
diagnostics is the SDK-owned `ErgoErrorSource` opaque source envelope.

`ErgoErrorSource` participates in `std::error::Error::source` and exposes a
dynamic error reference for advanced diagnostics. Users who need structured
access to a lower-crate taxonomy depend on that lower crate explicitly and
use `downcast_ref` against the source chain. SDK tutorials should treat that
as diagnostic escape-hatch behavior, not the normal application matching
model.

SDK-owned nested errors may still appear as direct `inner` fields when they
are part of SDK vocabulary, such as `ErgoConfigError`, `ErgoCaptureError`,
or `ErgoStepError`. Direct SDK-facing authoring/configuration types may also
remain direct when PUB-1 classifies them as intentional SDK surface; examples
include `EgressConfigError` and `EgressConfigParseError`. These exceptions do
not authorize arbitrary lower-crate diagnostic enums to become SDK API.

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

The PUB-1 audit classifies any lower-crate re-export not on the keep-set as:
wrap-type, wrap-function, hide behind opaque source chain, keep as direct
authoring surface, or remove from SDK surface entirely.

## What This Does Not Change

- SDK behavioral delegation (`SDK-CANON-1/2/3`) -- unchanged.
- Host's internal structure -- unchanged.
- CLI's direct use of `ergo-host` -- unchanged. CLI is in-tree;
  LAYER-3 governs in-tree client behavior.
- The `ergo init` scaffold's SDK dependency (`ergo-sdk`) -- already
  correct for normal SDK use. Applications that opt into lower-crate
  diagnostic downcasting add the relevant lower crate explicitly.

## Implementation Guidance

1. Audit (PUB-1) classifies every current lower-crate public re-export as:
   wrap-type / wrap-function / hide-behind-opaque-source /
   keep-as-carve-out / remove. The starting keep-set above is the
   input; PUB-1 produces the final classification table.
2. New `ErgoXxxError` types are defined in
   `crates/prod/clients/sdk-rust/src/error.rs` (new module).
3. Each SDK method's return type changes from `Result<T, HostXxxError>`
   to `Result<T, ErgoXxxError>`.
4. Conversion from host errors to SDK errors happens at the SDK method
   boundary (typically via `From` impls or explicit `.map_err(...)`).
5. SDK internal tests that currently match on lower-crate error variants
   are rewritten to match on SDK error variants. Where a test specifically
   needs lower-crate detail, it inspects `ErgoErrorSource` or the standard
   error source chain and downcasts to the expected lower-crate type.
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
