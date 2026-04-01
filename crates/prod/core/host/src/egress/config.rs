//! egress::config
//!
//! Purpose:
//! - Define the canonical host-owned egress configuration model, its TOML parse
//!   surface, and the deterministic normalization used for egress provenance.
//!
//! Owns:
//! - The validated public `EgressConfig`, `EgressRoute`, and
//!   `EgressChannelConfig` shapes.
//! - TOML decoding into that validated canonical host model.
//! - Canonical serde normalization used by `compute_egress_provenance(...)`.
//! - Config-owned invariants that do not depend on adapter, graph, or runtime
//!   state.
//!
//! Does not own:
//! - Startup validation against adapter acceptance, graph-emittable kinds, or
//!   handler ownership.
//! - Egress process lifecycle, handshake, or dispatch semantics.
//!
//! Connects to:
//! - CLI and SDK file-loading paths, which read TOML and call
//!   `parse_egress_config_toml(...)`.
//! - Programmatic SDK/embedded callers, which construct validated egress config
//!   values through constructors or `EgressConfigBuilder`.
//! - Egress validation and runtime startup, which consume the validated object
//!   model.
//! - Capture provenance, which hashes this file's normalized serde output.
//!
//! Safety notes:
//! - Field names, serde tags, and duration serialization are part of the public
//!   config/provenance contract; changing them changes compatibility or hashes.
//! - `BTreeMap` is intentional: deterministic key order is required for
//!   canonical serialization and provenance stability.
//! - Duration parsing accepts `ms|s|m|h`, but canonical serialization normalizes
//!   to `ms` or `s`.
//! - Parser error wording is user-visible because CLI and SDK currently wrap the
//!   returned string rather than replacing it.
//! - Provenance hashing uses an explicit private projection of the canonical
//!   config fields so future helper/state fields on `EgressConfig` do not
//!   silently widen the hash contract or trigger runtime panics.
//! - Route-to-channel references and process-command emptiness are config-owned
//!   invariants here, so later host stages never need to guess whether an
//!   `EgressConfig` is intrinsically malformed.

use std::collections::BTreeMap;
use std::time::Duration;

use ergo_prod_duration::parse_duration_literal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::Digest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EgressConfigError {
    EmptyChannelId,
    DuplicateChannelId {
        channel: String,
    },
    EmptyIntentKind,
    DuplicateIntentKind {
        intent_kind: String,
    },
    EmptyRouteChannel,
    EmptyProcessCommand {
        channel: Option<String>,
    },
    BlankProcessExecutable {
        channel: Option<String>,
    },
    RouteReferencesMissingChannel {
        intent_kind: String,
        channel: String,
    },
}

impl std::fmt::Display for EgressConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyChannelId => write!(f, "egress channel id must not be empty"),
            Self::DuplicateChannelId { channel } => {
                write!(f, "duplicate egress channel id '{channel}'")
            }
            Self::EmptyIntentKind => write!(f, "egress route kind must not be empty"),
            Self::DuplicateIntentKind { intent_kind } => {
                write!(f, "duplicate egress route kind '{intent_kind}'")
            }
            Self::EmptyRouteChannel => write!(f, "egress route channel must not be empty"),
            Self::EmptyProcessCommand {
                channel: Some(channel),
            } => write!(
                f,
                "egress channel '{channel}' process command must not be empty"
            ),
            Self::EmptyProcessCommand { channel: None } => {
                write!(f, "process egress channel command must not be empty")
            }
            Self::BlankProcessExecutable {
                channel: Some(channel),
            } => write!(
                f,
                "egress channel '{channel}' process executable must not be blank"
            ),
            Self::BlankProcessExecutable { channel: None } => {
                write!(f, "process egress channel executable must not be blank")
            }
            Self::RouteReferencesMissingChannel {
                intent_kind,
                channel,
            } => write!(
                f,
                "egress route for kind '{intent_kind}' references unknown channel '{channel}'"
            ),
        }
    }
}

impl std::error::Error for EgressConfigError {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "RawEgressConfig")]
pub struct EgressConfig {
    #[serde(with = "duration_serde")]
    default_ack_timeout: Duration,
    channels: BTreeMap<String, EgressChannelConfig>,
    routes: BTreeMap<String, EgressRoute>,
}

impl EgressConfig {
    pub fn new(
        default_ack_timeout: Duration,
        channels: BTreeMap<String, EgressChannelConfig>,
        routes: BTreeMap<String, EgressRoute>,
    ) -> Result<Self, EgressConfigError> {
        validate_channel_ids(&channels)?;
        validate_route_keys(&routes)?;
        validate_route_targets(&channels, &routes)?;

        Ok(Self {
            default_ack_timeout,
            channels,
            routes,
        })
    }

