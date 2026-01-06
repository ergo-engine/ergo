# Ergo Documentation

**[Start Here: INDEX.md](INDEX.md)** — Documentation entry point with reading order and topic navigation.

---

This directory is the authoritative home for specifications and doctrine.

---

## Authority Hierarchy

Documents are organized by authority level. When documents conflict, higher authority wins.

| Level | Meaning | Change Requirements |
|-------|---------|---------------------|
| **FROZEN** | Cannot change without v1 | Sebastian + joint agent escalation |
| **STABLE** | Stable contracts; additive changes only | Review by Claude + ChatGPT |
| **CANONICAL** | Derived checklists and terminology | Owned by Claude; tracks implementation |
| **CONTRACTS** | External interfaces | Review required |

**Rule:** If implementation contradicts a higher-authority document, implementation is wrong.

---

## Document Index

### FROZEN/

Core system laws. Changes require v1.

| Document | Scope |
|----------|-------|
| `ontology.md` | Four primitives, wiring matrix, causal roles |
| `execution_model.md` | Evaluation semantics, phase rules, determinism |
| `V0_FREEZE.md` | What is frozen vs patchable, version boundaries |
| `adapter_contract.md` | Trust boundary, replay guarantees, capture requirements |
| `SUPERVISOR.md` | Orchestration layer, episode semantics, replay |

### STABLE/

Stable specifications. May evolve without triggering v1, but changes require review.

| Document | Scope |
|----------|-------|
| `AUTHORING_LAYER.md` | Cluster concepts, fractal composition, boundary kinds |
| `CLUSTER_SPEC.md` | Data structures, inference algorithm, validation rules |

#### PRIMITIVE_MANIFESTS/

Contracts for each primitive role. Breaking changes require manifest version bump.

| Document | Scope |
|----------|-------|
| `source.md` | Source primitive contract |
| `compute.md` | Compute primitive contract |
| `trigger.md` | Trigger primitive contract |
| `action.md` | Action primitive contract |

### CANONICAL/

Operational documents. Owned by Claude (Structural Auditor). Updated as system evolves.

| Document | Scope |
|----------|-------|
| `PHASE_INVARIANTS.md` | Phase boundaries, enforcement loci, gap tracking |
| `TERMINOLOGY.md` | Canonical terms: primitive, implementation, cluster |

### CONTRACTS/

External interface specifications.

| Document | Scope |
|----------|-------|
| `UI_RUNTIME_CONTRACT.md` | Data structures UI must emit for runtime |

---

## Crate-Local Documentation

READMEs under `crates/` are informational for their respective packages.
They are not authoritative for system behavior.

---

## Quick Reference

**The four primitives:**
- Source → origin
- Compute → truth  
- Trigger → causality
- Action → agency

**The invariant:**
> All authoring constructs compile away before execution.
> The runtime sees only the four primitives and their wiring rules.

**The rule:**
> An invariant without an enforcement locus is not an invariant. It is a wish.
