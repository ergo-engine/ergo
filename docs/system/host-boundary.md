---
Authority: CANONICAL
Version: v1
Last Updated: 2026-04-20
Owner: Sebastian (Architect)
Scope: v1 host boundary — effect loop, context store, capture enrichment, provenance trinity
Change Rule: Operational log
---

# v1 Host Boundary — Invariants and Enforcement

## 0. Anchor

HEAD: `dbd376fe25eb45ea067285400fc362ef70c97694`

This document describes the v1 host boundary as it exists at the HEAD
above. Every §3–§8 claim cites a file path and line range in the code
tree at this commit. The re-anchor from original authoring HEAD
`7784f46f` to `dbd376f` reflects Session 2's three executed
transformations (S2.1 `DecisionLogEntry.effects` removal, S2.2 runtime
seam redesign, S2.3 host-module relocation). Downstream rewrites of
`07-orchestration.md`, `08-replay.md`, `supervisor.md`, and
`adapter.md` are deferred to Session 3 and must reconcile against §9.

Files described (short blob hash, path, line count at HEAD):

| hash | path | lines |
|---|---|---:|
| `88a261cf7364` | `crates/kernel/supervisor/src/lib.rs` | 530 |
| `59b4db69ef8e` | `crates/kernel/supervisor/src/capture.rs` | 461 |
| `882943fd5ab9` | `crates/kernel/supervisor/src/replay.rs` | 355 |
| `ccd3a6b80c9c` | `crates/kernel/adapter/src/lib.rs` | 793 |
| `6dc42bd9f215` | `crates/kernel/adapter/src/provenance.rs` | 107 |
| `bf160dbd5a09` | `crates/prod/core/host/src/host/buffering_invoker.rs` | 260 |
| `496cdab95622` | `crates/prod/core/host/src/host/context_store.rs` | 44 |
| `b499c9b41eec` | `crates/prod/core/host/src/host/coverage.rs` | 194 |
| `36ea02697c3d` | `crates/kernel/runtime/src/provenance.rs` | 397 |
| `6f734a45a193` | `crates/prod/core/host/src/runner.rs` | 929 |
| `5e4c45dfc69f` | `docs/invariants/07-orchestration.md` | 122 |
| `1ef3df307553` | `docs/invariants/08-replay.md` | 117 |
| `e5c1717abd4a` | `docs/orchestration/supervisor.md` | 531 |
| `84e6c31f1325` | `docs/orchestration/adapter.md` | 127 |
| `3e91dfd3f6c8` | `docs/system/kernel-prod-separation.md` | 146 |

The claim-verification pass (§11) re-checks these anchors as the final
step.

---

## 1. Scope

This document defines the v1 host boundary: which component owns which
part of episode execution, capture, and replay. It does not define new
semantics. It codifies the boundary reached by the 2026-02-16 →
2026-03-26 migration tracked retrospectively in
[`sup2-alignment.md`](../ledger/gap-work/closed/sup2-alignment.md).

What this doc establishes:

- The ownership contract between Supervisor, Host, and Adapter under v1
- The provenance trinity (`adapter_provenance`, `runtime_provenance`, `egress_provenance`)
- Effect buffer lifecycle and non-rollback posture
- Context merge precedence and schema gating
- Capture bundle composition (pre-host vs host-enriched fields)
- Strict replay contract (provenance match + decision/effect comparison)
- Rule-ID reconciliation across the `SUP-*`, `HST-*`, `REP-*`, `RTHANDLE-*`, `CXT-*`, and `RUN-CANON-*` families

What this doc does not establish:

- New rule IDs. All referenced rules already exist in `07-orchestration.md` and `08-replay.md`.
- New kernel semantics. Kernel meaning is frozen (see [`kernel.md`](kernel.md)).
- Session 3 rewrite of `supervisor.md`, `adapter.md`, `07-orchestration.md`, or `08-replay.md`. Those are deferred; §9 is the working table for that rewrite.

---

## 2. Roles in one diagram

