//! egress::validation
//!
//! Purpose:
//! - Validate host-owned egress routing configuration against adapter acceptance,
//!   graph-emittable effect kinds, and handler ownership before live run setup
//!   proceeds.
//!
//! Owns:
//! - Rejection when a routed kind is not accepted by the adapter contract.
//! - Delegation to host ownership coverage checks for graph-emittable kinds the
//!   adapter accepts so each one is owned by either a handler or an egress route.
//! - Warnings for configured routes that the current graph cannot emit.
//!
//! Does not own:
//! - Parsing or provenance for `EgressConfig`.
//! - Egress process startup/runtime protocol checks.
//! - Adapter semantics beyond consuming the already-materialized `AdapterProvides`
//!   acceptance surface.
//!
//! Connects to:
//! - `runner::validate_hosted_runner_configuration(...)`, which uses this module as
//!   the canonical live-egress validation seam.
//! - `ergo_adapter::host::ensure_handler_coverage(...)` for HST-5 ownership checks.
//!
//! Safety notes:
//! - Warning order is deterministic because `EgressConfig.routes` is a `BTreeMap`.
//! - Coverage still includes handler-owned kinds such as `set_context`; routing one
//!   of those kinds through egress conflicts with host-owned handler coverage.
//! - Non-emittable routes are warnings, not errors; they describe dead config for
//!   the current graph but do not prevent live setup.
//! - `EgressConfig` is already validated for intrinsic route/channel integrity in
//!   `config.rs`; this module only evaluates graph/adapter/handler context.

use std::collections::{BTreeSet, HashSet};

use ergo_adapter::host::{ensure_handler_coverage, HandlerCoverageError};
use ergo_adapter::AdapterProvides;

use super::EgressConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EgressValidationWarning {
    RouteForNonEmittableKind { intent_kind: String },
}

impl std::fmt::Display for EgressValidationWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RouteForNonEmittableKind { intent_kind } => write!(
                f,
                "egress route declared for non-emittable kind '{intent_kind}'"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EgressValidationError {
    RoutedKindNotAcceptedByAdapter { intent_kind: String },
    Coverage(HandlerCoverageError),
}

impl std::fmt::Display for EgressValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RoutedKindNotAcceptedByAdapter { intent_kind } => write!(
                f,
                "egress route kind '{intent_kind}' is not accepted by adapter (accepts.effects)"
            ),
            Self::Coverage(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for EgressValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Coverage(err) => Some(err),
            _ => None,
        }
    }
}

impl From<HandlerCoverageError> for EgressValidationError {
    fn from(value: HandlerCoverageError) -> Self {
        Self::Coverage(value)
    }
}

pub fn validate_egress_config(
    config: &EgressConfig,
    adapter_provides: &AdapterProvides,
    graph_emittable_effect_kinds: &HashSet<String>,
    registered_handler_kinds: &BTreeSet<String>,
) -> Result<Vec<EgressValidationWarning>, EgressValidationError> {
    let routed_kinds: HashSet<String> = config.routes().keys().cloned().collect();

    for intent_kind in config.routes().keys() {
        if !adapter_provides.effects.contains(intent_kind) {
            return Err(EgressValidationError::RoutedKindNotAcceptedByAdapter {
                intent_kind: intent_kind.clone(),
            });
        }
    }

    ensure_handler_coverage(
        adapter_provides,
        graph_emittable_effect_kinds,
        registered_handler_kinds,
        &routed_kinds,
    )?;

    let mut warnings = Vec::new();
    for intent_kind in config.routes().keys() {
        if !graph_emittable_effect_kinds.contains(intent_kind) {
            warnings.push(EgressValidationWarning::RouteForNonEmittableKind {
                intent_kind: intent_kind.clone(),
            });
        }
    }

    Ok(warnings)
}

#[cfg(test)]
mod tests;
