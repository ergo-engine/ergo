# Replay Overview

> **Navigation aid only.** Authoritative content lives in the linked documents.

---

## Replay Scope

Canonical replay (D3 Scope A) verifies:

- Same captured events + same provenances -> identical supervisor decisions
- Host-owned effect integrity (`decisions[].effects`) after host re-execution
- Event payload hash checks before rehydration

Scope limits:

- Cross-ingestion normalization parity is deferred (`INGEST-TIME-1`)
- Internal source/compute payload outputs are not replay-captured as first-class records

**Source:** [PHASE_INVARIANTS.md](../CANONICAL/PHASE_INVARIANTS.md) REP-SCOPE

---

## Capture Requirements

### Canonical Capture Bundle

Canonical capture and replay use a v2 bundle shape:

- `capture_version: "v2"`
- Required `adapter_provenance` field (adapter fingerprint or `none`)
- Required `runtime_provenance` field (`rpv1:sha256:<hex>`)
- Required `decisions[].effects` field (empty vector allowed; missing field invalid)
- Strict replay rejects duplicate `events[].event_id` values
- Unknown fields rejected at deserialization (`deny_unknown_fields`)
- Legacy `adapter_version` bundles rejected during deserialization in strict paths
- This repo treats capture bundles/fixtures as ephemeral artifacts; schema compatibility can be intentionally broken when fixtures are migrated in the same change

**Source:** [PHASE_INVARIANTS.md](../CANONICAL/PHASE_INVARIANTS.md) Canonical Run / Replay Strictness (v2)

---

## Supervisor Replay Invariants

| ID | Invariant | Enforcement |
|----|-----------|-------------|
| REP-1 | Capture records are self-validating | validate_hash() + rehydrate_checked() |
| REP-2 | Rehydration is deterministic | Record fields only, no external state |
| REP-3 | Fault injection keys on EventId only | Type enforcement |
| REP-4 | Capture/runtime type separation | Separate serde types |
| REP-5 | No wall-clock time in supervisor | grep test |
| REP-7 | Strict replay provenance contract match (adapter + runtime surface) | `replay_checked_strict` |

**Source:** [PHASE_INVARIANTS.md](../CANONICAL/PHASE_INVARIANTS.md) §8 + Canonical Run / Replay Strictness (v2)

---

## Host Replay Authority

Canonical replay execution is host-owned:

- strict preflight (`validate_bundle_strict`)
- event rehydration with hash checks
- `HostedRunner::replay_step(...)` execution
- effect-integrity comparison against host-enriched capture decisions
- host effect enrichment keyed by decision order (`decisions[i]`), not by `event_id`

The supervisor remains the scheduling authority inside this host replay path.

---

## DecisionLog

The append-only record of Supervisor decisions:

- External event received
- Decision: invoke / skip / defer
- Episode invocation ID
- RunTermination observed

**Source:** [SUPERVISOR.md](../FROZEN/SUPERVISOR.md) §2.5

---

## Trust Boundary

Source primitive determinism is **trust-based, not enforced**.

- Manifest declares `execution.deterministic = true`
- No compile-time restrictions on non-deterministic implementations
- Enforcement is by convention and code review

**Source:** [PHASE_INVARIANTS.md](../CANONICAL/PHASE_INVARIANTS.md) SOURCE-TRUST

---

## See Also

- [Architecture](architecture.md) — System layers
- [Governance](governance.md) — Closure register
