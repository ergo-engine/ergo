# Prod Code Map

---
Scope: Prod-layer code map (loader + host + sdk-rust + cli + shared)
Stops at: kernel boundary (`RuntimeInvoker`, `DecisionLog`, `CaptureBundle`
serde) on the inside; the SDK `Ergo` / `ProfileRunner` surface on the
outside.
Authority: informative (per `AGENTS.md` §1, canonical authority lives in
`docs/`).
Citations: path-and-symbol (e.g.,
`crates/prod/core/host/src/runner.rs::HostedRunner`).
Sibling: `crates/kernel/CODE_MAP.md` for the kernel half.
---

## What this doc is not

- Not a doctrine doc. Doctrine lives under `docs/system/` and
  `docs/invariants/`; this is a structural reference.
- Not a per-symbol API reference. Rustdoc generates that.
- Not authoritative. When this doc and a higher-authority doc disagree,
  the higher-authority doc wins (`AGENTS.md` §1).
- Not a public stability contract. Public stability is governed by
  `docs/api/` and the SDK crate docs, not by this map.
- Not a replacement for the kernel code map. The two are read together:
  the kernel map ends at `RuntimeInvoker` / `DecisionLog` /
  `CaptureBundle`; this map starts there and walks outward.

---

## 1. The six prod crates

The prod tree has two layers (`core/`, `clients/`) plus a small `shared/`
helper crate. Every prod crate ultimately re-exports or composes the
three kernel crates from `crates/kernel/` and never redefines kernel
meaning.

| Crate                   | Path                              | Owns                                                                                                                                                            | Does not own                                                                  | Depends on                                                                                                |
| ----------------------- | --------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| **`ergo-loader`**       | `crates/prod/core/loader`         | Filesystem and in-memory graph/cluster discovery, graph text decode, project (`ergo.toml`) load, sealed `PreparedGraphAssets` handoff, `LoaderError` taxonomy.  | Kernel semantic validation; host orchestration; runtime execution.            | `ergo-runtime` (only for `cluster::Version`).                                                             |
| **`ergo-host`**         | `crates/prod/core/host`           | The full host orchestration loop: usecases facade, `HostedRunner`, `BufferingRuntimeInvoker`, effect handlers, handler coverage, egress, capture enrichment.    | Kernel runtime/adapter/supervisor semantics; loader transport; CLI/SDK shape. | `ergo-adapter`, `ergo-runtime`, `ergo-supervisor`, `ergo-loader`, `ergo-prod-duration`.                   |
| **`ergo-sdk`**     | `crates/prod/clients/sdk-rust`    | The SDK `Ergo` engine, `ErgoBuilder`, `ProfileRunner`, `StopHandle`, SDK-branded `Ergo*Error` taxonomy, in-memory project plumbing.                             | Host orchestration internals; loader internals; kernel semantics.             | `ergo-adapter`, `ergo-host`, `ergo-loader`, `ergo-runtime`.                                               |
| **`ergo-cli`**          | `crates/prod/clients/cli`         | The `ergo` binary entry, argument parsing, CLI subcommand dispatch, exit codes, text/JSON output rendering.                                                     | Host orchestration; SDK engine; kernel semantics.                             | `ergo-host`, plus `ergo-adapter`/`ergo-runtime`/`ergo-supervisor` for replay/test paths; `ergo-fixtures`. |
| **`ergo-sdk-types`**    | `crates/prod/clients/sdk-types`   | Lightweight cross-binding serde types (currently `SdkVersion` only).                                                                                            | Anything else.                                                                | `serde` only.                                                                                             |
| **`ergo-prod-duration`** | `crates/prod/shared/duration`     | The shared `ms\|s\|m\|h` duration-literal parser used by host egress config and loader profile literals.                                                        | Serde wrappers; runtime timing policy.                                        | `std` only.                                                                                               |

Every prod-side crate carries a `//! crate_name` header in
`src/lib.rs` (or `src/main.rs`) with the Purpose / Owns / Does not own /
Connects to / Safety-notes shape from `AGENTS.md` §4A. The kernel side
matches: see `crates/kernel/runtime/src/lib.rs`,
`crates/kernel/adapter/src/lib.rs`, and
`crates/kernel/supervisor/src/lib.rs`.

### Dependency graph (prod-internal)

```
        ┌───────────────┐
        │   ergo-cli    │  (binary)
        └───────┬───────┘
                │
        ┌───────▼───────┐
        │ ergo-sdk │  (library facade for embedded callers)
        └───────┬───────┘
                │  re-exports a curated subset of …
        ┌───────▼───────┐       ┌───────────────┐
        │   ergo-host   │──────▶│  ergo-loader  │
        └───────┬───────┘       └───────┬───────┘
                │                       │
                └───────────┬───────────┘
                            │
                     (kernel boundary)
                            │
       ergo-adapter ─ ergo-runtime ─ ergo-supervisor
```

`ergo-sdk-types` and `ergo-prod-duration` sit outside this main chain;
they are leaf helpers consumed by `sdk-rust` / `host` / `loader`.

---


## 2. The nine-layer call stack

The forward path from a caller's `step(...)` call down to a primitive's
`compute(...)` traverses nine named layers. Each layer adds exactly one
concern; none of them re-implements a concern owned by a layer above or
below.

```
1. Caller             — CLI / Ergo / ProfileRunner / direct embedder
2. usecases facade    — request DTO → orchestration call
3. HostedRunner       — host-side step engine, effect routing, finalize gate
4. CapturingSession   — appends ExternalEventRecord, forwards to Supervisor
5. Supervisor         — episode bookkeeping, retry policy, decision logging
6. BufferingRuntimeInvoker — host-side RuntimeInvoker, captures effects
7. ReportingRuntimeHandle  — kernel adapter handle, drives execute_once
8. execute_once       — adapter-internal validate + dispatch wrapper
9. runtime execute_with_metadata — kernel synchronous executor
```

