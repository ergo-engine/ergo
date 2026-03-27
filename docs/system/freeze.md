---
Authority: FROZEN
Version: v0
Last Amended: 2026-03-26
Scope: What is frozen vs patchable, version boundaries
Verified Against Tag: v1.0.0-alpha.1
Change Rule: v1 only
---

# Primitive Ontology & Execution — v0 Freeze

This document defines what is frozen in v0, what may be patched, and where each constraint is specified.

Anything listed as frozen requires a v1 to change.

---

## 1. Ontological Primitives (Frozen)

The v0 system has exactly four ontological primitives:

- **Source** — origin of data
- **Compute** — derivation of truth from inputs
- **Trigger** — causality ("when something happens")
- **Action** — agency (external intent)

No additional ontological primitives may be introduced in v0.

📍 Defined in: `ontology.md`

---

## 2. Load-Bearing Invariants (Frozen)

The following invariants are foundational and frozen in v0.

### 2.1 Role Separation

- Sources have no inputs.
- Compute primitives must declare ≥1 input.
- Triggers emit events.
- Actions are terminal and stateless.
- Trigger and Compute share execution semantics; they differ in declared causal role and wiring permissions.

📍 Defined in: `ontology.md`, Compute/Trigger/Action manifest specs

### 2.2 Wiring Rules (v0)

The following wiring rules are authoritative:

```
Source → Compute     : allowed
Source → Trigger     : forbidden (v0)
Source → Action      : allowed only for non-Event payload inputs (non-Event payload types: Number/Series/Bool/String); does not satisfy Action gate requirement
Compute → Compute    : allowed
Compute → Trigger    : allowed
Compute → Action     : allowed only for non-Event payload inputs (non-Event payload types: Number/Series/Bool/String); does not satisfy Action gate requirement
Trigger → Trigger    : allowed
Trigger → Action     : allowed for Event gate inputs only (Event → Event); every Action must have at least one Trigger-provided Event input
Action → *           : forbidden (terminal)
* → Source           : forbidden
```

Graphs violating these rules are invalid.

Action input gating clarification (frozen):

- Every Action must have at least one Event input wired from a Trigger.
- Only Event inputs participate in Action execution gating.
- Non-Event Action inputs are scalar payload inputs (Number/Series/Bool/String) and may be wired from Source or Compute outputs.
- Scalar payload inputs do not satisfy the Action gate requirement.
- Trigger outputs may satisfy Action gate ports only (Event → Event). Trigger cannot supply scalar payload inputs (v0).

📍 Defined in: `ontology.md`

### 2.3 Graph Structure

- Graphs are directed acyclic graphs (DAGs).
- Cycles are forbidden.
- Trigger → Trigger chaining is allowed.
- Trigger cycles are forbidden.

📍 Defined in: `ontology.md`, `execution.md`

### 2.4 Execution Model

- Execution occurs in single evaluation passes.
- Each node executes at most once per pass.
- Nodes are evaluated in topological order.
- Primitive kinds do not impose global execution phases.

📍 Defined in: `execution.md`

### 2.5 Action Semantics

- All nodes must pass validation before any action executes.
- Actions execute sequentially in deterministic topological order inherited from their trigger dependencies.
- If multiple actions are topologically independent, the current implementation tie-breaks them deterministically by validated node/runtime order.
- Action outcome values such as `Rejected`, `Cancelled`, `Failed`, and `Skipped` are ordinary outputs; they do not themselves abort the pass.
- If runtime execution itself fails during a pass, subsequent actions are not executed and the episode-local buffered effects are dropped before post-episode dispatch.
- Post-episode host dispatch remains non-transactional; prior external effects are not automatically reversible.

📍 Defined in: `execution.md`

### 2.6 Trigger Statelessness

Triggers are ontologically stateless.

A Trigger:

- Gates whether an Action may attempt to affect the external world
- Evaluates inputs and emits `Emitted` or `NotEmitted`
- Has no memory of prior evaluations
- Does not store information, accumulate history, or own temporal memory

#### Execution-Local Bookkeeping

Trigger implementations may use ephemeral, execution-local bookkeeping during
evaluation. This bookkeeping:

- Is not state
- Is not observable or serializable
- Is not preserved across evaluations
- Does not participate in causality

#### Temporal Patterns

