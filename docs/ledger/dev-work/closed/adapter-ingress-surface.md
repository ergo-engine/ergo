---
Authority: PROJECT
Date: 2026-03-15
Author: Claude Opus 4.5 (Structural Auditor) + Codex (Rewrite/Reconciliation)
Status: CLOSED
Branch: feat/adapter-runtime
Tier: 2 (Extension Plumbing)
Depends-On: none (can parallel with Tier 1; consumed later by feat/ergo-init)
---

# Adapter Ingress Surface

## Scope

Define the prod-owned ingress-channel surface and host-owned execution
path where user-authored adapter packages connect to the canonical host
path.

This branch does not extract `HostedRunner` into a broad trait. Most
adapter/runtime machinery already exists and must be reused. The missing
seam is narrower:

1. Event ingress: how prod-owned or user-authored code produces
   `HostedEvent` values and feeds the canonical host step loop.

Settled branch decisions:

1. `RunGraphFromPathsRequest` carries `driver: DriverConfig` instead of
   `fixture_path: PathBuf`. `DriverConfig` is the current code term for
   the ingress-channel selector.
2. `ReplayGraphFromPathsRequest` accepts no `DriverConfig`; replay
   events come from capture bundles only.
3. `DriverConfig` sits alongside `adapter_path` on the host request,
   not inside `HostedAdapterConfig` or `CanonicalAdapterSetup`.
4. Ingress setup and adapter setup are sibling preparations in
   `crates/prod/core/host/src/usecases.rs`.
5. `HostedAdapterConfig` remains semantic-only host state: provides,
   binder, and adapter provenance.
6. Canonical run returns
   `RunGraphResponse = Result<RunOutcome, HostRunError>`, where
   `RunOutcome` is `Completed(RunSummary)` or
   `Interrupted(InterruptedRun)`.
7. The v0 public live ingress-channel model for this branch is
   `Process` only. `Fixture` remains the built-in reference ingress
   shape. Any Rust trait stays private to host implementation.
8. Driver config never lives in `adapter.yaml`; the adapter manifest
   remains purely semantic.
9. Workspace-level ergonomics for specifying `DriverConfig` are
   deferred to `feat/ergo-init`.

No new kernel semantics. No replacement of existing adapter validation,
event binding, context merge, capture, or replay machinery.

## Current Implemented State

### Existing Kernel Adapter Surface

Kernel adapter already owns:

- Core boundary types in `crates/kernel/adapter/src/lib.rs`:
  `ExternalEvent`, `ExecutionContext`, `EventPayload`, `EventTime`,
  `ExternalEventKind`, `GraphId`, `EventId`, `RuntimeHandle`,
  `RuntimeInvoker`, `RunResult`, `RunTermination`, and `ErrKind`.
- Manifest and provides surfaces in
  `crates/kernel/adapter/src/manifest.rs` and
  `crates/kernel/adapter/src/provides.rs`:
  `AdapterManifest`, `AdapterProvides`, `ContextKeyProvision`, and
  `AdapterProvides::from_manifest()`.
- Registration validation in
  `crates/kernel/adapter/src/validate.rs` via `validate_adapter()`.
- Composition validation in
  `crates/kernel/adapter/src/composition.rs` via
  `validate_source_adapter_composition()`,
  `validate_capture_format()`, and
  `validate_action_adapter_composition()`.
- Event binding in
  `crates/kernel/adapter/src/event_binding.rs` via `EventBinder`,
  `compile_event_binder()`, and
  `bind_semantic_event_with_binder()`.
- Host primitives in `crates/kernel/adapter/src/host/`:
  `EffectHandler`, `SetContextHandler`, `ContextStore`,
  `BufferingRuntimeInvoker`, and `ensure_handler_coverage()`.
- Capture and provenance in
  `crates/kernel/adapter/src/capture.rs` and
  `crates/kernel/adapter/src/provenance.rs`:
  `capture::ExternalEventRecord`, `rehydrate_checked()`, and
  `adapter_fingerprint()`.