### Forward path by layer (each row is one function boundary)

| Layer | Symbol                                                                                                            | Owns at this layer                                                                                                                       |
| ----- | ----------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | `crates/prod/clients/sdk-rust/src/lib.rs::Ergo` and `ProfileRunner::step`                                         | Public, SDK-branded errors; caller-thread lifecycle (`Ergo` is `!Send`/`!Sync` in v1).                                                  |
| 2     | `crates/prod/core/host/src/usecases/live_run.rs::run_graph_from_paths_with_surfaces_and_control` (and siblings)    | Loader handoff, `LivePrepOptions`, `RunControl` (stop handle + bounded-run), translation between SDK errors and host errors.            |
| 3     | `crates/prod/core/host/src/runner.rs::HostedRunner::step` → `execute_step`                                         | Per-step legality (`ensure_step_allowed`), event-id dedup, pre/post decision-log counts, effect routing, capture finalization state.    |
| 4     | `crates/kernel/supervisor/src/capture.rs::CapturingSession::on_event`                                              | Appending `ExternalEventRecord` into the bundle before delegating to the supervisor.                                                    |
| 5     | `crates/kernel/supervisor/src/lib.rs::Supervisor::on_event`                                                        | Episode/retry policy, decision-log entry construction, calling `RuntimeInvoker::run` once per attempt.                                  |
| 6     | `crates/prod/core/host/src/host/buffering_invoker.rs::BufferingRuntimeInvoker::run` (`impl RuntimeInvoker for …`)  | Replace-and-drain pending-effect buffer; bumps `run_call_count`; returns only the `RunTermination` to the supervisor.                   |
| 7     | `crates/kernel/adapter/src/lib.rs::ReportingRuntimeHandle::run_reporting`                                          | Holds `Arc<ExpandedGraph>`, `Arc<CorePrimitiveCatalog>`, `Arc<CoreRegistries>`, `AdapterProvides`; calls `execute_once`.                |
| 8     | `crates/kernel/adapter/src/lib.rs::execute_once`                                                                   | Deadline shortcut; `runtime_validate`; `state.validate_composition`; final `execute_with_metadata` call; `ExecError` → `RunTermination`. |
| 9     | `crates/kernel/runtime/src/runtime/execute.rs::execute_with_metadata`                                              | Kernel-owned synchronous execution and effect production; the kernel CODE_MAP §3 entry-point.                                            |

Layers 1–6 are prod-owned. Layers 7–9 are kernel-owned: layer 7 lives in
the kernel adapter crate because the runtime ownership chain (the `Arc`s
above) is kernel authority; the host's job at layer 6 is to interpose
between the supervisor and that authority without redefining it.

### Reverse path (effects)

Effects flow back through the same boundary in reverse, but with a
narrower interface. The supervisor never sees effects:
`BufferingRuntimeInvoker::run` *captures* them via
`engine.run_reporting(…, &mut effects)` and stores them on its private
`BufferState`. Only after `Supervisor::on_event` returns does
`HostedRunner::execute_step` call `self.runtime.drain_pending_effects()`
and then route each effect through the host-owned effect path:

```
ActionEffect ──▶ HostedRunner::dispatch_invoked_effects
              ├─▶ EffectHandler::apply  (host-internal kinds; e.g. SetContextHandler)
              ├─▶ EgressRuntime::dispatch (graph-emittable kinds routed to egress)
              └─▶ capture_enrichment::AppliedEffectsByDecision (sidecar)
```

The reverse path's owners are intentionally split: the kernel produces
effects, the host decides who applies them, and `host::coverage`
guarantees ahead of time that every accepted graph-emittable kind has
exactly one owner (§6).

### Why nine layers and not fewer

Three of the layers exist to keep ownership clean rather than to add
behavior:

- **Layer 2 (usecases)** isolates request-shape and stop-control concerns
  so layers 3+ do not have to know how a CLI argv differs from an SDK
  builder argument.
- **Layer 6 (`BufferingRuntimeInvoker`)** exists because the
  supervisor's `RuntimeInvoker` contract is termination-only — the
  supervisor must not see effects (§9, structural). The buffer is the
  host's way of carrying effects past the supervisor without widening
  the kernel trait.
- **Layer 8 (`execute_once`)** exists because the adapter wraps the
  runtime's validate-then-execute pair behind a single termination-only
  surface; layer 7 (`ReportingRuntimeHandle`) wants to expose that as
  one call.

Collapsing any of these three would leak ownership: layer 2 would push
caller-shape into orchestration, layer 6 would force `RuntimeInvoker` to
carry effects, and layer 8 would force `ReportingRuntimeHandle` to know
about `ExecError` variants. The map keeps them distinct.

---


## 3. Surface tiers: canonical vs lower-level vs internal

The host crate publishes three concentric public surfaces. The
distinction is structural, not cosmetic: each tier is governed by a
different stability and audit contract, and CLI/SDK consumers should
prefer the outermost canonical tier.

### Tier 1 — Canonical client-facing host seams

These are the seven canonical run/replay/validation entrypoints CLI and
SDK should route through. They live behind the comment block

> *"Canonical client-facing host seams. CLI and SDK should route product-level
> run, replay, validation, and manual-step orchestration through these
> exports."*

in `crates/prod/core/host/src/lib.rs`. They are:

| Symbol                                                                                    | Purpose                                          |
| ----------------------------------------------------------------------------------------- | ------------------------------------------------ |
| `usecases::run_graph_from_paths` / `..._with_control` / `..._with_surfaces[_and_control]` | Live run from filesystem paths.                  |
| `usecases::replay_graph_from_paths[_with_surfaces]`                                       | Replay from filesystem paths + capture bundle.   |
| `usecases::validate_graph_from_paths[_with_surfaces]`                                     | Validation of graph + clusters.                  |
| `usecases::validate_run_graph_from_paths[_with_surfaces]`                                 | Run-shaped validation (adapter, egress, deps).   |
| `usecases::prepare_hosted_runner_from_paths[_with_surfaces]`                              | Manual-step runner preparation from paths.       |
| `usecases::finalize_hosted_runner_capture`                                                | Capture finalization for the manual-step runner. |

