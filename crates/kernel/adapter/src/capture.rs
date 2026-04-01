//! capture
//!
//! Purpose:
//! - Define the kernel-owned capture record format for external events and the
//!   typed rehydration/integrity failures that replay consumes.
//!
//! Owns:
//! - `CaptureError` as the canonical event-record integrity/rehydration error.
//! - Deterministic payload hashing and `ExternalEventRecord` rehydration rules.
//!
//! Does not own:
//! - Replay orchestration or host-facing replay diagnostics.
//! - Higher-level capture bundle write policy.
//!
//! Connects to:
//! - `ergo_supervisor::replay`, which turns event-record failures into replay
//!   preflight/rehydration failures.
//! - Host replay, which should preserve this typed error instead of flattening it.
//!
//! Safety notes:
//! - Hash mismatch remains distinct from payload materialization failure.
//! - Invalid payload rehydration preserves `ExternalEventPayloadError` as the
//!   causal source rather than storing only rendered text.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    EventId, EventPayload, EventTime, ExternalEvent, ExternalEventKind, ExternalEventPayloadError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureError {
    /// X.11-like guard: payload hash does not match stored hash.
    PayloadHashMismatch { expected: String, actual: String },
    /// Payload bytes are hash-consistent but cannot be materialized into an ExternalEvent context.
    InvalidPayload { source: ExternalEventPayloadError },
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PayloadHashMismatch { expected, actual } => write!(
                f,
                "payload hash mismatch (expected '{}', actual '{}')",
                expected, actual
            ),
            Self::InvalidPayload { source } => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for CaptureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PayloadHashMismatch { .. } => None,
            Self::InvalidPayload { source } => Some(source),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalEventRecord {
    pub event_id: EventId,
    pub event_time: EventTime,
    pub kind: ExternalEventKind,
    pub payload: EventPayload,
    pub payload_hash: String,
}

impl ExternalEventRecord {
    pub fn from_event(event: &ExternalEvent) -> Self {
        let payload_hash = hash_payload(&event.payload);
        Self {
            event_id: event.event_id().clone(),
            event_time: event.at(),
            kind: event.kind(),
            payload: event.payload().clone(),
            payload_hash,
        }
    }

    /// Reconstructs an ExternalEvent without validating payload hash integrity.
    /// Payload bytes are still validated for JSON/object shape during rehydration.
    /// Prefer `rehydrate_checked()` in replay paths.
    ///
    /// This is `pub(crate)` to prevent external callers from bypassing
    /// hash validation. See HARDEN-REHYDRATE-1.
    pub(crate) fn rehydrate(&self) -> Result<ExternalEvent, CaptureError> {
        ExternalEvent::with_payload(
            self.event_id.clone(),
            self.kind,
            self.event_time,
            self.payload.clone(),
        )
        .map_err(|source| CaptureError::InvalidPayload { source })
    }

    pub fn rehydrate_checked(&self) -> Result<ExternalEvent, CaptureError> {
        let actual = hash_payload(&self.payload);
        if self.payload_hash != actual {
            return Err(CaptureError::PayloadHashMismatch {
                expected: self.payload_hash.clone(),
                actual,
            });
        }
        self.rehydrate()
    }

    /// Validates integrity of `payload.data` against the stored hash.
    pub fn validate_hash(&self) -> bool {
        self.payload_hash == hash_payload(&self.payload)
    }
}

pub fn hash_payload(payload: &EventPayload) -> String {
    let mut hasher = Sha256::new();
    hasher.update(&payload.data);
    let digest = hasher.finalize();
    hex::encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rehydrate_checked_ok_when_hash_matches() {
        let event = ExternalEvent::with_payload(
            EventId::new("evt-1"),
            ExternalEventKind::Pump,
            EventTime::from_duration(std::time::Duration::default()),
            EventPayload {
                data: br#"{"x":1}"#.to_vec(),
            },
        )
        .expect("object payload should construct event");

        let record = ExternalEventRecord::from_event(&event);
        assert!(record.rehydrate_checked().is_ok());
    }

    #[test]
    fn rehydrate_checked_err_when_hash_mismatch() {
        let event = ExternalEvent::with_payload(
            EventId::new("evt-2"),
            ExternalEventKind::Pump,
            EventTime::from_duration(std::time::Duration::default()),
            EventPayload {
                data: br#"{"x":1}"#.to_vec(),
            },
        )
        .expect("object payload should construct event");

        let mut record = ExternalEventRecord::from_event(&event);
        // Corrupt the payload to force mismatch
        record.payload.data = b"corrupted".to_vec();

        match record.rehydrate_checked() {
            Err(CaptureError::PayloadHashMismatch { expected, actual }) => {
                assert_ne!(expected, actual, "hashes should differ after corruption");
            }
            other => panic!("expected PayloadHashMismatch, got {:?}", other),
        }
    }

    #[test]
    fn rehydrate_checked_err_when_payload_not_json_object() {
        let event = ExternalEvent::with_payload(
            EventId::new("evt-3"),
            ExternalEventKind::Pump,
            EventTime::from_duration(std::time::Duration::default()),
            EventPayload {
                data: br#"{"x":1}"#.to_vec(),
            },
        )
        .expect("object payload should construct event");

        let mut record = ExternalEventRecord::from_event(&event);
        record.payload.data = br#""not-an-object""#.to_vec();
        record.payload_hash = hash_payload(&record.payload);

        match record.rehydrate_checked() {
            Err(CaptureError::InvalidPayload { source }) => {
                assert!(
                    source
                        .to_string()
                        .contains("payload must be a JSON object, got string"),
                    "unexpected detail: {source}"
                );
            }
            other => panic!("expected InvalidPayload, got {:?}", other),
        }
    }
}
