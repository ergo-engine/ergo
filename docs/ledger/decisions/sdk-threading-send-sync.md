---
Authority: PROJECT
Date: 2026-05-14
Decision-Owner: Sebastian
Participants: Augment, Codex
Status: PROPOSED
Resolves: Q-THREADING (raised during PUB-4 SDK rustdoc hardening)
Depends-On: docs/ledger/decisions/sdk-error-surface-wrapping.md
---

# Decision: SDK Engine Handle Threading Contract

## Context

This is not a new direction; it is the closure of work already
scheduled and publicly deferred.

- `TriggerPrimitive` already carries `: Send + Sync` as of PR-C
  (commit `fd9ad7e`, 2026-01-04), landed as part of the TRG-STATE-1
  closure (`crates/kernel/runtime/src/trigger/mod.rs:399`). The
  pattern this proposal extends is therefore already in-tree on one
  of the four primitive traits.
- The SDK rustdoc at `crates/prod/clients/sdk-rust/src/lib.rs:28-48`
  documents the current handle as single-threaded and states
  verbatim that "tightening those bounds is a kernel-level decision
  tracked separately and is on the post-v1 roadmap." This proposal
  is that tracked decision.

What surfaced the work now: PUB-4 hardening of `ergo-sdk-rust` added
a compile-time `assert_send_sync::<Ergo>()` to keep the documented
threading model honest. The assert failed at compile time, confirming
that the SDK's `Ergo` engine handle is **not** `Send + Sync` today.

Root cause: the runtime catalog stores the three remaining primitive
trait objects without thread-safety bounds.

```rust
// crates/kernel/runtime/src/catalog.rs (sketch)
HashMap<String, Box<dyn SourcePrimitive>>
HashMap<String, Box<dyn ComputePrimitive>>
HashMap<String, Box<dyn ActionPrimitive>>
// TriggerPrimitive entries already carry Send + Sync via PR-C.
```

Because `dyn SourcePrimitive` / `dyn ComputePrimitive` /
`dyn ActionPrimitive` carry no `Send + Sync` bounds, the compiler
conservatively assumes any implementor may be `!Send` or `!Sync`. That
propagates up: `CoreRegistries` is `!Sync`, `RuntimeSurfaces` is
`!Sync`, and therefore `Ergo` is `!Send + !Sync`.

This blocks the natural multi-threaded embedding pattern
(`Arc<Ergo>` shared across worker threads, `tokio::spawn` of an
SDK call, embedding inside `axum`/`actix`/any multi-threaded server).

## Proposal

Complete the pattern PR-C established for `TriggerPrimitive` by
tightening the three remaining primitive traits in `ergo-runtime`:

```rust
pub trait SourcePrimitive: Send + Sync { ... }
pub trait ComputePrimitive: Send + Sync { ... }
pub trait ActionPrimitive: Send + Sync { ... }
// pub trait TriggerPrimitive: Send + Sync { ... }  // already done (PR-C)
```

This automatically makes `CoreRegistries`, `RuntimeSurfaces`, `Ergo`,
and `StopHandle` (already `Send + Sync`) all `Send + Sync`. No SDK or
host changes are required beyond restoring the `assert_send_sync::<Ergo>()`
static guarantee and updating the documented threading model in
`crates/prod/clients/sdk-rust/src/lib.rs:28-48` to retire the
"post-v1 roadmap" deferral.

## Tradeoffs

What is gained:

- `Ergo` becomes embeddable in any multi-threaded host
  (`Arc<Ergo>`, async runtimes, web frameworks).
- The SDK threading section becomes enforceable by compile-time assert.
- Forward compatibility with future async primitive traits (most async
  runtimes require `Send` on returned futures).
- Custom primitives compose with `tokio::spawn`, `rayon::scope`, and
  similar concurrency primitives out of the box.

What is paid:

- Primitive authors lose `Rc<T>` and `RefCell<T>` shortcuts; they must
  use `Arc<T>` / `Mutex<T>` / `RwLock<T>`. Performance impact is
  bounded by uncontended atomic and lock costs (~ns scale) and is not
  measurable for any realistic Ergo primitive.
- Authors of FFI-wrapping primitives (e.g. some SQLite bindings, GPU
  contexts) must either choose a thread-safe wrapper or assert
  `unsafe impl Send + Sync` with documented justification.
