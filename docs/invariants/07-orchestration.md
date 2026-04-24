---
Authority: CANONICAL
Version: v1
Last Updated: 2026-04-24
Owner: Documentation
Scope: Orchestration-phase invariants for supervisor and host scheduling
Change Rule: Operational log
---

## 7. Orchestration Phase

**Scope:** Supervisor scheduling of episodes.

**Source:** supervisor.md

**Entry invariants:**

- Graph is validated (all V.* invariants hold)
- Adapter is available and compliant for production paths; optional for fixture paths

### Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| CXT-1 | ExecutionContext is externally supplied and adapter-governed | supervisor.md §3 | ✓ | — | — | ✓ |
| SUP-1 | Supervisor is graph-identity fixed | supervisor.md §3 | ✓ | — | — | — |
| SUP-2 | Supervisor is strategy-neutral | supervisor.md §3 | ✓ | — | — | ✓ |
| SUP-3 | Supervisor decisions are replayable | supervisor.md §3 | — | — | — | ✓ |
| SUP-4 | Retries only on mechanical failure | supervisor.md §3 | ✓ | — | — | ✓ |
| SUP-5 | ErrKind is mechanical only | supervisor.md §3 | ✓ | — | — | — |
| SUP-6 | Episode atomicity is invocation-scoped | supervisor.md §3 | — | — | — | — |
| SUP-7 | DecisionLog is write-only | supervisor.md §3 | ✓ | — | — | ✓ |
| SUP-TICK-1 | Pump events use deferred-retry scheduling (legacy `Tick` alias accepted) | — | — | — | — | ✓ |
| RTHANDLE-META-1 | RuntimeHandle forwards graph_id and event_id to metadata-aware runtime execution | — | ✓ | — | — | — |
| RTHANDLE-ID-1 | FaultRuntimeHandle keys injected outcomes on EventId only | — | ✓ | — | — | ✓ |
| RTHANDLE-ERRKIND-1 | Pre-execution failures map to ValidationFailed, not RuntimeError or SemanticError | supervisor.md §2.4 | — | — | ✓ | ✓ |

### Notes

- **CXT-1:** `ExecutionContext` is restricted to externally supplied and adapter-governed values for the current attempt; supervisor-derived or episode-derived state is excluded. For production paths, the adapter declarative contract governs which context keys may be populated and what event kinds may trigger evaluation. Host-side enforcement locus: `crates/prod/core/host/src/runner.rs:714-744` (`build_external_event`).
- **SUP-1:** Private `graph_id` field with no setters; set only at construction. Source: `crates/kernel/supervisor/src/lib.rs` (`Supervisor::graph_id`).
- **SUP-2:** `RuntimeInvoker::run()` returns `RunTermination` only; no `RunResult` exposure. Public seam: `crates/kernel/adapter/src/lib.rs` (`RuntimeInvoker::run`); `RunResult` is private to `ergo-adapter` at `crates/kernel/adapter/src/lib.rs:182`, so `SUP-2` is type-enforced at the public seam.
- **SUP-3:** Strict replay entry: `crates/kernel/supervisor/src/replay.rs:184` (`replay_checked_strict`).
- **SUP-4:** `should_retry()` matches only `NetworkTimeout|AdapterUnavailable|RuntimeError|TimedOut`. Source: `crates/kernel/supervisor/src/lib.rs:450`.
- **SUP-5:** `ErrKind` enum contains only mechanical variants; no domain-flavored errors. Source: `crates/kernel/adapter/src/lib.rs:164`.
- **SUP-6:** Invocation-scoped atomicity preserved by host non-rollback posture: `crates/prod/core/host/src/runner.rs:793` (`// SUP-6 alignment: no rollback on handler failure.`).
- **SUP-7:** `DecisionLog` trait has only `fn log()`; `records()` is on concrete impl, not trait. Source: `crates/kernel/supervisor/src/lib.rs:87`.
- **SUP-TICK-1:** Pump events have special deferred-retry behavior distinct from Command events; legacy `Tick` capture values deserialize to `Pump` via serde alias. Test: `replay_harness.rs` uses Command (not Pump) to avoid interference. Sources: `crates/kernel/supervisor/src/lib.rs:272` (Pump branch enqueues via `enqueue_deferred`); `crates/kernel/adapter/src/lib.rs:285` (`#[serde(alias = "Tick")]` on `ExternalEventKind::Pump`).
- **RTHANDLE-META-1:** `RuntimeHandle::run()` calls `execute_with_metadata(...)` and forwards `graph_id` / `event_id` into metadata-aware runtime execution. This is required for deterministic `intent_id` derivation when Actions declare external intents. Source: `crates/kernel/adapter/src/lib.rs:551` (`RuntimeHandle::run`), `:643` (`execute_with_metadata` call).
- **RTHANDLE-ID-1:** `FaultRuntimeHandle` still discards `graph_id` and keys injected outcomes on `EventId` only. The metadata-less runtime path rejects intent-emitting graphs, so the fault harness retains EventId-only determinism without becoming a live intent-ID source. Source: `crates/kernel/adapter/src/lib.rs:732-744` (`FaultRuntimeHandle`).
- **RTHANDLE-ERRKIND-1:** CLOSED (2026-02-06). `RuntimeHandle::run()` maps pre-execution failures to `ErrKind::ValidationFailed`, not `RuntimeError` or `SemanticError`.
  - **Prior bug (runtime_validate path):** `runtime_validate()` errors mapped to `ErrKind::RuntimeError`. Since `should_retry()` treats `RuntimeError` as retryable, this caused **pathological retries** of structurally invalid graphs — a graph that fails validation will fail identically on every retry.
  - **Prior bug (validate_composition path):** `validate_composition()` errors mapped to `ErrKind::SemanticError`. Non-retryable (correct behavior), but **wrong category** — `SemanticError` is for runtime deterministic failures (DivisionByZero, NonFiniteOutput per B.2), not validation-time COMP-* checks.
  - **Fix:** Both paths now return `ErrKind::ValidationFailed`, which is non-retryable (`should_retry` returns `false`) and categorically correct per supervisor.md §2.4.
  - **Note:** `ErrKind::ValidationFailed` was defined since v0 but never instantiated until this fix. Both error paths should have used it from the start.
  - **Test:** `runtime_handle_rejects_required_context_when_provides_empty` updated to assert `ValidationFailed`.

