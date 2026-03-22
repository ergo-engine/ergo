## 7. Orchestration Phase

**Scope:** Supervisor scheduling of episodes.

**Source:** supervisor.md (frozen)

**Entry invariants:**

- Graph is validated (all V.* invariants hold)
- Adapter is available and compliant

### Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| CXT-1 | ExecutionContext is adapter-only | supervisor.md §3 | ✓ | — | — | ✓ |
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

- **CXT-1:** `pub(crate)` constructor; compile_fail doctests verify no external construction.
- **SUP-1:** Private `graph_id` field with no setters; set only at construction.
- **SUP-2:** `RuntimeInvoker::run()` returns `RunTermination` only; no `RunResult` exposure.
- **SUP-4:** `should_retry()` matches only `NetworkTimeout|AdapterUnavailable|RuntimeError|TimedOut`.
- **SUP-5:** `ErrKind` enum contains only mechanical variants; no domain-flavored errors.
- **SUP-7:** `DecisionLog` trait has only `fn log()`; `records()` is on concrete impl, not trait.
- **SUP-TICK-1:** Pump events have special deferred-retry behavior distinct from Command events; legacy `Tick` capture values deserialize to `Pump` via serde alias. Test: `replay_harness.rs` uses Command (not Pump) to avoid interference.
- **RTHANDLE-META-1:** `RuntimeHandle::run()` calls `execute_with_metadata(...)` and forwards `graph_id` / `event_id` into metadata-aware runtime execution. This is required for deterministic `intent_id` derivation when Actions declare external intents.
- **RTHANDLE-ID-1:** `FaultRuntimeHandle` still discards `graph_id` and keys injected outcomes on `EventId` only. The metadata-less runtime path rejects intent-emitting graphs, so the fault harness retains EventId-only determinism without becoming a live intent-ID source.
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
| RUN-CANON-2 | Adapter binding is mandatory only for adapter-dependent graphs | host boundary contract | — | — | ✓ | ✓ |
| SDK-CANON-1 | SDK canonical execution must delegate to core host path when SDK run/replay APIs exist | CANONICAL scope rule | — | — | — | ✓ |
| SDK-CANON-2 | Project/profile resolution must translate into host-owned canonical requests, not invent a second execution model | product boundary contract | — | — | — | ✓ |
| SDK-CANON-3 | SDK custom primitives register in-process through the shared runtime registration and validation path used by core primitives | product boundary contract | — | — | — | ✓ |

### Host Usecase API

The host (`crates/prod/core/host`) exposes the canonical execution surface:

| Function | Level | Responsibility |
|----------|-------|---------------|
| `run_graph_from_paths` | Client entrypoint | Canonical run: loader decode/discovery, expansion, provenance, adapter validation/binder, host-owned ingress selection via `DriverConfig` (current code term for ingress-channel config), runner setup, eager egress startup/finalization, truthful `Completed` / `Interrupted` outcome reporting |
| `replay_graph_from_paths` | Client entrypoint | Canonical replay: loader decode, strict preflight, rehydration, and host-owned effect-integrity comparison; replay remains capture-driven and accepts no live channel config |
| `validate_graph_from_paths` | Client entrypoint | Canonical live-path validation: loader decode/discovery, expansion, dependency scan, adapter/egress/handler validation without constructing a hosted session or starting egress channels |
| `prepare_hosted_runner_from_paths` | Client entrypoint | Canonical manual-runner preparation: loader decode/discovery, expansion, adapter preflight/binder, hosted-runner construction, eager egress startup, returns `HostedRunner` for caller-driven stepping |
| `finalize_hosted_runner_capture` | Client entrypoint | Canonical manual-runner finalization: reject zero-step/non-finalizable runners, assert no pending acks, stop egress channels, return `CaptureBundle` |
| `run_graph` | Lower-level | Execute from pre-loaded graph with adapter and host-owned ingress-channel config |
| `replay_graph` | Lower-level | Strict replay from pre-loaded bundle |
| `run_fixture` | Utility | Direct execution from fixture events (built-in reference ingress path) |
| `scan_adapter_dependencies` | Lower-level | Detect adapter-dependent graphs from source/action manifests |
| `validate_adapter_composition` | Lower-level | Enforce COMP-* rules before execution |

