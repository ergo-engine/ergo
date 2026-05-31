---
Scope: Kernel-layer code map (runtime + adapter + supervisor)
Stops at: host boundary
Authority: informative (per AGENTS.md §1, canonical authority lives in `docs/`)
Citations: path-and-symbol (e.g., `crates/kernel/runtime/src/lib.rs::CoreRegistries`)
---

# Kernel Code Map

This doc is a structural map of the **kernel layer** of Ergo:

```
crates/kernel/
├── runtime/      ergo-runtime     — semantics, catalog, stdlib, execution core
├── adapter/      ergo-adapter     — adapter contract, event binding, invoker handles
└── supervisor/   ergo-supervisor  — episode scheduling, capture, replay
```

It exists so that an engineer (or agent) can answer four questions in a
single read, without reconstructing the model from doc transcripts:

1. What does each kernel crate own, and what does it explicitly not own?
2. How does a graph transform across the kernel pipeline?
3. Where are the kernel's two execution entry points, and how do they differ?
4. Where exactly does the kernel end and the host begin?

## What this doc is not

- **Not API documentation.** That is rustdoc's job. Method-level enumeration
  is out of scope.
- **Not decision rationale.** That is the ledger's job
  (`docs/ledger/decisions/`). This file states what is, not why.
- **Not a substitute for canonical system docs.** Authority lives in
  `docs/system/kernel.md`, `docs/system/execution.md`,
  `docs/system/kernel-prod-separation.md`, and the `docs/invariants/`
  tree. When this doc and a canonical doc disagree, the canonical doc
  wins and this doc is the bug.
- **Not a host or SDK map.** The kernel stops at the `RuntimeInvoker`
  seam and the `CaptureBundle` serde surface. Everything past that
  belongs in a prod-layer code map.

Citations use a path-and-symbol form (`crates/kernel/foo/src/bar.rs::Symbol`)
because line numbers drift on every edit and symbols are stable across
file reorganization.

---

## 1. The three kernel crates

Layered top to bottom; each depends only on those below it. There is no
back-edge: nothing in `runtime` imports `adapter` or `supervisor`, and
nothing in `adapter` imports `supervisor`.

| Crate            | Cargo name        | Depends on        | Owns                                                          | Does not own                                  |
|------------------|-------------------|-------------------|---------------------------------------------------------------|-----------------------------------------------|
| `runtime/`       | `ergo-runtime`    | —                 | Primitive ontology, expansion, validation, execution, stdlib  | Adapter contract, scheduling, capture/replay  |
| `adapter/`       | `ergo-adapter`    | `ergo-runtime`    | Adapter manifest, event binding, runtime invoker handles      | Episode scheduling, capture writing, replay   |
| `supervisor/`    | `ergo-supervisor` | both              | Episode scheduling, retries, capture bundle, strict replay    | Adapter semantics, primitive registration     |

Each crate states the same boundary explicitly in its `lib.rs` header
(`Owns:` / `Does not own:` / `Connects to:`):

- `crates/kernel/runtime/src/lib.rs` — currently a one-line header; the
  modules below it carry per-file `//!` headers in the §4A shape.
- `crates/kernel/adapter/src/lib.rs::ergo_adapter` — full §4A header.
- `crates/kernel/supervisor/src/lib.rs::ergo_supervisor` — full §4A header.

**Cargo `[features]`:** none of the kernel crates publish Cargo
features. The former supervisor demo scaffolding now lives only under
`crates/kernel/supervisor/tests/support/` for integration tests.

### Stdlib primitives shipped

The runtime ships a closed set of primitive implementations that
`crates/kernel/runtime/src/catalog.rs::build_core` registers
unconditionally. There is no plugin path inside the kernel; outside
primitives reach the runtime only by being registered through
`crates/kernel/runtime/src/catalog.rs::CatalogBuilder` before
construction.

| Kind     | Count | Directory                                                  |
|----------|-------|------------------------------------------------------------|
| Source   |   7   | `crates/kernel/runtime/src/source/implementations/`        |
| Compute  |  27   | `crates/kernel/runtime/src/compute/implementations/`       |
| Trigger  |   2   | `crates/kernel/runtime/src/trigger/implementations/`       |
| Action   |   6   | `crates/kernel/runtime/src/action/implementations/`        |

Implementations are uniform in shape: a one-field struct
`{ manifest: <Kind>PrimitiveManifest }` whose trait `manifest()` returns
`&self.manifest` and whose compute/produce/evaluate/execute method does
its work from inputs/parameters alone. No implementation in the
kernel's `implementations/` trees holds `Rc`, `Cell`, `RefCell`,
`Mutex`, raw pointers, FFI handles, or any per-instance mutable state.
This is the structural backing for the statelessness convention in §7
and the concurrency claim in §9.

---

## 2. Graph transformation pipeline

A graph passes through **four typed forms** inside the kernel. Each
transition has a single named entry function. Authoring forms cannot
reach execution, and validated forms cannot be constructed without
passing through validation — both are enforced by type, not convention.

```
ClusterDefinition       cluster::expand          ExpandedGraph
(authoring DAG with  ───────────────────────▶   (no NodeKind enum;
 NodeKind::{Impl,                                clusters compiled
 Cluster})                                       away)

ExpandedGraph           runtime::validate        ValidatedGraph
(topology +          ───────────────────────▶   (topo order, port
 identity)                                       metadata resolved)

ValidatedGraph          runtime::execute_        ExecutionReport
+ Registries         ───────────────────────▶   (node outputs +
+ ExecutionContext      with_metadata            ActionEffect list)
```

