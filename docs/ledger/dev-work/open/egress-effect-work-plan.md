---
Authority: PROJECT
Date: 2026-03-15
Author: Claude (Structural Auditor) + Sebastian (Architect)
Verified-By: Codex (Ontology Guardian)
Status: Active
Scope: v1
Source-Gaps: ../../gap-work/open/effect-realization-boundary.md
---

# Egress and Effect Realization — Work Plan

This file sequences the closure of all open rows in
[effect-realization-boundary.md](../../gap-work/open/effect-realization-boundary.md).

Each row is classified as one of:

- **Decision record** — a real fork with meaningfully different options
  and consequences. Must be filed in `docs/ledger/decisions/` before
  code begins.
- **Inline fork** — a smaller policy choice that is decided within the
  parent row's implementation, not as a standalone record.
- **Code/design** — the answer is known or follows from prior
  decisions. Implement it.

Classification was verified by Codex against codebase and doctrine.

## Decisions Already Landed

These decisions constrain everything below. Read them first.

| Decision | File | What it settles |
| --- | --- | --- |
| Effect dispatch and channel roles | `decisions/effect-dispatch-and-channel-roles.md` | Four-role lifecycle (Action → Adapter → Host → Channel). Replay split doctrine. Ingress/egress terminology. |
| v1 external effect intent model | `decisions/v1-external-effect-intent-model.md` | Option A (first-class intents). Two-correlated-projections. Manifest `effects.intents`. Mirror writes. Dispatch ordering. Egress handshake as classification. Startup coverage guarantee. |
| Intent payload shape | `decisions/intent-payload-shape.md` | Typed fields (`Vec<IntentField>`). JSON projection at egress boundary only. Registration-time validation. |
| Intent ID semantics | `decisions/intent-id-semantics.md` | Deterministic SHA-256 derivation (`eid1:sha256:hex`). Length-prefixed inputs. Replay-safe. |

## Classification Summary

**Phase 1 items (all CLOSED):**

1. ~~ActionEffect v1 payload shape~~ — DECIDED (`intent-payload-shape.md`)
2. ~~`intent_id` correlation semantics~~ — DECIDED (`intent-id-semantics.md`)
3. ~~Handler-vs-egress precedence~~ — RESOLVED inline (`ConflictingCoverage`)
4. ~~GW-EFX-3A replay effect-path split~~ — IMPLEMENTED
5. ~~`mirror_writes[].from_field` validation~~ — IMPLEMENTED
6. ~~Coverage validation evolution~~ — IMPLEMENTED

**Remaining (5 decision records):**

7. GW-EFX-3H — Egress acknowledgment and result semantics
8. GW-EFX-3I — Crash consistency and delivery guarantees
9. GW-EFX-3C — Egress provenance granularity and replay strictness
10. GW-EFX-3D — Routing config location, schema, and validation
11. GW-EFX-3J — Egress failure and partial-apply semantics

**Remaining (1 inline fork):**

12. Artifact-preserving policy on dispatch failure (inside GW-EFX-3B)

**Remaining (1 design/code item):**

13. GW-EFX-3E — Egress run-phase timing (downstream of ack model)

## Work Sequence

Three phases. Each phase must complete before the next begins.
Within a phase, rows can be worked in parallel unless noted.

---

### Phase 1 — Foundations — COMPLETE

All three items closed. Phase 2 is now open.

#### 1a. GW-EFX-3A — Replay effect-path split — CLOSED

- **Type:** Code. Decision already made.
- **What:** `HostedRunner::execute_step()` is shared by `step()` and
  `replay_step()`. Both drain effects and apply handlers identically.
  Gate the path so external effects are never dispatched during replay.
- **Output:** Modified `runner.rs` with live-vs-replay mode parameter.
  Test proving external handler is skipped during `replay_step()`.
- **Closed:** `StepMode` enum added. `step()` passes `Live`,
  `replay_step()` passes `Replay`. `execute_step()` accepts mode.
  Test `replay_step_threads_replay_mode_into_execute_step` passes.
  All workspace tests pass.

