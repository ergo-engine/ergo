---
Authority: STABLE
Version: v0
Last Updated: 2025-12-22
Last Amended: 2026-01-11
---

# Action Primitive Manifest — v0

This is the authoritative contract.

---

## 1. Definition

An Action Primitive is a deterministic command that attempts to apply a
side effect to an external execution environment,
gated by an event, and emits a terminal outcome event.

Actions:
- are the only primitives allowed to cause side effects
- do not compute signals
- do not infer intent
- do not decide when to act (triggers do that)
- only decide what to attempt

An action answers one question:

*"Given this event, what command should be attempted?"*

---

## 2. Required Manifest Fields

Every action primitive must declare all of the following.

---

### 2.1 Identity

```yaml
id: string
version: string
kind: action
```

Rules:
- `id` is unique and stable
- `version` is semver or monotonic
- `kind` must be `action`

---

### 2.2 Inputs

```yaml
inputs:
  - name: string
    type: event | number | bool | string
    required: true
    cardinality: single
```

Rules:
- At least one input must be an event
- Inputs are explicit, named, and typed
- No implicit access to external state or context
- All required context must be provided upstream

---

### 2.3 Outputs

```yaml
outputs:
  - name: outcome
    type: event
```

Rules:
- Actions always emit exactly one outcome event
- Outcome event represents:
  - attempted
  - succeeded
  - rejected
  - cancelled
  - failed
- No additional outputs allowed in v0

---

### 2.4 Parameters

```yaml
parameters:
  - name: string
    type: int | number | bool | string | enum
    default: any
    bounds: optional
```

Rules:
- Parameters are static presets
- Parameters must be serializable
- Parameters do not change at runtime
- No hidden parameters allowed

---

### 2.5 Execution Semantics

```yaml
execution:
  deterministic: true
  retryable: false
```

Rules:
- Determinism is required
- Retry behavior (if any) must be explicit
- Action may be attempted at most once per triggering event in v0

---

### 2.6 State

```yaml
state:
  allowed: false
```

Rules:
- Action primitives may not hold internal state
- All stateful behavior must live upstream (compute / trigger)
- Idempotency must be handled by the orchestrator

---

### 2.7 Side Effects

```yaml
side_effects: true
```

Rules:
- Action primitives are the only primitives where this is allowed
- Side effects are limited to declared external operations
- No filesystem, network, or logging side effects in v0

---

## 3. Outcome Event Semantics (Critical)

An outcome event is:
- discrete
- emitted exactly once per action attempt
- terminal (no persistence)

Rules:
- Outcome events do not carry payloads in v0
- Outcome events are **non-wireable** in v0:
  - They do not participate in graph causality
  - No downstream node may consume an Action outcome (Action → * is forbidden)
- Outcome events may be consumed only by **external sinks** (orchestrator logging/audit/replay), outside the graph
- Outcome events must always be emitted, even on failure

#### Amendment Record

> **Amended 2026-01-11** by Sebastian (Freeze Authority)
>
> Removed incorrect claim that Action outcome events are consumable by Triggers. Clarified that
> Action outcomes are non-wireable and exist only for external sinks (logging/audit/replay).
> This is a documentation alignment with the FROZEN wiring matrix and validator behavior.

---

## 4. Prohibited Behavior

An action primitive may not:
- Emit multiple events
- Emit continuous values
- Perform computation
- Read execution mode
- Hold internal state
- Chain side effects
- Access undeclared inputs or parameters
- Bypass manifest enforcement

Violation invalidates the primitive.

---

## 5. Composition Rule

Actions terminate causality within the graph.

- Compute produces values
- Trigger produces events
- Action attempts execution (external effect)

Wiring rule (v0):
- **Action → * is forbidden** (terminal)

Action outcomes do not feed into Compute/Trigger/Action. Any follow-on behavior must occur
via subsequent episodes: Actions write externally; Sources observe externally; Triggers gate
Actions based on those observations.

---

## 6. Orchestrator Contract

The orchestrator guarantees:
- Inputs match declared types
- Triggering event occurred
- Action is attempted exactly once
- Outcome event is emitted

The orchestrator does not:
- infer retries
- interpret outcome semantics
- guarantee operation success
- alter parameters

---

## 7. Canonical Action Examples (v0)

**submit_command**
- inputs: `trigger:event`, `command_spec:string`
- outputs: `outcome:event`
- state: `false`

**cancel_command**
- inputs: `trigger:event`, `command_id:string`
- outputs: `outcome:event`
- state: `false`

**modify_command**
- inputs: `trigger:event`, `command_id:string`, `new_spec:string`
- outputs: `outcome:event`
- state: `false`

**cancel_all**
- inputs: `trigger:event`
- outputs: `outcome:event`
- state: `false`

---

## 8. Scope

This document defines Action Primitive Manifest v0.

Out of scope:
- alerts
- notifications
- payload-carrying outcomes
- custom code execution
- retries
- partial outcomes as first-class events

These belong to later versions.

---

## 9. Contract Stability

This contract is STABLE.

Breaking changes require a manifest version bump.
