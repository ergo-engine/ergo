---
Authority: FROZEN
Version: v0
Last Amended: 2026-01-11
Scope: Four primitives, wiring matrix, causal roles
Verified Against Tag: v1.0.0-alpha.1
Change Rule: v1 only
---

# Ontology

This document defines the foundational ontology of the system.
It establishes what is primitive, what is composed, and what is explicitly out of scope.

This ontology is intentionally minimal.
It is designed to remain stable even as the system grows in complexity.

---

## 1. The Bar for Ontological Primitives

To qualify as a foundational ontological primitive, a candidate must satisfy all of the following:

1. **Irreducible** — It cannot be expressed as a composition of other primitives.
2. **Distinct Causal Role** — It plays a unique role in the causal structure of an executable system.
3. **Execution-Real** — It exists and operates at runtime, not only at authoring or conceptual time.
4. **Non-Normative** — It does not encode policy, preference, belief, or subjective judgment.
5. **Universally Required** — It is required for any executable decision system, not only trading systems.

Anything that fails one or more of these criteria is not an ontological primitive.

---

## 2. Ontological Primitives (Closed Set)

The system defines exactly four ontological primitives.
This set is closed.

### 2.1 Source — Origin

**Role:** Introduces data into the system.

A Source answers: *What exists at this evaluation point?*

Characteristics:
- No inputs
- No transformation of graph-derived inputs; adapter shaping only
- No inference
- No side effects
- Deterministic data materialization given captured adapter inputs

Source may "materialize" values from the environment, but may not "derive" values from other graph values. Adapter behavior is a trust boundary; semantic shaping must be declared as parameters or adapter metadata.

Source establishes origin, not meaning.

---

### 2.2 Compute — Truth

**Role:** Transforms values deterministically.

A Compute answers: *What is true, given these inputs?*

Characteristics:
- Pure
- Deterministic
- Side-effect free
- May be stateless or stateful (if explicitly declared)
- Must declare at least one input (zero-input nodes are Sources by definition)

Compute establishes truth, not causality or intent.

---

### 2.3 Trigger — Causality

**Role:** Converts continuous values into discrete events.

A Trigger answers: *When does something happen?*

Characteristics:
- Emits events
- No side effects
- Deterministic
- Stateless. Execution-local bookkeeping (ephemeral scratch during evaluation) is permitted but does not constitute state—it is not observable, serializable, or preserved across evaluations.

Trigger parameters may encode temporal structure only and must not condition on action semantics or outcomes. Trigger governs *when* events propagate; it is blind to downstream action content and consequences.

Trigger and Compute share execution semantics; they differ in declared causal role and wiring permissions.

Trigger establishes causality, not action.

#### Amendment Record

> **Amended 2025-12-28** by Sebastian (Freeze Authority)
>
> Prior language stating "May hold internal state" was a semantic error that conflated
> execution-local bookkeeping with ontological state. This amendment aligns ontology.md
> with execution_model.md §5 and V0_FREEZE.md §2.6. Triggers are stateless primitives.
>
> See: TRG-STATE-1 in PHASE_INVARIANTS.md

---

### 2.4 Action — Agency

**Role:** Attempts to affect the external world.

An Action answers: *What command is attempted as a result of this event?*

Characteristics:
- Consumes events
- Causes side effects
- Deterministic command emission
- No internal state
- Emits non-causal outputs (see below)

Action outputs are non-causal and take two forms:
- **Acknowledgment records** — emitted to orchestrator for logging/audit (exactly one per attempt)
- **Effect descriptions** — emitted to adapter for external application (zero or more per attempt)

Neither participates in graph causality. Acknowledgment records are metadata for accountability. Effect descriptions are operational instructions that the adapter applies to external stores after episode completion.

Action establishes agency, not logic or policy.

#### Amendment Record

> **Amended 2026-01-11** by Claude (Structural Auditor)
>
> Prior language mentioned only "acknowledgment records" without distinguishing them from
> effect descriptions. This amendment clarifies that Actions emit two types of non-causal
> outputs: acknowledgments (to orchestrator) and effects (to adapter). Both are non-causal;
> effects enable the cross-episode state pattern described in execution_model.md §5.4.
> Sebastian override authorization.

