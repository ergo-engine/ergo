---
Authority: FROZEN
Version: v0
Last Amended: 2025-12-28
Scope: Evaluation semantics, phase rules, determinism
Verified Against Tag: v1.0.0-alpha.1
Change Rule: v1 only
---

# Execution Model — v0

This document defines the minimal execution semantics required to implement a correct executor
for the v0 primitive ontology. It is not an ontology document and does not introduce new concepts.

The execution model is intentionally simple: single-pass, deterministic, acyclic evaluation.

---

## 1. Evaluation Pass

An **evaluation pass** is a single, discrete execution of a graph over a snapshot of inputs.

- A pass has a clear start and end.
- All nodes are evaluated at most once per pass.
- No node may observe effects produced by actions within the same pass.

Evaluation cadence (when passes occur) is defined by the orchestration layer and is out of scope
for this document.

Trigger semantics assume a discrete evaluation model; evaluation cadence is defined by the
execution environment.

---

## 2. Graph Structure

Graphs are directed acyclic graphs (DAGs).

- Cycles are forbidden.
- All dependencies must be statically resolvable before execution.
- Topological ordering defines evaluation order.

Graphs may contain nodes of different primitive kinds but must respect wiring rules defined
in ontology.md.

---

## 3. Node Evaluation Order

Nodes are evaluated in topological order, respecting declared dependencies and wiring rules.

Primitive kinds do not impose global execution phases; ordering is dependency-driven.

Source, Compute, Trigger, and Action represent causal roles, not execution phases. Compute and
Trigger may interleave as long as dependencies are satisfied.

---

## 4. Values and Events

- Events are values with restricted wiring rules.
- Events propagate through the graph like other outputs.
- No event queue or multi-pass propagation exists in v0.

Trigger nodes consume values and/or events and emit events.
Actions consume events and do not emit graph-propagated values.

The specialness of events is in the type system and wiring rules, not the execution model.

---

## 5. Trigger Execution Semantics

### 5.1 Triggers are Stateless

Triggers are ontologically stateless. A Trigger is a primitive causal role whose sole
responsibility is to gate whether an Action may attempt to affect the external world.
It does not store information, accumulate history, or own temporal memory.

Triggers:
- Evaluate their inputs on each invocation
- Emit `Emitted` or `NotEmitted` based solely on current input values
- Have no memory of prior evaluations
- Cannot observe, preserve, or depend on cross-evaluation information

### 5.2 Execution-Local Bookkeeping

Trigger evaluation may involve ephemeral, execution-local bookkeeping (temporary
comparisons, registers, scratch data) that exists only during evaluation. Such
bookkeeping:

- Is not represented in the causal graph
- Is not part of the runtime contract
- Is not preserved across evaluations
- Has no semantic identity
- Is not observable, replayable, or serializable

Execution-local bookkeeping does not constitute state. It exists only within a single
evaluation pass and is discarded before the next causal boundary.

### 5.3 Canonical Boundary Rule

> **Execution may use memory. The system may never observe, preserve, or depend on
> that memory.**

Memory may not:
- Participate in causality
- Survive evaluation
- Be wired, surfaced, or reasoned about

Only declared causality remains.

### 5.4 Temporal Patterns are Compositions

All apparent "stateful trigger" behavior (edge detection, hysteresis, debouncing,
counting, latching) must be expressed using explicit composition:

- Compute nodes for transformation
- Sources for reading persisted state from environment
- Actions for writing state to environment
- Clusters to encapsulate these patterns

The Trigger primitive itself remains stateless and level-sensitive.

### 5.5 Amendment Record

> **Amended 2025-12-28** by Sebastian (Freeze Authority)
>
> Prior language stating "Trigger nodes may hold internal state" was a semantic error
> that conflated execution-local bookkeeping with ontological state. This amendment
> corrects the error. Triggers are, and always were intended to be, stateless primitives.
>
> See: REP-6 closure in invariants/INDEX.md

Trigger chaining (Trigger → Trigger) is allowed.
Trigger cycles are forbidden.

---

## 6. Action Execution Semantics

- Actions are terminal nodes in the graph.
- Actions are executed at most once per evaluation pass.
- Actions must be stateless.
- Actions emit an acknowledgment record to the orchestrator (non-causal, for logging/audit).

Action effects are external and must not influence other nodes during the same pass.

Action acknowledgment records do not participate in graph causality. They are metadata for
accountability, not events for propagation.

---

## 7. Action Execution Order

All nodes must pass validation before any action executes.

Actions execute sequentially in topological order inherited from their trigger dependencies.

If multiple actions are topologically independent (no ordering relationship between their
upstream triggers), their relative execution order is undefined in v0.

If an action fails during execution:
- Subsequent actions in the same evaluation pass are aborted.
- Prior external effects are not reversible.
- No retry or compensation occurs within the same pass.
- Error handling and retries are orchestrator concerns.

---

## 8. Determinism

Within an evaluation pass:

- Node evaluation must be deterministic.
- Given identical inputs and identical declared node state (for stateful Compute nodes), outputs must be identical.
- No internal randomness is permitted.
- No hidden mutable state is permitted.

External nondeterminism is handled at the adapter boundary.

For Triggers: determinism means identical behavior given identical inputs (triggers are stateless).
For Actions: determinism means identical command emission given identical trigger events and parameters.

---

## 9. Out of Scope

This document does not specify:

- Evaluation cadence
- Multi-pass execution
- Feedback loops
- Multi-graph coordination
- Action outcome feedback into the graph
- Trigger state introspection APIs

These are explicitly deferred beyond v0.

Multi-phase orchestration (e.g., "wait for fill then place second leg") is handled via
environment-state observation through Sources in subsequent evaluation passes, not via
direct cyclic wiring.