    pub fn builder(default_ack_timeout: Duration) -> EgressConfigBuilder {
        EgressConfigBuilder::new(default_ack_timeout)
    }

    pub fn default_ack_timeout(&self) -> Duration {
        self.default_ack_timeout
    }

    pub fn channels(&self) -> &BTreeMap<String, EgressChannelConfig> {
        &self.channels
    }

    pub fn channel(&self, channel_id: &str) -> Option<&EgressChannelConfig> {
        self.channels.get(channel_id)
    }

    pub fn routes(&self) -> &BTreeMap<String, EgressRoute> {
        &self.routes
    }

    pub fn route(&self, intent_kind: &str) -> Option<&EgressRoute> {
        self.routes.get(intent_kind)
    }
}

impl TryFrom<RawEgressConfig> for EgressConfig {
    type Error = EgressConfigError;

    fn try_from(value: RawEgressConfig) -> Result<Self, Self::Error> {
        Self::new(
            value.default_ack_timeout,
            validate_raw_channels(value.channels)?,
            value.routes,
        )
    }
}

#[derive(Debug, Clone)]
pub struct EgressConfigBuilder {
    default_ack_timeout: Duration,
    channels: BTreeMap<String, EgressChannelConfig>,
    routes: BTreeMap<String, EgressRoute>,
}

impl EgressConfigBuilder {
    pub fn new(default_ack_timeout: Duration) -> Self {
        Self {
            default_ack_timeout,
            channels: BTreeMap::new(),
            routes: BTreeMap::new(),
        }
    }

    pub fn channel(
        mut self,
        channel_id: impl Into<String>,
        config: EgressChannelConfig,
    ) -> Result<Self, EgressConfigError> {
        let channel_id = channel_id.into();
        if channel_id.is_empty() {
            return Err(EgressConfigError::EmptyChannelId);
        }
        if self.channels.contains_key(&channel_id) {
            return Err(EgressConfigError::DuplicateChannelId {
                channel: channel_id,
            });
        }
        self.channels.insert(channel_id, config);
        Ok(self)
    }

    pub fn route(
        mut self,
        intent_kind: impl Into<String>,
        route: EgressRoute,
    ) -> Result<Self, EgressConfigError> {
        let intent_kind = intent_kind.into();
        if intent_kind.is_empty() {
            return Err(EgressConfigError::EmptyIntentKind);
        }
        if self.routes.contains_key(&intent_kind) {
            return Err(EgressConfigError::DuplicateIntentKind { intent_kind });
        }
        self.routes.insert(intent_kind, route);
        Ok(self)
    }

    pub fn build(self) -> Result<EgressConfig, EgressConfigError> {
        EgressConfig::new(self.default_ack_timeout, self.channels, self.routes)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "RawEgressRoute")]
pub struct EgressRoute {
    channel: String,
    #[serde(default, with = "duration_option_serde")]
    ack_timeout: Option<Duration>,
}

impl EgressRoute {
    pub fn new(
        channel: impl Into<String>,
        ack_timeout: Option<Duration>,
    ) -> Result<Self, EgressConfigError> {
        let channel = channel.into();
        if channel.is_empty() {
            return Err(EgressConfigError::EmptyRouteChannel);
        }
        Ok(Self {
            channel,
            ack_timeout,
        })
    }

    pub fn channel(&self) -> &str {
        &self.channel
    }

    pub fn ack_timeout(&self) -> Option<Duration> {
        self.ack_timeout
    }
}

impl TryFrom<RawEgressRoute> for EgressRoute {
    type Error = EgressConfigError;

    fn try_from(value: RawEgressRoute) -> Result<Self, Self::Error> {
        Self::new(value.channel, value.ack_timeout)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EgressChannelConfig {
    kind: EgressChannelKind,
}

impl EgressChannelConfig {
    pub fn process(command: Vec<String>) -> Result<Self, EgressConfigError> {
        if command.is_empty() {
            return Err(EgressConfigError::EmptyProcessCommand { channel: None });
        }
        if command[0].trim().is_empty() {
            return Err(EgressConfigError::BlankProcessExecutable { channel: None });
        }
        Ok(Self {
            kind: EgressChannelKind::Process { command },
        })
    }

    pub fn process_command(&self) -> Option<&[String]> {
        match &self.kind {
            EgressChannelKind::Process { command } => Some(command.as_slice()),
        }
    }
}

impl Serialize for EgressChannelConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.kind {
            EgressChannelKind::Process { command } => {
                SerializableEgressChannelConfig::Process { command }.serialize(serializer)
            }
        }
    }
}

