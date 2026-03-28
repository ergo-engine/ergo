//! egress::validation::tests
//!
//! Purpose:
//! - Keep scenario-heavy validation cases for the host live-egress setup seam out of
//!   the production module so the ownership and boundary logic remains easy to read.
//!
//! Owns:
//! - Regression coverage for route existence checks, adapter-acceptance enforcement,
//!   HST-5 coverage delegation, and warning emission behavior in
//!   `validate_egress_config(...)`.
//!
//! Does not own:
//! - CLI rendering or higher-level hosted runner orchestration behavior.
//!
//! Connects to:
//! - `super::validate_egress_config(...)`, which the canonical host runner setup path
//!   uses before live execution.
//!
//! Safety notes:
//! - These tests intentionally lock the current warning/error classification so dead
//!   egress config stays non-fatal while ownership gaps still fail setup.

use super::*;

use crate::egress::{EgressChannelConfig, EgressRoute};
use ergo_adapter::host::HandlerCoverageError;
use ergo_adapter::ContextKeyProvision;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
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

fn baseline_config() -> EgressConfig {
    EgressConfig {
        default_ack_timeout: Duration::from_secs(5),
        channels: BTreeMap::from([(
            "broker".to_string(),
            EgressChannelConfig::Process {
                command: vec!["sh".to_string(), "-c".to_string(), "echo ready".to_string()],
            },
        )]),
        routes: BTreeMap::from([(
            "place_order".to_string(),
            EgressRoute {
                channel: "broker".to_string(),
                ack_timeout: None,
            },
        )]),
    }
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
fn missing_channel_fails() {
    let mut config = baseline_config();
    config.routes.get_mut("place_order").expect("route").channel = "missing".to_string();
    let adapter = adapter_with_effects(&["place_order"]);
    let emittable = HashSet::from(["place_order".to_string()]);
    let handlers = BTreeSet::new();

    let err = validate_egress_config(&config, &adapter, &emittable, &handlers)
        .expect_err("missing channel must fail");
    assert!(matches!(
        err,
        EgressValidationError::RouteReferencesMissingChannel { .. }
    ));
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
    let config = EgressConfig {
        default_ack_timeout: Duration::from_secs(5),
        channels: BTreeMap::new(),
        routes: BTreeMap::new(),
    };
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
    let config = EgressConfig {
        default_ack_timeout: Duration::from_secs(5),
        channels: BTreeMap::from([(
            "broker".to_string(),
            EgressChannelConfig::Process {
                command: vec!["sh".to_string(), "-c".to_string(), "echo ready".to_string()],
            },
        )]),
        routes: BTreeMap::from([
            (
                "zeta".to_string(),
                EgressRoute {
                    channel: "broker".to_string(),
                    ack_timeout: None,
                },
            ),
            (
                "alpha".to_string(),
                EgressRoute {
                    channel: "broker".to_string(),
                    ack_timeout: None,
                },
            ),
        ]),
    };
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

    let missing_channel = EgressValidationError::RouteReferencesMissingChannel {
        intent_kind: "place_order".to_string(),
        channel: "broker".to_string(),
    };
    assert_eq!(
        missing_channel.to_string(),
        "egress route for kind 'place_order' references unknown channel 'broker'"
    );
    assert!(missing_channel.source().is_none());

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
