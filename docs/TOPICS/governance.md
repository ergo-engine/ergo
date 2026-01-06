# Governance Overview

> **Navigation aid only.** Authoritative content lives in the linked documents.

---

## Authority Hierarchy

| Level | Meaning | Change Requirements |
|-------|---------|---------------------|
| FROZEN | Cannot change without v1 | Sebastian + joint agent escalation |
| STABLE | Stable contracts; additive only | Review by Claude + ChatGPT |
| CANONICAL | Derived checklists and terminology | Owned by Claude; tracks implementation |
| CONTRACTS | External interfaces | Review required |

**Source:** [README.md](../README.md)

---

## v0 Freeze

The v0 kernel is closed. Work may continue, but the kernel's meaning is now a fixed reference point.

### Closed = semantics-stable reference point

- May patch bugs that violate declared invariants
- May not "improve behavior" by quietly changing meanings
- New semantics obligations require explicit v1 decision record

**Source:** [KERNEL_CLOSURE.md](../CANONICAL/KERNEL_CLOSURE.md)

---

## What is Frozen

- Ontological primitives (4)
- Wiring rules
- Graph structure (DAG)
- Execution model (single-pass, topological)
- Action semantics
- Trigger statelessness
- Determinism guarantees

**Source:** [V0_FREEZE.md](../FROZEN/V0_FREEZE.md) §2

---

## What May Change (Patchable)

- Executor implementation details
- Validation bugs
- Error messages and diagnostics
- Performance optimizations
- Non-normative documentation

**Source:** [V0_FREEZE.md](../FROZEN/V0_FREEZE.md) §5

---

## Closure Register

Tracks semantic gaps, hardening closures, and explicit v0 rejections.

Every closure must specify:

1. Disposition (CLOSE / REJECT / V1 SEMANTICS)
2. Enforcement locus
3. Test evidence
4. PR/commit reference

**Source:** [closure_register.md](../closure_register.md)

---

## Terminology Rules

Canonical terms prevent semantic drift:

- "Primitive" = ontological role only
- "Implementation" = concrete executable
- "Cluster" = composed structure

**Source:** [TERMINOLOGY.md](../CANONICAL/TERMINOLOGY.md)

---

## See Also

- [PHASE_INVARIANTS.md](../CANONICAL/PHASE_INVARIANTS.md) — Enforcement loci
- [V0_FREEZE.md](../FROZEN/V0_FREEZE.md) — Full freeze specification
