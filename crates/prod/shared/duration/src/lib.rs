//! ergo_prod_duration
//!
//! Purpose:
//! - Own the prod-internal authority for Ergo's simple authored duration
//!   literals.
//!
//! Owns:
//! - Parsing the shared `ms|s|m|h` integer-literal grammar used by current
//!   prod authored config surfaces.
//!
//! Does not own:
//! - Serde wrappers, file-level error context, or canonical formatting for any
//!   caller-specific config surface.
//! - Runtime timing policy or host provenance serialization.
//!
//! Connects to:
//! - `ergo-host`, which uses this parser for egress config durations.
//! - `ergo-loader`, which uses this parser for `ergo.toml` profile duration
//!   literals.
//!
//! Safety notes:
//! - Error strings are user-visible because host and loader currently wrap them
//!   rather than replacing them.
//! - Saturating minute/hour expansion is intentional and part of the current
//!   authored-literal contract.

use std::time::Duration;

pub fn parse_duration_literal(raw: &str) -> Result<Duration, String> {
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

#[cfg(test)]
mod tests;