---

## 3. Wiring Rules (v0)

The following wiring rules are authoritative for v0:

```
Source → Compute     : allowed
Source → Trigger     : forbidden (v0)
Compute → Compute    : allowed
Compute → Trigger    : allowed
Compute → Action     : forbidden (must be mediated by Trigger)
Trigger → Trigger    : allowed
Trigger → Action     : allowed
Action → *           : forbidden (terminal)
* → Source           : forbidden
```

Graphs violating these rules are invalid.

---

## 4. Macro-Primitives (Explicitly Not Ontological)

Some concepts are essential for authoring, but not ontological primitives.
These are macro-primitives.

Macro-primitives:
- Are composed from ontological primitives
- Exist for ergonomics and intent expression
- Compile away before execution
- Add no new runtime semantics

### 4.1 Risk (Policy, Not Mechanism)

Risk is not an ontological primitive.

Reason:
- Risk is normative
- Risk is subjective
- Risk encodes policy and preference
- Risk is fully expressible via Compute (measurement), Trigger (violation detection), and Action (enforcement)

Risk answers: *Is this intent allowed to proceed?*

That is a policy question, not a causal role.

Risk therefore exists as a macro-primitive: UI-visible, reusable, composable, compiled into compute → trigger → action.

The distinction between Risk and Trigger temporal operators (throttle, debounce, etc.):
- Trigger operators govern *when* events propagate (temporal semantics)
- Risk operators govern *whether* actions execute (acceptability of outcomes)
- Trigger operators are blind to action content; Risk operators are not

### 4.2 Constraints, Guards, Policies

These follow the same rule as Risk:
- Not execution-real
- Not causal
- Not irreducible

They are named compositions, not primitives.

---

## 5. Common Straw-Man Candidates (Rejected)

The following concepts are explicitly not primitives:

- **State** — orthogonal property, not a causal role
- **Time** — data/context, introduced via Source
- **Event** — value type, not a role
- **Intent** — authoring-time only
- **Belief / Uncertainty** — epistemic, not causal
- **Environment** — interface, not mechanism
- **Evaluation / Execution** — infrastructure, not a primitive

Each of these either collapses into an existing primitive or exists outside the execution ontology.

---

## 6. Composition Rule

All executable behavior in the system is expressed as compositions of:

**Source → Compute → Trigger → Action**

This is the canonical explanatory flow; the implementation is a DAG with role-constrained edges.

No additional ontological primitives are permitted.

Higher-level constructs must:
- Decompose into these primitives
- Obey their manifest contracts
- Compile away before execution

---

## 7. Stability Guarantee

This ontology is designed to be:
- Minimal
- Non-overlapping
- Deterministic
- Future-proof

New features must be added via composition, not new primitives.

---

## 8. Deferred Features and Invariant Preservation

The following features are explicitly out of scope for v0. When added in future versions, they must preserve the invariants listed:

- **Trigger state introspection** — must not allow mutation
- **Action feedback wiring** — must not introduce cycles in a single evaluation pass; must preserve DAG or introduce explicit multi-phase execution
- **Presence triggers (Source → Trigger)** — must not bypass Compute semantics
- **Outcome events** — must preserve DAG or introduce explicit multi-phase execution
- **Multi-graph coordination** — must remain an orchestration concern, not introduce new primitives

---

## 9. Final Declaration

The ontological primitive set is complete.

No additional foundational abstractions are required or permitted.

This ontology is the authoritative foundation of the system.

---

## 10. Authoring Layer

The authoring layer enables composition of primitives into reusable, nestable structures.
It is specified separately in AUTHORING_LAYER.md and CLUSTER_SPEC.md.

### Key Properties

- Authoring constructs are called clusters
- Clusters may contain primitives and other clusters (arbitrary nesting)
- Clusters have boundary kinds that mirror the four primitives: SourceLike, ComputeLike, TriggerLike, ActionLike
- The wiring matrix applies to clusters exactly as it applies to primitives

### The Invariant

All authoring constructs compile away before execution.

At runtime, only the four ontological primitives exist.
The authoring layer provides ergonomics and modularity without expanding the ontology.

📍 Specified in: AUTHORING_LAYER.md, CLUSTER_SPEC.md