### Tier 2 — Lower-level host building blocks

These remain public for advanced embedded callers and tests, but they
are explicitly *not* the canonical orchestration surface. They live
behind the comment

> *"Lower-level host building blocks. These remain public for advanced
> embedded callers and tests, but they are not the canonical
> orchestration surface that CLI and SDK should compose themselves."*

and include `run_graph`, `run_graph_with_control`,
`run_graph_from_assets`, `replay_graph`, `replay_graph_from_assets`,
`run_fixture`, `validate_graph`, `prepare_hosted_runner`,
`scan_adapter_dependencies`, `validate_adapter_composition`,
`load_graph_assets_from_paths`, `load_graph_assets_from_memory`. The
loader's `PreparedGraphAssets` and `InMemorySourceInput` are re-exported
here as part of the same tier.

### Tier 3 — `_with_surfaces` and SDK primitive registration

The `_with_surfaces` and `_with_surfaces_and_control` variants are the
same orchestration code with an explicit `RuntimeSurfaces` parameter
threaded through `live_prep` and `live_run`. `RuntimeSurfaces` is
defined at
`crates/prod/core/host/src/usecases.rs::RuntimeSurfaces`:

````rust
#[derive(Clone)]
pub struct RuntimeSurfaces {
    registries: Arc<CoreRegistries>,
    catalog: Arc<CorePrimitiveCatalog>,
}
````

This tier exists for one structural reason: the SDK `Ergo` engine
registers caller-supplied custom primitives into a `CatalogBuilder`,
freezes it to `CoreRegistries` + `CorePrimitiveCatalog`, wraps them in
`RuntimeSurfaces`, and passes that surface into every host entrypoint
so the runtime's catalog reflects the SDK's registration. See
`crates/prod/clients/sdk-rust/src/lib.rs::Ergo::run_with_control` for
the call site:

````rust
run_graph_from_paths_with_surfaces_and_control(
    request,
    self.runtime_surfaces.clone(),
    run_control_from_config(&config, control),
)
````

The non-`_with_surfaces` siblings construct a default `RuntimeSurfaces`
internally via `core_registries` / `build_core_catalog`, which is the
right default for CLI and tests but cannot register custom primitives.

### Tier 4 — Crate-private internals

`mod usecases/{live_prep, live_run, node_analysis, process_driver,
shared}`, `mod host/{buffering_invoker, context_store, coverage,
effects}`, `mod runner`, `mod replay`, `mod egress/*`, `mod
capture_enrichment`, etc. These are crate-private implementation
modules. The host's `lib.rs` curates exactly what crosses the crate
boundary; nothing else does.

### What this tiering enforces

- Tier 1 is the only surface the SDK ever consumes.
- Tier 2 is the surface that fixture tooling and direct embedded callers
  may use, but its stability bar is weaker than Tier 1.
- Tier 3's `_with_surfaces` postfix is load-bearing: it doubles as the
  SDK's primitive-registration path. Renaming or eliminating it would
  break the SDK builder pattern.
- Tier 4 has no stability contract and may move freely; the kernel
  CODE_MAP §10 (host boundary) only references Tier 1/2/3 symbols.

---

## 4. Sealed vs open type discipline

The prod layer carries two structurally different kinds of data across
boundaries, and the type system enforces the difference.

### Sealed: handoff carriers that protect invariants

`PreparedGraphAssets` is the canonical example. Defined at
`crates/prod/core/loader/src/io.rs::PreparedGraphAssets`:

````rust
// `PreparedGraphAssets` stays sealed because host prep depends on loader-owned
// invariants rather than caller-constructed reporting data.
#[derive(Debug, Clone)]
pub struct PreparedGraphAssets {
    root: DecodedAuthoringGraph,
    clusters: HashMap<(String, Version), DecodedAuthoringGraph>,
    cluster_diagnostic_labels: HashMap<(String, Version), String>,
    pub(crate) _sealed: (),
}
````

Three structural facts make this sealed:

1. All fields except the `_sealed` marker are private. Callers cannot
   construct `PreparedGraphAssets` outside the loader crate.
2. Accessors return `&` references, never `&mut`: `root()`, `clusters()`,
   `cluster_diagnostic_labels()`. No mutator exists.
3. The `pub(crate) _sealed: ()` marker means struct-literal construction
   from outside the crate is a hard compile error even if the other
   fields were made public later.

`HostedRunner::new_validated` is `pub(crate)` for the same reason: the
preferred path goes through `HostedRunner::new` (warning emission) or
through `prepare_hosted_runner_internal` (full setup pipeline), which
both validate before constructing.

### Open: reporting and request DTOs

The sibling types in the same file are deliberately open:

````rust
// Bundle DTOs stay open because they report discovered sources back to callers
// rather than protecting loader-owned invariants.
#[derive(Debug, Clone)]
pub struct FilesystemGraphBundle {
    pub root: DecodedAuthoringGraph,
    pub discovered_files: Vec<PathBuf>,
    pub source_map: BTreeMap<PathBuf, String>,
}
````

`FilesystemGraphBundle`, `InMemoryGraphBundle`, `HostedEvent`,
`HostedStepOutcome`, `RunSummary`, `InterruptedRun`,
`AdapterDependencySummary`, `AppliedWrite`, the various
`Run*Request`/`ReplayGraph*Request` DTOs — all of these have public
fields and exist to *report* or *carry* caller-constructed data, not to
protect an invariant. They are intentionally not sealed.

### How to tell them apart

