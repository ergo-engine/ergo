---
Authority: PROJECT
Date: 2026-05-14
Decision-Owner: Sebastian
Participants: Augment
Status: REJECTED
Rejected-Date: 2026-05-16
Rejected-By: Sebastian
Surfaced-By: PUB-4 SDK rustdoc hardening (Q-THREADING analysis)
Related: docs/ledger/decisions/sdk-threading-send-sync.md
---

# Rejected Decision: Structural Enforcement of Primitive Statelessness

> Statelessness itself is settled doctrine, enforced at registration
> by SRC-8 / CMP-9 / TRG-9 / ACT-10 and detected at runtime by
> capture/replay. What was rejected is one specific *enforcement
> mechanism* proposal layered on top of the existing nets.

## Rejection

Rejected on audit against the kernel. The proposal does not survive
its own framing. Recorded here so the rejection rationale is part of
the audit trail; the body below is preserved as the original proposal.

The proposal acknowledges (lines 51–53 of the original body) that
replay divergence makes the failure observable, then justifies a new
enforcement layer using three reasons that do not implicate the
kernel:

1. "The host loop is single-threaded, so concurrent corruption is
   not observable." That is the kernel's deliberate execution model,
   not a gap.
2. "Sequential corruption is observable but easy to miss in tests
   that build a fresh `Ergo` handle per test." That is a test
   methodology gap. Fix the tests, not the kernel.
3. "Capture/replay determinism testing exercises only primitives
   that happen to be stateless." Same: test-coverage gap.

The kernel's determinism enforcement is intentionally layered:

- Trait shape: primitive methods take `&self`, not `&mut self`.
- Manifest contract + registration validation: SRC-8, CMP-9, TRG-9,
  ACT-10 reject any primitive declaring disallowed state.
- Capture/replay: detects determinism violations at runtime
  regardless of how they were introduced (interior mutability,
  statics, FFI, ambient state, iteration-order nondeterminism).

The proposed Net 4 (derive macros / marker traits / `Stateless<T>`
newtypes) would only catch one vector — interior mutability on
struct fields — while missing static atomics, ambient I/O, FFI
handles, and any other source of nondeterminism that does not surface
in the field types. Replay is the universal net by design; the kernel
chose detection over structural prevention for determinism violations
in general, not just for the field-level case.

The genuinely useful observation in this doc is the test-coverage
gap: workspace determinism tests rebuild `Ergo` per test and so do
not exercise the multi-invocation reuse path that would expose a
stateful primitive. That belongs in a testing issue scoped to a
determinism-stress test reusing one `Ergo` handle across N runs and
asserting replay equivalence. It is not a kernel-doctrine question.

For the threading proposal's orthogonality argument, the entire
sibling proposal collapses to one sentence inlined into
`sdk-threading-send-sync.md` under Determinism:

> Adding `Send + Sync` does not introduce new state-related risk.
> Per-instance interior mutability would manifest as non-determinism
> and is caught by capture/replay; the manifest contract (SRC-8,
> CMP-9, TRG-9, ACT-10) remains the registration-time enforcement
> layer. No additional enforcement is required.

Pattern to internalize for future "latent bug class" proposals: ask
(a) is the bug exhibited anywhere in the repo, (b) is it detectable
by an existing mechanism, (c) does the proposed enforcement
comprehensively prevent the class or only one vector. If the answers
are "no, yes, partially" — as they are here — the right disposition
is a test or a doc note, not a kernel proposal.

---

## Context (original proposal, preserved for audit trail)

While analyzing the threading proposal for `Ergo`
(see `sdk-threading-send-sync.md`), a latent determinism bug class in
the runtime primitive contract was identified. This document records
the bug class and the proposed fix shape so it does not get lost.

The bug class exists today, regardless of any threading change. It is
not introduced by adding `Send + Sync` to primitive traits; that change
would only make the bug *more visible* by exposing it under concurrent
use in addition to sequential use.

## The Bug Class

The runtime primitive traits in `ergo-runtime`
(`SourcePrimitive`, `ComputePrimitive`, `ActionPrimitive`, and
`TriggerPrimitive`) do not forbid mutable state inside the primitive
struct itself. A primitive author can write:

```rust
struct CountingSource {
    counter: Mutex<u64>,         // or Cell<u64>, AtomicU64, RefCell<u64>
    cache: Mutex<HashMap<...>>,  // or any per-instance mutable cache
}

impl SourcePrimitive for CountingSource { ... }
```

This compiles, registers cleanly into the catalog, and runs. It also
silently breaks Ergo's determinism guarantee:

- Run 1 sees `counter == 0`, increments to `1`, captures decisions
  derived from `counter == 0`.
- Run 2 (same project, same fixture, same `Ergo` handle) sees
  `counter == 1`, captures decisions derived from `counter == 1`.
- Replay of run 1's capture against the same handle observes
  `counter >= 1` and may diverge from the captured decisions.

The first two runs alone are enough to break the foundational property
that captures be reproducible from the graph + events. Replay
divergence makes the failure observable.

## Why It Is Not Caught Today

- The host loop is single-threaded, so concurrent corruption is not
  observable. (This changes if the threading proposal lands.)
- Sequential corruption is observable but easy to miss in tests that
  build a fresh `Ergo` handle per test rather than reusing one.
- The kernel deterministic-execution doctrine treats primitive
  statelessness as an *expectation*, not an *enforcement*.
- Capture/replay determinism testing in the workspace exercises only
  primitives that happen to be stateless.

## Doctrinal Position

State that should persist across primitive invocations belongs in the
runtime's tracked state mechanism (the per-run state slots that flow
through `ExecutionContext`). That state is:

- captured in the bundle,
- restored during replay,
- isolated per run.

State held inside the primitive struct itself is none of those things.
It survives across runs, it is invisible to capture, and it cannot be
restored during replay. Holding such state is therefore a contract
violation regardless of threading.

## Proposed Fix Shape

This section names the *shape* of the fix; the concrete kernel design
is its own work item.

### Option 1: Type-system enforcement (preferred)

Make the primitive trait methods take `&self` (already the case) but
add a marker that disallows interior mutability at the field level.
Rust does not have a native "no interior mutability" bound, so the
implementation choices are:

- Require `Sync` *and* document that interior mutability across runs
  is a contract violation. (This catches the cross-thread case but
  not the single-thread sequential case.)
- Introduce a `#[derive(StatelessPrimitive)]` macro that asserts every
  field type implements a marker trait (e.g. `ImmutableData`) blanket-
  impl'd for plain data and `Arc<T>` of plain data, but not for
  `Cell`, `RefCell`, `Mutex`, `AtomicU64`, etc. Implementors who
  legitimately need shared immutable state (e.g. precomputed lookup
  tables in `Arc`) pass; implementors who hold mutable state get a
  compile error pointing at the offending field.
- Wrap the primitive struct behind a `Stateless<T>` newtype whose
  constructor enforces the same field check via a derive macro.

Recommended starting point: the derive macro approach. It localizes
the rule in one place, gives implementors actionable compile errors,
and keeps the primitive trait declarations clean.

### Option 2: Doctrinal + lint enforcement

Document the rule in `docs/invariants/` and add a `clippy` or
custom-lint check that flags `Cell`, `RefCell`, `Mutex`, atomics,
and similar types as fields of any type implementing one of the
primitive traits.

Weaker than option 1 (lints can be silenced; tests cannot reach
external implementors) but cheaper to land and may be a useful
intermediate step.

### Option 3: Runtime check at registration

When a primitive is registered into the catalog, run a one-time
self-check that invokes the primitive twice with identical inputs in
an isolated context and asserts identical outputs. Catches some
violations; misses any state that takes more than one invocation to
diverge.

## Why This Is Independent of `Send + Sync`

The threading proposal makes this bug class more visible (multi-
threaded corruption joins the existing single-threaded sequential
corruption), but it does not create or remove the bug. The fix shape
above is needed regardless of whether the threading proposal lands.

## Status and Next Steps

Status: PROPOSED. Captured here so it is not lost; not blocking v1.

If pursued, next steps are: (a) survey existing primitives for
violations (expected: zero, since the in-repo set is plain data),
(b) decide which option above to implement, (c) write a kernel plan
doc, (d) implement and gate via CI.

If deferred past v1: this document is the canonical record of the
bug class and the intended fix shape. It should be linked from any
SDK doc that promises determinism.
