---
Authority: PROJECT
Date: 2026-03-04
Author: Claude Opus 4.5 (Structural Auditor)
Status: OPEN
Branch: feat/adapter-runtime
Tier: 2 (Extension Plumbing)
Depends-On: none (can parallel with Tier 1)
---

# Adapter Runtime Contract

## Scope

Define a runtime integration surface where user-authored adapters connect to the host. Currently the adapter manifest (YAML) declares what an adapter provides and accepts, and the kernel validates it (ADP-*, COMP-*). But there is no code surface where a user implements the actual runtime behavior — event production, context provision, effect handling.

The host (`HostedRunner`) currently performs all adapter functions internally. This branch extracts those functions into a trait (or equivalent protocol) that external code can implement.

This is entirely prod-layer work. No kernel changes. No frozen doc changes.

## Current State

| Adapter Function | Who Does It Now | Where |
|-----------------|----------------|-------|
| Event production | Fixtures or test code | `shared/fixtures/`, test harnesses |
| Event binding (raw → ExternalEvent) | Host | `runner.rs::build_external_event()` |
| Context provision (external state → payload) | Host merges ContextStore | `runner.rs::build_external_event()` |
| Effect handling (set_context) | `SetContextHandler` | `adapter/src/host/effects.rs` |
| Effect application to store | Host | `runner.rs::execute_step()` |
| Manifest validation | Kernel | `adapter/src/validate.rs` |
| Composition validation | Kernel | `adapter/src/composition.rs` |

Manifest validation and composition validation stay in the kernel. They are not part of this branch.

## What's Needed

A trait or protocol that separates adapter authorship from host orchestration. The adapter author provides:

1. **Event source** — produces raw events (data ingress)
2. **Custom effect handlers** — handles effects beyond `set_context` (effect egress)

The host retains ownership of:

1. Event binding (applying the adapter's schema to raw events)
2. Context store management (merging store into payloads)
3. Composition validation (COMP-* checks)
4. Capture and replay

## Closure Ledger

| ID | Task | Closure Condition | Owner | Status |
|----|------|-------------------|-------|--------|
| AR-1 | Design adapter trait / protocol | API signature reviewed. Must separate event production and effect handling from host orchestration. Must not require kernel changes. | Codex + Claude | OPEN |
| AR-2 | Implement adapter trait | Trait compiles. Defines event production and effect handler registration surfaces. | Codex | OPEN |
| AR-3 | Implement fixture-backed simulation adapter | A built-in adapter that reads fixture files but runs through the full host pipeline (context merging, effect handling, composition validation). Closes fixture/adapter parity gap. | Codex | OPEN |
| AR-4 | Host runner consumes adapter trait | `HostedRunner` (or wrapper) accepts an adapter implementation at construction. Falls back to current behavior when no adapter provided. | Codex | OPEN |
| AR-5 | Custom effect handler registration | Adapter can register `EffectHandler` implementations beyond `SetContextHandler`. Host routes effects to registered handlers by kind. | Codex | OPEN |
| AR-6 | Manifest ↔ runtime contract alignment | Adapter manifest declarations (provides, accepts) are validated against what the runtime adapter actually implements. Mismatch fails at construction, not at runtime. | Codex | OPEN |
| AR-7 | Test: simulation adapter runs fixture through full host path | Same graph + same events produce identical capture bundles whether run through simulation adapter or future real adapter. | Codex | OPEN |
| AR-8 | Test: custom effect handler receives effects | Adapter registers a custom handler. Graph produces an effect of that kind. Handler receives it. | Codex | OPEN |
| AR-9 | Test: manifest/runtime mismatch rejected | Adapter manifest declares `set_context` but runtime adapter doesn't register a handler → construction fails. | Codex | OPEN |

## Design Constraints

- The adapter trait lives in `crates/prod/`. It is not a kernel concern.
- The kernel's `AdapterProvides`, `AdapterManifest`, `EffectHandler`, and composition validation are consumed by the adapter surface, not replaced by it.
- The simulation adapter (AR-3) is the reference implementation. All other adapters are user-authored in ergo workspaces after `feat/ergo-init`.
- No domain-specific language in this branch. "Event source" and "effect handler" are the terms. Not "market data feed" or "order router."

## Relationship to Existing Gaps

This branch closes:
- **Gap 2** (No adapter runtime surface)
- **Gap 5** (Fixture/adapter parity) — via the simulation adapter (AR-3)