```
                     ┌──────────────────────────────────────────┐
                     │                 Host                     │
                     │       (crates/prod/core/host)            │
                     │                                          │
  external event ──► │ 1. build_external_event                  │──► adapter binder
                     │    (context merge: incoming > store)     │
                     │                                          │
                     │ 2. session.on_event(e) ─────────┐        │
                     │    (CapturingSession wraps      │        │
                     │     Supervisor + CapturingLog)  ▼        │
                     │                     ┌──────────────────┐ │
                     │                     │   Supervisor     │ │
                     │                     │ (kernel)         │ │
                     │                     │                  │ │
                     │                     │ Decision-only:   │ │
                     │                     │ Skip | Invoke |  │ │
                     │                     │ Drop | Retry     │ │
                     │                     │                  │ │
                     │                     │ runtime.run() ─► RunTermination (no RunResult)
                     │                     └──────────────────┘ │
                     │                                  │       │
                     │ 3. drain_pending_effects ◄───────┘       │ (BufferingRuntimeInvoker shim)
                     │                                          │
                     │ 4. dispatch_invoked_effects              │
                     │    - handler.apply()   → ContextStore    │
                     │    - egress.dispatch() → external I/O    │
                     │                                          │
                     │ 5. enrich_bundle_with_host_artifacts     │
                     │    (decisions[i] ← effects, intent_acks, │
                     │     interruptions; egress_provenance)    │
                     └──────────────────────────────────────────┘
```

The Supervisor is termination-only (`SUP-2`). Effects produced by the
runtime during `runtime.run(...)` are *not* handed back to the
Supervisor. They are held by a host-owned buffer shim
(`BufferingRuntimeInvoker`) and drained by the host after
`on_event(...)` returns.

---

## 3. Ownership contract

### 3.1 Supervisor (kernel) — termination-only

Responsibilities:

- Apply mechanical constraints (rate limits, invoke/retry policy, deadlines)
- Record decisions into `DecisionLog` via `log_decision(...)`
- Invoke `RuntimeInvoker::run(...)` and observe only `RunTermination`

Non-responsibilities:

- Does not observe `ActionEffect`, `RunResult`, or domain payloads
- Does not own `ContextStore`
- Does not apply effects
- Does not enrich capture bundles (host-authored fields in §7.1)

Evidence:

- `crates/kernel/supervisor/src/lib.rs:212` — `Supervisor` struct
- `EpisodeInvocationRecord::from(&DecisionLogEntry)` in `crates/kernel/supervisor/src/lib.rs:172-187` hardcodes `effects: vec![]`, so kernel capture remains termination-only even after `DecisionLogEntry.effects` was removed.

### 3.2 Host (prod) — effect loop + context + enrichment

Responsibilities:

- Build `ExternalEvent` (context merge, schema gate, adapter binder) — `crates/prod/core/host/src/runner.rs:714`
- Hold the `ContextStore` — `runner.rs` (field on `HostedRunner`); read for merge at `runner.rs:722`; writes via handler at `runner.rs:795`
- Drain the per-step effect buffer via `runtime.drain_pending_effects()` — `runner.rs:576`
- Apply handler-owned effect kinds into `ContextStore` — `runner.rs:795` in `dispatch_invoked_effects`
- Dispatch egress-owned effect kinds through configured egress channels — `runner.rs:837` in `dispatch_invoked_effects`
- Enrich `CaptureBundle` with applied effects, intent acks, and interruptions — `runner.rs:650-655` via `enrich_bundle_with_host_artifacts`
- Stamp `egress_provenance` on the bundle — `runner.rs:649`

Non-responsibilities:

- Does not redefine kernel semantics or introduce new rule IDs
- Does not own the `RuntimeInvoker` trait (owned by kernel adapter)
- Does not compute `runtime_provenance` (owned by kernel runtime) or `adapter_provenance` (owned by kernel adapter)

### 3.3 Adapter (kernel) — declarative contract

Responsibilities:

- Declare context keys, event kinds, accepted effects, and capture fields (manifest)
- Produce `adapter_provenance` fingerprint — `crates/kernel/adapter/src/provenance.rs:10` (`fingerprint(manifest)` → `adapter:{id}@{version};sha256:{hex}`)
- Own the `RuntimeInvoker` trait as kernel contract
- Provide the host binder that maps semantic events to bound events

Non-responsibilities:

- Does not execute; runtime does
- Does not dispatch effects; host does
- Does not own `ContextStore` or effect handlers as semantic authority — those are host-layer concerns
- Host support types now live under `crates/prod/core/host/src/host/`, matching their host-owned responsibility

---

## 4. Provenance trinity

The v1 capture bundle carries three provenance strings. Each is produced
by a distinct layer and bounds a distinct failure domain.

### 4.1 `adapter_provenance`

- Scheme: `adapter:{id}@{version};sha256:{hex}`
- Produced by: `fingerprint(manifest)` — `crates/kernel/adapter/src/provenance.rs:10`
- Input: recursively key-sorted (canonicalized) `AdapterManifest` JSON
- Absent-adapter fallback: the constant string `"none"` (`NO_ADAPTER_PROVENANCE` in `crates/kernel/supervisor/src/lib.rs:50`)
- Matched on strict replay (`REP-7`): `validate_replay_provenance` — `crates/kernel/supervisor/src/replay.rs:229`

