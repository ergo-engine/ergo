//! egress::config::tests
//!
//! Purpose:
//! - Keep parse/provenance contract tests for the canonical host egress config
//!   seam out of the production module while locking its public schema and
//!   normalization behavior.

use super::*;

use serde_json::json;

fn sample_config_toml() -> &'static str {
    r#"
default_ack_timeout = "5s"

[channels.broker]
type = "process"
command = ["python3", "broker.py"]

[routes.place_order]
channel = "broker"
ack_timeout = "10s"

[routes.cancel_order]
channel = "broker"
"#
}

fn sample_config() -> EgressConfig {
    parse_egress_config_toml(sample_config_toml()).expect("sample config should parse")
}

#[test]
fn parse_example_toml() -> Result<(), String> {
    let config = parse_egress_config_toml(sample_config_toml())?;
    assert_eq!(config.default_ack_timeout, Duration::from_secs(5));
    assert!(matches!(
        config.channels.get("broker"),
        Some(EgressChannelConfig::Process { command })
            if command == &vec!["python3".to_string(), "broker.py".to_string()]
    ));
    assert_eq!(
        config
            .routes
            .get("place_order")
            .expect("place_order route")
            .ack_timeout,
        Some(Duration::from_secs(10))
    );
    assert_eq!(
        config
            .routes
            .get("cancel_order")
            .expect("cancel_order route")
            .ack_timeout,
        None
    );
    Ok(())
}

#[test]
fn parse_missing_required_field_fails() {
    let raw = r#"
[channels.broker]
type = "process"
command = ["python3", "broker.py"]
"#;
    let err = parse_egress_config_toml(raw).expect_err("missing default/routes should fail");
    assert!(
        err.contains("default_ack_timeout"),
        "unexpected error: {err}"
    );
}

#[test]
fn invalid_duration_literals_surface_current_messages() {
    let unsupported_suffix = parse_egress_config_toml(
        r#"
default_ack_timeout = "5d"

[channels.broker]
type = "process"
command = ["python3", "broker.py"]

[routes.place_order]
channel = "broker"
"#,
    )
    .expect_err("unsupported suffix should fail");
    assert!(
        unsupported_suffix.contains("unsupported duration '5d'"),
        "unexpected error: {unsupported_suffix}"
    );

    let invalid_number = parse_egress_config_toml(
        r#"
default_ack_timeout = "xs"

[channels.broker]
type = "process"
command = ["python3", "broker.py"]

[routes.place_order]
channel = "broker"
"#,
    )
    .expect_err("invalid number should fail");
    assert!(
        invalid_number.contains("invalid duration 'xs'"),
        "unexpected error: {invalid_number}"
    );
}

#[test]
fn duration_literals_normalize_in_canonical_json_output() -> Result<(), String> {
    let config = parse_egress_config_toml(
        r#"
default_ack_timeout = "1m"

[channels.broker]
type = "process"
command = ["python3", "broker.py"]

[routes.place_order]
channel = "broker"
ack_timeout = "1500ms"

[routes.cancel_order]
channel = "broker"
ack_timeout = "2h"
"#,
    )?;

    let actual = serde_json::to_value(&config).expect("config should serialize");
    assert_eq!(
        actual,
        json!({
            "default_ack_timeout": "60s",
            "channels": {
                "broker": {
                    "type": "process",
                    "command": ["python3", "broker.py"]
                }
            },
            "routes": {
                "cancel_order": {
                    "channel": "broker",
                    "ack_timeout": "7200s"
                },
                "place_order": {
                    "channel": "broker",
                    "ack_timeout": "1500ms"
                }
            }
        })
    );
    Ok(())
}

#[test]
fn canonical_json_shape_is_stable_for_sample_config() {
    let actual = serde_json::to_string(&sample_config()).expect("config should serialize");
    assert_eq!(
        actual,
        r#"{"default_ack_timeout":"5s","channels":{"broker":{"type":"process","command":["python3","broker.py"]}},"routes":{"cancel_order":{"channel":"broker","ack_timeout":null},"place_order":{"channel":"broker","ack_timeout":"10s"}}}"#
    );
}

#[test]
fn provenance_is_deterministic_for_same_config() {
    let config = sample_config();
    assert_eq!(
        compute_egress_provenance(&config),
        compute_egress_provenance(&config)
    );
}

#[test]
fn provenance_changes_when_timeout_changes() {
    let mut config = sample_config();
    let base = compute_egress_provenance(&config);
    config.default_ack_timeout = Duration::from_secs(8);
    let changed = compute_egress_provenance(&config);
    assert_ne!(base, changed);
}

#[test]
fn provenance_changes_when_route_changes() {
    let mut config = sample_config();
    let base = compute_egress_provenance(&config);
    config.routes.remove("cancel_order");
    let changed = compute_egress_provenance(&config);
    assert_ne!(base, changed);
}

#[test]
fn provenance_changes_when_channel_command_changes() -> Result<(), String> {
    let base = parse_egress_config_toml(
        r#"
default_ack_timeout = "60s"

[channels.broker]
type = "process"
command = ["python3", "broker.py"]

[routes.place_order]
channel = "broker"
ack_timeout = "1500ms"
"#,
    )?;
    let changed = parse_egress_config_toml(
        r#"
default_ack_timeout = "60s"

[channels.broker]
type = "process"
command = ["python3", "broker.py", "--paper"]

[routes.place_order]
channel = "broker"
ack_timeout = "1500ms"
"#,
    )?;

    assert_ne!(
        compute_egress_provenance(&base),
        compute_egress_provenance(&changed)
    );
    Ok(())
}

#[test]
fn provenance_golden_regression() {
    let config = sample_config();
    let actual = compute_egress_provenance(&config);
    assert_eq!(
        actual, "epv1:sha256:8f2d35f5153bc1e0f3e2e0309762c599bd22d14dfa7837f6b96a44934ec79d6e",
        "update this golden when config serialization intentionally changes"
    );
}
