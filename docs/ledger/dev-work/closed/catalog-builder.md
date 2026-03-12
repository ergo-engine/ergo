---
Authority: PROJECT
Date: 2026-03-09
Author: Claude Opus 4.5 (Structural Auditor) + Codex (Implementation)
Status: CLOSED
Branch: feat/catalog-builder
Tier: 2 (Extension Plumbing)
Depends-On: none (can parallel with feat/adapter-runtime; consumed later by feat/ergo-init)
---

# Catalog Builder

## Scope

Expose a public, additive builder in `crates/kernel/runtime/src/catalog.rs` that lets external code register additional implementations alongside the core stdlib at startup.

This branch preserves the existing validation and shared-build guarantees already closed in the closure register:

- `REG-SYNC-1` — catalog and registries are built from the same source
- `CAT-SYNC-1` — catalog/registry key parity test remains green
- `CAT-LOCKDOWN-1` — direct catalog mutation stays crate-private

No frozen doc changes. No trait changes. No validation rule changes. No plugin discovery/loading.

## Current State

| What | Where | Current Behavior | Limitation |
|------|-------|------------------|------------|
| `build_core()` | `crates/kernel/runtime/src/catalog.rs` | Builds `CoreRegistries` + `CorePrimitiveCatalog` from one shared hardcoded core implementation inventory | Only the built-in stdlib can be admitted |
| `build_core_catalog()` | `crates/kernel/runtime/src/catalog.rs` | Thin convenience wrapper over `build_core()` | Core-only surface |
| `core_registries()` | `crates/kernel/runtime/src/catalog.rs` | Thin convenience wrapper over `build_core()` | Core-only surface |
| `CorePrimitiveCatalog::new()` + `register_*()` | `crates/kernel/runtime/src/catalog.rs` | `pub(crate)` to enforce `CAT-LOCKDOWN-1` | External crates cannot construct or mutate the catalog directly |
| Host graph preparation | `crates/prod/core/host/src/usecases.rs` | Canonical path APIs materialize runtime surfaces internally with `build_core_catalog()` + `core_registries()` | Advanced injection is intentionally exposed only through sibling `_with_surfaces` helpers |

### Visibility Constraints (CAT-LOCKDOWN-1 detail)

| Symbol | Visibility | Implication |
|--------|-----------|-------------|
| `CorePrimitiveCatalog::new()` | `pub(crate)` | External crates cannot construct a catalog |
| `CorePrimitiveCatalog::register_source()` | `pub(crate)` | External crates cannot add to a catalog |
| `CorePrimitiveCatalog::register_compute()` | `pub(crate)` | Same |
| `CorePrimitiveCatalog::register_trigger()` | `pub(crate)` | Same |
| `CorePrimitiveCatalog::register_action()` | `pub(crate)` | Same |
| `SourceRegistry::register()` | `pub` | Can be called externally |
| `ComputeRegistry::register()` | `pub` | Can be called externally |
| `TriggerRegistry::register()` | `pub` | Can be called externally |
| `ActionRegistry::register()` | `pub` | Can be called externally |

The registries are open. The catalog is locked. The builder must live inside the crate to call `pub(crate)` catalog methods.

### Downstream Chain

The `(CoreRegistries, CorePrimitiveCatalog)` pair flows downstream as:

1. `build_core()` → `(registries, catalog)` in `catalog.rs`
2. Both wrapped in `Arc<>` and passed to `RuntimeHandle::new()` in `crates/kernel/adapter/src/lib.rs`
3. `RuntimeHandle` passed to `HostedRunner::new()` in `crates/prod/core/host/src/runner.rs`
4. `crates/prod/core/host/src/usecases.rs` keeps `run_graph_from_paths()` and `replay_graph_from_paths()` on internal core materialization, while sibling `_with_surfaces` helpers feed prebuilt `RuntimeSurfaces` through the same preparation path

## Implemented Shape

1. `ergo_runtime::catalog::CatalogBuilder` starts from the current core implementation set.
2. The builder admits external `Source`, `Compute`, `Trigger`, and `Action` implementations before finalization.
3. Finalization produces a `CorePrimitiveCatalog` + `CoreRegistries` pair from one shared implementation inventory.
4. External implementations reuse the existing registry validation and duplicate-key rejection paths.
5. Canonical host path APIs keep internal core materialization; advanced prebuilt-surface use is exposed through explicit sibling `_with_surfaces` helpers.
6. Discovery/loading remains deferred to `feat/ergo-init` and `GW-EI8-1`.

