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
    RouteReferencesMissingChannel {
        intent_kind: String,
        channel: String,
    },
    RoutedKindNotAcceptedByAdapter {
        intent_kind: String,
    },
    Coverage(HandlerCoverageError),
}

impl std::fmt::Display for EgressValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RouteReferencesMissingChannel {
                intent_kind,
                channel,
            } => write!(
                f,
                "egress route for kind '{intent_kind}' references unknown channel '{channel}'"
            ),
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
    let routed_kinds: HashSet<String> = config.routes.keys().cloned().collect();

    for (intent_kind, route) in &config.routes {
        if !config.channels.contains_key(&route.channel) {
            return Err(EgressValidationError::RouteReferencesMissingChannel {
                intent_kind: intent_kind.clone(),
                channel: route.channel.clone(),
            });
        }

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
    for intent_kind in config.routes.keys() {
        if !graph_emittable_effect_kinds.contains(intent_kind) {
            warnings.push(EgressValidationWarning::RouteForNonEmittableKind {
                intent_kind: intent_kind.clone(),
            });
        }
    }

    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::egress::{EgressChannelConfig, EgressRoute};
    use ergo_adapter::host::HandlerCoverageError;
    use ergo_adapter::ContextKeyProvision;
    use std::collections::{BTreeMap, HashMap};
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
}
