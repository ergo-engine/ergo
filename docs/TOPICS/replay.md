# Replay Overview

> **Navigation aid only.** Authoritative content lives in the linked documents.

---

## Replay Scope

Replay determinism covers **supervisor scheduling decisions only**.

- Same external events → identical scheduling decisions
- Internal graph execution is not captured
- Source outputs, compute results, action effects are not recorded

**Source:** [PHASE_INVARIANTS.md](../CANONICAL/PHASE_INVARIANTS.md) REP-SCOPE

---

## Capture Requirements

### Canonical Capture Bundle

Canonical capture and strict replay use a v1 bundle shape:

- `capture_version: "v1"`
- Required `adapter_provenance` field (adapter fingerprint or `none`)
- Unknown fields rejected at deserialization (`deny_unknown_fields`)
- Legacy `adapter_version` bundles rejected during deserialization in strict paths

**Source:** [PHASE_INVARIANTS.md](../CANONICAL/PHASE_INVARIANTS.md) Canonical Run / Replay Strictness (v1)

---

## Supervisor Replay Invariants

| ID | Invariant | Enforcement |
|----|-----------|-------------|
| REP-1 | Capture records are self-validating | validate_hash() + rehydrate_checked() |
| REP-2 | Rehydration is deterministic | Record fields only, no external state |
| REP-3 | Fault injection keys on EventId only | Type enforcement |
| REP-4 | Capture/runtime type separation | Separate serde types |
| REP-5 | No wall-clock time in supervisor | grep test |
| REP-7 | Strict replay provenance contract match | `replay_checked_strict` |

**Source:** [PHASE_INVARIANTS.md](../CANONICAL/PHASE_INVARIANTS.md) §8 + Canonical Run / Replay Strictness (v1)

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