### Existing Prod Host Assembly

Prod host already owns:

- `HostedEvent` in `crates/prod/core/host/src/runner.rs` as the
  canonical host event type.
- `HostedRunner` in `crates/prod/core/host/src/runner.rs` as the
  canonical host loop, including duplicate `event_id` rejection,
  capture enrichment, and effect drain/apply behavior.
- `HostedRunner::build_external_event()` and
  `HostedRunner::execute_step()` in
  `crates/prod/core/host/src/runner.rs` as the canonical handoff path.
- `RunGraphFromPathsRequest` in
  `crates/prod/core/host/src/usecases.rs`, which now carries
  `graph_path`, `cluster_paths`, `driver`, `adapter_path`, and capture
  options. `driver` is the current implementation field name for the
  ingress-channel selector.
- `RunGraphResponse`, `RunOutcome`, `RunSummary`, `InterruptedRun`, and
  `InterruptionReason` in
  `crates/prod/core/host/src/usecases.rs`.
- `ReplayGraphFromPathsRequest` in
  `crates/prod/core/host/src/usecases.rs`, which remains free of live
  ingress or egress channel config.
- Canonical client APIs `run_graph_from_paths()` and
  `replay_graph_from_paths()` in
  `crates/prod/core/host/src/usecases.rs`.
- Adapter setup in `prepare_adapter_setup()` and
  `validate_adapter_composition()` in
  `crates/prod/core/host/src/usecases.rs`.
- Ingress setup in `run_fixture_driver()`, `run_process_driver()`,
  `drain_process_after_end()`, and `finalize_run_summary()` in
  `crates/prod/core/host/src/usecases.rs`.
- Adapter dependency scan in `scan_adapter_dependencies()` in
  `crates/prod/core/host/src/usecases.rs`.

### Remaining Gaps

No branch-local closure gaps remain.

Follow-on work now tracked outside this branch:

- Current canonical host APIs accept one ingress-channel config per
  run. Multi-ingress host support remains future work; today's
  multi-source cases require an upstream multiplexer or a later host
  request-shape extension.
- Egress-channel configuration, lifecycle, routing, and protocol remain
  separate follow-on work. This branch does not close that surface.

## What Must Not Be Reimplemented

This branch must consume existing adapter and host machinery. It must
not replace, wrap with a parallel contract, or silently fork behavior.

Do not reimplement:

- `AdapterManifest` parsing and `AdapterProvides::from_manifest()`
- `validate_adapter()` and all `ADP-*` registration validation
- `validate_source_adapter_composition()`,
  `validate_action_adapter_composition()`, and
  `validate_capture_format()`
- `EventBinder`, `compile_event_binder()`, and
  `bind_semantic_event_with_binder()`
- `HostedRunner::build_external_event()` context merge and payload
  shaping rules
- `EffectHandler`, `SetContextHandler`, handler routing in
  `execute_step()`, and `ensure_handler_coverage()`
- `BufferingRuntimeInvoker` replace-only / drain-once lifecycle
- Capture enrichment, capture bundle production, and strict replay
  verification
- Replay event sourcing from capture bundles
- `adapter_fingerprint()` provenance generation
- `scan_adapter_dependencies()` in host usecases

## Implemented Host Contract

The implemented host contract is:

1. Canonical run ingress is host-owned through the current
   `DriverConfig::{Fixture, Process}` selector on
   `RunGraphFromPathsRequest`.
2. Canonical replay stays capture-driven and accepts no live ingress or
   egress channel config.
3. All ingress converges on the same host-owned path:
   `HostedEvent -> build_external_event() -> execute_step()`.
4. Adapter setup and ingress setup remain sibling host preparations.
   Ingress launch config never becomes adapter semantic state.
5. `Process` is the only public live ingress-channel shape in this
   branch. Host launches a direct child process from
   `command: Vec<String>` without shell interpretation.
