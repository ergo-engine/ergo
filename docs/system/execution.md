---
Authority: FROZEN
Version: v0
Last Amended: 2026-03-26
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

### 6.1 Terminal Position and Statelessness

- Actions are terminal nodes in the graph.
- Actions are executed at most once per evaluation pass.
- Actions must be stateless.

### 6.2 Action Outputs

Actions produce two forms of non-causal output. Neither participates in graph causality.

**Acknowledgment records** are Action outcome records used for accountability and audit surfaces.
Each Action node produces exactly one `outcome` event value per pass (`Attempted`/`Completed`/
`Rejected`/`Cancelled`/`Failed`, or `Skipped` when gated off). These records are non-causal metadata,
not events for propagation.

**Effect descriptions** are operational instructions emitted to the host
boundary for post-episode dispatch and realization after the episode
completes. An Action may declare zero or more effect writes and zero or
more effect intents in its manifest. Each write declaration names a
target context key, the expected value type, and which scalar input
port provides the value. Each intent declaration names an external
effect kind, a typed field set sourced from scalar inputs and/or static
parameters, and optional mirror writes that project selected field
values into host-managed context.

Effect descriptions are not returned by the Action trait implementation itself. They are derived
mechanically by runtime execution from manifest write declarations,
intent declarations, the Action input snapshot, and static parameter
values.

### 6.3 Effect Lifecycle

The effect lifecycle spans runtime evaluation, host dispatch, and
cross-episode state propagation.

**Within an episode (runtime to host dispatch):**

1. The runtime evaluates the graph. When an Action executes, runtime execution derives effect descriptions from the Action manifest write declarations, intent declarations, current input values, and any parameter-sourced intent fields.
2. The runtime buffers these effect descriptions. They do not influence any node during the same evaluation pass.
3. After the episode terminates, the host drains buffered effects and dispatches them according to their realization class. Host-internal effects may be realized directly by host effect handlers. Truly external effects cross a prod boundary realization such as an egress channel. Adapter effect acceptance (`accepts.effects`) and host ownership coverage (handler-owned kinds or egress-routed kinds) are validated before run; `set_context` application validates key existence, writability, and type before writing to the host context store.
4. The drained effect set is recorded in the capture bundle for replay verification. Host-internal effects may be replay-realized when needed to reconstruct deterministic cross-episode state. Truly external effects are re-derived and verified, not re-executed against live systems.

**Across episodes (adapter-bound host path):**

1. On the next external event, the host merges eligible context-store values into the incoming payload (eligible means adapter-declared context keys that are also allowed by the event schema). Incoming payload values take precedence over stored values.
2. The merged payload becomes `ExecutionContext` for the new episode. Sources read from this context via `ctx.value(key)`.

For fixture-driven paths (the sole adapter-exempt execution mode),
there is no context-store merge step.  All production execution paths
(process ingress, SDK manual stepping) require an adapter contract.

This lifecycle is the intended cross-episode causality path referenced
in ontology.md §2.4: Action intent -> host dispatch ->
external store/context -> Source reads. Causality flows through the
environment, not through graph wiring or Supervisor-injected state.

### 6.4 Invariant Preservation

The effect lifecycle preserves execution-model invariants:

- **Single-pass determinism (§1):** Effects are buffered, not applied, during evaluation. No node observes same-pass effects.
- **DAG structure (§2):** No feedback edges are introduced. Cross-episode flow is mediated outside the graph.
- **Trigger statelessness (§5.1):** Triggers remain stateless. Temporal memory patterns (once/count/latch) are expressed as composition: Sources read state, Computes transform, Triggers gate, Actions write state.
- **Action terminality:** Actions remain terminal. Effect declarations are manifest metadata, not downstream graph edges.

### 6.5 Wiring Rules for Effect-Bearing Actions

Actions that declare effect writes and/or intent fields sourced from
inputs require two input categories:

