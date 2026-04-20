use super::*;

#[test]
fn coverage_only_checks_graph_emittable_intersection() {
    let mut provides = AdapterProvides::default();
    provides.effects.insert("set_context".to_string());
    provides.effects.insert("send_notification".to_string());

    let graph_emittable = HashSet::from(["set_context".to_string()]);
    let handlers = BTreeSet::from(["set_context".to_string()]);
    let egress = HashSet::new();

    let result = ensure_handler_coverage(&provides, &graph_emittable, &handlers, &egress);
    assert!(result.is_ok());
}

#[test]
fn coverage_fails_when_graph_emittable_accepted_kind_has_no_handler() {
    let mut provides = AdapterProvides::default();
    provides.effects.insert("set_context".to_string());

    let graph_emittable = HashSet::from(["set_context".to_string()]);
    let handlers = BTreeSet::new();
    let egress = HashSet::new();

    let err = ensure_handler_coverage(&provides, &graph_emittable, &handlers, &egress)
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
    let egress = HashSet::new();

    let result = ensure_handler_coverage(&provides, &graph_emittable, &handlers, &egress);
    assert!(result.is_ok());
}

#[test]
fn egress_claimed_kind_satisfies_coverage_without_handler() {
    let mut provides = AdapterProvides::default();
    provides.effects.insert("place_order".to_string());

    let graph_emittable = HashSet::from(["place_order".to_string()]);
    let handlers = BTreeSet::new();
    let egress = HashSet::from(["place_order".to_string()]);

    let result = ensure_handler_coverage(&provides, &graph_emittable, &handlers, &egress);
    assert!(result.is_ok());
}

#[test]
fn coverage_fails_when_kind_is_neither_handler_nor_egress_claimed() {
    let mut provides = AdapterProvides::default();
    provides.effects.insert("place_order".to_string());

    let graph_emittable = HashSet::from(["place_order".to_string()]);
    let handlers = BTreeSet::new();
    let egress = HashSet::new();

    let err = ensure_handler_coverage(&provides, &graph_emittable, &handlers, &egress)
        .expect_err("uncovered kind must fail coverage");
    assert_eq!(
        err,
        HandlerCoverageError::MissingHandler {
            effect_kind: "place_order".to_string()
        }
    );
}

#[test]
fn coverage_fails_when_handler_and_egress_both_claim_same_kind() {
    let mut provides = AdapterProvides::default();
    provides.effects.insert("set_context".to_string());

    let graph_emittable = HashSet::from(["set_context".to_string()]);
    let handlers = BTreeSet::from(["set_context".to_string()]);
    let egress = HashSet::from(["set_context".to_string()]);

    let err = ensure_handler_coverage(&provides, &graph_emittable, &handlers, &egress)
        .expect_err("duplicate ownership must fail coverage");
    assert_eq!(
        err,
        HandlerCoverageError::ConflictingCoverage {
            effect_kind: "set_context".to_string()
        }
    );
}

#[test]
fn mixed_handler_and_egress_coverage_passes() {
    let mut provides = AdapterProvides::default();
    provides.effects.insert("set_context".to_string());
    provides.effects.insert("place_order".to_string());

    let graph_emittable = HashSet::from(["set_context".to_string(), "place_order".to_string()]);
    let handlers = BTreeSet::from(["set_context".to_string()]);
    let egress = HashSet::from(["place_order".to_string()]);

    let result = ensure_handler_coverage(&provides, &graph_emittable, &handlers, &egress);
    assert!(result.is_ok());
}
