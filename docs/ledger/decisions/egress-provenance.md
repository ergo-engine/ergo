---
Authority: PROJECT
Date: 2026-03-16
Decision-Owner: Sebastian (Architect)
Participants: Claude (Structural Auditor), Codex A + Codex B (Ontology Guardian)
Status: DECIDED
Scope: v1
Parent-Decision: egress-routing-config.md, egress-timing-lifecycle.md
Resolves: GW-EFX-3C
---

# Decision: Egress Channel Provenance in Capture

## Context

Capture artifacts currently include adapter provenance (manifest
fingerprint) and runtime provenance (`rpv1:sha256:...` hash of graph
structure). Per-ack channel identity exists via
`CapturedIntentAck.channel`. But there is no run-level record of what
egress configuration was in place — what channels existed, what
processes backed them, what routes mapped kinds to channels, and what
timeout policy governed dispatch.

This decision defines run-level egress provenance.

---

## Ruling

### Provenance field

Add `egress_provenance: Option<String>` to the top-level capture
bundle metadata, alongside `adapter_provenance` and
`runtime_provenance`.

```rust
pub struct CaptureBundle {
    // ... existing fields ...
    pub adapter_provenance: String,
    pub runtime_provenance: String,
    pub egress_provenance: Option<String>,  // NEW
}
```

`#[serde(default, skip_serializing_if = "Option::is_none")]` for
backward compatibility with existing captures.

### Hash scheme

`epv1:sha256:{hex}` — egress provenance v1.

Follows the existing convention (`rpv1:sha256:...` for runtime
provenance).

### Hash input

Hash the **full normalized `EgressConfig`**, including:

- `default_ack_timeout`
- `channels: BTreeMap<String, EgressChannelConfig>` (full config
  per channel, including process command)
- `routes: BTreeMap<String, EgressRoute>` (including per-route
  `ack_timeout`)

Exclude:

- Ready-handshake `handled_kinds` — these are runtime attestation,
  not configured provenance. They verify that the channel CAN handle
  the routed kinds, but they are observed at runtime, not configured.

### Why include timeouts

This was debated. The resolution:

`ack_timeout` is not cosmetic operational tuning. Under per-step
blocking with Option C artifact policy, timeout directly determines:

- Whether a step completes or interrupts
- Which intent acks make it into the capture
- Where the interruption marker lands

Two runs with identical routing but different timeout policy can have
materially different capture artifacts. Excluding timeout from
provenance would create false equivalence — the same provenance hash
for configurations that produce observably different run behavior.

Provenance should answer "what configuration was in place for this
run," not "what routes existed." The full config is the honest answer.

### When to store

- If `EgressConfig` is present for the run, always store
  `Some("epv1:sha256:...")`, even if no intent was emitted during
  the run.
- If no egress config exists (`egress_config: None`), store `None`.

### Replay strictness

**Audit-only for v0.** Egress provenance is stored in the capture
for human/tool inspection. Strict replay does NOT validate against
egress provenance. Strict replay continues to validate adapter and
runtime provenance only.

Rationale:

- Replay correctness is about event/capture/effect determinism.
- Egress config affects external realization policy, not graph
  replay determinism.
- Forcing egress config into replay would over-couple the replay
  surface for little gain.
- Replay doesn't have a live `EgressConfig` — it has
  `replay_external_kinds` derived from the graph. It can't recompute
  the hash.

If a comparison surface is needed later, it should be optional and
non-failing.

### Per-ack channel identity vs run-level provenance

These are complementary, not redundant:

- `CapturedIntentAck.channel` answers: "which channel accepted this
  specific intent?"
- `egress_provenance` answers: "what whole routing/implementation
  config was in place for this run?"

Different questions, both useful for audit.

### Future: structural comparator

A secondary `egress_structural_provenance` field may be added later
for low-noise comparison that excludes timeouts — answering only
"did routing/implementation change?" without being affected by
timeout tuning. This is not part of this decision. The canonical
`egress_provenance` field is the full-fidelity audit hash.

---

## Implementation

### Hash computation

```rust
fn compute_egress_provenance(config: &EgressConfig) -> String {
    let bytes = serde_json::to_vec(config)
        .expect("EgressConfig must be serializable");
    let digest = sha2::Sha256::digest(&bytes);
    format!("epv1:sha256:{}", hex::encode(digest))
}
```

`EgressConfig` uses `BTreeMap` for channels and routes (deterministic
serialization, established in the routing config decision). The hash
is stable across platforms given identical config.

### Integration point

Compute in the host run path after egress config validation, before
first event. Store on the capture bundle at finalization.

---

## What This Does NOT Decide

- **Structural-only comparator.** May be added as a secondary field
  later.
- **Replay validation against egress provenance.** Explicitly not
  done in v0.
- **Egress config capture beyond hash.** The full config is not
  stored in the capture, only its hash. If full config preservation
  is needed, that's a separate concern.

---

## Impacted Files

- `CaptureBundle` (or equivalent) — new `egress_provenance` field
- Host run path — compute hash after config validation
- Capture finalization — store hash on bundle
- `EgressConfig` serialization — already deterministic via BTreeMap
