---
Authority: FROZEN
Version: v0
Last Amended: 2026-01-11
Scope: Orchestration layer, episode semantics, replay
Verified Against Tag: v1.0.0-alpha.1
Change Rule: v1 only
---

# Execution Supervisor — v0

This document defines the Execution Supervisor: the minimal orchestration layer that governs
*when* episodes execute, without influencing *what* they compute.

This specification defines Execution Supervisor v0: single-graph, strategy-neutral, non-adaptive.

The Supervisor exists to introduce agency over time without violating ontology.

This is a freeze-candidate specification. Treat it as law.

---

## 1. Purpose & Role

### What the Execution Supervisor Is

The Execution Supervisor is a mechanical scheduler that:

- Receives external events
- Applies mechanical constraints (rate, concurrency, deadline)
- Invokes `runtime.run(graph, context)` to execute episodes
- Emits an append-only decision log

External events are mechanical signals (e.g., time ticks, data-arrival notifications, user
commands), not semantic outcomes produced by graph execution.

The Supervisor reasons over **episodes**, **time**, and **external events**.

It never reasons over **nodes**, **outputs**, or **domain semantics**.

### What the Execution Supervisor Is Not

The Supervisor is not:

- A strategy engine
- A graph selector
- A policy evaluator
- A domain-aware decision maker
- A retry-with-logic system

If the Supervisor requires domain knowledge to function, it has exceeded its mandate.

---

## 2. Core Concepts

### 2.1 Episode

An **Episode** is one atomic invocation of the runtime:

```
runtime.run(graph_id, execution_context) → RunTermination
```

Episode properties:

- **Atomic in invocation**: One call, one termination, one log entry
- **Not atomic in outcome**: Partial action execution is permitted per `execution_model.md §7`
- **Opaque to Supervisor**: The Supervisor does not inspect what happened inside

The Episode is the unit of Supervisor reasoning. The Supervisor schedules, invokes, and observes
termination of episodes. It does not decompose them.

**Lifecycle Scope:** Supervisor lifecycle is orchestration-only: instantiate per graph, run episodes, discard. Trigger state is episode-scoped. Each episode begins with fresh state (`HashMap::new()`). Cross-episode continuity is achieved exclusively via external writes (Actions) and reads (Sources). This is enforced by construction: all `ExternalEvent` factories in `ergo-adapter` create fresh `RuntimeExecutionContext`.

### 2.2 ExecutionContext

**ExecutionContext** is the input payload provided to an episode.

ExecutionContext contains:

- Adapter-provided external state snapshot (time, event payloads, environment reads)
- Run metadata (trace_id, run_id)

ExecutionContext does not contain:

- Supervisor-derived state
- Results of prior episodes
- Accumulated history
- Domain-specific signals

> **Provenance rule:** The prohibition on "results of prior episodes" applies to
> Supervisor-observed outcomes (RunTermination, decision records, ActionOutcomes)
> being injected into context. Environment state that was modified by prior Actions
> and obtained via adapter environment reads is permitted—that is the intended
> cross-episode causality path described below.

If information from Episode N must influence Episode N+1, it must:

1. Be written to an external store by Episode N's actions
2. Be read by Episode N+1's Sources from that external store

Causality flows through the environment, not through context injection.

### 2.3 RunTermination

**RunTermination** is the only information the Supervisor receives about episode completion.

```
RunTermination = Completed | TimedOut | Aborted | Failed(ErrKind)
```

RunTermination is mechanical. It describes *how* the episode ended, not *what* it produced.

The Supervisor does not receive **RunResult** (which contains ActionOutcomes and semantic
payloads). RunResult is written to external sinks and is only accessible to subsequent
episodes via Sources.

This boundary is load-bearing. It structurally prevents the Supervisor from being strategy-aware.

### 2.4 ErrKind

**ErrKind** enumerates mechanical failure modes.

Allowed variants (examples):

- `NetworkTimeout`
- `AdapterUnavailable`
- `ValidationFailed`
- `RuntimeError`
- `DeadlineExceeded`
- `Cancelled`