| Property                  | Sealed (e.g. `PreparedGraphAssets`)       | Open (e.g. `RunSummary`)             |
| ------------------------- | ----------------------------------------- | ------------------------------------ |
| Fields                    | Private + `pub(crate) _sealed: ()` marker | Public                                |
| Caller-constructable      | No (only the owning crate)                | Yes                                   |
| Mutation API              | None (no `&mut` accessors)                | Free assignment                       |
| Purpose                   | Carry a validated invariant               | Carry caller-visible data             |
| Visibility of constructor | `pub(crate)` or private                   | `pub` literal or trivial constructor  |

The discipline is mechanical: anything that crossed a validation gate
(loader discovery, host prep, supervisor freezing) is sealed; anything
purely caller-facing or report-shaped is open. The `Default` impl on
`SessionIntent` (`Production`) is the conservative bias for the open
shape; the `pub(crate)` on `session_intent` is the conservative bias
for the sealed-by-API surface (§5).

---

## 5. The `for_production` dual gate

Two independent gates determine whether a session is allowed to run as
production. They are evaluated at different times, and either one can
reject. This is the most important enforcement seam in the host
because it decides whether external data may enter the graph without
adapter governance.

### Gate A — Structural (visibility-enforced at compile time)

`SessionIntent` is at
`crates/prod/core/host/src/usecases.rs::SessionIntent`:

````rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionIntent {
    Production,
    Fixture,
}

impl Default for SessionIntent {
    fn default() -> Self {
        Self::Production
    }
}
````

The field that carries it on caller-facing requests is `pub(crate)`,
not `pub`:

````rust
pub struct PrepareHostedRunnerFromPathsRequest {
    pub graph_path: PathBuf,
    pub cluster_paths: Vec<PathBuf>,
    pub adapter_path: Option<PathBuf>,
    pub egress_config: Option<EgressConfig>,
    /// Derived from the caller's ingress configuration.  `pub(crate)` to
    /// prevent external callers from bypassing the production adapter gate
    /// by manually setting `Fixture` intent on a production-bound session.
    /// External callers must use [`Self::for_production`] or
    /// [`Self::for_fixture`].
    pub(crate) session_intent: SessionIntent,
}
````

External callers cannot construct these requests with struct-literal
syntax (the `pub(crate)` field blocks it). They must use:

- `for_production(graph_path, cluster_paths, adapter_path, egress_config)`
  — *non-optional* `PathBuf` adapter; production intent.
- `for_fixture(graph_path, cluster_paths, adapter_path_opt, egress_config)`
  — optional adapter; fixture intent.

The same shape repeats on `LivePrepOptions`. Together these constructors
make it structurally impossible to build a production request without
an adapter — the type system rejects it.

### Gate B — Validated (runtime check)

The compile-time gate is necessary but not sufficient: callers can still
reach `HostedRunner::new(adapter: None, ...)` through Tier 2's direct
APIs, or thread a fixture-driver `DriverConfig` into a path that should
have been production. Gate B catches those cases at runtime.

`ensure_production_adapter_bound` lives at
`crates/prod/core/host/src/usecases/live_prep.rs::ensure_production_adapter_bound`:

````rust
/// Production closure gate: rejects sessions with `SessionIntent::Production`
/// when no adapter contract is bound.
pub(super) fn ensure_production_adapter_bound(
    adapter_bound: bool,
    session_intent: SessionIntent,
) -> Result<(), HostRunError> {
    if !adapter_bound && session_intent == SessionIntent::Production {
        return Err(HostRunError::ProductionRequiresAdapter);
    }
    Ok(())
}
````

It is called from every public live-run or prep entrypoint in
`live_run.rs` and `live_prep.rs` (eight call sites today, all in those
two files). The session intent it sees is derived through
`session_intent_from_driver(&driver)`:

````rust
pub(super) fn session_intent_from_driver(driver: &DriverConfig) -> SessionIntent {
    match driver {
        DriverConfig::Process { .. } => SessionIntent::Production,
        DriverConfig::Fixture { .. } | DriverConfig::FixtureItems { .. } => SessionIntent::Fixture,
    }
}
````

### Why both gates exist

Gate A alone is not enough: there are public-but-Tier-2 entrypoints
(`prepare_hosted_runner`, `run_graph`) that accept `LivePrepOptions`
directly and could be reached with a manually-constructed
`LivePrepOptions::default()` whose default intent is `Production` —
exactly the safe default. Gate B is what makes that safe default
*enforceable*: it rejects the run rather than silently letting an
adapter-free session through.

Gate B alone is not enough either: without the `pub(crate)` field,
external callers could set `SessionIntent::Fixture` on a production
session and bypass Gate B by lying about intent. The `pub(crate)`
prevents that lie at compile time.

This is the dual-gate pattern: a structural narrowing of legal
constructions (Gate A) combined with a runtime check of the remaining
ambiguity (Gate B). Both gates must remain or the safety property is
lost.

### `ensure_adapter_requirement_satisfied` vs `ensure_production_adapter_bound`

Two adapter gates exist; they check different things. The comment on
`ensure_production_adapter_bound` is explicit:

> *"This gate is independent of the graph-dependency gate
> (`ensure_adapter_requirement_satisfied`). The graph-dependency gate
> checks whether the graph structurally needs an adapter (e.g. `required:
> true` context keys, action writes/intents). This gate checks whether
> the *execution path* demands a contract regardless of graph structure."*

The graph-dependency gate is the kernel-aligned check; the
session-intent gate is the prod-aligned check. They are deliberately
not collapsed.

---


## 6. Handler coverage and the `EffectHandler` boundary

The host owns the contract that decides who applies a graph-emittable
effect. This is a two-part contract: a trait that defines what handlers
look like (`EffectHandler`), and a coverage check that verifies exactly
one owner exists for each accepted kind (`ensure_handler_coverage`).

### `EffectHandler` is the only place in prod with `Send + Sync`

At `crates/prod/core/host/src/host/effects.rs::EffectHandler`:

````rust
pub trait EffectHandler: Send + Sync {
    fn kind(&self) -> &str;