The four types and their constructors:

- `crates/kernel/runtime/src/cluster.rs::ClusterDefinition` — authoring
  input. Fields are `pub`; this is the boundary loader/host hands the
  kernel.
- `crates/kernel/runtime/src/cluster.rs::ExpandedGraph` — output of
  `crates/kernel/runtime/src/cluster.rs::expand`. Holds only
  `ExpandedNode` (no `NodeKind` enum). Authoring clusters have been
  compiled away (X.9). `boundary_inputs` / `boundary_outputs` are
  retained for signature inference only and must not influence runtime
  execution.
- `crates/kernel/runtime/src/runtime/types.rs::ValidatedGraph` — output
  of `crates/kernel/runtime/src/runtime/validate.rs::validate`. Carries
  resolved `inputs`, `outputs`, `topo_order`, and `boundary_outputs`.
  Construction is gated by the validator: type-system level proof that
  invariants V.1–V.8 + E.3 have been checked.
- `crates/kernel/runtime/src/runtime/types.rs::ExecutionReport` — output
  of `crates/kernel/runtime/src/runtime/execute.rs::execute_with_metadata`.
  Holds `outputs` and `effects`.

### Validation enforcement (kernel-internal)

`crates/kernel/runtime/src/runtime/validate.rs::validate` enforces, in
order:

| Rule  | Enforcing function in `validate.rs`     | Error variant                              |
|-------|-----------------------------------------|--------------------------------------------|
| V.1   | `topological_sort`                      | `CycleDetected`                            |
| V.2   | `enforce_wiring_matrix`                 | `InvalidEdgeKind` / `MissingInputMetadata` |
| V.3   | `enforce_required_inputs`               | `MissingRequiredInput`                     |
| V.4   | `enforce_types`                         | `TypeMismatch`                             |
| V.5   | `enforce_action_gating`                 | `ActionNotGated`                           |
| V.7   | `enforce_single_edge_per_input`         | `MultipleInboundEdges`                     |
| V.8   | `validate` main loop                    | `MissingPrimitive`                         |
| E.3   | `validate` main loop (defense-in-depth) | `ExternalInputNotAllowed`                  |

E.3 is enforced twice: first at expansion (`cluster.rs::expand`), then
again here. V.6 ("all nodes validate before any execute") is a
meta-invariant satisfied by the fact that `validate` runs to completion
before `execute` is called — there is no V.6 function.

### Execution enforcement (runtime/execute.rs)

`crates/kernel/runtime/src/runtime/execute.rs::execute_with_metadata`
walks `topo_order` and dispatches per `PrimitiveKind`. It owns:

- topological node traversal and per-edge value propagation
- type conversion between trigger and action value domains
- context-key injection for source nodes (SRC-10, SRC-11)
- parameter binding for compute and trigger nodes
- action skip/invoke gating (R.7)
- non-finite numeric output rejection (NUM-FINITE-1)
- intent-id derivation via `crates/kernel/runtime/src/common/intent_id.rs::derive_intent_id`

The two-entry asymmetry inside `execute.rs`:

- `crates/kernel/runtime/src/runtime/execute.rs::execute` is for graphs
  with no intent-emitting actions. If one is present it returns
  `ExecError::IntentMetadataRequired { node }` (GW-EFX-META-1) — callers
  must use `execute_with_metadata` so deterministic intent IDs can be
  derived from `graph_id` + `event_id`.
- `execute_with_metadata` is the canonical entry; `execute` is a
  metadata-free escape hatch that fails closed when any action would
  emit an intent.

---

## 3. The two execution entry points

The kernel exposes two execution entries at different altitudes. Both
sit above the `validate → execute` pipeline; they differ in what wraps
that pipeline.

### Entry A — direct `runtime::run`

`crates/kernel/runtime/src/runtime/mod.rs::run`:

````rust
pub fn run<C: PrimitiveCatalog>(
    expanded: &ExpandedGraph,
    catalog: &C,
    registries: &Registries,
    ctx: &ExecutionContext,
) -> Result<ExecutionReport, RuntimeError>
````

A one-shot `validate → execute` call. **Not** episode-aware: no
retries, no deferral, no decision log, no capture. Used by tests, by
simple harnesses, and indirectly as the inner step of the orchestrated
entry through `execute_with_metadata`.

All four parameters are shared references; none is `&mut`. The returned
`ExecutionReport` is owned. `crates/kernel/runtime/src/runtime/execute.rs::execute_with_metadata`
constructs its `node_outputs` and `effects` as stack-local maps, drains
them into the report, and drops everything else. Calling
`run(&graph_b, &catalog, &registries, &ctx_b)` immediately after
`run(&graph_a, ...)` is therefore supported by construction — the
runtime crate has no per-graph singleton state. Multi-graph on one
thread is a today property; multi-thread is gated by §8.

### Entry B — `Supervisor::on_event`

`crates/kernel/supervisor/src/lib.rs::Supervisor` is the episode-driven
entry:

````rust
pub struct Supervisor<L: DecisionLog, R: RuntimeInvoker> { /* private */ }
````

It owns:

- episode id allocation (`crates/kernel/supervisor/src/lib.rs::EpisodeId`)
- a deterministic clock (`DeterministicClock`, private)
- concurrency cap (`Constraints::max_in_flight`)
- rate limiting (`Constraints::{max_per_window, rate_window}`)
- a deferral queue (`BTreeMap<(EventTime, EpisodeId), DeferredEpisode>`)
- retry classification (`Supervisor::should_retry` — only
  `NetworkTimeout`, `AdapterUnavailable`, `RuntimeError`, and
  `TimedOut` retry; `SemanticError` never does — B.2)
