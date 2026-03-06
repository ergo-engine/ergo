---
Authority: PROJECT
Date: 2026-03-04
Author: Claude Opus 4.5 (Structural Auditor)
Status: OPEN
Branch: feat/catalog-builder
Tier: 2 (Extension Plumbing)
Depends-On: none (can parallel with Tier 1)
---

# Pluggable Implementation Registration

## Scope

Make it possible for code outside the kernel to register implementations with the catalog and registries at startup. Currently `catalog.rs` builds from hardcoded lists. This branch adds a builder API that accepts external implementations while preserving all existing manifest validation (SRC-*, CMP-*, TRG-*, ACT-*).

No frozen doc changes. No trait changes. No validation rule changes. The builder is additive — it composes the existing registration and validation machinery into a public API.

## Current State

| What | Where | Problem |
|------|-------|---------|
| `core_source_primitives()` | `catalog.rs` | Returns `Vec<Box<dyn SourcePrimitive>>` — fixed list |
| `core_compute_primitives()` | `catalog.rs` | Same — fixed list |
| `core_trigger_primitives()` | `catalog.rs` | Same — fixed list |
| `core_action_primitives()` | `catalog.rs` | Same — fixed list |
| `build_core_catalog()` | `catalog.rs` | Builds `CorePrimitiveCatalog` from the above — no external input |
| `core_registries()` | `catalog.rs` | Builds `CoreRegistries` from the above — no external input |
| REG-SYNC-1 | closure register | Catalog and registries must be built from shared source. Any builder must preserve this. |
| CAT-LOCKDOWN-1 | closure register | Registration APIs are `pub(crate)`. External crates cannot construct or mutate catalog directly. |

## What's Needed

A public API that:

1. Starts with the core stdlib implementations (the existing hardcoded set)
2. Accepts additional implementations from external code
3. Validates each external implementation's manifest through the existing registry validation (SRC-*, CMP-*, TRG-*, ACT-*)
4. Produces a `CorePrimitiveCatalog` + `CoreRegistries` pair that includes both core and external implementations
5. Preserves REG-SYNC-1 (shared build path) and CAT-SYNC-1 (parity) guarantees

## Closure Ledger

| ID | Task | Closure Condition | Owner | Status |
|----|------|-------------------|-------|--------|
| CB-1 | Design builder API | API signature reviewed and approved. Must not expose `pub(crate)` internals. Must preserve REG-SYNC-1. | Codex + Claude | OPEN |
| CB-2 | Implement builder in kernel | `CatalogBuilder` (or equivalent) compiles. Accepts `Box<dyn SourcePrimitive>` etc. Returns `Result<(CorePrimitiveCatalog, CoreRegistries), RegistrationError>`. | Codex | OPEN |
| CB-3 | External manifest validation | External implementations go through the same `validate_manifest()` path as core implementations. No bypass. | Codex | OPEN |
| CB-4 | CAT-LOCKDOWN-1 preserved | Direct catalog construction remains `pub(crate)`. Builder is the only public path for external registration. | Codex | OPEN |
| CB-5 | REG-SYNC-1 preserved | Builder feeds both catalog and registries from the same implementation list. `registry_catalog_key_parity` test passes. | Codex | OPEN |
| CB-6 | Host integration | `HostedRunner::new()` (or a wrapper) accepts externally-built catalog + registries. | Codex | OPEN |
| CB-7 | CLI integration | CLI `run` command accepts a flag or config for external implementation paths/modules. Design TBD. | Codex | OPEN |
| CB-8 | Test: external implementation registered and executed | End-to-end: register a test implementation via builder, build graph referencing it, execute, verify output. | Codex | OPEN |
| CB-9 | Test: external implementation with invalid manifest rejected | Builder rejects an implementation whose manifest fails validation (e.g., SRC-4 violation). | Codex | OPEN |
| CB-10 | Test: duplicate ID rejected | Builder rejects external implementation with same ID as a core implementation (SRC-14, CMP-18, TRG-13, ACT-18). | Codex | OPEN |

## Design Constraints

- CAT-LOCKDOWN-1 must not be weakened. The builder is the controlled admission path.
- External implementations undergo identical validation to core implementations. No "trusted" bypass.
- The builder does not own plugin discovery (directory scanning, WASM loading, etc.). That's a prod-layer concern for `feat/ergo-init` or a later branch. The builder just takes implementations and validates them.
