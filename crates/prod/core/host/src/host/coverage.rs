//! host::coverage
//!
//! Purpose:
//! - Enforce host ownership coverage for graph-emittable effect kinds accepted
//!   by the adapter contract.
//!
//! Owns:
//! - `ensure_handler_coverage(...)` and the typed
//!   `HandlerCoverageError` failure surface.
//!
//! Does not own:
//! - Adapter acceptance semantics; it consumes an already-materialized
//!   `AdapterProvides`.
//! - Egress routing config parsing or handler implementation details.
//!
//! Connects to:
//! - `runner.rs` and `egress/validation.rs`, which use this as the canonical
//!   HST-5 ownership gate.
//!
//! Safety notes:
//! - Coverage is checked only for graph-emittable kinds that the adapter
//!   contract accepts.

use std::collections::{BTreeSet, HashSet};

use ergo_adapter::AdapterProvides;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandlerCoverageError {
    MissingHandler { effect_kind: String },
    ConflictingCoverage { effect_kind: String },
}

impl std::fmt::Display for HandlerCoverageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingHandler { effect_kind } => {
                write!(f, "missing effect handler for kind '{effect_kind}'")
            }
            Self::ConflictingCoverage { effect_kind } => write!(
                f,
                "ambiguous coverage for kind '{effect_kind}': claimed by both handler and egress"
            ),
        }
    }
}

impl std::error::Error for HandlerCoverageError {}

pub fn ensure_handler_coverage(
    provides: &AdapterProvides,
    graph_emittable_effect_kinds: &HashSet<String>,
    registered_handler_kinds: &BTreeSet<String>,
    egress_claimed_kinds: &HashSet<String>,
) -> Result<(), HandlerCoverageError> {
    for effect_kind in graph_emittable_effect_kinds {
        if !provides.effects.contains(effect_kind) {
            continue;
        }

        let has_handler = registered_handler_kinds.contains(effect_kind);
        let has_egress = egress_claimed_kinds.contains(effect_kind);

        if has_handler && has_egress {
            return Err(HandlerCoverageError::ConflictingCoverage {
                effect_kind: effect_kind.clone(),
            });
        }

        if !has_handler && !has_egress {
            return Err(HandlerCoverageError::MissingHandler {
                effect_kind: effect_kind.clone(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;