- decision logging through the `DecisionLog` trait
  (`crates/kernel/supervisor/src/lib.rs::DecisionLog`, write-only per SUP-7)

It does **not** execute graphs directly — it delegates through a
`RuntimeInvoker`. Two constructors:

- `Supervisor::new` — convenience that wires a `RuntimeHandle` with
  `AdapterProvides::default()`. No-adapter / demo path. Marked
  `#[allow(clippy::arc_with_non_send_sync)]` because the trait-bound
  asymmetry described in §8 means `Arc<ExpandedGraph>` etc. are
  intentionally not `Send+Sync` at v1.
- `Supervisor::with_runtime` — accepts any `R: RuntimeInvoker`, used by
  capture sessions, replay harnesses, and tests.

### The `RuntimeInvoker` seam

`crates/kernel/adapter/src/lib.rs::RuntimeInvoker`:

````rust
pub trait RuntimeInvoker {
    fn run(
        &self,
        graph_id: &GraphId,
        event_id: &EventId,
        ctx: &ExecutionContext,
        deadline: Option<Duration>,
    ) -> RunTermination;
}
````

This is the **inversion seam** between supervisor (kernel) and runtime
(kernel) — and, externally, between supervisor and the host's buffering
wrapper. Three in-tree impls:

| Type                                                          | Purpose                                                                          |
|---------------------------------------------------------------|----------------------------------------------------------------------------------|
| `crates/kernel/adapter/src/lib.rs::RuntimeHandle`             | Production-facing. `run` returns `RunTermination` only; effects discarded.       |
| `crates/kernel/adapter/src/lib.rs::ReportingRuntimeHandle`    | Drains `Vec<ActionEffect>` via `run_reporting`. Used by host buffering wrapper.  |
| `crates/kernel/adapter/src/lib.rs::FaultRuntimeHandle`        | Schedule of pre-canned terminations per `EventId`. Test-only.                    |

All three wrap the private `crates/kernel/adapter/src/lib.rs::RuntimeState`,
which holds `Arc<ExpandedGraph>`, `Arc<CorePrimitiveCatalog>`,
`Arc<CoreRegistries>`, and `AdapterProvides`. The duplication of
`RuntimeHandle` vs `ReportingRuntimeHandle` is structural debt
identified in the v1 audit but not yet collapsed — see Tier 1 item 2 of
the audit ledger.

Effects-draining is **only** exposed through
`ReportingRuntimeHandle::run_reporting`; `RuntimeInvoker::run` cannot
return effects. This is deliberate: the supervisor only needs
termination outcomes, and accidental effect duplication is prevented by
never giving the supervisor an effects channel.

### Ownership chain to a primitive

For the execution path through `RuntimeHandle`, the kernel chain from
top of stack to the trait object actually invoked is:

````
Supervisor<L, R>
  └── R: RuntimeInvoker                        (injected at construction)
       └── RuntimeHandle / ReportingRuntimeHandle
            └── RuntimeState  (private to ergo-adapter)
                 ├── Arc<ExpandedGraph>
                 ├── Arc<CorePrimitiveCatalog>
                 ├── Arc<CoreRegistries>
                 │    ├── ComputeRegistry  → HashMap<String, Box<dyn ComputePrimitive>>
                 │    ├── SourceRegistry   → HashMap<String, Box<dyn SourcePrimitive>>
                 │    ├── TriggerRegistry  → HashMap<String, Box<dyn TriggerPrimitive>>
                 │    └── ActionRegistry   → HashMap<String, Box<dyn ActionPrimitive>>
                 └── AdapterProvides
                      ↳ &dyn <Kind>Primitive  (borrowed per call by the executor)
````

Three properties of this chain matter:

1. Every layer above the trait object is `Arc`-wrapped so that two
   handles can share one catalog/registries pair on the same thread
   (the multi-graph case from Entry A's purity note).
2. Every trait method takes `&self`; the kernel never hands out `&mut`
   to a primitive. The `Option<&mut PrimitiveState>` slot on
   `ComputePrimitive::compute` is a reserved-but-unused trait surface
   — the executor always passes `None` and no other code path supplies
   one. See §9.
3. The registry getters return `&dyn <Kind>Primitive` rather than an
   owning handle; the executor only borrows for the duration of one
   node's execution.

---

## 4. Catalog and registries lifecycle

Build once, freeze, share. The catalog and the four registries have
exactly one construction path and no mutation API after construction.

### Construction

`crates/kernel/runtime/src/catalog.rs::build_core` is the canonical
entry. It returns the `(CoreRegistries, CorePrimitiveCatalog)` pair
built from the kernel's fixed primitive inventory:

````rust
pub fn build_core()
    -> Result<(CoreRegistries, CorePrimitiveCatalog), CoreRegistrationError>
````

Two thin facade functions sit on top for callers that only need one
half of the pair:

- `crates/kernel/runtime/src/catalog.rs::build_core_catalog` — catalog only
- `crates/kernel/runtime/src/catalog.rs::core_registries` — registries only

Callers that need to extend the stdlib use
`crates/kernel/runtime/src/catalog.rs::CatalogBuilder`, whose `build()`
delegates to the private `build_from_inventory(inventory)`. That
private function is the only call site in the crate for the four
`pub(crate) fn register_{compute,trigger,source,action}` mutators on
`CorePrimitiveCatalog`.

### Freeze

After `build_from_inventory` returns, the catalog and registries are
effectively immutable:

- The four `register_*` methods on `CorePrimitiveCatalog` are
  `pub(crate)` to `ergo_runtime`; no downstream crate can call them.
- `CoreRegistries` has no `&mut self` methods of its own.
- The four `*Registry` types each expose a `register` method, but it
  is invoked only inside `build_from_inventory` and in one adapter
  test fixture at `crates/kernel/adapter/src/tests.rs`.
- No `unregister`, `remove`, `clear`, or `replace` method exists on
  any of the four registries or on the catalog.
- No call to `Arc::get_mut` or `Arc::make_mut` exists anywhere in
  `crates/kernel/`. Once the pair is wrapped in `Arc`, the inner
  contents are permanent for the life of the process.

This is the structural backing for the multi-graph claim in §3:
because the catalog and registries are write-once, two `RuntimeHandle`s
on the same thread can safely share one
`Arc<CoreRegistries>` / `Arc<CorePrimitiveCatalog>` pair.

### Registration error

`crates/kernel/runtime/src/catalog.rs::CoreRegistrationError` is the
only failure shape `build_core` can produce. It wraps duplicate
registration (impossible for the stdlib unless the inventory is wrong)
and per-kind registration failures from the `*Registry::register`
methods.

---

## 5. Capture and replay

Capture and replay are entirely **inside the kernel**. The host's
involvement is limited to passing provenance strings and writing the
final bundle bytes to disk.

### Capture

`crates/kernel/supervisor/src/capture.rs::CapturingDecisionLog<L>` wraps
any `DecisionLog`. Every `log()` call mirrors the entry into a shared
`Arc<Mutex<CaptureBundle>>` and forwards to the inner log.

`crates/kernel/supervisor/src/capture.rs::CapturingSession<L, R>` is the
top-level capture wrapper:

````rust
pub struct CapturingSession<L: DecisionLog, R: RuntimeInvoker> {
    supervisor: Supervisor<CapturingDecisionLog<L>, R>,
    bundle: Arc<Mutex<CaptureBundle>>,
}
````

Two constructors:

- `CapturingSession::new` — `adapter_provenance` defaults to
  `NO_ADAPTER_PROVENANCE` ("none"). Demo / fixture path.
- `CapturingSession::new_with_provenance` — explicit `adapter_provenance`
  + `runtime_provenance`. This is the canonical adapter-bound path that
  the host calls.

Provenance strings are produced **outside** the supervisor:

- `crates/kernel/adapter/src/provenance.rs::fingerprint` computes the
  adapter fingerprint from `AdapterManifest`.
- `crates/kernel/runtime/src/provenance.rs::compute_runtime_provenance`
  computes the runtime fingerprint over `ExpandedGraph` +
  `PrimitiveCatalog`. Returns `Result<String, RuntimeProvenanceError>`.

The bundle on disk:

- `crates/kernel/supervisor/src/lib.rs::CaptureBundle` — `#[serde(deny_unknown_fields)]`
- `crates/kernel/supervisor/src/lib.rs::CAPTURE_FORMAT_VERSION` —
  currently `"v3"`, `pub(crate)` because callers should read it through
  the bundle, not pin it directly.
- `crates/kernel/supervisor/src/capture.rs::write_capture_bundle` —
  atomic write (temp file + sync + rename) returning typed
  `CaptureWriteError` per write stage.

### Replay

`crates/kernel/supervisor/src/replay.rs` is the kernel's replay
authority. Entry points:

| Function                                                                  | What it checks                                                 |
|---------------------------------------------------------------------------|----------------------------------------------------------------|
| `crates/kernel/supervisor/src/replay.rs::validate_bundle`                 | Capture version, duplicate event ids, payload hashes (lenient) |
| `crates/kernel/supervisor/src/replay.rs::validate_bundle_strict`          | Adds adapter + runtime provenance match                        |
| `crates/kernel/supervisor/src/replay.rs::replay`                          | Re-runs events through a `RuntimeInvoker` (lenient)            |
| `crates/kernel/supervisor/src/replay.rs::replay_checked`                  | `validate_bundle` + `replay`                                   |
| `crates/kernel/supervisor/src/replay.rs::replay_checked_strict`           | `validate_bundle_strict` + `replay` + per-effect hash compare  |
| `crates/kernel/supervisor/src/replay.rs::compare_decisions`               | Compares observed decisions against recorded ones              |
| `crates/kernel/supervisor/src/replay.rs::hash_effect`                     | Sibling of `crate::compute_effect_hash`, exposed for callers   |

`crates/kernel/supervisor/src/replay.rs::ReplayError` is the canonical
failure taxonomy: `UnsupportedVersion`, `HashMismatch`, `InvalidPayload`,
`AdapterProvenanceMismatch`, `RuntimeProvenanceMismatch`,
`UnexpectedAdapterProvidedForNoAdapterCapture`,
`AdapterRequiredForProvenancedCapture`, `DuplicateEventId`,
`EffectMismatch`, etc. The host wraps these at its own boundary; it
never invents new replay failure shapes.

### Egress provenance

Currently audit-only at v1: `CaptureBundle::egress_provenance` is stored
but does not gate strict replay. See `docs/system/current-architecture.md`
§5.

---

## 6. Vocabulary collisions

The kernel reuses several common names in distinct meanings. These are
not bugs — they are precise within each module — but they are the most
common source of confusion for new readers.

### "Catalog"

There are three things called or implementing some form of "catalog":

| Name                                                                       | What it is                                                                                                              |
|----------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------|
| `crates/kernel/runtime/src/cluster.rs::PrimitiveCatalog` (trait)           | The lookup interface used by expansion and validation: `fn get(&self, id, version) -> Option<PrimitiveMetadata>`        |
| `crates/kernel/runtime/src/catalog.rs::CorePrimitiveCatalog` (struct)      | The kernel's concrete impl of `PrimitiveCatalog`, populated with stdlib metadata.                                       |
| `crates/kernel/runtime/src/catalog.rs::CatalogBuilder`                     | Mutable builder for callers that need to register additional primitives without losing stdlib coverage.                 |

Higher layers may add a fourth (host-side or SDK-side) catalog wrapper;
that is out of scope here and does not change the kernel meaning.

### "Registries"

Two distinct types, both kernel-internal:

| Name                                                                       | Shape                                                       | Lifetime           |
|----------------------------------------------------------------------------|-------------------------------------------------------------|--------------------|
| `crates/kernel/runtime/src/catalog.rs::CoreRegistries`                     | Owned struct of 4 trait-object registries.                  | `'static`-friendly |
| `crates/kernel/runtime/src/runtime/types.rs::Registries<'a>`               | Bundle of 4 `&'a` references (sources, computes, triggers, actions). | Per-call    |

`CoreRegistries` is what you build once. `Registries<'a>` is what you
hand to `execute` per call; conversion is by reference projection.

### "ExecutionContext"

Two types share the name across the kernel boundary inside the kernel:

| Name                                                                       | Visibility           | Role                                                                                                     |
|----------------------------------------------------------------------------|----------------------|----------------------------------------------------------------------------------------------------------|
| `crates/kernel/runtime/src/runtime/types.rs::ExecutionContext`             | Public, `Default`    | Internal runtime context (HashMap<String, Value>).                                                       |
| `crates/kernel/adapter/src/lib.rs::ExecutionContext`                       | Public, opaque       | Outer wrapper. Constructor is `pub(crate)` so non-adapter callers cannot synthesize one (CXT-1).         |

The adapter wrapper is the only `ExecutionContext` that crosses the
kernel boundary outward. Its `inner` field is `pub(crate)`; only adapter
code can unwrap it before handing the runtime variant to `execute`. The
compile_fail doctests in `crates/kernel/adapter/src/lib.rs` enforce
this structurally.

### "Handle"

Three `*Handle` types live in the adapter crate, all `RuntimeInvoker`
impls; see §3 above. There is also `crates/kernel/adapter/src/lib.rs::RuntimeState`
which is the shared inner state — not a handle, deliberately private.

### "Primitive"

Four traits, one per kind:

- `crates/kernel/runtime/src/source/mod.rs::SourcePrimitive`
- `crates/kernel/runtime/src/compute/mod.rs::ComputePrimitive`
- `crates/kernel/runtime/src/trigger/mod.rs::TriggerPrimitive` — **the
  only one with `: Send + Sync`** (see §8)
- `crates/kernel/runtime/src/action/mod.rs::ActionPrimitive`

---

## 7. Enforcement axis: structural / validated / convention

Every kernel invariant lives in one of three tiers. Tagging each claim
explicitly avoids the "it's safe because callers know not to do that"
trap.

- **Structural** — Rust's type system enforces it. Violations don't
  compile.
- **Validated** — A runtime check at a known seam rejects the violating
  state. Violations compile but fail closed at a specific function.
- **Convention** — A documented contract with no programmatic
  enforcement. Violations may pass silently in normal runs and only
  surface through capture/replay divergence, audit, or review.

### Structural

| Invariant                                                      | Mechanism                                                                                                       |
|----------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------|
| `ValidatedGraph` cannot be constructed without validation      | Only `crates/kernel/runtime/src/runtime/validate.rs::validate` returns one.                                     |
| `ExpandedNode` carries no `NodeKind` enum (X.9)                | The type doesn't have the field. Authoring clusters cannot reach execution.                                     |
| Adapter `ExecutionContext` cannot be synthesized externally    | Constructor is `pub(crate)` (CXT-1). Enforced by compile_fail doctests in `crates/kernel/adapter/src/lib.rs`.   |
| `ExternalEventRecord::rehydrate` (unchecked) is `pub(crate)`   | External callers must use `rehydrate_checked` (HARDEN-REHYDRATE-1).                                             |
| `CaptureBundle` rejects unknown fields                         | `#[serde(deny_unknown_fields)]` at `crates/kernel/supervisor/src/lib.rs::CaptureBundle`.                        |
| `DecisionLog` is write-only (SUP-7)                            | Trait has only `fn log(&self, entry)`; no read method exists.                                                   |
| `TriggerPrimitive` impls are `Send + Sync`                     | Trait bound at `crates/kernel/runtime/src/trigger/mod.rs::TriggerPrimitive`. The other three primitives are not. |

### Validated

| Invariant                                                      | Enforcing seam                                                                                                  |
|----------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------|
| V.1–V.8, E.3                                                   | `crates/kernel/runtime/src/runtime/validate.rs::validate`                                                       |
| R.7 (NotEmitted gating)                                        | `crates/kernel/runtime/src/runtime/execute.rs::should_skip_action`; defensive `ActionSkipViolation` if missed.  |
| NUM-FINITE-1 (no NaN/Inf in numeric outputs)                   | Per-output guard inside `execute_with_metadata`.                                                                |
| Intent-id metadata requirement (GW-EFX-META-1)                 | `runtime::execute` returns `ExecError::IntentMetadataRequired` if metadata is missing.                          |
| Adapter manifest validity                                      | `crates/kernel/adapter/src/validate.rs::validate_adapter`, called by `crates/kernel/adapter/src/registry.rs::register`. |
| Composition (source ↔ adapter, action ↔ adapter)              | `crates/kernel/adapter/src/composition.rs::validate_source_adapter_composition` / `validate_action_adapter_composition`. |
| Capture format version                                         | `crates/kernel/adapter/src/composition.rs::validate_capture_format`, called from `RuntimeState::validate_composition`. |
| Replay payload-hash integrity                                  | `crates/kernel/adapter/src/capture.rs::ExternalEventRecord::rehydrate_checked` + `validate_hash`.               |
| Replay provenance match                                        | `crates/kernel/supervisor/src/replay.rs::validate_bundle_strict`.                                               |
| Replay effect-hash determinism                                 | `crates/kernel/supervisor/src/replay.rs::replay_checked_strict`.                                                |

### Convention

| Invariant                                                      | Where it's documented                                                                                           |
|----------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------|
| Trigger statelessness (TRG-STATE-1)                            | Trait doc comment on `TriggerPrimitive`; manifest-level `state.allowed == false` (TRG-9). Runtime detection is only via capture/replay divergence. Structural enforcement was rejected — see `docs/ledger/decisions/rejected-structural-enforcement-of-statelessness.md`. |
| Compute primitive statelessness contract                       | Same — `ComputePrimitive` doc comment, CMP-9 manifest check, capture/replay detection.                          |
| Source primitive determinism                                   | Same pattern.                                                                                                   |
| Action primitive determinism                                   | Same pattern.                                                                                                   |
| File-level `//!` header accuracy (per AGENTS.md §4A)           | Currently a review-time convention; no CI check.                                                                |
| Kernel determinism end-to-end                                  | Enforced indirectly: any non-determinism produces a `ReplayError::EffectMismatch` or `HashMismatch` on replay.  |

The mixed entries above (e.g., trigger statelessness — structural at the
manifest, validated at registration, convention at the trait body) are
honest: the kernel uses multiple enforcement tiers for the same
property, and pretending it's one or the other obscures the design.

---

## 8. Threading model and Send/Sync today

The kernel is single-threaded by construction at v1. Cross-thread use
is gated by trait-bound decisions that have not been taken yet. This
section pins down exactly which bound, which type, and which call site
each constraint sits at.

### What is `Send + Sync` today

Among the four primitive traits, only one declares the bound:

| Trait                                                                       | `Send + Sync`? | Declaration                                                 |
|-----------------------------------------------------------------------------|----------------|-------------------------------------------------------------|
| `crates/kernel/runtime/src/trigger/mod.rs::TriggerPrimitive`                | Yes            | `pub trait TriggerPrimitive: Send + Sync { … }`             |
| `crates/kernel/runtime/src/source/mod.rs::SourcePrimitive`                  | No             | `pub trait SourcePrimitive { … }`                           |
| `crates/kernel/runtime/src/compute/mod.rs::ComputePrimitive`                | No             | `pub trait ComputePrimitive { … }`                          |
| `crates/kernel/runtime/src/action/mod.rs::ActionPrimitive`                  | No             | `pub trait ActionPrimitive { … }`                           |

### Why three of four force the kernel to be single-thread

The deduction chain that produces a non-`Send` `RuntimeHandle`:

1. `Box<dyn ComputePrimitive>` is `Send + Sync` only if the trait
   declares those bounds. It does not.
2. Therefore `HashMap<String, Box<dyn ComputePrimitive>>` is not
   `Send + Sync`, and the same holds for the source and action
   registries.
3. Therefore `crates/kernel/runtime/src/catalog.rs::CoreRegistries` is
   not `Send + Sync` — it owns the three offending registry maps.
4. Therefore `Arc<CoreRegistries>` is not `Send`.
5. Therefore `crates/kernel/adapter/src/lib.rs::RuntimeState`, which
   contains that `Arc`, is not `Send`.
6. Therefore the three public handles
   (`crates/kernel/adapter/src/lib.rs::RuntimeHandle`,
   `crates/kernel/adapter/src/lib.rs::ReportingRuntimeHandle`,
   `crates/kernel/adapter/src/lib.rs::FaultRuntimeHandle`) are not
   `Send + Sync`.
7. Therefore `Supervisor<L, R>` is not `Send + Sync` whenever
   `R: RuntimeInvoker` is one of those handles.

The `Arc<CorePrimitiveCatalog>` part of the chain is already
`Send + Sync` — the catalog holds only plain data
(`HashMap<(String, Version), PrimitiveMetadata>`). The blocker is
purely the trigger/source/compute/action asymmetry.

### The `TriggerPrimitive: Send + Sync` bound is dead today

No code in the workspace requires `TriggerPrimitive: Send + Sync`:

- `crates/kernel/runtime/src/trigger/registry.rs::TriggerRegistry`
  stores trait objects in a `HashMap`, which does not require its
  values to be `Send + Sync`.
- The executor takes `&dyn TriggerPrimitive` and calls `evaluate`
  synchronously inside `execute_trigger`. No `spawn`, channel, or
  mutex is involved.
- No `T: TriggerPrimitive + Send + Sync` bound and no
  `where R: RuntimeInvoker + Send + Sync` bound exists anywhere in
  `crates/kernel/` or `crates/prod/`.

Removing `Send + Sync` from `TriggerPrimitive` would compile cleanly
today. The bound is a roadmap artifact, not a load-bearing constraint —
the most likely reading is that the project intends to extend the
bound to the other three traits and `TriggerPrimitive` simply got
there first.

### `#[allow(clippy::arc_with_non_send_sync)]` suppression sites

Clippy fires `arc_with_non_send_sync` whenever `Arc::new(x)` is called
on a non-`Send + Sync` `x`. Because the three primitive traits lack
the bound, every `Arc<CoreRegistries>` / `Arc<ReportingRuntimeHandle>`
construction tripped the lint and has been individually suppressed.

Kernel-internal sites:

| Site                                                                                  | Scope          |
|---------------------------------------------------------------------------------------|----------------|
| `crates/kernel/supervisor/src/lib.rs` (around `Supervisor::new`)                      | item-level     |
| `crates/kernel/supervisor/tests/integration.rs` — three test fns                      | item-level ×3  |

Host-side sites that exist because of the same kernel decision:

| Site                                                                                       | Scope                     |
|--------------------------------------------------------------------------------------------|---------------------------|
| `crates/prod/core/host/src/host/buffering_invoker.rs::BufferingRuntimeInvoker::new`        | item-level                |
| `crates/prod/core/host/src/runner/tests.rs` (test helper)                                  | item-level                |
| `crates/prod/core/host/src/replay/tests.rs` (test helper)                                  | item-level                |
| `crates/prod/core/host/src/usecases/live_run.rs`                                           | module-level (`#![allow]`)|
| `crates/prod/core/host/src/usecases/live_prep.rs`                                          | module-level (`#![allow]`)|

The kernel `runtime/` crate itself contains **zero** `Arc::new` calls;
the wrapping happens at the kernel/host seam. This is the operational
shape of "kernel borrows, host owns" — the kernel is borrow-based,
prod is `Arc`-share-based, and the lint suppressions mark the seam.

### Minimal change that would unblock cross-thread use

Adding `: Send + Sync` to the three trait declarations
(`SourcePrimitive`, `ComputePrimitive`, `ActionPrimitive`) is
sufficient:

- The auto-derivation chain then makes `CoreRegistries`,
  `RuntimeState`, and every `*RuntimeHandle` `Send + Sync`.
- All nine suppression sites above become removable, and their
  rationale comments become stale.
- Every stdlib primitive implementation already satisfies the bound
  (§1 stdlib inventory): each is `struct X { manifest: Manifest }`
  over plain data.
- The SDK threading-model comments at
  `crates/prod/clients/sdk-rust/src/lib.rs` (which refer to "those
  bounds" as a post-v1 roadmap item) would need rewording.

The decision itself is tracked in
`docs/ledger/decisions/sdk-threading-send-sync.md`.

---

## 9. Concurrency primitive map

A separate question from "is the kernel `Send + Sync`?" is "does the
kernel use concurrency primitives at all?" The answer differs sharply
between the execution path and everything else.

### Execution path: zero

Across the entire `crates/kernel/runtime/src/` tree there are no
matches for `spawn`, `Mutex`, `RwLock`, `channel(`, atomics,
`thread::`, `tokio::`, `lazy_static`, `once_cell`, `OnceLock`,
`OnceCell`, or `static mut`. The execution path from
`crates/kernel/runtime/src/runtime/mod.rs::run` through
`execute_with_metadata` down to `execute_compute` / `execute_source` /
`execute_trigger` / `execute_action` is synchronous and uses no
concurrency primitive of any kind.

### Outside the execution path: concentrated at capture and test seams

Where they do appear in the kernel:

| Site                                                                            | What it is                                                              |
|---------------------------------------------------------------------------------|-------------------------------------------------------------------------|
| `crates/kernel/supervisor/src/capture.rs` — `Arc<Mutex<CaptureBundle>>`         | Shared bundle for `CapturingDecisionLog` / `CapturingSession`           |
| `crates/kernel/supervisor/src/capture.rs` — `AtomicU64` temp-file counter       | Unique-id source for atomic-write temp files                            |
| `crates/kernel/supervisor/src/capture.rs` — `std::thread::sleep`                | Filesystem retry backoff on transient rename errors                     |
| `crates/kernel/supervisor/src/replay.rs::MemoryDecisionLog`                     | `Arc<Mutex<Vec<DecisionLogEntry>>>` — convenience log fixture           |
| `crates/kernel/adapter/src/lib.rs::FaultRuntimeHandle`                          | `Arc<Mutex<HashMap<EventId, Vec<RunTermination>>>>` — test-only injector|
| `crates/kernel/adapter/tests/fixture_stress.rs`                                 | `AtomicUsize` counter — test infrastructure only                        |
| `crates/kernel/supervisor/tests/integration.rs` — `CapturingLog`                | `Arc<Mutex<Vec<DecisionLogEntry>>>` — test fixture                      |

Every appearance is one of:

- (a) shared-mutable state required for capture's cross-thread append
  semantics — the only execution-adjacent case,
- (b) a kernel-shipped test fixture (`MemoryDecisionLog`,
  `FaultRuntimeHandle`),
- (c) test scaffolding inside `tests/`.

None is reachable from the live execution path. If a concurrency
primitive is being added to `crates/kernel/runtime/src/`, stop and
escalate — that is a semantic change to the kernel's threading model.

### `PrimitiveState`: a reserved-but-unused trait surface

`crates/kernel/runtime/src/compute/mod.rs::ComputePrimitive::compute`
accepts an explicit state parameter:

````rust
fn compute(
    &self,
    inputs: &HashMap<String, Value>,
    parameters: &HashMap<String, Value>,
    state: Option<&mut PrimitiveState>,
) -> Result<HashMap<String, Value>, ComputeError>;
````

The executor always passes `None` here — see the
`primitive.compute(&mapped_inputs, &mapped_parameters, None)` call in
`crates/kernel/runtime/src/runtime/execute.rs`.
`crates/kernel/runtime/src/compute/mod.rs::PrimitiveState` is never
allocated anywhere in the kernel, and no other code path supplies one.
There is no channel by which a primitive can receive state from a
prior call — the parameter is a reserved trait surface, not a
state-passing mechanism. The other three primitive traits do not
accept a state parameter at all.

Beyond the missing channel, statelessness is positively enforced at
two layers:

- **Registration-time, structural in the manifest.** The four registry
  validators reject stateful manifests via rule IDs
  `SRC-8` / `CMP-9` / `TRG-9` / `ACT-10`. Source, trigger, and action
  primitives must declare `state.allowed == false`; compute primitives
  may declare `state.allowed == true` only when they also declare
  `state.resettable == true` (CMP-9). Every stdlib primitive declares
  `state.allowed: false` (see §1).
- **Runtime, behavioural.** If a primitive author smuggles state
  through interior mutability inside `&self`, capture/replay divergence
  detects the resulting non-determinism (see §5).

Structural prevention via marker traits (e.g. forbidding `Cell`/`Mutex`
in primitive types at the type system level) was considered and
rejected; see `docs/ledger/decisions/rejected-structural-enforcement-of-statelessness.md`.

Combined with §4 (catalog and registries are write-once) and the
implementation-shape note in §1's stdlib inventory, this gives the
kernel's statefulness picture in one sentence: **the executor holds
local maps, primitives are manifest-validated to be stateless and
behaviourally re-checked via capture/replay, and the only kernel state
that crosses a call boundary is what the supervisor records in a
`DecisionLog`.**

---

## 10. What the kernel does not own (the host boundary)

The kernel ends at three concrete surfaces. Everything beyond them is
host or SDK territory and out of scope here.

### Surface A — `RuntimeInvoker`

Downstream of the supervisor, the kernel only ever sees an opaque
`R: RuntimeInvoker`. The host's `BufferingRuntimeInvoker` wraps a
`ReportingRuntimeHandle` to drain effects per call; the kernel never
sees that wrapper directly.

### Surface B — `DecisionLog` (and `CapturingDecisionLog`)

The kernel writes through `DecisionLog::log`. Where those entries go is
the host's problem (in-memory `MemoryDecisionLog`, the host's audit
trail, etc.). `MemoryDecisionLog` lives in
`crates/kernel/supervisor/src/replay.rs::MemoryDecisionLog` as a
convenience; production callers may swap it.

### Surface C — `CaptureBundle` serde

Once `CapturingSession` produces a `CaptureBundle`, the kernel's job is
done. `write_capture_bundle` is provided for atomicity but the kernel
makes no claim about where the file goes, how it's named, or how it's
retained. Strict replay reads back a `CaptureBundle` and revalidates
it; that's the round-trip the kernel guarantees.

### Things the kernel does **not** own

- Profile resolution, project layout, `ergo.toml` parsing.
- YAML loading, file discovery, `ClusterDefinition` decoding.
- Ingress channels (event arrival from the outside world).
- Egress channels and effect dispatch routing.
- Per-event effect buffering and post-episode drain.
- Adapter binding decisions (which adapter to use for which run).
- Replay descriptor construction or product-facing replay UX.
- Process-level lifecycle (start, stop, signal handling).

If you find yourself adding any of the above to a kernel crate, stop
and escalate — that is host or SDK code regardless of how cleanly it
fits.

---

## 11. Re-verification

This doc is informative, not authoritative. To verify it against the
code:

1. Confirm the three lib.rs headers still match §1's "Owns / Does not
   own" claims.
2. `grep -nE "^pub fn|^pub struct|^pub enum|^pub trait" crates/kernel/*/src/lib.rs`
   should still produce the cited symbols.
3. Run `cargo test -p ergo-runtime -p ergo-adapter -p ergo-supervisor`
   to confirm the validation rule mapping table in §2 still holds
   (errors `ErrorInfo::rule_id` tests live in
   `crates/kernel/runtime/src/runtime/types.rs`).
4. Re-check the §1 stdlib counts by listing
   `crates/kernel/runtime/src/{source,compute,trigger,action}/implementations/`.
   If a count shifts, either the table is stale or a new primitive was
   added without updating the inventory in `catalog.rs`.
5. Re-check §8's trait-bound table by grepping
   `^pub trait (Source|Compute|Trigger|Action)Primitive` in the four
   primitive `mod.rs` files. Any change to the `Send + Sync` posture
   invalidates §8 entirely and probably §6 as well.
6. Re-check the §8 suppression-site list with
   `grep -rn "arc_with_non_send_sync" --include="*.rs" crates/`. If the
   count differs from nine, the table is stale.
7. Re-check §9's "execution path: zero" claim with
   `grep -rnE "spawn|Mutex|RwLock|Atomic|thread::|tokio::|once_cell|OnceLock|OnceCell" --include="*.rs" crates/kernel/runtime/src/`.
   Any match in that tree is either a violation or a doc-update trigger.
8. If a cited symbol no longer exists, the doc is out of date and the
   citation must be updated or removed — silent drift is the failure
   mode this doc is most exposed to.

When in doubt, prefer canonical docs:

- `docs/system/kernel.md`
- `docs/system/execution.md`
- `docs/system/kernel-prod-separation.md`
- `docs/invariants/` (per-phase enforcement)
- `docs/ledger/decisions/` (rationale)