Forbidden variants:

- `OrderRejected`
- `InsufficientFunds`
- `PositionAlreadyOpen`
- Any variant encoding domain semantics

**Test**: ErrKind must be interpretable without knowing what graph ran.

If a failure is domain-flavored, it belongs in RunResult (as data), not ErrKind (as termination).

### 2.5 DecisionLog

The **DecisionLog** is an append-only record of Supervisor decisions.

Each entry contains:

- External event received (or hash)
- Decision: invoke / skip / defer
- Schedule time (if deferred)
- Episode invocation ID
- RunTermination observed

The DecisionLog enables:

- Replay verification
- Audit trail
- Determinism proof

If a decision is not logged, it did not happen.

**Encoding convention:** For `Decision::Defer` and `Decision::Skip`, the `termination` field
in `DecisionLogEntry` is not semantically meaningful — no episode was invoked. The `Decision`
variant is authoritative; `termination` is only valid for `Decision::Invoke`.

---

## 3. Invariants

### CXT-1: ExecutionContext is adapter-only

ExecutionContext contains only adapter-provided external state and run metadata.
Supervisor-derived state in ExecutionContext is forbidden.

| Aspect | Specification |
|--------|---------------|
| **Invariant** | ExecutionContext cannot contain Supervisor-derived or episode-derived data |
| **Enforcement** | Type: private fields, constructor only accessible to adapter |
| **Violation** | Ontology breach: causality bypasses Source |

### SUP-1: Supervisor is graph-identity fixed

A Supervisor instance is bound to exactly one immutable GraphId at construction.
It cannot select, switch, or reload graphs.

| Aspect | Specification |
|--------|---------------|
| **Invariant** | Supervisor.graph_id is set at construction with no mutators |
| **Enforcement** | Type: no setter, no swap/reload API, no trait method accepting new graph |
| **Violation** | Graph selection leakage into orchestration |

Graph update policy: To run a new graph version, terminate the old Supervisor and
instantiate a new one. There is no hot-swap.

### SUP-2: Supervisor is strategy-neutral

The same Supervisor type works for any graph. Two Supervisors given identical external
event streams must produce identical run schedules (modulo mechanical config).

| Aspect | Specification |
|--------|---------------|
| **Invariant** | Supervisor does not receive RunResult; only RunTermination |
| **Enforcement** | API: run() returns RunTermination, not RunResult |
| **Violation** | Policy creep: Supervisor interprets domain outcomes |

### SUP-3: Supervisor decisions are replayable

Every Supervisor decision must be captured and deterministically replayable.

Replay determinism requires identical logical decisions and ordering; wall-clock timestamps
need not match exactly.

| Aspect | Specification |
|--------|---------------|
| **Invariant** | All decisions emit to DecisionLog; replay produces identical schedule |
| **Enforcement** | Mandatory DecisionLog emission; replay harness in CI |
| **Violation** | Silent divergence between backtest and live |

### SUP-4: Retries only on mechanical failure

Supervisor may retry only on transport/infrastructure failures, not on semantic outcomes.

| Aspect | Specification |
|--------|---------------|
| **Invariant** | Retry state machine keys only on Err/Timeout; semantic results inaccessible |
| **Enforcement** | API: Supervisor cannot access RunResult; retry logic in mechanical constraint module |
| **Violation** | Retry becomes strategy |

Allowed retry triggers:

- `ErrKind::NetworkTimeout`
- `ErrKind::AdapterUnavailable`
- `ErrKind::RuntimeError`

Forbidden retry triggers:

- `RunTermination::Completed` with undesirable ActionOutcome
- Any condition requiring inspection of RunResult

### SUP-5: ErrKind is mechanical only

ErrKind variants must be interpretable without domain knowledge.

| Aspect | Specification |
|--------|---------------|
| **Invariant** | ErrKind contains only infrastructure/validation failures |
| **Enforcement** | Enum definition in runtime; allowlist test; no domain error passthrough |
| **Violation** | Domain semantics leak into termination; Supervisor becomes strategy-aware |