impl<'de> Deserialize<'de> for EgressChannelConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawEgressChannelConfig::deserialize(deserializer)?;
        match raw {
            RawEgressChannelConfig::Process { command } => {
                Self::process(command).map_err(serde::de::Error::custom)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EgressChannelKind {
    Process { command: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RawEgressConfig {
    #[serde(with = "duration_serde")]
    default_ack_timeout: Duration,
    channels: BTreeMap<String, RawEgressChannelConfig>,
    routes: BTreeMap<String, EgressRoute>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RawEgressRoute {
    channel: String,
    #[serde(default, with = "duration_option_serde")]
    ack_timeout: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
enum RawEgressChannelConfig {
    #[serde(rename = "process")]
    Process { command: Vec<String> },
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum SerializableEgressChannelConfig<'a> {
    #[serde(rename = "process")]
    Process { command: &'a [String] },
}

#[derive(Serialize)]
struct EgressProvenanceConfig<'a> {
    #[serde(with = "duration_serde")]
    default_ack_timeout: Duration,
    channels: BTreeMap<&'a str, EgressProvenanceChannel<'a>>,
    routes: BTreeMap<&'a str, EgressProvenanceRoute<'a>>,
}

#[derive(Serialize)]
struct EgressProvenanceRoute<'a> {
    channel: &'a str,
    #[serde(default, with = "duration_option_serde")]
    ack_timeout: Option<Duration>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum EgressProvenanceChannel<'a> {
    #[serde(rename = "process")]
    Process { command: &'a [String] },
}

pub fn parse_egress_config_toml(raw: &str) -> Result<EgressConfig, String> {
    toml::from_str(raw).map_err(|err| format!("parse egress config TOML: {err}"))
}

pub fn compute_egress_provenance(config: &EgressConfig) -> String {
    let bytes = serde_json::to_vec(&provenance_projection(config))
        .expect("egress provenance projection must be serializable");
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

fn validate_channel_ids(
    channels: &BTreeMap<String, EgressChannelConfig>,
) -> Result<(), EgressConfigError> {
    for channel_id in channels.keys() {
        if channel_id.is_empty() {
            return Err(EgressConfigError::EmptyChannelId);
        }
    }
    Ok(())
}

fn validate_route_keys(routes: &BTreeMap<String, EgressRoute>) -> Result<(), EgressConfigError> {
    for intent_kind in routes.keys() {
        if intent_kind.is_empty() {
            return Err(EgressConfigError::EmptyIntentKind);
        }
    }
    Ok(())
}

fn validate_route_targets(
    channels: &BTreeMap<String, EgressChannelConfig>,
    routes: &BTreeMap<String, EgressRoute>,
) -> Result<(), EgressConfigError> {
    for (intent_kind, route) in routes {
        if !channels.contains_key(route.channel()) {
            return Err(EgressConfigError::RouteReferencesMissingChannel {
                intent_kind: intent_kind.clone(),
                channel: route.channel().to_string(),
            });
        }
    }
    Ok(())
}

fn validate_raw_channels(
    raw_channels: BTreeMap<String, RawEgressChannelConfig>,
) -> Result<BTreeMap<String, EgressChannelConfig>, EgressConfigError> {
    let mut channels = BTreeMap::new();
    for (channel_id, raw_channel) in raw_channels {
        let config = match raw_channel {
            RawEgressChannelConfig::Process { command } => {
                if command.is_empty() {
                    return Err(EgressConfigError::EmptyProcessCommand {
                        channel: Some(channel_id),
                    });
                }
                if command[0].trim().is_empty() {
                    return Err(EgressConfigError::BlankProcessExecutable {
                        channel: Some(channel_id),
                    });
                }
                EgressChannelConfig {
                    kind: EgressChannelKind::Process { command },
                }
            }
        };
        channels.insert(channel_id, config);
    }
    Ok(channels)
}

fn provenance_projection(config: &EgressConfig) -> EgressProvenanceConfig<'_> {
    EgressProvenanceConfig {
        default_ack_timeout: config.default_ack_timeout(),
        channels: config
            .channels()
            .iter()
            .map(|(channel_id, channel)| (channel_id.as_str(), provenance_channel(channel)))
            .collect(),
        routes: config
            .routes()
            .iter()
            .map(|(intent_kind, route)| {
                (
                    intent_kind.as_str(),
                    EgressProvenanceRoute {
                        channel: route.channel(),
                        ack_timeout: route.ack_timeout(),
                    },
                )
            })
            .collect(),
    }
}

fn provenance_channel(channel: &EgressChannelConfig) -> EgressProvenanceChannel<'_> {
    match &channel.kind {
        EgressChannelKind::Process { command } => EgressProvenanceChannel::Process { command },
    }
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
