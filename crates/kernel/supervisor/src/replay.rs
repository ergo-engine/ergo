//! replay
//!
//! Purpose:
//! - Define the kernel-owned strict replay validation/comparison flow and the
//!   typed replay error surface used by supervisor, host, and CLI replay paths.
//!
//! Owns:
//! - `ReplayError` as the canonical replay preflight/comparison failure taxonomy.
//! - Strict replay provenance, duplicate-id, and effect comparison checks.
//!
//! Does not own:
//! - Host replay setup/orchestration or host-facing replay descriptors.
//! - Capture record hashing/materialization rules owned by `ergo_adapter::capture`.
//!
//! Connects to:
//! - `ergo_host::replay`, which wraps these failures at the host replay boundary.
//! - CLI replay formatting and supervisor replay harness tests.
//!
//! Safety notes:
//! - `Display` is intentionally local here so higher layers can carry `ReplayError`
//!   directly instead of formatting it with `Debug`.

use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Mutex};

use ergo_adapter::capture::{CaptureError, ExternalEventRecord};
use ergo_adapter::{EventId, ExternalEventPayloadError, RuntimeInvoker};
use ergo_runtime::common::ActionEffect;

use crate::{
    CaptureBundle, CapturedActionEffect, DecisionLog, DecisionLogEntry, EpisodeInvocationRecord,
    Supervisor, NO_ADAPTER_PROVENANCE,
};

#[derive(Clone, Default)]
pub struct MemoryDecisionLog {
    entries: Arc<Mutex<Vec<DecisionLogEntry>>>,
}

impl DecisionLog for MemoryDecisionLog {
    fn log(&self, entry: DecisionLogEntry) {
        let mut guard = self.entries.lock().expect("decision log poisoned");
        guard.push(entry);
    }
}

