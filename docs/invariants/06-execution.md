## 6. Execution Phase

**Scope:** Running the validated graph.

**Entry invariants:**
- All V.* invariants hold
- State is initialized per lifecycle rules

### Exit Invariants

| ID | Invariant | Spec | Type | Assertion | Validation | Test |
|----|-----------|:----:|:----:|:---------:|:----------:|:----:|
| R.1 | Each node executes at most once per pass | execution.md §1 | — | — | — | ✓ |
| R.2 | Nodes execute in topological order | execution.md §3 | — | — | — | ✓ |
| R.3 | No node observes effects from actions in same pass | execution.md §1 | — | — | ✓ | ✓ |
| R.4 | Action failure aborts subsequent actions in same pass | execution.md §7 | — | — | — | ✓ |
| R.5 | Triggers are stateless (TRG-STATE-1) | execution.md §5 | — | — | ✓ | ✓ |
| R.6 | Outputs are deterministic given inputs + state | execution.md §8 | — | — | — | ✓ |
| R.7 | Actions execute only when trigger event emitted | execution.md §7 | — | — | — | ✓ |

### Notes

- **R.3:** ✅ **CLOSED.** Compositionally enforced by existing invariants:
  - F.2: Action outputs are non-wireable (`cluster.rs:324: wireable = meta.kind != PrimitiveKind::Action`)
  - X.5: "Actions are terminal; Action → * is forbidden" (validated at D.3, V.2)
  - Since no edge can originate from an Action, no node can observe action effects.
  - No separate test needed — enforcement is structural via wiring matrix validation.
- **R.4:** ✅ **CLOSED (by design).** `Result::Err` propagation via `?` is sufficient. `ActionOutcome::Failed` is data, not control flow — structural halt must be expressed via Trigger gating/wiring, not implicit runtime payload semantics.
- **R.5 / TRG-STATE-1:** ✅ **CLOSED.** Triggers are ontologically stateless.

### TRG-STATE-1: Triggers are stateless

| Aspect | Specification |
|--------|---------------|
| **Invariant** | Trigger implementations must not use observable, preservable, or causally meaningful state |
| **Enforcement** | Manifest: `state: StateSpec { allowed: false }` required for all triggers |
| **Locus** | Registry validation at registration time; manifest schema |
| **Violation** | Trigger with `allowed: true` rejected by registry |

**Rationale:** Triggers are ontologically stateless. A Trigger gates whether an Action
may attempt to affect the external world. It does not store information, accumulate
history, or own temporal memory. Execution-local bookkeeping (ephemeral scratch data
during evaluation) is permitted but does not constitute state — it is not observable,
serializable, or preserved across evaluations.

**Canonical Boundary Rule:** Execution may use memory. The system may never observe,
preserve, or depend on that memory.

**Temporal patterns** (once, count, latch, debounce) requiring cross-evaluation memory
must be implemented as clusters with explicit state flow through environment.

**Authority:** Sebastian (Freeze Authority), 2025-12-28

- **Enforcement locus confirmed (2025-01-05):** Statelessness is enforced at two levels:
  1. **Type system:** `TriggerPrimitive::evaluate()` signature takes `&self` (not `&mut self`), no state parameter, no `PrimitiveState` argument. State cannot be smuggled through trait API.
  2. **Registry validation:** `TriggerRegistry::validate_manifest()` rejects any trigger with `state.allowed = true` (returns `StatefulTriggerNotAllowed`). Test: `trg_state_1_stateful_trigger_rejected`.

- **R.7:** ✅ **CLOSED.** Runtime gates Action execution on `TriggerEvent::Emitted`. Implementation:
  - `should_skip_action()` in execute.rs checks for any `TriggerEvent::NotEmitted` input (AND semantics)
  - Skipped actions return `ActionOutcome::Skipped` for Event outputs
  - Test: `r7_action_skipped_when_trigger_not_emitted` verifies enforcement
  - **Strengthened (2025-01-05):** `map_to_action_value()` now uses explicit pattern matching on `TriggerEvent::Emitted` and `TriggerEvent::NotEmitted` rather than wildcard. NotEmitted case includes `unreachable!("R.7 violation: NotEmitted must be caught by should_skip_action")` to prevent silent acceptance of future TriggerEvent variants. Location: `execute.rs:345-351`.
