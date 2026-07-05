//! ergo_runtime
//!
//! Purpose:
//! - Define the kernel-owned primitive ontology and the graph pipeline that
//!   turns a `ClusterDefinition` into an `ExecutionReport` (expand → validate
//!   → execute).
//!
//! Most users should start with `ergo-sdk` for embedded Rust usage or
//! `ergo-cli` for command-line usage. Depend on this crate directly only when you
//! are building lower-level kernel integrations, custom primitive registration
//! surfaces, or runtime-focused tooling.
//!
//! Owns:
//! - The four primitive trait families (source/compute/trigger/action) and
//!   their manifest types.
//! - `CorePrimitiveCatalog`, `CoreRegistries`, and the stdlib primitive
//!   implementations registered through `build_core`.
//! - Cluster expansion (`cluster::expand`), validation (`runtime::validate`),
//!   and synchronous execution (`runtime::execute_with_metadata`).
//! - Typed runtime errors (`RuntimeError`, `ExecError`, `ValidationError`).
//!
//! Does not own:
//! - The adapter contract, event binding, or runtime invoker handles
//!   (owned by `ergo_adapter`).
//! - Episode scheduling, capture, or replay (owned by `ergo_supervisor`).
//! - Host orchestration, I/O realization, or product-facing error shaping.
//!
//! Connects to:
//! - `ergo_adapter`, which consumes catalogs/registries and re-exports
//!   `ExecutionContext` through its wrapper without redefining runtime
//!   meaning.
//! - `ergo_supervisor`, which drives `execute_with_metadata` through a
//!   `RuntimeInvoker` and records decisions independently of runtime state.
//!
//! Safety notes:
//! - The runtime is synchronous and single-threaded by construction. No
//!   concurrency primitive appears anywhere in this crate; adding one is a
//!   semantic change to the kernel threading model.
//! - `CorePrimitiveCatalog` and `CoreRegistries` are build-once via
//!   `build_core` / `CatalogBuilder` and have no mutation API after
//!   construction; the `pub(crate) fn register_*` mutators are reachable
//!   only from `build_from_inventory`.
//! - Three of the four primitive traits intentionally omit `Send + Sync`
//!   at v1; tightening those bounds is tracked in
//!   `docs/ledger/decisions/sdk-threading-send-sync.md` and would propagate
//!   structurally through `CoreRegistries`, `RuntimeState`, and every
//!   `*RuntimeHandle`.
//! - Primitive `compute` accepts `Option<&mut PrimitiveState>` but the
//!   executor always passes `None`; statefulness is detected by
//!   capture/replay divergence rather than structural enforcement.

pub const RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn runtime_version() -> &'static str {
    RUNTIME_VERSION
}

pub mod action;
pub mod catalog;
pub mod cluster;
pub mod common;
pub mod compute;
pub mod provenance;
pub mod runtime;
pub mod source;
pub mod trigger;