### 4.2 `runtime_provenance`

- Scheme: `rpv1:sha256:{hex}` (only scheme defined in v1; see `RuntimeProvenanceScheme::Rpv1`)
- Produced by: `compute_runtime_provenance` — `crates/kernel/runtime/src/provenance.rs:52`
- Input: canonical `ExpandedGraph` plus primitive catalog metadata, JSON-serialized with sorted keys
- Matched on strict replay (`REP-7`): same validator as `adapter_provenance`

### 4.3 `egress_provenance`

- Produced by: host (post-step) — `crates/prod/core/host/src/runner.rs:649`
- Records the egress runtime configuration used during live dispatch
- Field shape: `CaptureBundle.egress_provenance: Option<String>` — `crates/kernel/supervisor/src/lib.rs:200`; `None` for fixture/adapterless runs
- Decision record: [`docs/ledger/decisions/egress-provenance.md`](../ledger/decisions/egress-provenance.md)

Replay semantics:

- `adapter_provenance` and `runtime_provenance` are compared strictly by `replay_checked_strict` (`REP-7`)
- `egress_provenance` is not compared by the kernel replay validator; it is a host-side attestation carried alongside the bundle, not a strict-replay gate

### 4.4 Why three, not one

Each provenance string bounds a different failure domain:

- `adapter_provenance` pins the compatibility contract (what the host validated against)
- `runtime_provenance` pins the expanded graph and primitive versions (what the runtime actually executed)
- `egress_provenance` pins the boundary-I/O configuration (what realized external effects)

Collapsing them into a single hash would hide which layer changed.
Keeping them separate preserves layer-specific diagnostic capability on
replay-mismatch failures.

---

## 5. Context merge precedence (HST-6)

### 5.1 Incoming > store

For every `on_event(...)`:

1. Host reads `ContextStore.snapshot()` — `runner.rs:722`
2. Host merges adapter-declared, schema-allowed store keys into a candidate payload
3. Host overlays incoming event payload fields on top — `runner.rs:728-730`
4. Final merged payload is passed to the adapter binder at `runner.rs:732-740`

Overlay rule: **keys present in the incoming event replace any same-named keys from the store.**

### 5.2 Schema gate on store-supplied keys

A key survives from the store into the merged payload only if all three
conditions hold:

- `adapter.provides.context.contains_key(key)` — declared in the manifest
- `allowed_schema_keys(adapter, &semantic_kind).contains(key)` — permitted for this event kind
- Key is present in `ContextStore.snapshot()` at step time

Incoming event keys bypass the store gate but still flow through the
adapter binder's semantic event validation.

### 5.3 Why this order

`HST-6` makes merge deterministic across replays of identical captured
events. The incoming payload is authoritative. The store is re-built
during replay from captured `set_context` effects in the same decision
order, so any value that was in the live store is reconstructible, and
any value that came in on the event is carried on the event record
itself.

---

## 6. Effect buffer lifecycle (HST-7)

### 6.1 Replace-only, drain-once

The runtime does not call back into the supervisor with effects. It
writes effects into a host-owned buffer held by
`BufferingRuntimeInvoker`:

- Each `run(...)` **replaces** `pending_effects` with the latest invocation's effects — `crates/prod/core/host/src/host/buffering_invoker.rs:132` (`guard.pending_effects = effects;` inside `impl RuntimeInvoker for BufferingRuntimeInvoker`, lines 117-136; the sink `Vec` is populated by `self.engine.run_reporting(..., &mut effects)` at lines 126-128)
- Each host step **drains** via `std::mem::take(...)` — `buffering_invoker.rs:99`
- Before the next `on_event`, host asserts `pending_effect_count() == 0` — `runner.rs:547-551`

Replace (not append) semantics are why `HST-4` holds: a retry that
re-invokes the runtime overwrites stale effects rather than accumulating
them.

### 6.2 Non-rollback commitment

Once the host drains and dispatches effects, no rollback is possible:

- `SetContextHandler` writes into `ContextStore` directly
- Egress dispatch is irreversible by construction (external I/O is committed when the channel acks)
- If the outcome terminates abnormally, prior effects from this decision are still committed (`SUP-6` — invocation-scoped atomicity)

Evidence: `runner.rs:793` — comment `"SUP-6 alignment: no rollback on handler failure."`

### 6.3 Buffer shim location

The `BufferingRuntimeInvoker` shim lives in
`crates/prod/core/host/src/host/buffering_invoker.rs`. It is host
behavior and now sits under the host crate's support-module boundary,
which matches the v1 ownership contract.