- **Event inputs** (from Triggers): the causal gate that determines whether the Action executes (R.7).
- **Scalar payload inputs** (from Sources or Computes): values carried by effect writes and any intent fields sourced via `from_input`.

Intent fields sourced via `from_param` come from static parameters and
do not require upstream graph edges.

Validation enforces this distinction per destination Action input port type:

- Event input ports accept only Trigger-provided Event edges.
- Non-Event Action input ports (v0: Number/Series/Bool/String) may accept Source/Compute edges.
- Scalar payload edges never satisfy the Action gate requirement.

### 6.6 Amendment Record

> **Amended 2026-03-15** by Codex (Docs)
>
> Clarified the distinction between host dispatch, host-internal
> realization, and prod boundary channel realization for truly external
> effects. Replay text now records that host-internal effects may be
> replay-realized for determinism, while truly external effects are
> verified rather than re-executed against live systems.
> Sebastian freeze-authority authorization.
>
> **Amended 2026-03-16** by Codex (Docs)
>
> Widened the frozen effect-shape wording from write-only declarations
> to write and intent declarations, and sharpened the nondeterminism
> language so adapter vocabulary, host dispatch, and prod boundary
> channel realization are distinguished explicitly.
> Sebastian freeze-authority authorization.

---

## 7. Action Execution Order

All nodes must pass validation before any action executes.

Actions execute sequentially in topological order inherited from their trigger dependencies.
For topologically independent actions, the current implementation still produces a deterministic
tie-break order from the validated topological sort.

Action outcome values (`Completed`, `Rejected`, `Cancelled`, `Failed`, `Skipped`) are ordinary
node outputs, not control-flow abort signals. A pass aborts only when runtime execution itself
fails or the runtime returns a failing termination.

When runtime execution aborts:

- Subsequent actions in the same evaluation pass are not executed.
- Episode-local buffered effects are not dispatched.
- No retry or compensation occurs within the same pass.

Post-episode host dispatch is a separate, non-transactional phase. Error handling and retries
there are orchestrator concerns.

---

## 8. Determinism

Within an evaluation pass:

- Node evaluation must be deterministic.
- Given identical inputs, parameters, and any explicitly supplied node state, outputs must be identical.
- No internal randomness is permitted.
- No hidden mutable state is permitted.

The current prod runtime executes Compute primitives without persisted compute state, so
determinism is grounded in the input snapshot plus resolved parameters.

External nondeterminism enters through declared ingress payloads
(adapter-shaped external event payloads for production paths, or
adapter-exempt fixture payloads for fixture-driven testing) and leaves
through post-episode host dispatch to prod boundary channels.

For Triggers: determinism means identical behavior given identical inputs (triggers are stateless).
For Actions: determinism means identical outcomes and derived effects given identical input
snapshots and parameters.

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

---

## 10. Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| v0 | 2025-12-28 | Original | Initial freeze |
| v0.1 | 2026-03-04 | Claude (Structural Auditor) | §6 rewritten to document dual non-causal Action outputs (outcome + effects), runtime-to-host effect lifecycle, adapter-bound cross-episode context flow, and per-port wiring constraints for Action gating vs scalar payload. Aligned with ontology.md §2.4. Sebastian override authorization. |
| v0.2 | 2026-03-16 | Codex (Docs) | §6 widened from write-only effect wording to write + intent declarations, and §8 sharpened the external nondeterminism boundary to distinguish adapter vocabulary from host dispatch and prod boundary channel realization. Sebastian freeze-authority authorization. |
| v0.3 | 2026-03-26 | Codex (Docs) | Corrected current-prod behavior for independent Action ordering, pass-abort semantics, scalar Action payload types, and determinism wording around compute state and adapter-independent ingress. |
| v0.4 | 2026-04-12 | Claude (Structural Auditor) | §5 and §8 updated to reflect production closure: replaced "adapter-independent" with "fixture-driven (adapter-exempt)" to match enforcement of mandatory adapter contracts for all production execution paths. |
