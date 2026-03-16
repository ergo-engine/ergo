use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EgressConfig {
    #[serde(with = "duration_serde")]
    pub default_ack_timeout: Duration,
    pub channels: BTreeMap<String, EgressChannelConfig>,
    pub routes: BTreeMap<String, EgressRoute>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EgressRoute {
    pub channel: String,
    #[serde(default, with = "duration_option_serde")]
    pub ack_timeout: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum EgressChannelConfig {
    #[serde(rename = "process")]
    Process { command: Vec<String> },
}

pub fn parse_egress_config_toml(raw: &str) -> Result<EgressConfig, String> {
    toml::from_str(raw).map_err(|err| format!("parse egress config TOML: {err}"))
}

mod duration_serde {
    use super::{format_duration, parse_duration_literal};
    use super::{Deserialize, Deserializer, Duration, Serializer};

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format_duration(*duration))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        parse_duration_literal(&raw).map_err(serde::de::Error::custom)
    }
}

mod duration_option_serde {
    use super::{format_duration, parse_duration_literal};
    use super::{Deserialize, Deserializer, Duration, Serializer};

    pub fn serialize<S>(value: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(duration) => serializer.serialize_some(&format_duration(*duration)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = Option::<String>::deserialize(deserializer)?;
        raw.map(|value| parse_duration_literal(&value))
            .transpose()
            .map_err(serde::de::Error::custom)
    }
}

fn parse_duration_literal(raw: &str) -> Result<Duration, String> {
    if let Some(value) = raw.strip_suffix("ms") {
        let millis = value
            .parse::<u64>()
            .map_err(|err| format!("invalid duration '{raw}': {err}"))?;
        return Ok(Duration::from_millis(millis));
    }

    if let Some(value) = raw.strip_suffix('s') {
        let secs = value
            .parse::<u64>()
            .map_err(|err| format!("invalid duration '{raw}': {err}"))?;
        return Ok(Duration::from_secs(secs));
    }

    if let Some(value) = raw.strip_suffix('m') {
        let mins = value
            .parse::<u64>()
            .map_err(|err| format!("invalid duration '{raw}': {err}"))?;
        return Ok(Duration::from_secs(mins.saturating_mul(60)));
    }

    if let Some(value) = raw.strip_suffix('h') {
        let hours = value
            .parse::<u64>()
            .map_err(|err| format!("invalid duration '{raw}': {err}"))?;
        return Ok(Duration::from_secs(
            hours.saturating_mul(60).saturating_mul(60),
        ));
    }

    Err(format!(
        "unsupported duration '{raw}' (expected suffix ms|s|m|h)"
    ))
}

fn format_duration(duration: Duration) -> String {
    let millis = duration.as_millis();
    if millis % 1000 == 0 {
        format!("{}s", millis / 1000)
    } else {
        format!("{millis}ms")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_example_toml() -> Result<(), String> {
        let raw = r#"
default_ack_timeout = "5s"

[channels.broker]
type = "process"
command = ["python3", "broker.py"]

[routes.place_order]
channel = "broker"
ack_timeout = "10s"

[routes.cancel_order]
channel = "broker"
"#;

        let config = parse_egress_config_toml(raw)?;
        assert_eq!(config.default_ack_timeout, Duration::from_secs(5));
        assert!(matches!(
            config.channels.get("broker"),
            Some(EgressChannelConfig::Process { command }) if command == &vec!["python3".to_string(), "broker.py".to_string()]
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
}