---

## 7. Capture enrichment

### 7.1 Pre-host vs host-enriched fields

| `CaptureBundle` field | Author | Site |
|---|---|---|
| `capture_version` | kernel capture | `crates/kernel/supervisor/src/capture.rs:230` |
| `graph_id` | kernel capture | `capture.rs:231` |
| `config` | kernel capture | `capture.rs:232` |
| `events` | kernel capture (`CapturingSession::on_event`) | `capture.rs:249` (`guard.events.push(...)`) |
| `decisions` (non-effect fields) | kernel capture (`CapturingDecisionLog`) | `capture.rs:187-196` (`CapturingDecisionLog::log` body; `EpisodeInvocationRecord::from(&entry)` at line 191, push at line 194) |
| `decisions[i].effects` | **host** (authoritative writer; kernel capture initializes empty defaults first, see §7.3) | `runner.rs:650-655` via `enrich_bundle_with_host_artifacts` |
| `decisions[i].intent_acks` | host | same |
| `decisions[i].interruptions` | host | same |
| `adapter_provenance` | host seed → kernel capture | `runner.rs:455-465` (host seed) / `capture.rs:235` (kernel store in `CaptureBundle` literal at `capture.rs:229-238`) |
| `runtime_provenance` | host seed → kernel capture | `runner.rs:465-466` (host seed, passed into `CapturingSession::new_with_provenance`) / `capture.rs:236` (kernel store) |
| `egress_provenance` | host (post-step) | `runner.rs:649` |

### 7.2 Association by decision index, not `event_id`

Host enrichment keys on `decisions[i]` position (via
`AppliedEffectsByDecision`), not on `event_id`. This is safety-relevant
because duplicate `event_id` values could otherwise overwrite prior
decisions' effects. `HST-9` rejects duplicate `event_id` values
defensively at the host step boundary; the index-based association is a
second-line guarantee.

Evidence: `runner.rs:759-761` — `self.applied_effects.record(decision_index, drained_effects.to_vec())` inside `dispatch_invoked_effects`, guarded by `if !drained_effects.is_empty()`.

### 7.3 Why host, not supervisor

If the supervisor wrote `decisions[i].effects` with authoritative
content, it would have to observe effects — contradicting `SUP-2`
(strategy-neutrality) and bleeding `ActionEffect` into the kernel
scheduling layer. The v1 solution:

- Kernel capture initializes `EpisodeInvocationRecord.effects` to `vec![]` in `EpisodeInvocationRecord::from(&entry)` and `CapturingDecisionLog::log` pushes that record directly.
- Host is the only authoritative source of non-empty effect content. Post-step, host overwrites `record.effects` for every decision index covered by the per-decision sidecar (`AppliedEffectsByDecision`) via `enrich_bundle_with_host_artifacts` (§7.1). Decision indices outside the sidecar's recorded range keep whatever the supervisor wrote — which in production is `vec![]`.

The sidecar records only Invoke decisions whose drained effect buffer is non-empty (`runner.rs:759-761` — guarded by `if !drained_effects.is_empty()`). Decisions that fall outside that record set therefore retain the kernel-written placeholder. Known cases at HEAD:

- Skip / Defer decisions (never invoke runtime; `HST-3` forces zero effects, so `dispatch_invoked_effects` is not called for them — `runner.rs:590-604`).
- Invoke decisions that ran but emitted no effects (`dispatch_invoked_effects` skips `applied_effects.record` when `drained_effects` is empty).
- Adapterless / fixture runs (no adapter present, so `dispatch_invoked_effects` returns early before any `record` call — `runner.rs:752-757`; such runs also require zero effects by construction).

The kernel-written `record.effects` field is therefore not a trusted
content channel. It is a default-empty placeholder that
survives only for decisions in the cases above, and any non-empty
effect content on the bundle comes from host enrichment.

---

## 8. Replay contract (strict)

### 8.1 Entry

`replay_checked_strict(bundle, runtime, expectations)` — `crates/kernel/supervisor/src/replay.rs:184`.

### 8.2 Preflight (`validate_bundle_strict`)

1. Capture version match (`REP-1` — self-validating form) — `replay.rs:159-163`
2. All event records pass `validate_hash()` (`REP-1` — rehydration integrity) — `replay.rs:165-171`
3. No duplicate `event_id`s (`REP-8`) — `replay.rs:257-268`
4. Provenance match (`REP-7`) — `replay.rs:229-255`:
   - `adapter_provenance == expected_adapter_provenance`, with the `"none"` bidirectional guard (`AdapterRequiredForProvenancedCapture` / `UnexpectedAdapterProvidedForNoAdapterCapture`)
   - `runtime_provenance == expected_runtime_provenance`

