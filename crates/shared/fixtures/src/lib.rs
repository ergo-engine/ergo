//! ergo_fixtures
//!
//! Purpose:
//! - Provide shared fixture conversion, inspection, and validation helpers used
//!   by CLI and support tooling.
//!
//! Owns:
//! - Thin product-facing fixture utilities and DTOs built on adapter-owned
//!   fixture parsing.
//!
//! Does not own:
//! - Canonical fixture parsing grammar or adapter event payload semantics.
//!
//! Connects to:
//! - `ergo_adapter::fixture` for parsing.
//! - CLI/support tooling that inspects or converts fixture artifacts.
//!
//! Safety notes:
//! - This crate should preserve typed adapter parse failures until an output
//!   boundary deliberately renders them.

use serde::{Deserialize, Serialize};

pub mod csv;
pub mod report;

pub use csv::{convert_csv_to_fixture, parse_event_kind, CsvToFixtureOptions};
pub use ergo_adapter::fixture::FixtureParseError;
pub use report::{
    analyze_fixture, inspect_fixture, render_inspect_json, render_inspect_text,
    render_validate_json, render_validate_text, stats_from_analysis, validate_analysis,
    validate_fixture, EpisodeSummaryV1, FixtureAnalysis, FixtureInspectOutputV1, FixtureIssueV1,
    FixtureStatsV1, FixtureValidateOutputV1, FixtureValidationReport,
    FIXTURE_OUTPUT_SCHEMA_VERSION,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureEnvelope {
    pub id: String,
    pub payload: serde_json::Value,
}
