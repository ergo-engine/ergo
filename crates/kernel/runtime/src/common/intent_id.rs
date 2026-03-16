use sha2::{Digest, Sha256};

/// Derive a deterministic correlation ID for an emitted intent.
pub fn derive_intent_id(
    graph_id: &str,
    event_id: &str,
    node_runtime_id: &str,
    intent_kind: &str,
    intent_ordinal: usize,
) -> String {
    let version_tag = "eid1";
    let ordinal = intent_ordinal.to_string();
    let mut bytes = Vec::new();
    for segment in [
        version_tag,
        graph_id,
        event_id,
        node_runtime_id,
        intent_kind,
        ordinal.as_str(),
    ] {
        push_len_prefixed(&mut bytes, segment);
    }

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    format!("eid1:sha256:{}", to_hex(&digest))
}

fn push_len_prefixed(out: &mut Vec<u8>, segment: &str) {
    let bytes = segment.as_bytes();
    let len = u32::try_from(bytes.len()).expect("intent_id segment exceeds u32 max length");
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(bytes);
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::derive_intent_id;

    #[test]
    fn derive_intent_id_is_deterministic() {
        let id1 = derive_intent_id("g1", "evt1", "n0", "place_order", 0);
        let id2 = derive_intent_id("g1", "evt1", "n0", "place_order", 0);
        assert_eq!(id1, id2);
    }

    #[test]
    fn derive_intent_id_changes_with_event_id() {
        let id1 = derive_intent_id("g1", "evt1", "n0", "place_order", 0);
        let id2 = derive_intent_id("g1", "evt2", "n0", "place_order", 0);
        assert_ne!(id1, id2);
    }

    #[test]
    fn derive_intent_id_changes_with_node_runtime_id() {
        let id1 = derive_intent_id("g1", "evt1", "n0", "place_order", 0);
        let id2 = derive_intent_id("g1", "evt1", "n1", "place_order", 0);
        assert_ne!(id1, id2);
    }

    #[test]
    fn derive_intent_id_changes_with_intent_ordinal() {
        let id1 = derive_intent_id("g1", "evt1", "n0", "place_order", 0);
        let id2 = derive_intent_id("g1", "evt1", "n0", "place_order", 1);
        assert_ne!(id1, id2);
    }

    #[test]
    fn derive_intent_id_golden_regression() {
        let actual = derive_intent_id("graph_alpha", "evt_001", "n42", "place_order", 3);
        let expected =
            "eid1:sha256:157aed720bd20c1712cede0a499cf6607f687401a29d70ce5df5d2778f85a9fa";
        assert_eq!(actual, expected);
    }
}