Behaviors requiring memory (once, count, latch, debounce, edge detection) are
**not triggers**. They are compositional patterns expressed as clusters using:

- Source (read persisted state)
- Compute (evaluate policy)
- Trigger (emit event based on computed boolean)
- Action (write updated state)

#### Boundary Rule

> Execution may use memory. The system may never observe, preserve, or depend on
> that memory.

#### Amendment Record

> **Amended 2025-12-28** by Sebastian (Freeze Authority)
>
> Prior language stating "Triggers may hold internal state" was a semantic error.
> This amendment corrects the error to align with the system's actual invariant
> structure. Triggers are stateless primitives. REP-6 is closed by clarification.

📍 Defined in: `execution.md`

### 2.7 Determinism

- Given identical inputs and identical declared node state, node outputs must be identical.
- External nondeterminism is confined to declared ingress payload boundaries
  (adapter-shaped or adapter-independent external event payloads). Host
  dispatch and prod boundary channel realization operate after the
  episode within the deterministic capture/replay contract.

📍 Defined in: `execution.md`, `adapter.md`

#### Amendment Record

> **Amended 2026-03-16** by Codex (Docs)
>
> Sharpened the determinism wording so "adapter boundary" refers to the
> declarative vocabulary boundary, while host dispatch and prod boundary
> channel realization remain post-episode operational mechanisms within
> the capture/replay contract.
> Sebastian freeze-authority authorization.

### 2.8 Trigger vs Risk Distinction

- Trigger parameters may encode temporal structure only; they govern *when* events propagate.
- Risk parameters govern *whether* actions execute (acceptability of outcomes).
- Trigger operators are blind to downstream action content and consequences.
- Risk operators are not blind to action content.

📍 Defined in: `ontology.md`

---

## 3. Adapter Contract (Load-Bearing)

Adapters form a trust boundary between the external world and the graph.

Adapters must satisfy:

1. **Replay determinism** — Identical captured inputs must produce identical outputs.
2. **Declared semantic shaping** — Any transformation that changes semantic meaning (units, currency, aggregation, interpolation, timezone) must be declared.
3. **Capture support** — Adapters must support input capture sufficient for replay.

Enforcement is trust-based in v0. Violations invalidate Source guarantees.

📍 Defined in: `adapter.md`

---

## 4. Explicitly Out of Scope for v0 (Non-Frozen)

The following are intentionally excluded from v0 and must not be solved by introducing new primitives:

- Multi-pass or iterative execution
- Action outcome feedback into the graph
- Presence-based triggers (Source → Trigger)
- Multi-graph coordination or portfolio-level logic
- Trigger state introspection APIs
- Adapter format standardization (v1 concern)

Any future addition must preserve all frozen invariants above.

---

## 5. What May Change in v0.x (Patchable)

The following may be modified without breaking v0:

- Executor implementation details
- Validation bugs
- Error messages and diagnostics
- Performance optimizations
- Non-normative documentation

No v0.x change may:

- Add or remove primitives
- Change wiring rules
- Alter execution semantics
- Weaken determinism guarantees

---

## 6. Versioning Rule

If a change requires violating any frozen item in this document, it is a v1 change.

---

## 7. Authoring Layer (Not Frozen)

The authoring layer (clusters, macros, fractal composition) is explicitly outside the v0 freeze.
It may evolve without triggering a v1.

### 7.1 What the Authoring Layer Includes

- Cluster definitions and boundaries
- Fractal composition (arbitrary nesting)
- Parameter binding and exposure
- Cluster versioning and reuse
- Signature inference algorithms

These are specified in concepts.md and cluster-spec.md.

### 7.2 The Frozen Invariant

The authoring layer must satisfy one invariant:

> All authoring constructs compile away before execution.
> The runtime sees only the four primitives and their wiring rules.

This invariant is frozen. The authoring layer mechanics are not.

### 7.3 What This Means

- Cluster format may change without breaking v0
- Signature inference may be refined without breaking v0
- New authoring features may be added without breaking v0

As long as the expanded graph:

- Contains only Source, Compute, Trigger, Action
- Obeys the frozen wiring rules
- Executes per the frozen execution model

📍 Defined in: concepts.md, cluster-spec.md

---

## Status

**v0 frozen**

---

## Authority

This document, together with ontology.md, execution.md, adapter.md,
concepts.md, and cluster-spec.md, is the canonical reference for system behavior.