Clients (CLI, SDK) call the **client entrypoint** APIs for canonical run, replay, validation, and manual stepping. They do not own loader composition, adapter binding, dependency scanning, or host finalization ordering. The lower-level APIs remain available for non-client host callers.

Notes:

- HST-1/HST-7: canonical mode drains buffered effects from host runtime wrapper after `on_event`, then applies handler-owned kinds through host handlers and dispatches egress-owned kinds through configured egress channels.
- Canonical run ingress is host-owned. Clients translate flags/arguments into host request types; they do not own ingress-channel launch or replay semantics.
- Canonical run interruption is host-owned. `Interrupted(...)` is only truthful when host can finalize a trustworthy capture artifact; replay remains capture-driven and accepts no `DriverConfig`.
- Process-driver startup and termination grace windows are host operational policy. They bound how long host waits to observe protocol truth, but they do not change what counts as `Completed` versus `Interrupted`.
- Metadata-less runtime execution rejects intent-emitting graphs. Canonical host paths therefore use metadata-aware execution whenever Actions declare external intents.
- **HST-4:** Enforced by `BufferingRuntimeInvoker` replace semantics: each `run()` call replaces the pending effect buffer, so retried runs cannot accumulate effects from prior attempts.
- Host capture enrichment associates applied effects by decision order (`decisions[i]`), not by `event_id`, so duplicate fixture/event IDs cannot overwrite prior decision effects.
- HST-9: duplicate `event_id` rejection is enforced at `HostedRunner`, so non-CLI host callers cannot bypass identity guarantees. Host replay execution flows through the host replay path, which performs strict preflight, event rehydration with hash checks, and effect-integrity comparison around the `HostedRunner::replay_step(...)` primitive.
- HST-7 commit rule follows SUP-6 partial execution semantics: commit if drained buffer is non-empty regardless of final termination; no transactional rollback.
- DOC-GATE-1 enforcement script: `tools/verify_doctrine_gate.sh`; integrated via `tools/verify_runtime_surface.sh`.
- RUN-CANON-1: canonical run entrypoints require a non-optional `DriverConfig` (`RunGraphFromPathsRequest.driver`, `RunGraphRequest.driver`), so host canonical run always receives an explicit event source (`Fixture` or `Process`) instead of inventing an implicit direct-run path. Driver-specific validation then rejects empty/invalid driver configuration before canonical execution begins.
- RUN-CANON-2: `validate_live_runner_setup(...)` scans adapter dependencies before execution and `ensure_adapter_requirement_satisfied(...)` rejects adapter-dependent graphs when `adapter_path` is absent. The host error surface reports this as `RUN-CANON-2` with offending required-context/write nodes when available.
- SDK-CANON-1: now exercised by `ergo-sdk-rust`. SDK `run_profile`, `replay_profile`, `validate_project`, and `runner_for_profile` delegate canonical orchestration to host entrypoint APIs, and `ProfileRunner::finish()` delegates finalization through `finalize_hosted_runner_capture(...)`; `ergo init` scaffolds against that real surface rather than a placeholder.
- SDK-CANON-2: SDK profile-facing APIs resolve project profiles through loader-owned resolution, then translate resolved assets into host requests instead of becoming an alternate orchestration authority. `run_profile(...)` resolves and delegates to `run_graph_from_paths_with_surfaces_and_control(...)`; `replay_profile(...)` resolves and delegates to host replay; `validate_project()` resolves every profile and delegates to host validation; `runner_for_profile(...)` resolves a normal run profile and delegates manual-runner preparation to host.
- SDK-CANON-3: `ErgoBuilder` forwards custom Sources, Computes, Triggers, and Actions into `CatalogBuilder`, and `CatalogBuilder::build()` registers the combined core + custom inventory through the same runtime registry/catalog path. The SDK therefore does not create an SDK-only primitive validation lane; invalid or duplicate custom primitives fail through the shared runtime registration surface.

---