    fn apply(
        &self,
        effect: &ActionEffect,
        store: &mut ContextStore,
        provides: &AdapterProvides,
    ) -> Result<Vec<AppliedWrite>, EffectApplyError>;
}
````

The bound is correct here even though `RuntimeHandle` /
`ReportingRuntimeHandle` are not `Send + Sync` (see kernel CODE_MAP §8).
The reason is structural: handlers are stored as
`BTreeMap<String, Arc<dyn EffectHandler>>` on `HostedRunner`. The `Arc`
crosses no thread boundary today, but the trait bound exists so the
handler registry remains structurally portable if the host is ever
threaded — and so SDK consumers can store their custom handlers in
caller-side `Arc`s without the compiler complaining.

This is one of the two `Send + Sync` decisions made on purpose at the
prod boundary:

1. `EffectHandler: Send + Sync` — host-owned, structural.
2. `StopHandle: Send + Sync` — SDK-owned, asserted in a const-eval
   block at `crates/prod/clients/sdk-rust/src/lib.rs` (`fn
   assert_send_sync<T: Send + Sync>(); … assert_send_sync::<StopHandle>();`).
   This is load-bearing because the SDK threading model lets a
   supervising thread call `StopHandle::stop` while `Ergo::run_with_stop`
   is blocked on the calling thread.

Everything else in prod inherits non-`Send`/non-`Sync` from the kernel
side and is documented as such on the `Ergo` engine.

### `SetContextHandler` is the only built-in handler

At `crates/prod/core/host/src/host/effects.rs::SetContextHandler`:

````rust
#[derive(Debug, Default)]
pub struct SetContextHandler;

impl EffectHandler for SetContextHandler {
    fn kind(&self) -> &str { "set_context" }

    fn apply(&self, effect: &ActionEffect, store: &mut ContextStore,
             provides: &AdapterProvides)
        -> Result<Vec<AppliedWrite>, EffectApplyError> { … }
    }
````

Its enforcement responsibilities are explicit:

1. **Declared-key check** — the effect's write key must be present in
   `provides.context` (else `UndeclaredKey`).
2. **Writable check** — the matching `ContextSpec` must permit writes
   (else `NonWritableKey`).
3. **Type check** — value type must match the declared spec (else
   `TypeMismatch`).
4. **Conversion check** — JSON ↔ runtime value conversion must succeed
   (else `InvalidValueConversion`).
5. **Kind dispatch** — refuses any kind other than its own (else
   `UnhandledEffectKind`).

The header on `effects.rs` notes one explicit non-property: *"Partial
writes are not rolled back when a later write fails."* That is an
intentional choice — the supervisor decides retry policy, not the
handler.

### `ensure_handler_coverage` is the structural gate

At `crates/prod/core/host/src/host/coverage.rs::ensure_handler_coverage`:

````rust
pub fn ensure_handler_coverage(
    provides: &AdapterProvides,
    graph_emittable_effect_kinds: &HashSet<String>,
    registered_handler_kinds: &BTreeSet<String>,
    egress_claimed_kinds: &HashSet<String>,
) -> Result<(), HandlerCoverageError> { … }
````

For each effect kind the graph can emit and the adapter accepts, the
check ensures exactly one owner exists:

| Handler | Egress | Outcome                                      |
| :-----: | :----: | -------------------------------------------- |
| ✓       | ✗      | OK (handler-owned)                           |
| ✗       | ✓      | OK (egress-owned)                            |
| ✓       | ✓      | `HandlerCoverageError::ConflictingCoverage`  |
| ✗       | ✗      | `HandlerCoverageError::MissingHandler`       |

`HandlerCoverageError` lives at the same path and has only those two
variants. The check is called from `runner.rs` and
`egress/validation.rs`; both call sites treat it as the HST-5 ownership
gate. Without this check, an effect could be silently dropped (no owner)
or applied twice (two owners) — both of which would corrupt the capture
record. The check turns either ambiguity into a typed error.

### Why this lives in prod and not kernel

The kernel produces effects (`ActionEffect`) but has no opinion about
*who applies them*. Application is a host responsibility because it
depends on out-of-graph state (`ContextStore`, egress channels,
external processes). The kernel boundary is intentionally narrow: it
hands back effects through `ReportingRuntimeHandle::run_reporting(…,
effects_out)` and stops there. Everything in §6 is the prod-side
counterpart that names, dispatches, and verifies who owns those
effects.

---

## 7. Capture finalization as a state machine

`HostedRunner` carries a private `CaptureFinalizationState` field
(defined at
`crates/prod/core/host/src/runner.rs::CaptureFinalizationState`).
The state has four reachable values, and they govern whether the runner
can step or finalize:

| State              | `step()` allowed? | `into_capture_bundle()` allowed? |
| ------------------ | :---------------: | :-------------------------------: |
| `NoCommittedSteps` | ✓                 | ✗ (lifecycle violation)           |
| `Eligible`         | ✓                 | ✓                                 |
| `FinalizeOnly`     | ✗                 | ✓                                 |
| `Fatal`            | ✗                 | ✗                                 |

The transitions are driven by:

- successful `step` → `NoCommittedSteps`/`Eligible` advance via
  `CaptureFinalizationState::on_step_success`;
- recoverable dispatch failure → `FinalizeOnly` (the run still has
  a truthful capture, but no more events should be added);
- non-recoverable step failure → `Fatal`.

The finalization pipeline is spread across three files and the runner
header is the single source of truth for that split:

> *"`CaptureFinalizationState` is load-bearing: `FinalizeOnly` permits
> capture finalization but blocks further stepping, while `Fatal` blocks
> both. This is the runner-owned gate in the capture finalization
> pipeline:
> runner.rs: `CaptureFinalizationState` (gate),
> `ensure_capture_finalizable()`, `into_capture_bundle()` (extraction);
> live_prep.rs: `HostedRunnerFinalizeFailure` (staged error),
> `finalize_hosted_runner_capture_with_stage()` (3-step orchestration);
> live_run.rs: `FinalizedRunCapture` (summary DTO),
> `finalize_run_capture()` (driver-level validation)."*

