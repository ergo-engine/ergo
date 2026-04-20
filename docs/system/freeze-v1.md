---
Authority: CANONICAL
Version: v1
Last Updated: 2026-04-20
Owner: Sebastian (Architect)
Scope: v1 architecture freeze surface — supervisor termination-only contract, host-owned effect boundary, provenance trinity, persisted-format types
Change Rule: Commit-body acknowledgment (see §6)
---

# v1 Architecture Freeze Declaration

## 0. Anchor

HEAD: `0218a5fd01f90649de8da8d3924d694aecec7dae`

This declaration freezes the v1 host-boundary architecture surface as
observed at the HEAD above. Each symbol in §3 is named with its crate
path at this commit. The invariant specification this declaration
commits to is [`host-boundary.md`](host-boundary.md) (CANONICAL v1).

The re-anchor from the original authoring HEAD `7784f46f` to
`0218a5f` reflects Session 2's three executed transformations (S2.1,
S2.2, S2.3). §4 retains the pre-authorization language as a
historical note so the execution chain is replayable.

---

## 1. What This Freezes

The v1 freeze covers **symbols**, not **files**. Physical file layout
is allowed to move without touching this list.

Freeze categories:

1. **Supervisor termination-only contract** — what the kernel supervisor observes and what it does not
2. **Runtime seam** — `RuntimeHandle` / `RuntimeInvoker::run` signature shape
3. **Provenance trinity** — `adapter_provenance`, `runtime_provenance`, `egress_provenance` schemes and authoring locus
4. **Host-owned semantic authority** — `ContextStore`, effect loop, capture enrichment
5. **Persisted formats** — capture-bundle types that cross the serde boundary

---

## 2. Relationship to `freeze.md` (v0)

This document **adds** the v1 architecture layer. It does not replace
[`freeze.md`](freeze.md) (v0 primitive-ontology freeze).

| Layer | Document | Status |
|---|---|---|
| Primitive ontology (Source/Compute/Trigger/Action, wiring rules, execution model) | `freeze.md` | v0 FROZEN, still authoritative |
| Host-boundary architecture (this document) | `freeze-v1.md` | v1 CANONICAL |
| Invariant specification (the "why") | `host-boundary.md` | v1 CANONICAL |
| Canonical HST/SUP/REP rule IDs | `07-orchestration.md`, `08-replay.md`, `rule-registry.md` | v1 CANONICAL |

A future change touching the primitive ontology constitutes a v2
decision in the sense of `freeze.md`. This document does not relax
that.

---

## 3. Frozen Surface

Every entry names a symbol, its crate path at HEAD `0218a5f`, and the
behavior it commits to. Files may move; symbols and contracts do not,
except under §4 pre-authorized transformations.

### 3.1 Supervisor Termination-Only Contract

| Symbol | Path | Commitment |
|---|---|---|
| `Supervisor` (struct) | `crates/kernel/supervisor/src/lib.rs` | Mechanical scheduler; no observation of `ActionEffect`, `RunResult`, or domain payloads (`SUP-2`) |
| `DecisionLog` (trait) | `crates/kernel/supervisor/src/lib.rs` | Trait surface is write-only: declares `log(...)` and nothing else (`SUP-7`) |
| `NO_ADAPTER_PROVENANCE` (const `"none"`) | `crates/kernel/supervisor/src/lib.rs` | Sentinel for adapterless captures; `REP-7` bidirectional guard keys on this exact string |
| `RunTermination` (enum, `Serialize`/`Deserialize`) | `crates/kernel/adapter/src/lib.rs` | Persisted on `EpisodeInvocationRecord.termination`; variant set and payload shape are part of the capture-bundle serde surface (adding variants or widening payload requires a `capture_version` bump) |
| `EpisodeInvocationRecord` (struct, `Serialize`/`Deserialize`) | `crates/kernel/supervisor/src/lib.rs` | Capture-bundle decision record; field set is persisted |
| `CapturingDecisionLog` / `CapturingSession` | `crates/kernel/supervisor/src/capture.rs` | Kernel-side capture wrapper; authors non-effect decision fields only (host owns `effects`, `intent_acks`, `interruptions` per §3.4) |