### SUP-6: Episode atomicity is invocation-scoped

Episode atomicity means: one run() call, one RunTermination, one DecisionLog entry.

| Aspect | Specification |
|--------|---------------|
| **Invariant** | Partial action execution within episode is permitted; no transactional rollback |
| **Enforcement** | Spec: inherits from execution_model.md §7 |
| **Violation** | Expectation of outcome atomicity leads to incorrect compensation logic |

Cancellation maps to `RunTermination::Aborted` and means "stop executing remaining actions."
It does not mean "undo prior effects."

### SUP-7: DecisionLog is write-only

The Supervisor may only emit entries to the DecisionLog.
The DecisionLog interface exposes no read, query, subscribe, or callback capability to the Supervisor.

| Aspect | Specification |
|--------|---------------|
| **Invariant** | Supervisor constructor requires DecisionLog impl; no default/no-logger path |
| **Enforcement** | Type: constructor signature; trait definition permits append(entry) only; no read APIs |
| **Violation** | Logging becomes optional (SUP-3 weakened) or feedback path introduced (policy creep) |

---

## 4. Failure Modes

### 4.1 Policy Smuggling

**Vector**: ErrKind encodes domain outcomes.

If `ErrKind::OrderRejected` exists, Supervisor can branch on it. This is policy.

**Prevention**: SUP-5 (ErrKind mechanical only).

### 4.2 Retry Drift

**Vector**: "Retry on failure" expands to "retry with adjusted parameters on rejection."

Mechanical retry: bounded backoff on network timeout.
Policy retry: "try again with smaller size if rejected."

The latter requires semantic interpretation and is forbidden.

**Prevention**: SUP-4 (retry on mechanical failure only) + SUP-2 (no RunResult access).

### 4.3 Replay Divergence

**Vector**: Supervisor makes decisions not captured in DecisionLog.

If Supervisor uses wall-clock time, randomness, or undocumented state, replay diverges.

**Prevention**: SUP-3 (mandatory logging) + deterministic constraint primitives only.

### 4.4 Graph Selection Leakage

**Vector**: Supervisor chooses which graph to run based on outcomes.

"If last run failed, try fallback graph" is graph selection. It belongs in a higher layer.

**Prevention**: SUP-1 (graph fixed at construction).

### 4.5 Context Accumulation

**Vector**: ExecutionContext grows to include derived state across episodes.

"Pass running P&L to next episode" smuggles state into context.

**Prevention**: CXT-1 (adapter-only) + type enforcement.

### 4.6 Intra-Episode Granularity Pressure

**Vector**: Real-world pressure to "cancel order if not filled in 5 seconds."

This pressure pushes toward sub-episode callbacks or splitting episodes.

**Response**: Cancellation is mechanical termination (Aborted). Time-based cancellation
is a deadline constraint, not semantic logic. If finer control is needed, model it as
multiple episodes with Source observation between them.

---

## 5. Explicit Non-Responsibilities

The Supervisor does not:

| Concern | Owner |
|---------|-------|
| Domain logic | Graph (via Compute/Trigger/Action) |
| Graph selection or mutation | Higher layer (Scenario Planner) — out of scope for v0 |
| Semantic interpretation of outcomes | Sources in subsequent episodes |
| Partial execution control | Runtime (execution_model.md §7) |
| Retry with parameter adjustment | Graph logic, not Supervisor |
| Transaction rollback | Not supported; external effects are not reversible |
| Multi-graph coordination | Higher layer — out of scope for v0 |

If a capability does not appear in the Supervisor API, it is forbidden by omission.

---

## 6. Drift Tripwires

These are not invariants. They are alarms.

### SUP-META: Complexity Ceiling

If Supervisor logic exceeds ~500 lines (excluding mechanical scheduling primitives),
it has likely absorbed policy.

**Response**: Escalate before proceeding. Do not expand; refactor or extract.