#### 1b. GW-EFX-3G (remaining) — Data model, intent_id, from_field — CLOSED

All three sub-items closed. 22 files changed, +745 lines. Audited line-by-line.

**1b-i. ActionEffect v1 payload shape — CLOSED**

- **Decision:** Typed fields. Intent payloads are `Vec<IntentField>`
  with `name: String` + `value: Value`. Manifest-declared, validated
  at registration. JSON projection at egress boundary only.
  Compatibility check at startup against adapter JSON Schema.
- **Record:** `docs/ledger/decisions/intent-payload-shape.md`
- **Closed:** Decision landed. Typed fields chosen over arbitrary JSON
  for registration-time guarantees, replay determinism, and pattern
  consistency with `effects.writes`. Codex refinement adopted: JSON
  projection at egress boundary for interop.

**1b-ii. `intent_id` correlation semantics — CLOSED**

- **Decision:** Deterministic derivation via SHA-256 of length-prefixed
  inputs: `eid1` version tag + `graph_id` + `event_id` +
  `node_runtime_id` + `intent_kind` + `intent_ordinal`. Produces
  `"eid1:sha256:{hex}"`. Follows Ergo's existing provenance/hash
  idiom. Replay-safe. Unique per intent occurrence.
- **Record:** `docs/ledger/decisions/intent-id-semantics.md`
- **Closed:** Deterministic derivation chosen. Random UUID rejected
  (breaks replay). Per-step counter rejected (fragile, not globally
  unique). Requires plumbing `event_id` and `graph_id` to the runtime
  intent emission site.

**1b-iii. `mirror_writes[].from_field` validation — CLOSED**

- **What:** Every `from_field` in `mirror_writes` must reference a
  declared intent field in the same `intents` entry. Registration-time
  check in the action manifest validator.
- **Closed:** ACT-32 (`MirrorWriteFromFieldNotFound`) and ACT-33
  (`MirrorWriteTypeMismatch`) added to `action/registry.rs`. Validation
  scoped to same intent’s fields via `field_types` HashMap. 9 tests
  covering all intent validation scenarios. All workspace tests pass.
  PHASE_INVARIANTS.md row still needed (tracked separately).

#### 1c. GW-EFX-3F (remaining) — Coverage validation evolution — CLOSED

- **Type:** Mostly code, with one inline fork.
- **What:** `ensure_handler_coverage()` only knows registered
  in-process handlers. It needs to accept egress-claimed effect kinds
  as covered. Plus: startup invariant that no run begins if an
  emittable intent kind lacks egress coverage.
- **Inline fork resolved:** Both handler and egress claim same kind →
  error at startup (`ConflictingCoverage`). Exactly one owner per kind.
- **Closed:** `ensure_handler_coverage` widened with
  `egress_claimed_kinds` parameter. `ConflictingCoverage` error variant
  added. Runner passes `&HashSet::new()` (no egress config yet).
  4 new tests + 3 existing tests updated. All workspace tests pass.

---

### Deferred Validations (Phase 2-dependent)

These items were flagged during the Phase 1 audit but cannot be
closed until Phase 2 work provides the necessary infrastructure.

**CHECK-15: End-to-end intent_id determinism test**

- **Source:** Phase 1 audit, CHECK-15 (FLAG)
- **What:** `derive_intent_id()` is unit-tested for determinism, but
  the full derivation path — from host step through runtime emission
  to capture and replay comparison — does not yet exist. The
  derivation inputs (`graph_id`, `event_id`, `node_runtime_id`,
  `intent_kind`, `intent_ordinal`) are individually proven
  deterministic, but their composition through the emission path is
  not yet exercised by an integration test.
- **Why deferred:** Intent emission wiring (plumbing `event_id` and
  `graph_id` to the runtime emit path) is Phase 2 work. The test
  cannot be written until that plumbing exists.
- **Closure condition:** An integration test exists that captures a
  run with at least one emitted intent, replays it, and verifies
  `compare_decisions()` passes — proving `intent_id` is identical
  across capture and replay.
