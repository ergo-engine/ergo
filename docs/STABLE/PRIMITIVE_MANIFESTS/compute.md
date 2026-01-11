---
Authority: STABLE
Version: v0
Last Updated: 2026-01-11
---

# Compute Primitive Manifest — v0

## 1. Definition

A Compute Primitive is a pure, deterministic transform that maps
typed inputs → typed outputs on a declared execution cadence.

It:
- has no side effects
- performs no I/O
- does not know about execution mode
- is composable only via a graph
- contains no strategy or intent logic

The primitive itself is atomic.
All composition happens at the graph level.

---

## 2. Required Manifest Fields

Every compute primitive must declare all of the following.

### 2.1 Identity

```yaml
id: string
version: string
kind: compute
```

- `id` is stable and unique
- `version` is semver or monotonic
- `kind` must be `compute`

---

### 2.2 Inputs

```yaml
inputs:
  - name: string
    type: series | number | bool | event
    required: true
    cardinality: single | multiple
```

Rules:
- Inputs are explicit, named, and typed
- No implicit inputs exist
- Time, identifier, state, or context must be provided upstream
- Cardinality must be declared
- At least one input is required (zero-input nodes are Sources by definition)

---

### 2.3 Outputs

```yaml
outputs:
  - name: string
    type: series | number | bool | event
```

Rules:
- Outputs are named and typed
- Multiple outputs are first-class
- No undeclared outputs are permitted
- All declared outputs must be produced

---

### 2.4 Parameters (Presets Only)

```yaml
parameters:
  - name: string
    type: int | number | bool | string | enum
    default: any
    bounds: optional
```

Rules:
- Parameters are static presets
- Parameters do not change during execution
- Parameters must be fully serializable
- No hidden or dynamic parameters allowed

---

### 2.5 Execution Semantics

```yaml
execution:
  cadence: continuous | event
  deterministic: true
  may_error: true
```

Rules:

- `continuous` = evaluated every bar / tick
- `event` = evaluated only on upstream event emission
- Determinism is required
- Errors are permitted for domain-specific edge cases (see §2.8)

---

### 2.6 State

```yaml
state:
  allowed: true | false
  description: optional
```

Rules:
- State is allowed only if:
  - deterministic
  - resettable
  - time-indexed
- Rolling / windowed state is valid
- External or hidden state is forbidden

---

### 2.7 Side Effects

```yaml
side_effects: false
```

Hard rule:

- Compute primitives may not perform I/O
- May not access network, filesystem, or external state
- If it touches the world, it is not compute

---

### 2.8 Error Semantics

```yaml
errors:
  allowed: true
  deterministic: true
```

Rules:

- Compute primitives may produce errors for domain-specific edge cases
- Errors must be deterministic (same inputs → same error)
- When execution succeeds, all declared outputs must be produced
- When execution fails, no outputs are produced
- Errors surface as `ExecError::ComputeFailed` at runtime

Error types (B.2):

- `DivisionByZero` — Division by zero attempted
- `NonFiniteResult` — Result overflowed to infinity or NaN

Errors are semantic failures, not infrastructure failures. They are non-retryable.

See: PHASE_INVARIANTS.md B.2, NUM-FINITE-1

---

## 3. Prohibited Behavior

A compute primitive may not:

- Read or write files
- Access network resources
- Inspect execution mode
- Emit actions or alerts
- Access external state
- Mutate global state
- Branch on execution mode
- Accept undeclared inputs or parameters
- Emit undeclared outputs
- Emit non-deterministic errors

Violation invalidates the primitive.

---

## 4. Composition Rule

Compute primitives are atomic.
Composition occurs only at the graph level.

No nested graphs.
No internal orchestration.
No embedded control flow.

This preserves determinism and debuggability.

---

## 5. Orchestrator Contract

The orchestrator guarantees:
- Inputs match declared types
- Execution cadence is respected
- State is reset deterministically
- Outputs are captured exactly as declared

The orchestrator does not:
- infer intent
- modify parameters
- tolerate non-determinism
- interpret semantics

---

## 6. Consequences

Because this contract is enforced:
- The manifest is the single source of truth
- User-defined primitives cannot cheat
- Node UIs are pure projections of the manifest
- Canvas values are presets, not logic
- All execution modes share identical execution paths

---

## 7. Scope

This document defines Compute Primitive Manifest v0.

---

## 8. Contract Stability

This contract is STABLE.

Breaking changes require a manifest version bump.