### SUP-AUDIT: Review Gate

Any PR touching Supervisor internals requires doctrine review.

**Response**: Tag for Claude/ChatGPT review under AGENT_CONTRACT.md v1.1.

### SUP-SMELL: "Just This Once" Pressure

If implementation pressure suggests "just add one hook for this domain case,"
that is the signal to stop.

**Response**: The hook belongs in the graph, the adapter, or a named higher layer.
Not in Supervisor.

---

## 7. Allowed Supervisor Capabilities

For clarity, the complete list of what the Supervisor may do:

| Capability | Mechanical? | Notes |
|------------|-------------|-------|
| Receive external events | ✅ | OnEvent handler |
| Schedule episode at time T | ✅ | Timer/cron |
| Rate-limit episodes | ✅ | max_per_window |
| Limit concurrent episodes | ✅ | max_in_flight |
| Enforce deadline per episode | ✅ | max_run_time → Aborted |
| Retry on mechanical failure | ✅ | Bounded backoff on Err/Timeout |
| Invoke runtime.run() | ✅ | Core purpose |
| Emit DecisionLog entry | ✅ | Mandatory |
| Observe RunTermination | ✅ | Completion status only |

Anything not on this list is out of scope.

---

## 8. Relationship to Other Layers

```
┌─────────────────────────────────────────────────────────────┐
│  Scenario Planner / Campaign Manager  (OUT OF SCOPE v0)    │
│  - Multi-graph selection                                    │
│  - Parameter sweeps                                         │
│  - A/B scenarios                                            │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  Execution Supervisor  (THIS DOCUMENT)                      │
│  - Single graph, repeated episodes                          │
│  - Mechanical scheduling                                    │
│  - No domain awareness                                      │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  Runtime  (FROZEN: execution_model.md)                      │
│  - validate() → run()                                       │
│  - Deterministic DAG execution                              │
│  - ActionOutcome emission                                   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  Adapter  (FROZEN: adapter_contract.md)                     │
│  - Transport (HTTP, in-process, etc.)                       │
│  - External state provision                                 │
│  - Capture for replay                                       │
└─────────────────────────────────────────────────────────────┘
```

---

## 9. Freeze Status

This document is **FROZEN**.

Changes require joint escalation per AGENT_CONTRACT.md v1.1.

---

## 10. Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| v0.1 | 2025-12-27 | Claude (Structural Auditor) | Initial draft |
| v0.2 | 2025-12-27 | Claude (Structural Auditor) | Added SUP-7 (DecisionLog write-only); ChatGPT polish edits |
| v0.3 | 2025-12-27 | Claude Code | Freeze finalization; Sebastian approval |
| v0.4 | 2025-12-27 | Claude Prime | Added lifecycle scope note (§2.1) — state is episode-scoped by construction |
| v0.5 | 2026-01-11 | Claude (Structural Auditor) | Added provenance rule clarification (§2.2) — pins interpretation that "results of prior episodes" refers to Supervisor-injected outcomes, not adapter-provided environment state modified by prior Actions. Sebastian override authorization. |

---

## 11. Signatures

**Claude acknowledgment:**
> I have reviewed this specification under AGENT_CONTRACT.md v1.1. I attest that:
> - All invariants (CXT-1, SUP-1 through SUP-7) have named enforcement loci
> - No ontological expansion is proposed
> - No policy-smuggling vectors were identified
> - The specification is ready for freeze
>
> Signed: Claude (Structural Auditor / Doctrine Owner)

**ChatGPT review:**
> I confirm Option A stands as doctrinally sound. The DecisionLog trait is semantically
> append-only, and no policy-smuggling vector is introduced. I consider SUPERVISOR.md v0
> structurally complete and freeze-ready.
>
> Signed: ChatGPT (Integrator Support / Build Orchestrator)

**Sebastian approval:**
> SUPERVISOR.md v0 is frozen. Changes require joint agent escalation.
>
> Signed: Sebastian (Author / Architect) — approved via Claude Code proxy 
