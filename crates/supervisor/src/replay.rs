use std::sync::{Arc, Mutex};

use ergo_adapter::capture::{CaptureError, ExternalEventRecord};
use ergo_adapter::{EventId, RuntimeInvoker};

use crate::{
    CaptureBundle, DecisionLog, DecisionLogEntry, EpisodeInvocationRecord, Supervisor,
    NO_ADAPTER_PROVENANCE,
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
        guard.iter().map(EpisodeInvocationRecord::from).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    UnsupportedVersion { capture_version: String },
    HashMismatch { event_id: EventId },
    AdapterProvenanceMismatch { expected: String, got: String },
    UnexpectedAdapterProvidedForNoAdapterCapture,
    AdapterRequiredForProvenancedCapture,
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
    adapter_fingerprint: Option<&str>,
) -> Result<Vec<EpisodeInvocationRecord>, ReplayError> {
    validate_bundle(bundle)?;
    validate_replay_provenance(bundle, adapter_fingerprint)?;
    replay_inner(bundle, runtime)
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
    adapter_fingerprint: Option<&str>,
) -> Result<(), ReplayError> {
    let provenance = bundle.adapter_provenance.as_str();
    if provenance == NO_ADAPTER_PROVENANCE {
        if adapter_fingerprint.is_some() {
            return Err(ReplayError::UnexpectedAdapterProvidedForNoAdapterCapture);
        }
        return Ok(());
    }

    let Some(got) = adapter_fingerprint else {
        return Err(ReplayError::AdapterRequiredForProvenancedCapture);
    };

    if got != provenance {
        return Err(ReplayError::AdapterProvenanceMismatch {
            expected: provenance.to_string(),
            got: got.to_string(),
        });
    }

    Ok(())
}

fn rehydrate_event(
    record: &ExternalEventRecord,
) -> Result<ergo_adapter::ExternalEvent, ReplayError> {
    record.rehydrate_checked().map_err(|err| match err {
        CaptureError::PayloadHashMismatch { .. } => ReplayError::HashMismatch {
            event_id: record.event_id.clone(),
        },
    })
}
