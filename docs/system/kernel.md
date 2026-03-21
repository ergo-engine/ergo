---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-21
Owner: Claude (Structural Auditor)
Scope: v0 baseline declaration, v1 workstream rules
Change Rule: Operational log
---

# Kernel Closure and v1 Workstream Declaration

**Status:** The v0 kernel is closed. Work may continue, but the kernel's meaning is now a fixed reference point.

---

## What "Kernel" Means

Kernel = runtime + supervisor + adapter + contracts that define:

- Execution semantics (ExpandedGraph → validate → execute)
- Primitive ontology (Source / Compute / Trigger / Action)
- Determinism + replay integrity posture
- Core stdlib atoms (domain-neutral)
- Contract surfaces used by clients (UI contract, capture/replay format)

---

## What "Closed" Means

Closed ≠ abandoned. Closed = semantics-stable reference point.

- We may patch bugs that violate already-declared invariants.
- We may not "improve behavior" by quietly changing meanings.
- Any new semantics obligations require an explicit v1 decision record.

---

## Crate Boundaries

The kernel closure is physically enforced by workspace layout and CI boundary guards.

### Workspace Layout

```
crates/
  kernel/
    runtime/          # Execution semantics, catalog, stdlib
    adapter/          # Trust boundary, capture, replay types
    supervisor/       # Episode scheduling, decision log
  prod/
    core/
      host/           # Canonical host loop, adapter composition, usecase API
      loader/         # YAML decode, file discovery, cluster tree loading
    clients/
      cli/            # Thin CLI adapter over loader + host
      sdk-rust/       # Rust SDK adapter over host
      sdk-types/      # Shared SDK interface types
  shared/
    test-support/     # Test utilities (dev-dependency only)
    fixtures/         # Fixture data (dev-dependency only)
```

### Boundary Rules

| Rule | Constraint | Enforcement |
|------|-----------|-------------|
| LAYER-1 | Kernel crates must not depend on `prod/*` or `shared/*` at runtime | `tools/verify_layer_boundaries.sh` |
| LAYER-2 | `RuleViolation` is kernel-owned; loader and clients must not define or return rule violations | `tools/verify_layer_boundaries.sh` |
| LAYER-3 | Clients must not import loader/parser internals | `tools/verify_layer_boundaries.sh` |
| LAYER-DEV | `shared/*` is allowed in kernel `[dev-dependencies]` only | `tools/verify_layer_boundaries.sh` |

### Loader / Kernel Split

The catalog-access boundary (yaml-format.md §8.3) defines the loader/kernel divide:

- **Loader** (`prod/core/loader`): transport + decode + discovery. Operates without catalog. Produces `ClusterDefinition`.
- **Kernel** (`kernel/*`): semantic enforcement. Requires catalog. Owns expansion, validation, execution, and all `RuleViolation` types.

### Host Responsibility

The host (`prod/core/host`) owns:

- Adapter dependency scan, composition validation, and live egress/handler validation
- Usecase API surface: `run_graph_from_paths`, `replay_graph_from_paths`, `validate_graph_from_paths`, `prepare_hosted_runner_from_paths`, `finalize_hosted_runner_capture` (canonical client entrypoints); `run_graph`, `replay_graph`, `run_fixture` (lower-level)
- Canonical run ingress selection through host-owned `DriverConfig`
  (current implementation term for ingress-channel configuration);
  replay remains capture-driven and takes no live channel config
- Post-episode effect dispatch at host boundary (HST-1 through HST-9);
  host-internal effects may be realized locally while true external I/O
  belongs to prod boundary channels
- Hosted-runner finalization ordering and lifecycle truth for manual stepping (`ensure_no_pending_egress_acks` → `stop_egress_channels` → `CaptureBundle`, with zero-step/non-finalizable rejection before finalization)
- Canonical run outcome reporting for product callers (`Completed` vs `Interrupted` when the host can return a trustworthy artifact)
- Canonical composition of loader + kernel semantics for product entrypoints, including validation and manual-stepping surfaces; kernel remains semantic authority

See [Kernel/Prod Separation and Host Intent](kernel-prod-separation.md).

---

## Two Parallel Tracks Going Forward

### 1. v0 Kernel (Closed / Stable Reference)

**Changes allowed:**

- Invariant enforcement fixes
- Clarifications
- Bug fixes consistent with doctrine

**Changes not allowed:**

- Meaning changes
- New coercions
- New hidden defaults
- Domain-loaded naming

### 2. v1 Workstream (Exploration / Expansion)

New semantics are allowed, but must be:

- Explicitly specified
- Phase-bounded (where enforced)
- Regression-tested
- Tagged/recorded as new obligations

---

## Compatibility Posture

- Backward compatibility is required only for explicitly versioned persisted formats (e.g., capture bundles).
- Renames affecting persisted formats require a compatibility plan (serde alias + legacy test).

---

## Reference Clients

The reference client (`crates/reference-client`) has been removed from the workspace. Its invariant (UI-REF-CLIENT-1: UI authoring is non-canonical) is documented in invariants/08-replay.md. Future reference clients must follow the same rules:

- They must reflect contracts accurately
- They must not define runtime semantics
- Any "helpful UI behavior" must be explicit and versioned/documented as UI-only

---

## Release/Tag Discipline

Every kernel-changing merge must correspond to:

- Updated invariants/closure entries (if applicable)
- A tag that anchors the reference point when appropriate

If a change would confuse a new contributor reading the docs, it needs a doctrine update or a separate PR.

---

## v1 Workstream Log

Tracks semantic changes that exceed v0 scope.

| Item | PR  | Tag            | Description                                                                                                                  |
|------|-----|----------------|------------------------------------------------------------------------------------------------------------------------------|
| B.2  | #35 | v1.0.0-alpha.1 | Divide-by-zero semantics: strict divide errors, safe_divide with fallback, NUM-FINITE-1 guard, SemanticError classification |

---

## Declaration

**v0 kernel is now the baseline. v1 begins as a workstream, not a public promise.**
