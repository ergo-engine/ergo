//! egress::config
//!
//! Purpose:
//! - Define the canonical host-owned egress configuration model, its TOML parse
//!   surface, and the deterministic normalization used for egress provenance.
//!
//! Owns:
//! - The public `EgressConfig`, `EgressRoute`, and `EgressChannelConfig` shapes.
//! - TOML decoding into that canonical internal model.
//! - Canonical serde normalization used by `compute_egress_provenance(...)`.
//!
//! Does not own:
//! - Startup validation of routes against adapters, handlers, or graph-emittable
//!   kinds.
//! - Egress process lifecycle, handshake, or dispatch semantics.
//!
//! Connects to:
//! - CLI and SDK file-loading paths, which read TOML and call
//!   `parse_egress_config_toml(...)`.
//! - Egress validation and runtime startup, which consume the parsed object model.
//! - Capture provenance, which hashes this file's normalized serde output.
//!
//! Safety notes:
//! - Field names, serde tags, and duration serialization are part of the public
//!   config/provenance contract; changing them changes compatibility or hashes.
//! - `BTreeMap` is intentional: deterministic key order is required for canonical
//!   serialization and provenance stability.
//! - Duration parsing accepts `ms|s|m|h`, but canonical serialization normalizes
//!   to `ms` or `s`.
//! - Parser error wording is user-visible because CLI and SDK currently wrap the
//!   returned string rather than replacing it.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::Digest;

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

pub fn compute_egress_provenance(config: &EgressConfig) -> String {
    let bytes = serde_json::to_vec(config).expect("EgressConfig must be serializable");
    let digest = sha2::Sha256::digest(&bytes);
    format!("epv1:sha256:{}", hex::encode(digest))
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
mod tests;
