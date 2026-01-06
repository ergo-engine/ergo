# Architecture Overview

> **Navigation aid only.** Authoritative content lives in the linked documents.

---

## System Layers

The Ergo system is organized into distinct layers with clear trust boundaries.

### Layer Diagram

```
Scenario Planner (out of scope v0)
        │
   Supervisor  ← SUPERVISOR.md
        │
     Runtime   ← execution_model.md
        │
     Adapter   ← adapter_contract.md
```

---

## Key Documents

### Kernel Definition

- **[KERNEL_CLOSURE.md](../CANONICAL/KERNEL_CLOSURE.md)** — Defines what "kernel" means: runtime + supervisor + adapter + contracts

### Trust Boundaries

- **[adapter_contract.md](../FROZEN/adapter_contract.md)** — Adapter compliance requirements, replay determinism, declared semantic shaping

### Orchestration

- **[SUPERVISOR.md](../FROZEN/SUPERVISOR.md)** — Episode scheduling, strategy-neutral constraints, DecisionLog

### Authoring vs Runtime

- **[AUTHORING_LAYER.md](../STABLE/AUTHORING_LAYER.md)** — Clusters compile away before execution; runtime sees only primitives

---

## Core Invariant

> All authoring constructs compile away before execution.
> The runtime sees only the four primitives and their wiring rules.

— V0_FREEZE.md §7.2

---

## See Also

- [Semantics](semantics.md) — Execution rules
- [Contracts](contracts.md) — External interfaces