## Closure Ledger

| ID | Task | Closure Condition | Owner | Status |
|----|------|-------------------|-------|--------|
| CB-1 | Design public builder API | `ergo_runtime::catalog` exposes `CatalogBuilder` (or equivalent) that starts with core implementations and allows additive registration by ontological role (`Source`, `Compute`, `Trigger`, `Action`) without exposing crate-private registration internals. | Codex + Claude | CLOSED |
| CB-2 | Preserve shared build path | Builder finalization constructs catalog + registries from one shared implementation inventory; `debug_assert_registry_catalog_key_parity()` runs after all extensions are added. Core-only helpers remain thin wrappers over that path or equivalent shared source. | Codex | CLOSED |
| CB-3 | Admit external implementations by kind | Builder accepts external `Box<dyn SourcePrimitive>`, `Box<dyn ComputePrimitive>`, `Box<dyn TriggerPrimitive>`, and `Box<dyn ActionPrimitive>` before finalization. | Codex | CLOSED |
| CB-4 | Reuse existing validation path | External implementations are admitted only through existing registry registration and existing catalog mapping. No trusted bypass. No alternate duplicate-resolution path. | Codex | CLOSED |
| CB-5 | Preserve `CAT-LOCKDOWN-1` | `CorePrimitiveCatalog::new()` and direct `register_*` methods remain `pub(crate)`; external code cannot mutate the catalog directly before or after build. | Codex | CLOSED |
| CB-6 | Host/runtime injection path | `crates/prod/core/host/src/usecases.rs` keeps `run_graph_from_paths()` and `replay_graph_from_paths()` as canonical APIs that delegate to internal core materialization, and exposes pre-built surface support through sibling `_with_surfaces` helpers that reuse the same internal preparation path. Host-level tests prove: injected surfaces are consumed on run, injected surfaces are consumed on replay, and the default core-only path still succeeds unchanged. | Codex | CLOSED |
| CB-7 | Test: core parity unchanged | `registry_catalog_key_parity`, `hello_world_graph_executes_with_core_catalog_and_registries`, and `supervisor_with_real_runtime_executes_hello_world` all pass after the builder is introduced. | Codex | CLOSED |
| CB-8 | Test: external implementation registered and executed | End-to-end test registers a test implementation via the builder, validates a graph that references it, executes it, and verifies output. | Codex | CLOSED |
| CB-9 | Test: invalid manifest rejected | Builder rejects an implementation whose manifest fails existing validation with the existing typed error/rule mapping. | Codex | CLOSED |
| CB-10 | Test: duplicate ID rejected | Builder rejects an external implementation whose `id` collides with an existing implementation in the same ontological role/registry, regardless of version, whether the existing implementation is core or previously-added external. This mirrors `SRC-14`, `CMP-18`, `TRG-13`, and `ACT-18`. | Codex | CLOSED |
| CB-11 | Downstream contract documented | This ledger file explicitly states that this branch covers registration only; discovery, loading, and workspace UX ownership remain with `feat/ergo-init` and `GW-EI8-1`. The ledger is the closure authority for this branch; README updates are secondary and not required for closure. | Codex | CLOSED |

## Design Constraints

- The builder lives in the runtime crate because it composes existing registry registration and catalog metadata mapping.
- Existing convenience helpers (`build_core()`, `build_core_catalog()`, `core_registries()`) must stay behaviorally aligned with the builder path.
- External implementations must go through the same manifest validation as core implementations. No "trusted plugin" fast path.
- This branch does not choose or implement a loading mechanism. No CLI flags, path scanning, crate compilation, dynamic loading, or WASM loading.
- `feat/ergo-init` remains the owner of workspace discovery/loading once `GW-EI8-1` is resolved.

## What This Branch Enables

After `feat/catalog-builder` merges, downstream prod-layer code can assemble runtime surfaces from:

1. The built-in core stdlib
2. Additional externally-authored implementations

That gives `feat/ergo-init` a stable registration target while keeping plugin discovery/loading as a separate, explicitly gated concern.
