# Semantics Overview

> **Navigation aid only.** Authoritative content lives in the linked documents.

---

## Primitive Ontology

The system has exactly four ontological primitives. This set is closed.

| Primitive | Causal Role | Key Characteristic |
|-----------|-------------|---------------------|
| Source | Origin | No inputs, introduces data |
| Compute | Truth | Pure transformation |
| Trigger | Causality | Converts values to events |
| Action | Agency | Attempts external effects |

**Source:** [ontology.md](../FROZEN/ontology.md) §2

---

## Wiring Matrix

```
Source  → Compute   : allowed
Source  → Trigger   : forbidden (v0)
Source  → Action    : allowed for scalar payload inputs only (trigger event gate still required)
Compute → Compute   : allowed
Compute → Trigger   : allowed
Compute → Action    : allowed for scalar payload inputs only (trigger event gate still required)
Trigger → Trigger   : allowed
Trigger → Action    : allowed (event gating)
Action  → *         : forbidden (terminal)
*       → Source    : forbidden
```

**Source:** [ontology.md](../FROZEN/ontology.md) §3

**Pending freeze amendment note:** `FROZEN/ontology.md` still contains the legacy coarse row
`Compute → Action : forbidden`. STABLE Action/Cluster contracts now refine Action inputs into
Trigger-gated `event` inputs and scalar payload inputs from `Source`/`Compute` without
changing primitive roles.

---

## Execution Model

### Single-Pass Evaluation

- Each node executes at most once per pass
- Topological order
- No cycles

### Determinism

- Identical inputs + identical state = identical outputs
- External nondeterminism confined to adapter boundary

**Source:** [execution_model.md](../FROZEN/execution_model.md)

---

## Cluster Expansion

Clusters flatten to primitives before execution:

- Signature inference from expanded graph
- BoundaryKind mirrors primitive kinds
- All parameters must be bound

**Source:** [CLUSTER_SPEC.md](../STABLE/CLUSTER_SPEC.md) §3–7

---

## Phase Invariants

Every phase boundary has enforced invariants:

- Definition (D.1–D.11)
- Instantiation (I.1–I.6)
- Expansion (E.1–E.8)
- Inference (F.1–F.6)
- Validation (V.1–V.7)
- Execution (R.1–R.7)
- Orchestration (CXT-1, SUP-1–7)
- Replay (REP-1–5)

**Source:** [PHASE_INVARIANTS.md](../CANONICAL/PHASE_INVARIANTS.md)

---

## See Also

- [Architecture](architecture.md) — System layers
- [Governance](governance.md) — Change rules