impl MemoryDecisionLog {
    pub fn records(&self) -> Vec<EpisodeInvocationRecord> {
        let guard = self.entries.lock().expect("decision log poisoned");
        guard
            .iter()
            .map(|entry| {
                let mut record = EpisodeInvocationRecord::from(entry);
                // Hash effects for comparison with captured data.
                let captured: Vec<CapturedActionEffect> = entry
                    .effects
                    .iter()
                    .map(|effect| CapturedActionEffect {
                        effect_hash: hash_effect(effect),
                        effect: effect.clone(),
                    })
                    .collect();
                record.effects = captured;
                record
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReplayError {
    UnsupportedVersion {
        capture_version: String,
    },
    HashMismatch {
        event_id: EventId,
    },
    InvalidPayload {
        event_id: EventId,
        source: ExternalEventPayloadError,
    },
    AdapterProvenanceMismatch {
        expected: String,
        got: String,
    },
    RuntimeProvenanceMismatch {
        expected: String,
        got: String,
    },
    UnexpectedAdapterProvidedForNoAdapterCapture,
    AdapterRequiredForProvenancedCapture,
    DuplicateEventId {
        event_id: EventId,
    },
    EffectMismatch {
        event_id: EventId,
        effect_index: usize,
        expected: Option<CapturedActionEffect>,
        actual: Option<CapturedActionEffect>,
        detail: String,
    },
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion { capture_version } => {
                write!(f, "unsupported capture version '{capture_version}'")
            }
            Self::HashMismatch { event_id } => {
                write!(f, "payload hash mismatch for event '{}'", event_id.as_str())
            }
            Self::InvalidPayload { event_id, source } => {
                write!(
                    f,
                    "invalid payload for event '{}': {source}",
                    event_id.as_str()
                )
            }
            Self::AdapterProvenanceMismatch { expected, got } => write!(
                f,
                "adapter provenance mismatch: expected '{expected}', got '{got}'"
            ),
            Self::RuntimeProvenanceMismatch { expected, got } => write!(
                f,
                "runtime provenance mismatch: expected '{expected}', got '{got}'"
            ),
            Self::UnexpectedAdapterProvidedForNoAdapterCapture => {
                write!(
                    f,
                    "bundle provenance is 'none'; adapter must not be provided"
                )
            }
            Self::AdapterRequiredForProvenancedCapture => {
                write!(f, "bundle is adapter-provenanced; adapter is required")
            }
            Self::DuplicateEventId { event_id } => write!(
                f,
                "duplicate event_id '{}' in strict replay capture input",
                event_id.as_str()
            ),
            Self::EffectMismatch {
                event_id,
                effect_index,
                detail,
                ..
            } => write!(
                f,
                "effect mismatch at index {} for event '{}': {}",
                effect_index,
                event_id.as_str(),
                detail
            ),
        }
    }
}

impl std::error::Error for ReplayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidPayload { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StrictReplayExpectations<'a> {
    pub expected_adapter_provenance: &'a str,
    pub expected_runtime_provenance: &'a str,
}

pub fn validate_bundle(bundle: &CaptureBundle) -> Result<(), ReplayError> {
    if bundle.capture_version != crate::CAPTURE_FORMAT_VERSION {
        return Err(ReplayError::UnsupportedVersion {
            capture_version: bundle.capture_version.clone(),
        });
    }

    for record in &bundle.events {
        if !record.validate_hash() {
            return Err(ReplayError::HashMismatch {
                event_id: record.event_id.clone(),
            });
        }
    }

    Ok(())
}

pub fn replay_checked<R: RuntimeInvoker + Clone>(
    bundle: &CaptureBundle,
    runtime: R,
) -> Result<Vec<EpisodeInvocationRecord>, ReplayError> {
    validate_bundle(bundle)?;
    replay_inner(bundle, runtime)
}

pub fn replay_checked_strict<R: RuntimeInvoker + Clone>(
    bundle: &CaptureBundle,
    runtime: R,
    expectations: StrictReplayExpectations<'_>,
) -> Result<Vec<EpisodeInvocationRecord>, ReplayError> {
    validate_bundle_strict(bundle, expectations)?;
    replay_inner(bundle, runtime)
}

pub fn validate_bundle_strict(
    bundle: &CaptureBundle,
    expectations: StrictReplayExpectations<'_>,
) -> Result<(), ReplayError> {
    validate_bundle(bundle)?;
    validate_unique_event_ids(bundle)?;
    validate_replay_provenance(bundle, expectations)?;
    Ok(())
}

pub fn replay<R: RuntimeInvoker + Clone>(
    bundle: &CaptureBundle,
    runtime: R,
) -> Vec<EpisodeInvocationRecord> {
    replay_checked(bundle, runtime).expect("capture bundle validation failed")
}

fn replay_inner<R: RuntimeInvoker + Clone>(
    bundle: &CaptureBundle,
    runtime: R,
) -> Result<Vec<EpisodeInvocationRecord>, ReplayError> {
    let decision_log = MemoryDecisionLog::default();
    let mut supervisor = Supervisor::with_runtime(
        bundle.graph_id.clone(),
        bundle.config.clone(),
        decision_log.clone(),
        runtime,
    );

    for record in &bundle.events {
        supervisor.on_event(rehydrate_event(record)?);
    }

    Ok(decision_log.records())
}

fn validate_replay_provenance(
    bundle: &CaptureBundle,
    expectations: StrictReplayExpectations<'_>,
) -> Result<(), ReplayError> {
    let provenance = bundle.adapter_provenance.as_str();
    if provenance == NO_ADAPTER_PROVENANCE {
        if expectations.expected_adapter_provenance != NO_ADAPTER_PROVENANCE {
            return Err(ReplayError::UnexpectedAdapterProvidedForNoAdapterCapture);
        }
    } else if expectations.expected_adapter_provenance == NO_ADAPTER_PROVENANCE {
        return Err(ReplayError::AdapterRequiredForProvenancedCapture);
    } else if expectations.expected_adapter_provenance != provenance {
        return Err(ReplayError::AdapterProvenanceMismatch {
            expected: provenance.to_string(),
            got: expectations.expected_adapter_provenance.to_string(),
        });
    }

    if expectations.expected_runtime_provenance != bundle.runtime_provenance {
        return Err(ReplayError::RuntimeProvenanceMismatch {
            expected: expectations.expected_runtime_provenance.to_string(),
            got: bundle.runtime_provenance.clone(),
        });
    }

    Ok(())
}

fn validate_unique_event_ids(bundle: &CaptureBundle) -> Result<(), ReplayError> {
    let mut seen = HashSet::new();
    for record in &bundle.events {
        let id = record.event_id.as_str().to_string();
        if !seen.insert(id.clone()) {
            return Err(ReplayError::DuplicateEventId {
                event_id: EventId::new(id),
            });
        }
    }
    Ok(())
}

/// Compare replayed decisions against captured decisions with strict effect semantics.
///
/// Returns `Ok(true)` if all decisions match (including effects),
/// `Ok(false)` for decision-stream mismatch (non-effect fields), or `Err` for effect mismatch.
pub fn compare_decisions(
    captured: &[EpisodeInvocationRecord],
    replayed: &[EpisodeInvocationRecord],
) -> Result<bool, ReplayError> {
    if captured.len() != replayed.len() {
        return Ok(false);
    }

    for (cap, rep) in captured.iter().zip(replayed.iter()) {
        // Compare non-effect fields
        if cap.event_id != rep.event_id
            || cap.decision != rep.decision
            || cap.schedule_at != rep.schedule_at
            || cap.episode_id != rep.episode_id
            || cap.deadline != rep.deadline
            || cap.termination != rep.termination
            || cap.retry_count != rep.retry_count
        {
            return Ok(false);
        }

        let captured_effects = &cap.effects;
        let replayed_effects = &rep.effects;

        if captured_effects.len() != replayed_effects.len() {
            return Err(ReplayError::EffectMismatch {
                event_id: cap.event_id.clone(),
                effect_index: captured_effects.len().min(replayed_effects.len()),
                expected: captured_effects.get(replayed_effects.len()).cloned(),
                actual: replayed_effects.get(captured_effects.len()).cloned(),
                detail: format!(
                    "expected {} effects, got {}",
                    captured_effects.len(),
                    replayed_effects.len()
                ),
            });
        }

        for (idx, (cap_eff, rep_eff)) in captured_effects
            .iter()
            .zip(replayed_effects.iter())
            .enumerate()
        {
            if cap_eff.effect != rep_eff.effect || cap_eff.effect_hash != rep_eff.effect_hash {
                return Err(ReplayError::EffectMismatch {
                    event_id: cap.event_id.clone(),
                    effect_index: idx,
                    expected: Some(cap_eff.clone()),
                    actual: Some(rep_eff.clone()),
                    detail: format!(
                        "effect mismatch at index {}: expected hash '{}', got '{}'",
                        idx, cap_eff.effect_hash, rep_eff.effect_hash
                    ),
                });
            }
        }
    }

    Ok(true)
}

/// Delegate to the shared hashing function in the crate root.
pub fn hash_effect(effect: &ActionEffect) -> String {
    crate::compute_effect_hash(effect)
}

fn rehydrate_event(
    record: &ExternalEventRecord,
) -> Result<ergo_adapter::ExternalEvent, ReplayError> {
    record.rehydrate_checked().map_err(|err| match err {
        CaptureError::PayloadHashMismatch { .. } => ReplayError::HashMismatch {
            event_id: record.event_id.clone(),
        },
        CaptureError::InvalidPayload { source } => ReplayError::InvalidPayload {
            event_id: record.event_id.clone(),
            source,
        },
    })
}

#[cfg(test)]
mod tests;
