---
Authority: PROJECT
Date: 2026-03-16
Decision-Owner: Sebastian (Architect)
Recorder: Claude Code (Implementation Assistant)
Status: DECIDED
Decides: GW-EI8-1
Unblocks: feat/ergo-init (EI-8)
---

# Decision: Custom Implementation Loading Mechanism

## Ruling

The v1 mechanism for loading custom implementations is **in-process
Rust crate compilation**. The user implements traits from `ergo-runtime`
(`SourcePrimitive`, `ComputePrimitive`, `TriggerPrimitive`,
`ActionPrimitive`) in a Rust crate that compiles and links against the
same `ergo-runtime` dependency as the engine.

Custom implementations enter the runtime through `CatalogBuilder`,
which already exists and already enforces the full registration
validation pipeline identically for custom and stdlib primitives.

## Rationale

### Same trait surface as stdlib

User code implements the same traits as stdlib primitives. `AckAction`,
`NumberSource`, `Add`, `EmitIfTrue` — all use the identical trait
interface. No adapter layer, no serialization boundary, no separate
API surface.

### All validation rules fire identically

`CatalogBuilder::build()` runs the same registration pipeline for
custom code as for stdlib. ACT-1 through ACT-33, COMP-\* rules,
manifest validation, duplicate ID rejection — all enforced. No bypass
path exists. This is already proven by existing tests:

- `catalog_builder_admits_external_implementations_by_kind`
- `catalog_builder_rejects_invalid_manifest_via_existing_validation`
- `catalog_builder_rejects_duplicate_core_id_even_with_new_version`

### No ABI fragility

Rust has no stable ABI. Dynamic libraries break across compiler
versions — vtable layout for `dyn ComputePrimitive` is not guaranteed
stable. In-process compilation means the user's crate compiles against
the same `ergo-runtime` dependency as the engine. Type safety is
compile-time, not runtime.

### No sandbox overhead

WASM adds a runtime layer (wasmtime/wasmer), limits what user code can
do (no filesystem, no network without host functions), and introduces a
value-marshaling boundary between WASM and native on every `compute()`
or `execute()` call. For v1, the user's code runs in the same process
as the engine.

### Determinism is enforced by trait contracts, not by sandboxing

`ComputePrimitive::compute()` must be deterministic given identical
inputs. `ActionPrimitive::execute()` returns outputs only — side
effects are manifest-declared, not implementation-emitted. The
execution model enforces correctness, not the loading mechanism.

## Rejected Alternatives

### Dynamic library loading (Option 2)

Platform-specific (`.so` vs `.dylib` vs `.dll`). ABI-fragile across
Rust compiler versions. `#[repr(C)]` or vtable stability not
guaranteed. Would require a C FFI layer around the trait surface,
adding complexity without benefit.

### WASM-based loading (Option 3)

Adds wasmtime/wasmer dependency. Value marshaling overhead on every
`compute()` / `execute()` call. User code cannot use Rust's full
ecosystem (no `std::fs`, no `reqwest`, no `tokio` without host function
plumbing). Sandboxing is a security feature Ergo does not need in v1 —
the user is running their own code on their own machine.

## Explicit Non-Decisions

This decision selects the loading mechanism only. It does not decide:

- **Build orchestration.** How `ergo run` triggers `cargo build` on the
  user's crate, finds the compiled artifact, and links it. That is
  `feat/ergo-init` implementation scope (EI-8).
- **Workspace layout for implementations.** The `implementations/`
  directory convention is defined in `ergo-init.md`, not here.
- **Hot reloading.** Not in scope. User rebuilds and reruns.
- **Plugin versioning.** The user's crate depends on a specific
  `ergo-runtime` version. Compatibility is Cargo's job.
- **Future WASM support.** This decision does not prevent adding WASM
  loading later as an alternative mechanism. It selects in-process Rust
  as the v1 mechanism.

## Security Stance

User code runs in-process with the same permissions as the Ergo
process. This is acceptable for v1 because:

- The user is running their own code on their own machine.
- Ergo is not a multi-tenant platform.
- The trait contracts enforce determinism and manifest-declared effects.
- If sandboxing is needed later (multi-tenant, untrusted code), WASM
  can be added as a v2 loading mechanism alongside in-process Rust.

## Required Tests

### Already tested (CatalogBuilder integration)

Custom implementations register through the same validation pipeline as
stdlib. Tests in `catalog.rs` cover:

- Registration alongside core (`catalog_builder_admits_external_implementations_by_kind`)
- Manifest validation rejection (`catalog_builder_rejects_invalid_manifest_via_existing_validation`)
- Duplicate ID rejection (`catalog_builder_rejects_duplicate_core_id_even_with_new_version`)

These prove the integration point works.

### Deferred to EI-8 (build orchestration)

- Crate discovery from `implementations/`
- `cargo build` invocation
- Compiled artifact linking into `CatalogBuilder`

These are implementation-mechanism tests, not loading-decision tests.

## Impacted Ledger Files

- [custom-implementation-loading.md](../gap-work/closed/custom-implementation-loading.md)

## Follow-Up Actions

1. Move GW-EI8-1 into the closed gap lane and reference this decision.
2. Proceed with EI-8 implementation in `feat/ergo-init` using
   in-process Rust crate registration against `CatalogBuilder`.
