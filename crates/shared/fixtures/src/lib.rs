use serde::{Deserialize, Serialize};

pub mod csv;
pub mod report;

pub use csv::{convert_csv_to_fixture, parse_event_kind, CsvToFixtureOptions};
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