### 8.3 Decision comparison

`compare_decisions(captured, replayed)` — `replay.rs:274`:

- Non-effect decision fields compared positionally (`event_id`, `decision`, `schedule_at`, `episode_id`, `deadline`, `termination`, `retry_count`) — `replay.rs:284-293`
- `decisions[i].effects` compared by `(effect, effect_hash)` pair equality — `replay.rs:312-329`
- Mismatch in effect count or content returns `ReplayError::EffectMismatch`

### 8.4 What replay does not verify

- `egress_provenance` — informational; not kernel-gated
- Live boundary I/O — replay is capture-driven; no live channels are started
- Cross-ingestion normalization parity — deferred (`INGEST-TIME-1`, per `08-replay.md`)

---

## 9. Rule-ID reconciliation (working table)

This table is the working document for the deferred Session 3 rewrite
of `supervisor.md`, `adapter.md`, `07-orchestration.md`, and
`08-replay.md`. It enumerates every rule currently declared in those
docs that touches the v1 host boundary and states its v1 disposition.

Status values:

- **applies** — rule holds verbatim at HEAD; no rewrite needed beyond source citations
- **clarified** — rule holds but prose needs tightening in the Session 3 rewrite (e.g. to name the correct enforcement locus under v1)
- **relocated** — rule's enforcement locus moved between layers during the v0 → v1 migration; prose still reads correctly but the authority line in the spec doc should change
- **closed** — rule is retired and retained only as a historical anchor
- **process** — rule governs workflow rather than runtime behavior; out of semantic scope for this doc
- **out-of-scope** — rule belongs to a layer this doc does not address (tracked for completeness)

