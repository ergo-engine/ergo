use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{EventId, EventPayload, EventTime, ExternalEvent, ExternalEventKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureError {
    /// X.11-like guard: payload hash does not match stored hash.
    PayloadHashMismatch { expected: String, actual: String },
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

    /// Reconstructs an ExternalEvent without validating payload integrity.
    /// Prefer `rehydrate_checked()` in replay paths.
    pub fn rehydrate(&self) -> ExternalEvent {
        ExternalEvent::with_payload(
            self.event_id.clone(),
            self.kind,
            self.event_time,
            self.payload.clone(),
        )
    }

    pub fn rehydrate_checked(&self) -> Result<ExternalEvent, CaptureError> {
        let actual = hash_payload(&self.payload);
        if self.payload_hash != actual {
            return Err(CaptureError::PayloadHashMismatch {
                expected: self.payload_hash.clone(),
                actual,
            });
        }
        Ok(self.rehydrate())
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
            ExternalEventKind::Tick,
            EventTime::from_duration(std::time::Duration::default()),
            EventPayload {
                data: b"hello".to_vec(),
            },
        );

        let record = ExternalEventRecord::from_event(&event);
        assert!(record.rehydrate_checked().is_ok());
    }

    #[test]
    fn rehydrate_checked_err_when_hash_mismatch() {
        let event = ExternalEvent::with_payload(
            EventId::new("evt-2"),
            ExternalEventKind::Tick,
            EventTime::from_duration(std::time::Duration::default()),
            EventPayload {
                data: b"hello".to_vec(),
            },
        );

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
}