6. `Process` ingress protocol is host-owned:
   `stdout` is UTF-8 JSON Lines with `hello`, `event`, and `end`
   envelopes; `stderr` is diagnostics only; `stdin` is unused in v0.
7. Backpressure is synchronous and host-owned: one event at a time
   through the canonical host step path. Child blocking on `stdout` is
   the v0 backpressure mechanism.
8. Run outcome is first-class at the host API:
   `Completed` means the host finalized a trustworthy artifact after
   valid protocol completion; `Interrupted` means execution had already
   started and the host still finalized a trustworthy partial artifact;
   `HostRunError` means the host cannot return a trustworthy artifact.
9. "Run started" means the first successful `HostedRunner::step()` that
   advances capture state, regardless of `Invoke` vs `Defer` vs `Skip`.
10. Protocol truth and host waiting policy are separate. `Completed`
    requires valid `hello`, one or more committed events, `end`,
    `stdout` EOF, child exit status `0`, and successful capture
    finalization. Non-zero exit after `end` is never completion.
11. `Process` waiting behavior is private host policy, not protocol law.
    Host uses bounded `startup_grace` before the first protocol
    observation / `hello`, bounded `termination_grace` after `end` or
    `stdout` EOF, and `poll_interval` only as an implementation detail
    of that waiting policy.
12. Bad `Process` ingress channels cannot wedge canonical host forever.
    Silent startup, malformed protocol bytes, post-`end` extra output,
    `stdout` EOF before exit, and hang-after-`end` all map to truthful
    `HostRunError` / `Interrupted` outcomes under host-owned bounded
    waits.
13. Current canonical host run accepts one ingress-channel config per
    run. Multi-ingress topologies are future host work and are not
    implied by this branch.
14. v0 lifecycle management is scoped to the direct child process.
    Full descendant process-tree containment is not part of this
    branch and must not be implied by closure.
15. Egress-channel configuration, lifecycle, and protocol are not part
    of this branch. Host-internal effect handling remains internal
    machinery, not a user-facing extension surface delivered here.

## Closure Ledger

`AR-1`
Finalized host ingress API.
Status: `CLOSED`
Closure: The ledger records the final host API shape:
`RunGraphFromPathsRequest` carries `driver: DriverConfig`, replay
requests carry no `DriverConfig`, `RunGraphResponse` exposes
`Completed` vs `Interrupted`, `Process` is the only public live
ingress-channel model, and host-owned backpressure/blocking semantics
are documented.

`AR-2`
Keep adapter setup and ingress setup as sibling preparations.
Status: `CLOSED`
Closure: Host usecases prepare adapter state and ingress state through
separate preparation paths. `HostedAdapterConfig` and
`CanonicalAdapterSetup` remain semantic adapter state only; ingress
config is not stored there.

`AR-3`
Preserve canonical host path.
Status: `CLOSED`
Closure: All ingress yields or materializes `HostedEvent` as applicable
and feeds `HostedRunner::step()`. No ingress channel constructs
canonical
`ExternalEvent` or bypasses `build_external_event()` /
`execute_step()`.

`AR-4`
Replay guardrail preserved.
Status: `CLOSED`
Closure: `ReplayGraphFromPathsRequest` accepts no `DriverConfig`;
replay events come only from capture bundles; strict replay path remains
unchanged.

`AR-6`
Implement `DriverConfig::Fixture`.
Status: `CLOSED`
Closure: Current fixture-backed canonical execution is expressed through
`DriverConfig::Fixture` while preserving context merge, effect routing,
capture, and replay behavior.

`AR-7`
Implement public live ingress-channel shape.
Status: `CLOSED`
Closure: `DriverConfig::Process` is the single public live
ingress-channel model. Host owns process launch, `hello` / `event` /
`end` protocol handling, `Completed` vs `Interrupted` mapping, bounded
startup behavior, and protocol-failure classification.