| Rule ID | v1 status | Evidence |
|---|---|---|
| `CXT-1` | clarified | `runner.rs:714-744` enforces adapter-governed context keys; spec prose in `supervisor.md §3` still correctly says "externally supplied and adapter-governed" but should name the host-side enforcement locus |
| `SUP-1` | applies | `crates/kernel/supervisor/src/lib.rs` — `Supervisor::graph_id` is private with no setter; set only at construction |
| `SUP-2` | applies | `RuntimeInvoker::run()` returns `RunTermination` only (`crates/kernel/adapter/src/lib.rs`); no kernel supervisor path observes `RunResult`. The rule holds verbatim at HEAD. Post-S2.2, `RunResult` is private to `ergo-adapter` (`crates/kernel/adapter/src/lib.rs:182`) and `RuntimeHandle::run`'s public signature returns `RunTermination` only, so `SUP-2` is type-enforced at the public seam rather than preserved by the shim's existence. |
| `SUP-3` | applies | Replay harness in `crates/kernel/supervisor/tests/replay_harness.rs`; strict entry at `replay.rs:184` |
| `SUP-4` | applies | `should_retry()` matches only `NetworkTimeout | AdapterUnavailable | RuntimeError | TimedOut` — `supervisor/src/lib.rs` |
| `SUP-5` | applies | `ErrKind` enum in `supervisor/src/lib.rs` has only mechanical variants |
| `SUP-6` | applies | Invocation-scoped atomicity preserved by host non-rollback posture — §6.2; `runner.rs:793` |
| `SUP-7` | applies | `DecisionLog` trait declares only `fn log()` in `crates/kernel/supervisor/src/lib.rs`; `records()` is on the concrete `MemoryDecisionLog`/`CapturingDecisionLog` impls, not on the trait. The write-only property holds verbatim at HEAD. |
| `SUP-TICK-1` | applies | `supervisor/src/lib.rs` — Pump scheduling; legacy `Tick` alias in serde `#[serde(alias = "Tick")]` |
| `RTHANDLE-META-1` | applies | `crates/kernel/adapter/src/lib.rs` — `RuntimeHandle::run()` forwards `graph_id` and `event_id` into `execute_with_metadata(...)` |
| `RTHANDLE-ID-1` | applies | `FaultRuntimeHandle` keys injected outcomes on `EventId` only |
| `RTHANDLE-ERRKIND-1` | closed | Fix landed 2026-02-06; `RuntimeHandle::run()` maps pre-execution failures to `ErrKind::ValidationFailed`. Historical anchor only. |
| `HST-1` | applies | Host applies effects at the boundary; not read back from `DecisionLog` — `runner.rs:576` (drain), `runner.rs:746` (dispatch) |
| `HST-2` | applies | `SetContextHandler::apply` in `crates/prod/core/host/src/host/effects.rs` validates declared key, writable, type |
| `HST-3` | applies | `runner.rs:599-603` — non-invoke decisions must produce zero effects |
| `HST-4` | applies | Replace semantics — `buffering_invoker.rs:132` — §6.1 |
| `HST-5` | applies | `ensure_handler_coverage` — `crates/prod/core/host/src/host/coverage.rs:50-78` |
| `HST-6` | applies | Incoming > store overlay — `runner.rs:721-730` — §5.1 |
| `HST-7` | applies | Replace-only, drain-once, commit-non-empty, no rollback — §6 |
| `HST-8` | applies | One `on_event` lifecycle per step — `runner.rs:556-566` |
| `HST-9` | applies | Duplicate `event_id` rejection at `HostedRunner::execute_step` — `runner.rs:542-545` |
| `RUN-CANON-1` | applies | Canonical run requires explicit `DriverConfig` — host request types in `crates/prod/core/host/src/` |
| `RUN-CANON-2` | applies | Adapter binding mandatory for production; three-gate enforcement described in `07-orchestration.md` notes |
| `DOC-GATE-1` | process | Workflow rule — out of runtime scope for this doc |
| `SDK-CANON-1` | out-of-scope | SDK-layer delegation; see `docs/system/kernel-prod-separation.md §3` |
| `SDK-CANON-2` | out-of-scope | Same |
| `SDK-CANON-3` | out-of-scope | Same |
| `REP-1` | applies | `ExternalEventRecord::validate_hash()` — `replay.rs:165-171` |
| `REP-2` | applies | `rehydrate_event` — `replay.rs:340` |
| `REP-3` | applies | Fault injection keys on `EventId` — `RTHANDLE-ID-1` mirror |
| `REP-4` | applies | Capture types are in `kernel/supervisor/src/capture.rs`; runtime types are in `kernel/runtime`; the two are distinct |
| `REP-5` | applies | Supervisor does not read wall-clock time; `schedule_at` is externally supplied |
| `REP-6` | closed | `08-replay.md` lines 58–62: "Prior documentation suggesting 'triggers may hold internal state' was a semantic error that conflated execution-local bookkeeping with ontological state. Triggers are stateless (see `TRG-STATE-1`). There is no trigger state to capture. Temporal patterns requiring memory (once, count, latch, debounce) must be implemented as clusters." Closed by clarification 2025-12-28. |
| `REP-7` | applies | `validate_replay_provenance` — `replay.rs:229-255` — §8.2 |
| `REP-8` | applies | `validate_unique_event_ids` — `replay.rs:257-268` — §8.2 |
| `REP-SCOPE` | applies | Scope A (supervisor scheduling + host-owned effect integrity, same-ingestion path) |
| `SOURCE-TRUST` | applies | Trust-based; `docs/orchestration/supervisor.md §2.3` |
| `INGEST-TIME-1` | deferred | Cross-ingestion normalization parity — explicitly deferred in `08-replay.md` |

**Coverage check:** the table covers every rule declared in the
"Invariants" tables of `07-orchestration.md` (§7) and `08-replay.md`
(§8), plus `CXT-1` from the same. `TRG-STATE-*` and the primitive
families (`ADP-*`, `SRC-*`, `CMP-*`, `TRG-*`, `ACT-*`, `COMP-*`,
`D.*`/`I.*`/`E.*`/`V.*`) are out of scope for this doc (they govern
declaration and composition, not the host boundary).

---

## 10. Known v1 technical debt (historical)

At the original authoring HEAD `7784f46f`, this section listed a
single residual item — S2.2, a public-seam tightening that
`RunResult` remained publicly importable from the kernel adapter
crate while `BufferingRuntimeInvoker` had moved under
`ergo-host::host`. That item was discharged in Session 2 at HEAD
`dbd376f`:

- `RuntimeHandle::run`'s public signature now returns `RunTermination` only.
- A separate adapter-layer type `ReportingRuntimeHandle` carries the low-level reporting seam `run_reporting(..., effects_out: &mut Vec<ActionEffect>) -> RunTermination`, consumed only by `BufferingRuntimeInvoker` in `ergo-host`.
- `RunResult` is internal to `ergo-adapter` and no longer part of the public or freeze surface.

§3.2 of this document reflects the post-execution shape. This section
is retained as a historical anchor; no outstanding v1 debt is tracked
here at current HEAD.

---

## 11. Claim verification (read-back pass)