- **Becomes closable after:** 2a (dispatch plumbing) lands.

---

### Phase 2 — Delivery Semantics

These define how egress works from the user's perspective.

#### 2a. GW-EFX-3B — Dispatch plumbing

- **Type:** Code, with one inline fork.
- **What:** Host applies mirror writes via `SetContextHandler`, then
  forwards external intent records to egress process. Mirror failure
  blocks dispatch. Dispatch failure produces interrupted outcome.
- **Inline fork:** Artifact-preserving policy on dispatch failure. If
  egress dispatch fails after mirror writes succeed, does the host
  write a partial capture artifact, a full artifact, or no artifact?
  The v1 intent model decision explicitly leaves this open. Must be
  decided during implementation.
- **Output:** Modified `runner.rs` and `usecases.rs`. Integration test
  with mock egress process.
- **Depends on:** 1a, 1b, 1c.

#### 2b. GW-EFX-3H — Egress acknowledgment and result semantics — DECISION RECORD

- **The fork:** What does the host wait for from the egress process?
  - **(a) Accepted intent** — egress received the record. Host moves
    on. Fast. But "received" ≠ "done."
  - **(b) Completed work** — egress did the external action and
    confirmed. Slow. But host knows the outcome before next step.
  - **(c) Async confirmation** — host fires and forgets. Egress
    result returns later as an ingress event keyed by `intent_id`.
    Decoupled. But capture artifact can't record outcome inline.
- **Why it matters:** Shapes user expectations, timeout behavior,
  capture semantics, and what "delivered" means for crash consistency.