### 3.2 Runtime Seam

| Symbol | Path | Commitment |
|---|---|---|
| `RuntimeInvoker` (trait) | `crates/kernel/adapter/src/lib.rs` | Kernel-owned contract for invoking a runtime; termination-only observable surface to the supervisor |
| `RuntimeHandle` (struct) | `crates/kernel/adapter/src/lib.rs` | Adapter-layer handle used by the supervisor to drive runtime execution |
| `RuntimeHandle::run` | `crates/kernel/adapter/src/lib.rs` | Public signature returns `RunTermination` only; no public path observes effects. `SUP-2` is type-enforced by this seam (post-S2.2). |
| `ReportingRuntimeHandle` (struct) | `crates/kernel/adapter/src/lib.rs` | Adapter-layer handle exposing the low-level reporting seam `run_reporting(graph_id, event_id, ctx, deadline, &mut Vec<ActionEffect>) -> RunTermination`. Consumed only by `BufferingRuntimeInvoker` in `ergo-host`. Effect observation lives on this type rather than on `RuntimeHandle`. |

### 3.3 Provenance Trinity

| Symbol | Path | Commitment |
|---|---|---|
| `adapter_provenance` scheme | `crates/kernel/adapter/src/provenance.rs::fingerprint` | String format `adapter:{id}@{version};sha256:{hex}`; SHA-256 over key-sorted canonicalized manifest JSON |
| `runtime_provenance` scheme | `crates/kernel/runtime/src/provenance.rs::compute_runtime_provenance` | String format `rpv1:sha256:{hex}`; `Rpv1` is the only defined scheme in v1 |
| `egress_provenance` authoring locus | `crates/prod/core/host/src/runner.rs` | Host stamps the bundle post-step; kernel strict-replay validator does not gate on this field (`REP-7` covers adapter + runtime only) |
| `CaptureBundle.{adapter_provenance, runtime_provenance, egress_provenance}` | `crates/kernel/supervisor/src/lib.rs` | Field names and types (two `String`, one `Option<String>`) are persisted |

### 3.4 Host-Owned Semantic Authority

The following behaviors are host-owned regardless of current file
location or support-module layout.

| Behavior | Ownership commitment |
|---|---|
| `ContextStore` read/write authority | Host; supervisor does not observe |
| Effect loop (drain + dispatch) | Host; supervisor does not observe |
| Handler-owned effect application (`SetContextHandler::apply`) | Host |
| Egress dispatch | Host |
| Capture enrichment of `decisions[i].{effects, intent_acks, interruptions}` via `enrich_bundle_with_host_artifacts` | Host is the authoritative writer, keyed on decision index; kernel capture initializes empty `effects`, and host finalization binds authoritative non-empty effects later |
| Context merge precedence (incoming > store) | Host (`HST-6`) |
| Effect-handler coverage gate (`ensure_handler_coverage`) | Host (`HST-5`) |

### 3.5 Persisted Formats

Capture-bundle types that cross the serde boundary. Field-level
changes require explicit serde-compatibility handling (`capture_version`
bump or alias path).

| Symbol | Path | Notes |
|---|---|---|
| `CaptureBundle` | `crates/kernel/supervisor/src/lib.rs` | Current `capture_version` is `v3`; kernel replay enforces strict match |
| `EpisodeInvocationRecord` | `crates/kernel/supervisor/src/lib.rs` | See §3.1 |
| `ExternalEventRecord` | `crates/kernel/adapter/src/capture.rs` | SHA-256 hash contract (`REP-1`); re-exported into supervisor via `use ergo_adapter::capture::ExternalEventRecord` |
| `CapturedActionEffect` | `crates/kernel/supervisor/src/lib.rs` | `(effect, effect_hash)` comparison pair used by strict replay (`replay.rs:312-329`) |
| `RunTermination` | `crates/kernel/adapter/src/lib.rs` | See §3.1 |

