use std::collections::{BTreeSet, HashSet};

use crate::AdapterProvides;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandlerCoverageError {
    MissingHandler { effect_kind: String },
}

impl std::fmt::Display for HandlerCoverageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingHandler { effect_kind } => {
                write!(f, "missing effect handler for kind '{effect_kind}'")
            }
        }
    }
}

impl std::error::Error for HandlerCoverageError {}

pub fn ensure_handler_coverage(
    provides: &AdapterProvides,
    graph_emittable_effect_kinds: &HashSet<String>,
    registered_handler_kinds: &BTreeSet<String>,
) -> Result<(), HandlerCoverageError> {
    for effect_kind in graph_emittable_effect_kinds {
        if !provides.effects.contains(effect_kind) {
            continue;
        }

        if !registered_handler_kinds.contains(effect_kind) {
            return Err(HandlerCoverageError::MissingHandler {
                effect_kind: effect_kind.clone(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_only_checks_graph_emittable_intersection() {
        let mut provides = AdapterProvides::default();
        provides.effects.insert("set_context".to_string());
        provides.effects.insert("send_notification".to_string());

        let graph_emittable = HashSet::from(["set_context".to_string()]);
        let handlers = BTreeSet::from(["set_context".to_string()]);

        let result = ensure_handler_coverage(&provides, &graph_emittable, &handlers);
        assert!(result.is_ok());
    }

    #[test]
    fn coverage_fails_when_graph_emittable_accepted_kind_has_no_handler() {
        let mut provides = AdapterProvides::default();
        provides.effects.insert("set_context".to_string());

        let graph_emittable = HashSet::from(["set_context".to_string()]);
        let handlers = BTreeSet::new();

        let err = ensure_handler_coverage(&provides, &graph_emittable, &handlers)
            .expect_err("missing handler must fail coverage");
        assert_eq!(
            err,
            HandlerCoverageError::MissingHandler {
                effect_kind: "set_context".to_string()
            }
        );
    }

    #[test]
    fn non_accepted_graph_kind_is_not_coverage_obligation() {
        let mut provides = AdapterProvides::default();
        provides.effects.insert("set_context".to_string());

        let graph_emittable = HashSet::from(["send_notification".to_string()]);
        let handlers = BTreeSet::new();

        let result = ensure_handler_coverage(&provides, &graph_emittable, &handlers);
        assert!(result.is_ok());
    }
}
