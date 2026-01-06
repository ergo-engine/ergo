# Standard Library Overview

> **Navigation aid only.** Authoritative content lives in the linked documents.

---

## Core Primitives

The v0 kernel includes domain-neutral standard library atoms.

### Sources

- `number_source` — Produces a configured number value
- `string_source` — Produces a configured string value (STRING-SOURCE-1)

### Computes

- `add`, `multiply`, `subtract`, `divide` — Arithmetic
- `gt`, `gte`, `lt`, `lte` — Comparison
- `abs`, `min`, `max` — Unary/binary operations
- `select_bool` — Conditional selection

### Triggers

- `emit_if_true` — Emits event when input is true

### Actions

- `ack_action` — Acknowledges trigger (test primitive)

**Catalog:** `crates/runtime/src/catalog.rs`

---

## Kernel Closure Rules

New core implementations require:

1. Vertical proof demonstrating necessity
2. New invariant with explicit enforcement locus
3. Action implementations in core = zero by design

**Source:** [PHASE_INVARIANTS.md](../CANONICAL/PHASE_INVARIANTS.md) Core v0.1 Freeze Declaration

---

## Primitive Manifests

Each primitive role has a manifest contract:

- Source — No inputs, deterministic origin
- Compute — Pure transformation, ≥1 input
- Trigger — Stateless event emission
- Action — Terminal, gated by events

**Source:** [PRIMITIVE_MANIFESTS/](../STABLE/PRIMITIVE_MANIFESTS/)

---

## Surface Coverage

Every `ValueType` must have at least one source producer (X.12).

| ValueType | Source |
|-----------|--------|
| Number | number_source |
| Bool | number_source + compute |
| String | string_source |
| Series | (derived from compute) |
| Event | (derived from trigger) |

**Source:** [PHASE_INVARIANTS.md](../CANONICAL/PHASE_INVARIANTS.md) X.12 / STRING-SOURCE-1

---

## See Also

- [KERNEL_CLOSURE.md](../CANONICAL/KERNEL_CLOSURE.md) — What "closed" means
- [Governance](governance.md) — Change rules