---

## 4. Pre-Authorized Transformations (historical)

The carve-out below was pre-authorized by this freeze and executed
during Session 2. It is retained as a historical note so the
pre-authorization chain is replayable; §3.2 above reflects the
post-execution shape.

### 4.1 S2.2 — `RuntimeHandle::run` seam redesign (discharged)

**Status:** Executed. Landed at HEAD `0218a5f` (Session 2 S2.2).

**Prior signature (HEAD `7784f46f`):** `RuntimeHandle::run(...) -> RunResult { termination, effects }`. Any holder of a `RuntimeHandle` — including prod-side callers outside the buffering shim — could observe effects directly off the return value, so `SUP-2` was preserved by the shim's existence rather than enforced by the type.

**Executed transformation:** `RuntimeHandle::run`'s public signature now returns `RunTermination` only. A separate adapter-layer type `ReportingRuntimeHandle` carries the low-level reporting seam `run_reporting(..., effects_out: &mut Vec<ActionEffect>) -> RunTermination`, which is consumed only by `BufferingRuntimeInvoker` in `ergo-host`. `RunResult` remains inside the adapter crate as a private type; it is no longer part of the freeze surface. Post-S2.2, `SUP-2` is type-enforced by the public seam rather than preserved by the shim's existence.

**Step-zero audit (historical):** Codex's five-site audit of the original `RunResult`-producing sites in `adapter/src/lib.rs` at HEAD `7784f46f` (lines 459, 468, 476, 498, 512) fed S2.2 planning. Post-execution those sites live inside the private `execute_once` helper in `adapter/src/lib.rs`.

---

## 5. Explicit Non-Scope

This freeze does not cover:

- Physical module/file locations (covered by S2.3; layout is free to move)
- Function-internal implementation details where no symbol or serde shape is involved
- v0 primitive ontology (covered by `freeze.md`)
- Authoring layer (covered by `freeze.md` §7)
- Workflow/process rules (`DOC-GATE-1` and similar)
- SDK composition (`SDK-CANON-*`; covered by `kernel-prod-separation.md`)

---

## 6. Change Protocol

**Rule:** Changes to symbols in §3 require explicit acknowledgment in the commit body naming which symbol changed and why.

That is the entire protocol.

**Rationale note:** The v0 freeze (`freeze.md`) referenced a joint-escalation workflow that was not defined in any reachable doc and was not honored in practice. Lighter discipline that will be followed beats heavier discipline that won't. This is a solo-dev-plus-AI codebase; protocol weight has to be proportionate to enforcement capacity.

The symbol-specific scope of §3 keeps the surface narrow enough that drift on it is notable. When drift does happen, [`host-boundary.md`](host-boundary.md) (invariant spec) and the Session 1 retrospective at [`v1-host-boundary-migration.md`](../ledger/decisions/v1-host-boundary-migration.md) provide the working memory for cheap reconstruction.

---

## 7. Companion Documents

- [`host-boundary.md`](host-boundary.md) — v1 CANONICAL invariant specification; the "why" behind every §3 commitment
- [`freeze.md`](freeze.md) — v0 primitive-ontology freeze; still authoritative for Source/Compute/Trigger/Action
- [`kernel.md`](kernel.md) — v0 kernel closure declaration; this document is its v1 successor for host-boundary concerns
- [`kernel-prod-separation.md`](kernel-prod-separation.md) — kernel/prod boundary rules; §3.4 of this document names the same boundary in symbol terms
- [`rule-registry.md`](../invariants/rule-registry.md) — canonical HST/SUP/REP rule IDs
- [`07-orchestration.md`](../invariants/07-orchestration.md) — orchestration-phase invariant tables
- [`08-replay.md`](../invariants/08-replay.md) — replay-phase invariant tables
