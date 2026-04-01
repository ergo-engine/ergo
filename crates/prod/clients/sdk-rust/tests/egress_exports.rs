//! egress_exports
//!
//! Purpose:
//! - Verify from outside the SDK crate that the public SDK surface exposes the
//!   validated egress-config construction and TOML-parse paths without requiring
//!   downstream callers to depend on `ergo_host` directly.
//!
//! Owns:
//! - Integration-style compile/runtime checks for SDK re-exports of egress
//!   config builder, config error, and TOML parser access.
//!
//! Does not own:
//! - Host egress config semantics or validation rules themselves.
//!
//! Connects to:
//! - `ergo_sdk_rust` public exports, which should mirror the intended
//!   programmatic egress authoring surface.
//!
//! Safety notes:
//! - These tests compile against the SDK from the outside so missing re-exports
//!   fail here even if the SDK crate itself still compiles internally.

use std::time::Duration;

use ergo_sdk_rust::{
    parse_egress_config_toml, EgressChannelConfig, EgressConfig, EgressConfigBuilder,
    EgressConfigError, EgressRoute,
};

fn add_route(builder: EgressConfigBuilder) -> Result<EgressConfigBuilder, EgressConfigError> {
    builder.route(
        "place_order",
        EgressRoute::new("broker", Some(Duration::from_secs(10)))?,
    )
}

#[test]
fn sdk_reexports_validated_egress_construction_types() {
    let channel =
        EgressChannelConfig::process(vec!["python3".to_string(), "broker.py".to_string()])
            .expect("channel config should build");
    let builder: EgressConfigBuilder = EgressConfig::builder(Duration::from_secs(5))
        .channel("broker", channel)
        .expect("channel insert should succeed");

    let config = add_route(builder)
        .expect("route insert should succeed")
        .build()
        .expect("config should build");

    assert_eq!(config.default_ack_timeout(), Duration::from_secs(5));
    assert_eq!(
        EgressChannelConfig::process(Vec::<String>::new()),
        Err(EgressConfigError::EmptyProcessCommand { channel: None })
    );
}

#[test]
fn sdk_reexports_egress_toml_parser() {
    let config = parse_egress_config_toml(
        r#"
default_ack_timeout = "5s"

[channels.broker]
type = "process"
command = ["python3", "broker.py"]

[routes.place_order]
channel = "broker"
ack_timeout = "10s"
"#,
    )
    .expect("SDK TOML parser should return validated config");

    assert_eq!(config.default_ack_timeout(), Duration::from_secs(5));
    assert_eq!(
        config
            .route("place_order")
            .expect("route should exist")
            .ack_timeout(),
        Some(Duration::from_secs(10))
    );
}