- The contract is one-directional. Once published, the bound cannot be
  removed without a breaking change. (Adding it later is also breaking,
  so the choice is between two breaking-change windows; the early
  window matches the ecosystem direction-of-travel.)

## Determinism

This change does not affect kernel determinism. `Send + Sync` are
compile-time markers and generate no runtime code. The host loop
remains single-threaded, per-run state is created fresh per run, and
capture/replay continues to operate on the deterministic event stream.

What this change does *not* enable:

- Parallel execution of graph nodes within a run (would require a
  deterministic scheduler in the kernel — separate, larger work).
- Concurrent invocation of the same primitive instance from a single
  host loop (the host loop still serializes calls).

Adding `Send + Sync` does not introduce new state-related risk.
Per-instance interior mutability would manifest as non-determinism
and is caught by capture/replay; the manifest contract (SRC-8,
CMP-9, TRG-9, ACT-10) remains the registration-time enforcement
layer. No additional enforcement is required.

## Audit Plan

Before landing the trait change, audit every in-repo
`impl SourcePrimitive`, `impl ComputePrimitive`, `impl ActionPrimitive`:

1. Confirm fields are `Send + Sync` (plain data, `Arc`, atomics,
   thread-safe synchronization primitives).
2. Flag any `Rc`, `RefCell`, raw pointers, or non-thread-safe FFI
   handles. For each, decide between thread-safe replacement vs
   `unsafe impl Send + Sync` with documented justification.
3. Check `crates/kernel/runtime/src/{source,compute,action}/` plus
   any custom primitives elsewhere in the workspace.
   (`trigger/` is excluded: PR-C already tightened `TriggerPrimitive`
   and audited its implementors.)

Expected outcome: the in-repo primitives are already plain data and
will pass without changes. Migration cost lands on hypothetical
external primitive implementors only.

## Implementation Plan

Single commit shape (after audit confirms zero in-repo migration):

1. Add `: Send + Sync` to the three remaining trait declarations:
   - `crates/kernel/runtime/src/source/mod.rs` (`SourcePrimitive`)
   - `crates/kernel/runtime/src/compute/mod.rs` (`ComputePrimitive`)
   - `crates/kernel/runtime/src/action/mod.rs` (`ActionPrimitive`)

   `TriggerPrimitive` in `crates/kernel/runtime/src/trigger/mod.rs`
   already carries the bound (PR-C) and is not touched.
2. Restore `assert_send_sync::<Ergo>()` in
   `crates/prod/clients/sdk-rust/src/lib.rs` (extend the existing
   `StopHandle` assert block).
3. Update the crate-level *Threading model* section in
   `crates/prod/clients/sdk-rust/src/lib.rs` (lines 28-48 of the
   current file) to retire the "post-v1 roadmap" deferral and
   document the new contract: `Ergo` is `Send + Sync`, may be shared
   via `Arc`, concurrent calls execute independent host runs.
4. Update the `Ergo` struct doc to match.
5. Run `cargo check --workspace --all-targets` and the SDK test +
   doctest suite.

Cross-reference: the audited primitives list and the plan execution
record live in `docs/plans/sdk-threading-send-sync-plan.md` (created
when this decision moves to DECIDED).

## What This Does Not Change

- Host loop scheduling — unchanged.
- Determinism guarantees — unchanged (see Determinism section above).
- The primitive contract beyond thread safety — unchanged. Statelessness
  remains enforced at registration via SRC-8, CMP-9, TRG-9, ACT-10 and
  detected at runtime via capture/replay; no additional layer is added.
- `StopHandle` — already `Send + Sync`.
- SDK error surface — unchanged.

## Status and Next Steps

Status: PROPOSED. Awaits owner decision on landing window.

Scope is the closure of work already scheduled by PR-C
(`TriggerPrimitive`) and publicly deferred by
`crates/prod/clients/sdk-rust/src/lib.rs:28-48` ("post-v1 roadmap").
The decision pending is the *when*, not the *whether-it-was-planned*.

If accepted, next steps are: (a) run the in-repo primitive audit, (b)
write the plan doc, (c) execute the single-commit change, (d) update
the SDK threading documentation accordingly, retiring the post-v1
deferral language in `lib.rs:38-41`.

If deferred further, the documented single-thread model in
`crates/prod/clients/sdk-rust/src/lib.rs` remains the current contract.
The deferral language already in `lib.rs:38-41` continues to point at
this decision as the tracking record.