`AR-8`
Preserve manifest/runtime alignment.
Status: `CLOSED`
Closure: Construction path reuses `validate_adapter()`,
`AdapterProvides::from_manifest()`, `compile_event_binder()`,
`validate_source_adapter_composition()`,
`validate_action_adapter_composition()`, `validate_capture_format()`,
and `ensure_handler_coverage()` so manifest/runtime mismatches fail
before stepping begins. Driver config remains outside `adapter.yaml`.

`AR-9`
Lifecycle, shutdown, and drain semantics.
Status: `CLOSED`
Closure: Host-owned ingress execution defines and implements graceful
shutdown, bounded startup / terminal waits, drain behavior, and
truthful outcome mapping for supported live ingress-channel shapes.
`Completed` requires protocol-complete clean exit; bad ingress channels
cannot wedge canonical host forever.

`AR-10`
Driver authoring guide published.
Status: `CLOSED`
Closure: A project-level developer doc exists under `docs/` at
`docs/authoring/ingress-channel-guide.md`. It covers `HostedEvent` wire
format, manifest/schema mapping, conceptual host handoff, working
examples for supported ingress shapes, and explicit prohibitions
against constructing `ExternalEvent`, bypassing `step()`, or owning
capture/replay semantics.

`AR-11`
Test: replay-valid fixture capture.
Status: `CLOSED`
Closure: Fixture ingress runs through the canonical host path, produces
a capture bundle, and that bundle passes strict replay.

`AR-14`
Test: live-source failure preserves host semantics.
Status: `CLOSED`
Closure: `Process` ingress-channel tests cover silent startup before `hello`,
malformed bytes before first committed step, malformed bytes after a
committed step with replay-valid interrupted artifact, non-zero exit
after `end`, extra `stdout` after `end`, hang after `end`, and
`stdout` EOF before exit. Post-start failures only report
`Interrupted(...)` when finalization succeeds.

## Design Constraints

- This branch is host-led orchestration work, but it reuses existing
  kernel-adapter host primitives.
- Prod/host owns the current `DriverConfig` ingress selector, the
  event-ingress contract, process launch, and canonical orchestration
  shape.
- Kernel-adapter already owns `EffectHandler`, `SetContextHandler`,
  `ContextStore`, `BufferingRuntimeInvoker`, and
  `ensure_handler_coverage()`. Those surfaces are consumed, not
  replaced.
- Driver config never lives in `adapter.yaml`; the adapter manifest
  remains purely semantic.
- Workspace-level ingress-channel discovery and ergonomic wiring belong to
  `feat/ergo-init`, not this branch.
- Current canonical host APIs accept one ingress-channel config per
  run. Multiple live sources require upstream multiplexing or future
  host multi-ingress support.
- Egress-channel configuration, lifecycle, and protocol are separate
  follow-on work. This branch does not expose custom host-handler
  injection as a delivered extension surface.
- No new kernel rule IDs, no semantic rewrites, and no new replay
  mechanism are allowed.
- Replay continues to source events from capture bundles only; no replay
  API accepts `DriverConfig`.
- The fixture path is the built-in reference ingress for this branch;
  `Process` is the public live ingress-channel shape.
- `startup_grace` and `termination_grace` are private host operational
  policy. They bound how long host waits to observe protocol truth, but
  they do not change what counts as clean completion.
- `Process` lifecycle management in v0 applies to the direct child
  process only. Full process-tree containment is out of scope.
- No domain-specific language in this branch. Use `event ingress`,
  `HostedEvent`, `process ingress channel`, and `host-internal effect
  handler`, not vertical-specific terms.

## Relationship to Existing Gaps

This branch closes:

- Gap 2: no adapter ingress surface / host ingress path
- Gap 5: fixture/adapter parity via the simulation adapter and
  canonical host-path reuse

This branch does not close:

- `GW-EFX-2`: multi-ingress host direction
- `GW-EFX-3`: egress-channel contract and lifecycle
