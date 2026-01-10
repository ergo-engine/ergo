# Standard Library Overview

> **Navigation aid only.** Authoritative content lives in the linked documents.

---

## Core Primitives

The v0 kernel includes domain-neutral standard library atoms.

Total stdlib implementations: 30

### Sources (4)

- `number_source` — Produces a configured number value
- `boolean_source` — Produces a configured boolean value
- `string_source` — Produces a configured string value (STRING-SOURCE-1)
- `context_number_source` — Reads number from ExecutionContext (CONTEXT-NUMBER-SOURCE-1)

### Computes (23)

- `const_number` — Outputs a constant number
- `const_bool` — Outputs a constant boolean
- `add` — Adds two numbers
- `subtract` — Subtracts two numbers
- `multiply` — Multiplies two numbers
- `divide` — Division (v0.2.0: strict, errors on zero/non-finite per B.2)
- `safe_divide` — Division with required fallback for zero/non-finite (B.2)
- `abs` — Absolute value
- `negate` — Negates a number
- `gt` — Greater than comparison
- `gte` — Greater than or equal comparison
- `lt` — Less than comparison
- `lte` — Less than or equal comparison
- `eq` — Equality comparison
- `neq` — Inequality comparison
- `min` — Minimum of two numbers
- `max` — Maximum of two numbers
- `and` — Logical AND
- `or` — Logical OR
- `not` — Logical NOT
- `select` — Select between two numbers based on condition
- `select_bool` — Select between two booleans based on condition

### Triggers (1)

- `emit_if_true` — Emits when input is true

### Actions (2)

- `ack_action` — Acknowledges execution
- `annotate_action` — Adds annotation to execution context

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
| Number | number_source, context_number_source |
| Bool | boolean_source |
| String | string_source |
| Series | (derived from compute) |
| Event | (derived from trigger) |

**Source:** [PHASE_INVARIANTS.md](../CANONICAL/PHASE_INVARIANTS.md) X.12 / STRING-SOURCE-1

---

## See Also

- [KERNEL_CLOSURE.md](../CANONICAL/KERNEL_CLOSURE.md) — What "closed" means
- [Governance](governance.md) — Change rules