Each row below is a semantic claim made in §§3–8. Every claim resolves
to the stated file and line range at HEAD
`dbd376fe25eb45ea067285400fc362ef70c97694`. This section was the
pre-merge gate at original authoring (HEAD `7784f46f`); it was
re-anchored post-Session 2 at HEAD `dbd376f` to reflect the three
executed transformations (S2.1, S2.2, S2.3). If any row does not
resolve at current HEAD, the claim is retracted or rewritten.

| # | Claim (section) | Stated source | Verified |
|---|---|---|:---:|
| 1 | Supervisor observes only `RunTermination` (§3.1) | `crates/kernel/supervisor/src/lib.rs` — `Supervisor` + `RuntimeInvoker::run` signature in `crates/kernel/adapter/src/lib.rs` | ✓ |
| 2 | Kernel capture initializes `EpisodeInvocationRecord.effects` to `vec![]` before host enrichment (§3.1, §7.3) | `supervisor/src/lib.rs:172-187` — `impl From<&DecisionLogEntry> for EpisodeInvocationRecord` hardcodes `effects: vec![]`; `capture.rs:187-196` pushes that record directly | ✓ |
| 3 | Host builds `ExternalEvent` with context merge (§3.2, §5) | `runner.rs:714-744` `build_external_event` | ✓ |
| 4 | Host drains the per-step effect buffer (§3.2, §6.1) | `runner.rs:576` `self.runtime.drain_pending_effects()` | ✓ |
| 5 | Host dispatches handler-owned effects into `ContextStore` (§3.2) | `runner.rs:794-796` — `handler.apply(effect, &mut self.context_store, ...)` | ✓ |
| 6 | Host enriches bundle with effects / intent_acks / interruptions (§3.2, §7.1) | `runner.rs:650-655` `enrich_bundle_with_host_artifacts(&mut bundle, self.applied_effects.effects(), self.applied_intent_acks.intent_acks(), self.interruptions.interruptions())` | ✓ |
| 7 | Host stamps `egress_provenance` on the bundle (§3.2, §4.3, §7.1) | `runner.rs:649` `bundle.egress_provenance = self.egress_provenance.clone()` | ✓ |
| 8 | `adapter_provenance` scheme is `adapter:{id}@{version};sha256:{hex}` (§4.1) | `crates/kernel/adapter/src/provenance.rs:20-23` `format!("adapter:{}@{};sha256:{}", manifest.id, manifest.version, hash)` | ✓ |
| 9 | `adapter_provenance` is a canonicalized-JSON SHA-256 (§4.1) | `provenance.rs:10-23` — `canonicalize` recursively sorts object keys; `serde_json::to_vec` then `Sha256::new()` | ✓ |
| 10 | `NO_ADAPTER_PROVENANCE == "none"` (§4.1) | `crates/kernel/supervisor/src/lib.rs:50` `pub const NO_ADAPTER_PROVENANCE: &str = "none";` | ✓ |
| 11 | `runtime_provenance` scheme is `rpv1:sha256:{hex}` (§4.2) | `crates/kernel/runtime/src/provenance.rs:74-78` `format!("{}:sha256:{}", RuntimeProvenanceScheme::Rpv1.prefix(), to_hex(&digest))` with `prefix() == "rpv1"` | ✓ |
| 12 | Context merge overlays incoming > store (§5.1) | `runner.rs:721-730` — store keys inserted first (lines 722-726), incoming keys inserted after (lines 728-730) | ✓ |
| 13 | Store gate requires manifest declaration + schema-allowed + present in snapshot (§5.2) | `runner.rs:722-726` — conditional on `adapter.provides.context.contains_key(key) && allowed_store_keys.contains(key)`, iterating over `self.context_store.snapshot()` | ✓ |
| 14 | Effect buffer replace on `run()` (§6.1) | `buffering_invoker.rs:132` `guard.pending_effects = effects;` assignment (not extend), inside `impl RuntimeInvoker for BufferingRuntimeInvoker` at lines 117-136; the sink `Vec` is populated by `self.engine.run_reporting(..., &mut effects)` at lines 126-128 | ✓ |
| 15 | Effect buffer drain uses `std::mem::take` (§6.1) | `buffering_invoker.rs:99` `std::mem::take(&mut guard.pending_effects)` | ✓ |
| 16 | Host asserts empty buffer before next `on_event` (§6.1) | `runner.rs:547-551` `if self.runtime.pending_effect_count() != 0 { return Err(HostedStepError::LifecycleViolation { ... }); }` | ✓ |
| 17 | No rollback on handler failure (§6.2) | `runner.rs:793` comment `// SUP-6 alignment: no rollback on handler failure.` | ✓ |
| 18 | Host enrichment is by decision index (§7.2) | `runner.rs:759-761` — `self.applied_effects.record(decision_index, drained_effects.to_vec())` guarded by `if !drained_effects.is_empty()` | ✓ |
| 19 | Kernel capture writes only the empty `record.effects` placeholder; non-empty effects come from host enrichment (§7.3) | `crates/kernel/supervisor/src/capture.rs:187-196` — `CapturingDecisionLog::log` pushes `EpisodeInvocationRecord::from(&entry)` directly; `runner.rs:650-655` later enriches authoritative host effects | ✓ |
| 20 | Strict replay entrypoint (§8.1) | `replay.rs:184-191` `pub fn replay_checked_strict<R: RuntimeInvoker + Clone>(...) -> Result<Vec<EpisodeInvocationRecord>, ReplayError>` | ✓ |
| 21 | Preflight version match (§8.2) | `replay.rs:159-163` `if bundle.capture_version != crate::CAPTURE_FORMAT_VERSION` | ✓ |
| 22 | Preflight event hash validation (§8.2) | `replay.rs:165-171` `for record in &bundle.events { if !record.validate_hash() { ... } }` | ✓ |
| 23 | Preflight duplicate `event_id` rejection (§8.2) | `replay.rs:257-268` `validate_unique_event_ids` | ✓ |
| 24 | Provenance match with `"none"` bidirectional guard (§8.2) | `replay.rs:234-239` — `AdapterRequiredForProvenancedCapture` and `UnexpectedAdapterProvidedForNoAdapterCapture` variants | ✓ |
| 25 | Decision comparison covers non-effect fields positionally (§8.3) | `replay.rs:284-293` — `cap.event_id != rep.event_id || cap.decision != rep.decision || ...` | ✓ |
| 26 | Effect comparison uses `(effect, effect_hash)` equality (§8.3) | `replay.rs:312-329` `if cap_eff.effect != rep_eff.effect || cap_eff.effect_hash != rep_eff.effect_hash` | ✓ |