- **Output:** Decision record in `docs/ledger/decisions/`.
- **Depends on:** Easier to decide after 1b (intent shape helps
  reason about what's acknowledged). Not strictly blocked.

#### 2c. GW-EFX-3D — Routing configuration — DECISION RECORD

- **The fork:** Where does the user declare "effect kind `place_order`
  routes to egress channel `broker.py`"? Real options:
  - **`ergo.toml`** — project-level config. Discoverable. But doesn't
    exist yet (feat/ergo-init is backburner).
  - **Standalone config file** — `egress.toml` or similar, passed via
    `--egress-config`. Works without ergo-init.
  - **Host run request** — programmatic, in the `RunGraphFromPathsRequest`
    struct. Works for SDK users. Not ergonomic for CLI.
  - **Adapter manifest section** — would extend the adapter contract.
    Rejected by doctrine (adapter is declarative vocabulary, not
    prod routing).
- **Also open:** Schema shape, validation rules, relationship to
  adapter's `accepts.effects`.
- **Why it matters:** This is what the user touches. Bad choice here
  means bad UX.
- **Output:** Decision record. Route-table schema.
- **Depends on:** 1b (route table references intent kinds).

#### 2d. GW-EFX-3E — Egress run-phase timing

- **Type:** Downstream design. Answer follows from ack model.
- **What:** When does the egress process start? Does it outlive the
  run? Does the host wait for drain before writing capture?
- **Possible small fork:** Start-at-run-start vs lazy start on first
  intent. Unlikely to block anything but worth noting.
- **Output:** Lifecycle definition. Constrains 2a implementation.
- **Depends on:** 2b (ack model affects drain semantics).

#### 2e. GW-EFX-3I — Crash consistency — DECISION RECORD

- **The fork:** If the host crashes around dispatch, what's the
  delivery model?
  - **Best-effort** — intent may or may not arrive. Simple. Unreliable.
  - **At-most-once** — intent arrives zero or one times. No dupes.
    But may lose intents.
  - **At-least-once with dedup** — intent arrives at least once.
    Egress must handle duplicates. More complex. More reliable.
  - **Egress-owned idempotency** — host doesn't care. Egress is
    responsible for dedup using `intent_id`. Cleanest separation.
    But pushes complexity to user code.
- **Why it matters:** This is where duplicate real-world effects
  appear. A user who places the same order twice because of a crash
  window will not forgive the system.
- **Output:** Decision record in `docs/ledger/decisions/`.
- **Depends on:** 2b (ack model determines what "delivered" means),
  2d (timing determines the crash window).

---

### Phase 3 — Cleanup

These follow from Phase 2 decisions.

#### 3a. GW-EFX-3J — Egress failure and partial-apply semantics — DECISION RECORD

- **The fork:** User-visible failure taxonomy for egress. Current
  `InterruptionReason` is ingress-only. Real options:
  - Partial delivery across multiple intent targets: acceptable,
    or all-or-nothing?
  - Failure granularity: one catch-all egress error, or distinct
    variants for launch, protocol, I/O, delivery, and drain failures?
  - How do these compose with the crash consistency model (2e)?
- **Why it matters:** Users need to write error handling code against
  these variants. Wrong granularity means they can't distinguish
  recoverable from terminal failures.
- **Output:** Decision record. New enum variants or error types. Tests.
- **Depends on:** 2b (ack model), 2d (timing), 2e (crash policy).

#### 3b. GW-EFX-3C — Egress channel provenance — DECISION RECORD

- **The fork:** Current capture has adapter and runtime provenance
  only. Adding egress provenance requires decisions:
  - **Identity granularity:** Per-channel? Per-route-table? Per-run
    config hash?
  - **What's included:** Process path? Version? Config hash?
    Handshake-declared kinds?
  - **Replay strictness:** Reject on mismatch? Warn? Ignore?
- **Why it matters:** Without egress provenance, replay can't verify
  the same egress contract was in place. With the wrong granularity,
  replay is either too strict (breaks on benign changes) or too
  loose (misses real contract drift).
- **Output:** Decision record. Extended `CaptureBundle`. Replay
  validation. Tests.
- **Depends on:** 2c (routing config defines egress identity),
  2d (lifecycle defines when provenance is captured).

---

### Not Sequenced Here

#### GW-EFX-2 — Multi-ingress host direction

Independent of egress work. Can be decided at any time. Does not
block and is not blocked by any row above.

---

## Dependency Graph

```text
1a (replay split) --------\
                            \
1b (data model) ----> 1c (coverage) ----> 2a (plumbing)
                  \                         |
                   --> 2c (routing)          |
                                            |
         2b (ack model) --> 2d (timing) --> 2e (crash) --> 3a (failure taxonomy)
                              \
                               --> 3b (provenance)
```

## Summary

| Phase | Row | Type | Status |
| --- | --- | --- | --- |
| 1 | 3A — replay split | Code | CLOSED |
| 1 | 3G — payload shape | Decision record | CLOSED |
| 1 | 3G — intent_id | Decision record | CLOSED |
| 1 | 3G — from_field validation | Code | CLOSED |
| 1 | 3F — coverage | Code + inline fork | CLOSED |
| 2 | 3B — dispatch plumbing | Code + inline fork | Ready (Phase 1 done) |
| 2 | 3H — ack model | Decision record | Ready (Phase 1 done) |
| 2 | 3D — routing config | Decision record | Ready (Phase 1 done) |
| 2 | 3E — timing | Downstream design | Waiting on 2b |
| 2 | 3I — crash consistency | Decision record | Waiting on 2b, 2d |
| 3 | 3J — failure taxonomy | Decision record | Waiting on 2b, 2d, 2e |
| 3 | 3C — egress provenance | Decision record | Waiting on 2c, 2d |

**Phase 1: COMPLETE** (2 decisions landed, 1 inline fork resolved, 3 code items done).
**Remaining: 5 decision records. 1 inline fork. 1 design/code item.**
GW-EFX-2 is independent.

## Rules

- No row moves to CLOSED without a verifiable closure condition met.
- Decision records land in `docs/ledger/decisions/` before code begins.
- Inline forks are documented in the implementing PR, not as separate
  records.
- Code changes follow the normal branch/ledger process.
- If any decision changes a prior decision, the earlier record gets an
  amendment, not a silent overwrite.