The SDK mirrors this in `ProfileRunner`'s own state enum (`Active`,
`FinalizableAfterDispatchFailure`, `Failed`, `Finished`). The two state
machines are not the same — the SDK adds `Finished` (drop semantics) and
omits `NoCommittedSteps` (folded into the lifecycle-violation message in
`finish()`) — but the host gate is what enforces truth. The SDK state is
a UX layer over it.

---


## 8. Vocabulary collisions

Several terms get reused at different layers. The kernel CODE_MAP §6
already disambiguates `Catalog`, `Registries`, `ExecutionContext`,
`Handle`, and `Primitive`; this section adds the prod-side collisions.

### `Runner`

| Symbol                                                              | Layer | Meaning                                                                                                            |
| ------------------------------------------------------------------- | :---: | ------------------------------------------------------------------------------------------------------------------ |
| `HostedRunner` (`runner.rs`)                                        | host  | The host's per-step engine. Owns `CapturingSession`, `BufferingRuntimeInvoker`, handlers, capture finalization.    |
| `ProfileRunner` (`sdk-rust/src/lib.rs`)                             | SDK   | Caller-facing wrapper around `HostedRunner` plus a stricter state machine and SDK-branded errors.                  |
| `BufferingRuntimeInvoker` (`host/buffering_invoker.rs`)             | host  | "Run-er" only in the kernel `RuntimeInvoker::run` sense. Not a step runner.                                        |

The first two are step runners; the third only shares `run` as a method
name. Treat `HostedRunner` and `ProfileRunner` as paired (caller-facing
vs SDK-facing); never confuse either with `BufferingRuntimeInvoker`.

### `Session`

| Symbol                                                       | Layer    | Meaning                                                                                  |
| ------------------------------------------------------------ | :------: | ---------------------------------------------------------------------------------------- |
| `CapturingSession` (`kernel/supervisor/src/capture.rs`)      | kernel   | The supervisor + capture wrapper. Held *inside* `HostedRunner`; lifecycle = one episode. |
| `SessionIntent` (`prod/host/src/usecases.rs`)                | host     | Production-vs-Fixture intent (§5). Not a session; just intent metadata.                  |

Despite the shared word, these are unrelated types. `SessionIntent` does
not parameterize `CapturingSession`; the kernel-side `CapturingSession`
neither knows nor cares about `SessionIntent`.

### `Handler`

| Symbol                                                       | Owner | Meaning                                                                                |
| ------------------------------------------------------------ | :---: | -------------------------------------------------------------------------------------- |
| `EffectHandler` trait (`host/effects.rs`)                    | host  | Handler for effect application. Has the `Send + Sync` bound that `RuntimeHandle` lacks. |
| `RuntimeHandle` / `ReportingRuntimeHandle` (kernel adapter)  | kernel| "Handle" to the runtime, not "Handler" of anything.                                    |
| `HostStopHandle` / `StopHandle`                              | host/SDK | Stop signal carrier. `Send + Sync` on the SDK side (asserted at compile time).        |

`EffectHandler` is the only one with the `-er` suffix; the others are
all `Handle` (singular). Watch for this in PR titles.

### `Invoker`

Used in exactly one place: `RuntimeInvoker` (kernel adapter trait). The
host's `BufferingRuntimeInvoker` is an implementation of that trait, not
a second concept. Do not invent a second `*Invoker` type for any other
purpose.

### `Provenance`

Three different provenance strings travel through the host:

| String                  | Carried by                                              | Source                                                          |
| ----------------------- | ------------------------------------------------------- | --------------------------------------------------------------- |
| `runtime_provenance`    | `HostedRunner::new(_validated)` and `CapturingSession`  | `compute_runtime_provenance` from `ergo_runtime` (kernel).      |
| `adapter_provenance`    | `HostedRunner::new(_validated)` and `CapturingSession`  | `HostedAdapterConfig::adapter_provenance`, or `NO_ADAPTER_PROVENANCE`. |
| `egress_provenance`     | `HostedRunner` field                                    | Caller-supplied; required when an `EgressConfig` is present.    |

The capture bundle stores all three; they are not interchangeable.
`NO_ADAPTER_PROVENANCE` is the kernel-owned sentinel used only on the
fixture path.

---

## 9. Enforcement axis: structural / validated / convention

The kernel CODE_MAP §7 lists 12 enforcement rows. The prod side adds
these:

### Structural (type-system enforcement; refactor-safe)

- **`PreparedGraphAssets` is unforgeable outside the loader.** Private
  fields + `pub(crate) _sealed: ()` marker. Callers can only obtain one
  by calling `load_graph_assets_*` (`crates/prod/core/loader/src/io.rs`).
- **`session_intent` cannot be set by external callers.** The
  `pub(crate)` field on `PrepareHostedRunnerFromPathsRequest` and
  `LivePrepOptions` forces use of `for_production` / `for_fixture`
  (`crates/prod/core/host/src/usecases.rs`).
- **`for_production` requires an adapter `PathBuf`.** The signature
  takes `adapter_path: PathBuf`, not `Option<PathBuf>`. There is no way
  to construct a production request without an adapter.
- **`RuntimeInvoker::run` does not carry effects.** The supervisor
  cannot observe effects because the trait signature does not include
  them; `BufferingRuntimeInvoker` is the seam that captures them
  out-of-band (`crates/prod/core/host/src/host/buffering_invoker.rs`).
- **`EffectHandler: Send + Sync`.** Compile-time bound on the trait
  (`crates/prod/core/host/src/host/effects.rs`).
