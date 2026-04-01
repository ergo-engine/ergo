//! egress::validation::tests
//!
//! Purpose:
//! - Keep scenario-heavy validation cases for the host live-egress setup seam
//!   out of the production module so the ownership and boundary logic remains
//!   easy to read.
//!
//! Owns:
//! - Regression coverage for adapter-acceptance enforcement, HST-5 coverage
//!   delegation, and warning emission behavior in `validate_egress_config(...)`.
//!
//! Does not own:
//! - Config construction invariants, CLI rendering, or higher-level hosted
//!   runner orchestration behavior.
//!
//! Connects to:
//! - `super::validate_egress_config(...)`, which the canonical host runner setup
//!   path uses before live execution.
//!
//! Safety notes:
//! - These tests intentionally lock the current warning/error classification so
//!   dead egress config stays non-fatal while ownership gaps still fail setup.

use super::*;

use crate::egress::{EgressChannelConfig, EgressRoute};
use ergo_adapter::host::HandlerCoverageError;
use ergo_adapter::ContextKeyProvision;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::error::Error as _;
use std::time::Duration;

fn adapter_with_effects(effects: &[&str]) -> AdapterProvides {
    AdapterProvides {
        context: HashMap::from([(
            "k".to_string(),
            ContextKeyProvision {
                ty: "String".to_string(),
                required: false,
                writable: true,
            },
        )]),
        events: HashSet::new(),
        effects: effects.iter().map(|item| item.to_string()).collect(),
        effect_schemas: HashMap::new(),
        event_schemas: HashMap::new(),
        capture_format_version: "v2".to_string(),
        adapter_fingerprint: "adapter:test".to_string(),
    }
}

fn process_channel(command: &[&str]) -> EgressChannelConfig {
    EgressChannelConfig::process(command.iter().map(|item| item.to_string()).collect())
        .expect("channel config should be valid")
}

fn route(channel: &str, ack_timeout: Option<Duration>) -> EgressRoute {
    EgressRoute::new(channel, ack_timeout).expect("route should be valid")
}

fn baseline_config() -> EgressConfig {
    EgressConfig::builder(Duration::from_secs(5))
        .channel("broker", process_channel(&["sh", "-c", "echo ready"]))
        .expect("channel should insert")
        .route("place_order", route("broker", None))
        .expect("route should insert")
        .build()
        .expect("config should build")
}

#[test]
fn valid_config_passes() {
    let config = baseline_config();
    let adapter = adapter_with_effects(&["place_order"]);
    let emittable = HashSet::from(["place_order".to_string()]);
    let handlers = BTreeSet::new();

    let warnings = validate_egress_config(&config, &adapter, &emittable, &handlers)
        .expect("config should be valid");
    assert!(warnings.is_empty());
}

#[test]
fn non_accepted_kind_fails() {
    let config = baseline_config();
    let adapter = adapter_with_effects(&["set_context"]);
    let emittable = HashSet::from(["place_order".to_string()]);
    let handlers = BTreeSet::new();

    let err = validate_egress_config(&config, &adapter, &emittable, &handlers)
        .expect_err("non-accepted kind must fail");
    assert!(matches!(
        err,
        EgressValidationError::RoutedKindNotAcceptedByAdapter { .. }
    ));
}

#[test]
fn missing_route_for_emittable_kind_fails_via_coverage() {
    let config = EgressConfig::builder(Duration::from_secs(5))
        .build()
        .expect("empty config should still be structurally valid");
    let adapter = adapter_with_effects(&["place_order"]);
    let emittable = HashSet::from(["place_order".to_string()]);
    let handlers = BTreeSet::new();

    let err = validate_egress_config(&config, &adapter, &emittable, &handlers)
        .expect_err("coverage should fail");
    assert!(matches!(
        err,
        EgressValidationError::Coverage(HandlerCoverageError::MissingHandler { .. })
    ));
}

#[test]
fn non_emittable_route_yields_warning() {
    let config = baseline_config();
    let adapter = adapter_with_effects(&["place_order"]);
    let emittable = HashSet::new();
    let handlers = BTreeSet::new();

    let warnings = validate_egress_config(&config, &adapter, &emittable, &handlers)
        .expect("non-emittable route should be warning");
    assert_eq!(
        warnings,
        vec![EgressValidationWarning::RouteForNonEmittableKind {
            intent_kind: "place_order".to_string(),
        }]
    );
}

#[test]
fn non_emittable_warnings_follow_route_key_order() {
    let config = EgressConfig::builder(Duration::from_secs(5))
        .channel("broker", process_channel(&["sh", "-c", "echo ready"]))
        .expect("channel should insert")
        .route("zeta", route("broker", None))
        .expect("zeta route should insert")
        .route("alpha", route("broker", None))
        .expect("alpha route should insert")
        .build()
        .expect("config should build");
    let adapter = adapter_with_effects(&["alpha", "zeta"]);
    let emittable = HashSet::new();
    let handlers = BTreeSet::new();

    let warnings = validate_egress_config(&config, &adapter, &emittable, &handlers)
        .expect("non-emittable routes should remain warnings");
    assert_eq!(
        warnings,
        vec![
            EgressValidationWarning::RouteForNonEmittableKind {
                intent_kind: "alpha".to_string(),
            },
            EgressValidationWarning::RouteForNonEmittableKind {
                intent_kind: "zeta".to_string(),
            },
        ]
    );
}

#[test]
fn handler_and_egress_conflict_fails() {
    let config = baseline_config();
    let adapter = adapter_with_effects(&["place_order"]);
    let emittable = HashSet::from(["place_order".to_string()]);
    let handlers = BTreeSet::from(["place_order".to_string()]);

    let err = validate_egress_config(&config, &adapter, &emittable, &handlers)
        .expect_err("conflict should fail");
    assert!(matches!(
        err,
        EgressValidationError::Coverage(HandlerCoverageError::ConflictingCoverage { .. })
    ));
}

#[test]
fn warning_and_error_display_contracts_are_stable() {
    let warning = EgressValidationWarning::RouteForNonEmittableKind {
        intent_kind: "place_order".to_string(),
    };
    assert_eq!(
        warning.to_string(),
        "egress route declared for non-emittable kind 'place_order'"
    );

    let not_accepted = EgressValidationError::RoutedKindNotAcceptedByAdapter {
        intent_kind: "place_order".to_string(),
    };
    assert_eq!(
        not_accepted.to_string(),
        "egress route kind 'place_order' is not accepted by adapter (accepts.effects)"
    );
    assert!(not_accepted.source().is_none());

    let coverage = EgressValidationError::Coverage(HandlerCoverageError::ConflictingCoverage {
        effect_kind: "set_context".to_string(),
    });
    assert_eq!(
        coverage.to_string(),
        "ambiguous coverage for kind 'set_context': claimed by both handler and egress"
    );
    assert_eq!(
        coverage
            .source()
            .expect("coverage error should expose its underlying source")
            .to_string(),
        "ambiguous coverage for kind 'set_context': claimed by both handler and egress"
    );
}