Footnote on scope: `INGEST-TIME-1` (cross-ingestion normalization
parity) is not verified here because it is explicitly deferred in
`08-replay.md`; it appears in §9 only for completeness.

---

## 12. Supersession notes (non-normative)

This doc does not rewrite any existing spec. It establishes a
canonical v1 reference that the Session 3 rewrite will reconcile
against.

- [`docs/system/kernel-prod-separation.md`](kernel-prod-separation.md) (CANONICAL v1) — compatible. This doc is strictly more specific about the host boundary. No rewrite of kernel-prod-separation is implied.
- [`docs/invariants/07-orchestration.md`](../invariants/07-orchestration.md) (CANONICAL v1) — compatible rule table. Session 3 may add source citations to the `HST-*` and `SUP-*` rows by referencing §9 of this doc.
- [`docs/invariants/08-replay.md`](../invariants/08-replay.md) (CANONICAL v1) — compatible. §§4 and §8 of this doc provide the rationale that `08-replay.md` intentionally omits.
- [`docs/orchestration/supervisor.md`](../orchestration/supervisor.md) (FROZEN, marked `Version: v0`) — semantically correct for v1 supervisor behavior. The `v0` version tag predates the 2026-02-16 migration. Re-anchoring the authority line is the subject of Artifact C (v1 freeze declaration), not this doc.
- [`docs/orchestration/adapter.md`](../orchestration/adapter.md) (FROZEN, marked `Version: v0`) — same posture; re-anchored by Artifact C.
- [`docs/ledger/gap-work/closed/sup2-alignment.md`](../ledger/gap-work/closed/sup2-alignment.md) (CLOSED retrospective) — this doc is its forward-facing companion. The ledger retrospectively tracked the v0 → v1 migration; this doc states the resulting v1 boundary as a living reference.

---

## 13. References

- [Kernel Closure and v1 Workstream Declaration](kernel.md)
- [Current Architecture](current-architecture.md)
- [Kernel/Prod Separation and Host Intent](kernel-prod-separation.md)
- [Orchestration Phase Invariants](../invariants/07-orchestration.md)
- [Replay Phase Invariants](../invariants/08-replay.md)
- [Execution Supervisor (frozen)](../orchestration/supervisor.md)
- [Adapter Contract (frozen)](../orchestration/adapter.md)
- [v1 External Effect Intent Model](../ledger/decisions/v1-external-effect-intent-model.md)
- [Egress Provenance](../ledger/decisions/egress-provenance.md)
- [SUP-2 Alignment (closed retrospective)](../ledger/gap-work/closed/sup2-alignment.md)