- **`StopHandle: Send + Sync` asserted at compile time.** The const
  `fn assert_send_sync<T: Send + Sync>(); … assert_send_sync::<StopHandle>();`
  block in the SDK is checked at every build
  (`crates/prod/clients/sdk-rust/src/lib.rs`).
- **`HostedRunner::new_validated` is `pub(crate)`.** Outside callers
  must go through `new` (warning emission) or `prepare_hosted_runner_internal`
  (full pipeline).
- **`RuntimeSurfaces::into_shared_parts` is `pub(crate)`.** SDK can
  hand the surface in via `_with_surfaces`, but cannot strip it for
  parts.

### Validated (runtime-checked; typed errors)

- **`ensure_production_adapter_bound`** rejects production sessions
  without an adapter (`HostRunError::ProductionRequiresAdapter`).
- **`ensure_handler_coverage`** rejects missing or conflicting effect
  ownership (`HandlerCoverageError::{MissingHandler, ConflictingCoverage}`).
- **`validate_hosted_runner_configuration`** rejects illegal egress /
  adapter / replay-kinds combinations
  (`HostedEgressValidationError::*`).
- **`HostedRunner::execute_step` pre/post invariants:** rejects
  duplicate event ids, pending-buffer-not-drained, wrong number of
  decision entries, wrong number of run calls, and effects on non-invoke
  decisions (`HostedStepError::{DuplicateEventId, LifecycleViolation,
  MissingDecisionEntry}`).
- **`EffectApplyError` variants** — declared/writable/type/conversion/
  dispatch checks in `SetContextHandler::apply` (§6).
- **`HostedRunner::ensure_capture_finalizable` / `into_capture_bundle`**
  consult `CaptureFinalizationState` (§7).

### Convention (documented; not enforced)

- **Tier-1 vs Tier-2 vs Tier-3 routing.** The host's `lib.rs` comments
  tell CLI/SDK to prefer Tier 1; nothing in the type system requires it.
- **Partial-write rollback in `SetContextHandler`.** The header
  documents that partial writes are *not* rolled back; that is an
  intentional non-enforcement.
- **Three-provenance-string discipline.** The three strings are
  carried by name only; the type system does not distinguish them.

---


## 10. What prod does not own (the kernel boundary)

The kernel CODE_MAP §10 lists what the kernel does *not* own. The
inverse — what prod is forbidden from redefining — is equally explicit.

The prod layer touches kernel authority through exactly three named
surfaces. Every other kernel concern is consumed unchanged.

### Surface 1 — `RuntimeInvoker` (adapter trait)

`crates/kernel/adapter/src/lib.rs::RuntimeInvoker`. The host's
`BufferingRuntimeInvoker` `impl`s it; the supervisor calls it. Prod must
not:

- widen the trait (e.g., add an `effects_out` parameter).
- subclass it in a different module under a renamed trait.
- bypass it by calling `ReportingRuntimeHandle::run_reporting` directly
  from the supervisor.

The point of the trait is that the supervisor sees only the termination,
not the effects. Carrying effects belongs in `BufferingRuntimeInvoker`'s
private state.

### Surface 2 — `DecisionLog` (supervisor trait)

`crates/kernel/supervisor/src/lib.rs::DecisionLog`. The host's
`HostDecisionLog` implements it; the supervisor logs into it. Prod must
not:

- mutate decision-log entries after the supervisor has recorded them
  (the host reads, the kernel writes).
- attach product-shaped metadata to `DecisionLogEntry`; the
  capture-enrichment sidecar exists for that purpose
  (`crates/prod/core/host/src/capture_enrichment.rs`).

### Surface 3 — `CaptureBundle` serde shape

`crates/kernel/supervisor/src/lib.rs::CaptureBundle` and
`CAPTURE_FORMAT_VERSION`. The host writes bundles via
`write_capture_bundle`; the SDK re-exports the same writer. Prod must
not:

- change the on-wire shape of `CaptureBundle` from outside the
  supervisor crate.
- add new top-level fields without bumping `CAPTURE_FORMAT_VERSION`.
- write capture files through any path that bypasses
  `write_capture_bundle` (which does atomic temp-file + sync + rename).

### What prod must not redefine

- **Primitive semantics.** Custom primitives registered through
  `CatalogBuilder` are kernel-validated; prod does not get to invent a
  fifth primitive kind beyond `Source` / `Compute` / `Trigger` / `Action`.
- **Adapter contract meaning.** `AdapterProvides`, `ExternalEvent`,
  `EventId`, `EventTime`, and friends come from `ergo_adapter`. The host
  uses them but does not define them.
- **Replay decision comparison.** Replay equality is owned by
  `ergo_supervisor::replay::compare_decisions`. The host's
  `HostedReplayError` wraps but does not reinterpret kernel replay
  errors.
- **Validation rule shape.** V-rule structure (the kernel CODE_MAP
  §2 table) is owned by `ergo_runtime`. The host's
  `validate_run_graph_*` entrypoints orchestrate runs of those rules;
  they do not add new rules.
- **Capture format version semantics.** `CAPTURE_FORMAT_VERSION` is
  bumped only when the supervisor's bundle shape changes; prod cannot
  bump it for prod-only fields (those go in the capture-enrichment
  sidecar instead).

### Why this discipline matters

The kernel CODE_MAP §10 already states the kernel's negative space.
This section is the matching contract on the prod side: it is not
enough for the kernel to say "I do not own X"; the prod layer must also
say "I do not redefine the things the kernel owns." Together the two
sections close the boundary from both sides.

---

## 11. Re-verification

Use the same path-and-symbol convention as the kernel CODE_MAP. The
commands below resolve every citation in this doc against the current
code; rerun them after any prod refactor.

### A. Path-and-symbol existence checks

For each citation of the form `path/to/file.rs::Symbol`, the following
grep pattern should return at least one match:

```sh
grep -nE "(fn|struct|enum|trait|const|impl|mod|use|type) +(<[^>]+> +)?<Symbol>\\b" <path>
```

Concrete spot checks for this doc:

