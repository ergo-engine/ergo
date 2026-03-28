//! replay
//!
//! Purpose:
//! - Host-owned lower-level strict replay seam over a fully prepared
//!   `HostedRunner`.
//! - Performs strict replay preflight, event rehydration, replay stepping, and
//!   host-level result shaping for callers that already have a replay-configured
//!   runner and capture bundle.
//!
//! Owns:
//! - `HostedReplayError`, which preserves the host replay phases
//!   (`Preflight`, `EventRehydrate`, `Step`, `Compare`, `DecisionMismatch`).
//! - The host wrapper around supervisor strict replay preflight and host-owned
//!   decision/effect integrity comparison.
//! - `decision_counts(...)` as a small post-replay summary helper.
//!
//! Does not own:
//! - Replay doctrine, bundle validation, or decision/effect comparison
//!   semantics in `ergo_supervisor::replay`.
//! - Replay-step execution semantics in `runner.rs`.
//! - Canonical client-facing replay orchestration in `usecases.rs`.
//!
//! Connects to:
//! - `ergo_supervisor::replay` for strict preflight and comparison.
//! - `ergo_adapter::capture` for event rehydration and payload integrity.
//! - `HostedRunner::replay_step(...)` for host replay realization.
//! - `usecases.rs` and `replay_error_surface.rs`, which consume this seam.
//!
//! Safety notes:
//! - `replay_bundle_strict(...)` always performs strict preflight before any
//!   event rehydration or stepping.
//! - Effect drift and non-effect decision-stream drift intentionally remain
//!   separate outcomes: effect mismatches surface through `Compare(...)`,
//!   while non-effect decision mismatch becomes `DecisionMismatch`.
//! - `EventRehydrate` exhaustively matches the foreign `CaptureError` enum, so
//!   adapter-side capture error changes must be mapped here deliberately.
//! - `ReplayError` currently does not expose `Display` / `Error`, so
//!   `HostedReplayError` uses debug formatting for `Preflight` and `Compare`
//!   and cannot chain them through `source()`; broader replay-surface cleanup
//!   is tracked in issue #68.
//! - `decision_counts(...)` makes three passes and assumes the current three
//!   `Decision` variants; that follow-up cleanup is also tracked in issue #68.

use ergo_adapter::capture::CaptureError;
use ergo_supervisor::replay::{
    compare_decisions, validate_bundle_strict, ReplayError, StrictReplayExpectations,
};
use ergo_supervisor::{CaptureBundle, Decision};

use crate::{HostedRunner, HostedStepError};

#[derive(Debug)]
pub enum HostedReplayError {
    Preflight(ReplayError),
    EventRehydrate { event_id: String, detail: String },
    Step(HostedStepError),
    Compare(ReplayError),
    DecisionMismatch,
}

impl std::fmt::Display for HostedReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preflight(err) => write!(f, "strict replay preflight failed: {err:?}"),
            Self::EventRehydrate { event_id, detail } => write!(
                f,
                "failed to rehydrate event '{}' during host replay: {}",
                event_id, detail
            ),
            Self::Step(err) => write!(f, "host replay step failed: {err}"),
            Self::Compare(err) => write!(f, "replay decision comparison failed: {err:?}"),
            Self::DecisionMismatch => {
                write!(f, "replay decisions do not match captured decisions")
            }
        }
    }
}

impl std::error::Error for HostedReplayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Step(err) => Some(err),
            _ => None,
        }
    }
}

fn map_rehydrate_error(event_id: &str, err: CaptureError) -> HostedReplayError {
    HostedReplayError::EventRehydrate {
        event_id: event_id.to_string(),
        detail: match err {
            CaptureError::PayloadHashMismatch { expected, actual } => format!(
                "payload hash mismatch (expected '{}', actual '{}')",
                expected, actual
            ),
            CaptureError::InvalidPayload { detail } => detail,
        },
    }
}

pub fn replay_bundle_strict(
    bundle: &CaptureBundle,
    mut runner: HostedRunner,
    expectations: StrictReplayExpectations<'_>,
) -> Result<CaptureBundle, HostedReplayError> {
    validate_bundle_strict(bundle, expectations).map_err(HostedReplayError::Preflight)?;

    for record in &bundle.events {
        let event = record
            .rehydrate_checked()
            .map_err(|err| map_rehydrate_error(record.event_id.as_str(), err))?;
        runner.replay_step(event).map_err(HostedReplayError::Step)?;
    }

    let replayed_bundle = runner.into_capture_bundle();
    let matches = compare_decisions(&bundle.decisions, &replayed_bundle.decisions)
        .map_err(HostedReplayError::Compare)?;
    if !matches {
        return Err(HostedReplayError::DecisionMismatch);
    }

    Ok(replayed_bundle)
}

pub fn decision_counts(bundle: &CaptureBundle) -> (usize, usize, usize) {
    let invoke_count = bundle
        .decisions
        .iter()
        .filter(|record| record.decision == Decision::Invoke)
        .count();
    let defer_count = bundle
        .decisions
        .iter()
        .filter(|record| record.decision == Decision::Defer)
        .count();
    let skip_count = bundle
        .decisions
        .iter()
        .filter(|record| record.decision == Decision::Skip)
        .count();

    (invoke_count, defer_count, skip_count)
}

#[cfg(test)]
mod tests;
