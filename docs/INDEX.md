# Documentation Index

> **Single-Source Rule:** All authoritative content lives in FROZEN/, STABLE/, CANONICAL/, or CONTRACTS/. Topic summaries in TOPICS/ are navigation aids only — they link to sources, never restate laws.

---

## Start Here

New to the system? Read these in order:

1. [KERNEL_CLOSURE](CANONICAL/KERNEL_CLOSURE.md) — What "kernel" and "closed" mean
2. [ontology](FROZEN/ontology.md) — The four primitives and their causal roles
3. [execution_model](FROZEN/execution_model.md) — How graphs evaluate
4. [V0_FREEZE](FROZEN/V0_FREEZE.md) — What is frozen vs patchable
5. [CLUSTER_SPEC](STABLE/CLUSTER_SPEC.md) — Data structures for composition
6. [UI_RUNTIME_CONTRACT](CONTRACTS/UI_RUNTIME_CONTRACT.md) — What a UI must emit
7. [AUTHORING_LAYER](STABLE/AUTHORING_LAYER.md) — Cluster composition concepts
8. [PHASE_INVARIANTS](CANONICAL/PHASE_INVARIANTS.md) — Enforcement loci for all invariants
9. [TERMINOLOGY](CANONICAL/TERMINOLOGY.md) — Canonical terms and usage

---

## By Topic

Quick navigation by concern:

| Topic | Summary | Key Documents |
|-------|---------|---------------|
| [Architecture](TOPICS/architecture.md) | System layers and trust boundaries | KERNEL_CLOSURE, adapter_contract, SUPERVISOR, AUTHORING_LAYER |
| [Semantics](TOPICS/semantics.md) | Execution rules and phase boundaries | ontology, execution_model, CLUSTER_SPEC, PHASE_INVARIANTS |
| [Contracts](TOPICS/contracts.md) | External interface specifications | UI_RUNTIME_CONTRACT, adapter_contract |
| [Authoring](TOPICS/authoring.md) | Building clusters and compositions | AUTHORING_LAYER, CLUSTER_SPEC |
| [Replay](TOPICS/replay.md) | Capture and replay determinism | SUPERVISOR (replay), adapter_contract (capture) |
| [Standard Library](TOPICS/stdlib.md) | Core primitives and implementations | KERNEL_CLOSURE, PHASE_INVARIANTS |
| [Governance](TOPICS/governance.md) | Change rules and version boundaries | V0_FREEZE, closure_register, PHASE_INVARIANTS, TERMINOLOGY |

---

## By Authority Level

Documents organized by their change requirements:

### FROZEN/ (v1 required to change)

| Document | Scope |
|----------|-------|
| [ontology.md](FROZEN/ontology.md) | Four primitives, wiring matrix, causal roles |
| [execution_model.md](FROZEN/execution_model.md) | Evaluation semantics, phase rules, determinism |
| [V0_FREEZE.md](FROZEN/V0_FREEZE.md) | What is frozen vs patchable, version boundaries |
| [adapter_contract.md](FROZEN/adapter_contract.md) | Trust boundary, replay guarantees, capture requirements |
| [SUPERVISOR.md](FROZEN/SUPERVISOR.md) | Orchestration layer, episode semantics, replay |

### STABLE/ (additive changes only)

| Document | Scope |
|----------|-------|
| [AUTHORING_LAYER.md](STABLE/AUTHORING_LAYER.md) | Cluster concepts, fractal composition, boundary kinds |
| [CLUSTER_SPEC.md](STABLE/CLUSTER_SPEC.md) | Data structures, inference algorithm, validation rules |
| [PRIMITIVE_MANIFESTS/](STABLE/PRIMITIVE_MANIFESTS/) | Contracts for Source, Compute, Trigger, Action |

### CANONICAL/ (tracks implementation)

| Document | Scope |
|----------|-------|
| [KERNEL_CLOSURE.md](CANONICAL/KERNEL_CLOSURE.md) | v0 baseline declaration, v1 workstream rules |
| [PHASE_INVARIANTS.md](CANONICAL/PHASE_INVARIANTS.md) | Phase boundaries, enforcement loci, gap tracking |
| [TERMINOLOGY.md](CANONICAL/TERMINOLOGY.md) | Canonical terms: primitive, implementation, cluster |

### CONTRACTS/ (external interfaces)

| Document | Scope |
|----------|-------|
| [UI_RUNTIME_CONTRACT.md](CONTRACTS/UI_RUNTIME_CONTRACT.md) | Data structures UI must emit for runtime |
| [INDEX.md](CONTRACTS/INDEX.md) | Contract index with brief descriptions |

---

## Documentation Rules

### Single-Source Principle

Every fact has exactly one authoritative location. Topic summaries and navigation aids:
- Link to sources
- Provide context
- Never restate laws

If you find duplicate content, the FROZEN/STABLE/CANONICAL/CONTRACTS version is authoritative.

### How Documents Work Together

- **PHASE_INVARIANTS.md** tracks which invariants are enforced where
- **closure_register.md** tracks semantic gaps and their resolutions
- Both reference spec documents (ontology, execution_model, etc.) as the source of truth

### Verified Against

All documents verified against tag: `v1.0.0-alpha.1`

---

## Crate-Local Documentation

READMEs under `crates/` are informational for their respective packages.
They are not authoritative for system behavior.