### Canonical Host Loop (ergo-host)

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| HST-1 | Effect application locus is host boundary, not DecisionLog readback | supervisor.md §3, adapter.md | — | — | ✓ | ✓ |
| HST-2 | `set_context` validates declared key, writable, and type | action.md #COMP-11..14 | — | — | ✓ | ✓ |
| HST-3 | Non-invoke decisions apply no effects | supervisor.md §3 | — | — | ✓ | ✓ |
| HST-4 | Retry path cannot duplicate committed effects | supervisor.md §3 | — | — | ✓ | ✓ |
| HST-5 | Run setup fails when graph-emittable accepted effect lacks host ownership (handler or egress route) | action.md #COMP-14/#COMP-17..19, adapter.md #2.4 | — | — | ✓ | ✓ |
| HST-6 | Merge precedence is deterministic (`incoming > store`) | execution.md §3 | — | — | ✓ | ✓ |
| HST-7 | Buffer lifecycle is replace-only, drain-once, commit-non-empty, no rollback | supervisor.md §2.3 | — | — | ✓ | ✓ |
| HST-8 | Canonical host loop enforces one `on_event` lifecycle per step cycle | supervisor.md §2.2 | — | — | ✓ | ✓ |
| HST-9 | Canonical host runner rejects duplicate `event_id` values across step lifecycle (including replay_step) | host boundary contract | — | — | ✓ | ✓ |
| DOC-GATE-1 | Canonical-complete claims blocked while doctrine ledger has open rows | CANONICAL process rule | — | — | ✓ | ✓ |
| RUN-CANON-1 | Canonical graph run requires explicit event source | host boundary contract | ✓ | — | ✓ | — |
| RUN-CANON-2 | Adapter binding is mandatory for all production execution paths and for adapter-dependent graphs | host boundary contract | — | — | ✓ | ✓ |
| SDK-CANON-1 | SDK canonical execution must delegate to core host path when SDK run/replay APIs exist | CANONICAL scope rule | — | — | — | ✓ |
| SDK-CANON-2 | Project/profile resolution must translate into host-owned canonical requests, not invent a second execution model | product boundary contract | — | — | — | ✓ |
| SDK-CANON-3 | SDK custom primitives register in-process through the shared runtime registration and validation path used by core primitives | product boundary contract | — | — | — | ✓ |

### Host Usecase API

The host (`crates/prod/core/host`) exposes the canonical execution surface:

| Function | Level | Responsibility |
|----------|-------|---------------|
| `run_graph_from_paths` | Client entrypoint | Canonical run: loader decode/discovery, expansion, provenance, adapter validation/binder, host-owned ingress selection via `DriverConfig` (current code term for ingress-channel config), runner setup, eager egress startup/finalization, truthful `Completed` / `Interrupted` outcome reporting |
| `replay_graph_from_paths` | Client entrypoint | Canonical replay: loader decode, strict preflight, rehydration, and host-owned effect-integrity comparison; replay remains capture-driven and accepts no live channel config |
| `validate_graph_from_paths` | Client entrypoint | Prep-only live-path validation: loader decode/discovery, expansion, dependency scan, adapter/egress/handler validation without constructing a hosted session, preflighting a driver, or starting egress channels |
| `validate_run_graph_from_paths` | Client entrypoint | Canonical full-run validation: everything in prep validation plus `DriverConfig` preflight for the explicit ingress shape that canonical run will use |
| `prepare_hosted_runner_from_paths` | Client entrypoint | Canonical manual-runner preparation: loader decode/discovery, expansion, adapter preflight/binder, hosted-runner construction, eager egress startup, returns `HostedRunner` for caller-driven stepping |
| `finalize_hosted_runner_capture` | Client entrypoint | Canonical manual-runner finalization: reject zero-step/non-finalizable runners, assert no pending acks, stop egress channels, return `CaptureBundle` |
| `load_graph_assets_from_paths` | Lower-level | Loader-backed path loading to sealed `PreparedGraphAssets` for the live prep lane |
| `load_graph_assets_from_memory` | Lower-level | Loader-backed in-memory loading to sealed `PreparedGraphAssets` for the live prep lane |
| `validate_graph` | Lower-level | Validate preloaded `PreparedGraphAssets` plus `LivePrepOptions` (including lower-level adapter transport) without starting egress or constructing a started session (`validate_graph_with_surfaces` is the injected-runtime variant) |
| `prepare_hosted_runner` | Lower-level | Prepare a `HostedRunner` from preloaded `PreparedGraphAssets` plus `LivePrepOptions`, then start egress (`prepare_hosted_runner_with_surfaces` is the injected-runtime variant) |
| `run_graph_from_assets` | Lower-level | Execute from preloaded `PreparedGraphAssets` plus `LivePrepOptions`, explicit `DriverConfig`, and explicit `CapturePolicy`; path-backed default naming does not apply (`run_graph_from_assets_with_surfaces` / `..._with_control` are the injected-runtime and bounded-run variants) |
| `run_graph` | Lower-level | Execute from an already prepared `HostedRunner` with path-shaped capture/output policy and host-owned ingress-channel config |
| `replay_graph` | Lower-level | Strict replay from pre-loaded bundle |
| `graph_to_dot_from_assets` | Lower-level | Render DOT from preloaded `PreparedGraphAssets` with source-label diagnostics instead of path discovery |
| `validate_manifest_text` / `validate_manifest_value` | Lower-level | Validate manifest content from labeled text/object inputs without requiring a manifest path |
| `check_compose_text` / `check_compose_values` | Lower-level | Validate adapter composition from labeled text/object inputs without requiring manifest paths |
| `run_fixture` | Utility | Direct execution from fixture events (built-in reference ingress path) |
| `scan_adapter_dependencies` | Lower-level | Detect adapter-dependent graphs from source/action manifests |
| `validate_adapter_composition` | Lower-level | Enforce COMP-* rules before execution |

Clients (CLI, SDK) call the **client entrypoint** APIs for canonical run, replay, validation, and manual stepping. They do not own loader composition, adapter binding, dependency scanning, or host finalization ordering. The lower-level APIs remain available for non-client host callers.

Notes:

- **HST-1 / HST-7:** canonical mode drains buffered effects from host runtime wrapper after `on_event`, then applies handler-owned kinds through host handlers and dispatches egress-owned kinds through configured egress channels. Sources: `crates/prod/core/host/src/runner.rs:576` (drain via `self.runtime.drain_pending_effects()`), `:746` (`dispatch_invoked_effects`); `crates/prod/core/host/src/host/buffering_invoker.rs:99` (drain via `std::mem::take`), `:132` (replace via `guard.pending_effects = effects`).
- **HST-2:** `set_context` effect handler validates declared key, writable, and type against the adapter manifest before applying. Source: `crates/prod/core/host/src/host/effects.rs` (`SetContextHandler::apply`).
- **HST-3:** Non-invoke decisions must produce zero pending effects; a drained buffer on any non-invoke decision is rejected as a lifecycle violation. Source: `crates/prod/core/host/src/runner.rs:599-603`.
- **HST-4:** Enforced by `BufferingRuntimeInvoker` replace semantics: each `run()` call replaces the pending effect buffer, so retried runs cannot accumulate effects from prior attempts. Source: `crates/prod/core/host/src/host/buffering_invoker.rs:132`.
- **HST-5:** Run setup rejects graphs where a graph-emittable accepted effect kind has neither a registered host handler nor an egress-claimed route. Source: `crates/prod/core/host/src/host/coverage.rs:50-78` (`ensure_handler_coverage`).
- **HST-6:** Context merge is a deterministic overlay — store-scoped keys are inserted first, then incoming payload keys, so incoming values win on collision. Source: `crates/prod/core/host/src/runner.rs:721-730`.
- **HST-8:** Each step runs exactly one `on_event` lifecycle; the runner asserts the decision log grows by exactly one entry per `on_event` call. Source: `crates/prod/core/host/src/runner.rs:556-566`.
- **HST-9:** Duplicate `event_id` rejection is enforced at `HostedRunner` before `on_event`, so non-CLI host callers cannot bypass identity guarantees. Host replay execution flows through the host replay path, which performs strict preflight, event rehydration with hash checks, and effect-integrity comparison around the `HostedRunner::replay_step(...)` primitive. Source: `crates/prod/core/host/src/runner.rs:542-545`.
- Canonical run ingress is host-owned. Clients translate flags/arguments into host request types; they do not own ingress-channel launch or replay semantics.
- Canonical run interruption is host-owned. `Interrupted(...)` is only truthful when host can finalize a trustworthy capture artifact; replay remains capture-driven and accepts no `DriverConfig`.
- Process-driver startup and termination grace windows are host operational policy. They bound how long host waits to observe protocol truth, but they do not change what counts as `Completed` versus `Interrupted`.
- Metadata-less runtime execution rejects intent-emitting graphs. Canonical host paths therefore use metadata-aware execution whenever Actions declare external intents.
- Host capture enrichment associates applied effects by decision order (`decisions[i]`), not by `event_id`, so duplicate fixture/event IDs cannot overwrite prior decision effects.
- HST-7 commit rule follows SUP-6 partial execution semantics: commit if drained buffer is non-empty regardless of final termination; no transactional rollback.
- DOC-GATE-1 enforcement script: `tools/verify_doctrine_gate.sh`; integrated via `tools/verify_runtime_surface.sh`.
- RUN-CANON-1: canonical run entrypoints require a non-optional `DriverConfig` (`RunGraphFromPathsRequest.driver`, `RunGraphRequest.driver`), so host canonical run always receives an explicit event source (`Fixture`, `FixtureItems`, or `Process`) instead of inventing an implicit direct-run path. Driver-specific validation then rejects empty/invalid driver configuration before canonical execution begins.
- RUN-CANON-2: Enforced through two independent gates plus structural constructor enforcement. Gate 1 (graph-dependency): `validate_live_runner_setup_from_assets(...)` scans adapter dependencies and `ensure_adapter_requirement_satisfied(...)` rejects adapter-dependent graphs (those with `required: true` context keys or action writes/intents) when no adapter is provided. Gate 2 (production closure): `ensure_production_adapter_bound(...)` rejects `SessionIntent::Production` sessions when no adapter is provided, regardless of graph dependency analysis. Gate 3 (structural): `LivePrepOptions::for_production(adapter, ...)` and `PrepareHostedRunnerFromPathsRequest::for_production(graph_path, ..., adapter_path, ...)` require a non-optional adapter argument, making adapterless production construction a compile-time error for external callers. `SessionIntent` is derived from `DriverConfig` for canonical run paths (`Process` → `Production`, `Fixture`/`FixtureItems` → `Fixture`) and structurally set by `for_production`/`for_fixture` constructors for manual-runner preparation paths. The `session_intent` field is `pub(crate)` on both `LivePrepOptions` and `PrepareHostedRunnerFromPathsRequest`, preventing external callers from overriding the derived intent. Both runtime gates report through the `RUN-CANON-2` rule id. Fixture and fixture-items drivers are exempt from Gate 2 as the known legacy exception. Replay paths are exempt from Gate 2 because replay is governed by capture provenance matching, not session intent.
- SDK-CANON-1: now exercised by `ergo-sdk-rust`. SDK `run_profile`, `replay_profile`, `validate_project`, and `runner_for_profile` delegate canonical orchestration to host entrypoint APIs, with SDK validation delegating to the run-validation seam (`validate_run_graph_from_paths...`) rather than inventing an SDK-only validation lane; `ProfileRunner::finish()` delegates finalization through `finalize_hosted_runner_capture(...)`; `ergo init` scaffolds against that real surface rather than a placeholder.
- SDK-CANON-2: SDK profile-facing APIs may resolve projects through either loader-owned filesystem discovery or SDK-owned in-memory project snapshots, but both must translate into host-owned requests instead of becoming an alternate orchestration authority. `run_profile(...)`, `replay_profile(...)`, `validate_project()`, `runner_for_profile(...)`, and `replay_profile_bundle(...)` therefore resolve through transport-neutral SDK planning and then delegate to canonical host run/replay/validation/manual-runner seams.
- SDK-CANON-3: `ErgoBuilder` forwards custom Sources, Computes, Triggers, and Actions into `CatalogBuilder`, and `CatalogBuilder::build()` registers the combined core + custom inventory through the same runtime registry/catalog path. The SDK therefore does not create an SDK-only primitive validation lane; invalid or duplicate custom primitives fail through the shared runtime registration surface.

---