```sh
# §2 layer symbols
grep -nE "pub struct Ergo\\b|impl Ergo\\b" crates/prod/clients/sdk-rust/src/lib.rs
grep -nE "pub struct ProfileRunner\\b" crates/prod/clients/sdk-rust/src/lib.rs
grep -nE "pub fn run_graph_from_paths_with_surfaces_and_control" crates/prod/core/host/src/usecases/live_run.rs
grep -nE "pub struct HostedRunner\\b|pub fn step\\b|fn execute_step\\b" crates/prod/core/host/src/runner.rs
grep -nE "pub struct CapturingSession\\b|pub fn on_event\\b" crates/kernel/supervisor/src/capture.rs
grep -nE "pub struct BufferingRuntimeInvoker\\b|impl RuntimeInvoker for BufferingRuntimeInvoker" crates/prod/core/host/src/host/buffering_invoker.rs
grep -nE "pub struct ReportingRuntimeHandle\\b|pub fn run_reporting\\b" crates/kernel/adapter/src/lib.rs
grep -nE "fn execute_once\\b" crates/kernel/adapter/src/lib.rs

# §3 surface tiers
grep -nE "pub fn .*_with_surfaces" crates/prod/core/host/src/usecases.rs crates/prod/core/host/src/usecases/*.rs
grep -nE "pub struct RuntimeSurfaces\\b" crates/prod/core/host/src/usecases.rs

# §4 sealed/open
grep -nE "pub struct PreparedGraphAssets\\b" crates/prod/core/loader/src/io.rs
grep -nE "pub\\(crate\\) _sealed: \\(\\)" crates/prod/core/loader/src/io.rs

# §5 dual gate
grep -nE "pub enum SessionIntent\\b" crates/prod/core/host/src/usecases.rs
grep -nE "pub\\(crate\\) session_intent: SessionIntent" crates/prod/core/host/src/usecases.rs
grep -nE "pub\\(super\\) fn ensure_production_adapter_bound\\b" crates/prod/core/host/src/usecases/live_prep.rs
grep -nE "pub fn for_production\\b|pub fn for_fixture\\b" crates/prod/core/host/src/usecases.rs

# §6 handler coverage
grep -nE "pub trait EffectHandler: Send \\+ Sync" crates/prod/core/host/src/host/effects.rs
grep -nE "pub struct SetContextHandler\\b" crates/prod/core/host/src/host/effects.rs
grep -nE "pub fn ensure_handler_coverage\\b" crates/prod/core/host/src/host/coverage.rs
grep -nE "MissingHandler|ConflictingCoverage" crates/prod/core/host/src/host/coverage.rs

# §7 finalization state
grep -nE "enum CaptureFinalizationState\\b" crates/prod/core/host/src/runner.rs
grep -nE "NoCommittedSteps|Eligible|FinalizeOnly|Fatal" crates/prod/core/host/src/runner.rs

# §10 kernel boundary
grep -nE "pub trait RuntimeInvoker\\b" crates/kernel/adapter/src/lib.rs
grep -nE "pub trait DecisionLog\\b" crates/kernel/supervisor/src/lib.rs
grep -nE "pub struct CaptureBundle\\b|CAPTURE_FORMAT_VERSION" crates/kernel/supervisor/src/lib.rs
```

### B. Structural drift checks (claims that change shape)

The following invariants are claimed in the doc above; rerun these
greps to confirm they still hold.

```sh
# §5 — every public live entrypoint calls the dual-gate function (≥ 8 sites today)
grep -nE "ensure_production_adapter_bound" crates/prod/core/host/src/usecases/live_run.rs crates/prod/core/host/src/usecases/live_prep.rs | wc -l

# §6 — EffectHandler must keep its Send + Sync bound (host-side, not kernel)
grep -qE "^pub trait EffectHandler: Send \\+ Sync" crates/prod/core/host/src/host/effects.rs \
  || echo "TRAIT BOUND DRIFT: EffectHandler no longer requires Send + Sync"

# §6 — StopHandle must keep its compile-time Send + Sync assertion
grep -qE "assert_send_sync::<StopHandle>" crates/prod/clients/sdk-rust/src/lib.rs \
  || echo "STRUCTURAL DRIFT: StopHandle no longer asserted Send + Sync at compile time"

# §6 — exactly one built-in EffectHandler implementor in host (today: SetContextHandler)
count=$(grep -rE "^impl EffectHandler for " --include="*.rs" crates/prod/core/host/src/ | wc -l | tr -d ' ')
[ "$count" = "1" ] || echo "BUILT-IN HANDLER COUNT DRIFT: expected 1 impl EffectHandler in prod/core/host, got $count"

# §4 — PreparedGraphAssets must keep its pub(crate) _sealed marker
grep -qE "pub\\(crate\\) _sealed: \\(\\)" crates/prod/core/loader/src/io.rs \
  || echo "STRUCTURAL DRIFT: PreparedGraphAssets no longer carries pub(crate) _sealed: () marker"

# §5 — SessionIntent default must remain Production
grep -qE "Self::Production" crates/prod/core/host/src/usecases.rs \
  || echo "SAFETY DRIFT: SessionIntent::default() may no longer return Production"

# §9 — RuntimeInvoker::run must not carry effects in its signature
grep -A6 "pub trait RuntimeInvoker" crates/kernel/adapter/src/lib.rs | grep -qE "effects_out|Vec<ActionEffect>" \
  && echo "BOUNDARY DRIFT: RuntimeInvoker::run now references effects in its signature"
```

### C. Crate inventory check

```sh
# §1 — prod tree must still have exactly six crates
count=$(find crates/prod -name Cargo.toml -not -path "*/target/*" | wc -l | tr -d ' ')
[ "$count" = "6" ] || echo "PROD CRATE COUNT DRIFT: expected 6, got $count (update §1)"
```

If any check above fails, update the relevant section of this doc in
the same change.

---
