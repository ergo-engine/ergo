//! host
//!
//! Purpose:
//! - Group the host-owned support types that implement the canonical effect
//!   loop, context storage, and host-side runtime buffering used by
//!   `HostedRunner`.
//!
//! Owns:
//! - `BufferingRuntimeInvoker`, `ContextStore`, handler coverage checks, and
//!   the host-owned effect-handler types.
//!
//! Does not own:
//! - Kernel runtime invocation contracts from `ergo_adapter`.
//! - Hosted-runner orchestration, replay, or top-level public error shaping.
//!
//! Connects to:
//! - `runner.rs`, which is the dominant consumer of these support types.
//! - `error.rs` and `egress/validation.rs`, which expose the typed failure
//!   surfaces produced here.
//!
//! Safety notes:
//! - This module is host-owned regardless of its former adapter-crate location.
//! - `RuntimeResultProvider` remains private to `buffering_invoker.rs`; it is a
//!   host testability seam, not a public contract.

mod buffering_invoker;
mod context_store;
mod coverage;
mod effects;

pub use buffering_invoker::BufferingRuntimeInvoker;
pub use context_store::ContextStore;
pub use coverage::{ensure_handler_coverage, HandlerCoverageError};
pub use effects::{AppliedWrite, EffectApplyError, EffectHandler, SetContextHandler};
