---
Authority: CONTRACTS
Version: v1
Last Updated: 2026-03-26
Owner: Documentation
Scope: Current extension-contract roadmap and remaining contract gaps
Change Rule: Review required
---

# Extension Contracts Roadmap

The extension contract surface is no longer an alpha sketch. The current v1
contracts are implemented and live in the stable/canonical docs.

This file tracks what remains intentionally deferred so we do not confuse a
historical design artifact with current product truth.

## Current Contract Sources

Use these as the authoritative current contract set:

- `docs/primitives/adapter.md`
- `docs/primitives/source.md`
- `docs/primitives/compute.md`
- `docs/primitives/trigger.md`
- `docs/primitives/action.md`
- `docs/authoring/cluster-spec.md`
- `docs/authoring/yaml-format.md`
- `docs/invariants/`

The host/SDK product boundary is documented separately in:

- `docs/system/current-architecture.md`
- `docs/system/kernel-prod-separation.md`
- `docs/authoring/project-convention.md`

## Shipped Today

The current implementation already provides:

- manifest registration validation for adapters and all four primitive roles
- adapter composition validation for source requirements, capture format, writes,
  mirror writes, and external intent schemas
- deterministic kernel expansion, validation, execution, capture, and replay
- SDK-first project resolution across filesystem and in-memory project lanes
- host-owned canonical run, replay, validation, and manual-runner orchestration

## Deferred Contract Work

The remaining intentionally deferred items are:

- **Adapter capture-completeness rules**
  `ADP-15`, `ADP-16`, and `COMP-15` remain deferred. Current replay already
  validates host-owned effect integrity, but adapter manifests do not yet
  canonically declare full context/effect capture coverage.
- **Future semantic routing or policy by semantic event kind**
  Semantic event kinds are already open-world strings at the adapter boundary,
  but supervisor policy still operates on transport `ExternalEventKind`.
- **Any transport/plugin widening beyond in-process Rust registration**
  v1 uses in-process primitive registration through the SDK/runtime catalogs.
  Dynamic plugin loading is not part of the shipped contract.

## Change Discipline

- Update the stable/canonical contract docs first.
- Update this roadmap only after the shipped contract set changes or a deferred
  item is explicitly opened.
- If this file and the stable/canonical docs disagree, the stable/canonical docs
  win.

## Historical Note

Older docs and comments may still refer to “Phase 1” through “Phase 5” of the
extension-contract buildout. Those phases are historical delivery language, not
current authority boundaries.
