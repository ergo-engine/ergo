---
Authority: PROJECT
Date: 2026-05-16
Author: Sebastian (Architect) + Augment
Status: OPEN
Branch: TBD
Tier: 4 (Production hardening — test coverage)
Surfaced-By: >-
  docs/ledger/decisions/rejected-structural-enforcement-of-statelessness.md
  (the genuinely useful observation extracted from the rejected proposal)
---

# Determinism Stress Test: Handle-Reuse Across Runs

## Scope

Add workspace test coverage that exercises the multi-invocation reuse
path of a single `Ergo` handle. Today's determinism tests rebuild a
fresh `Ergo` per test, which does not exercise the path where a
stateful primitive's per-instance state would survive across runs and
diverge from replay.

The kernel's determinism enforcement is already layered (trait shape,
manifest validation via SRC-8/CMP-9/TRG-9/ACT-10, and capture/replay
as the universal runtime net). What is missing is a test that actually
puts the runtime net under load along the reuse axis.

## Context

The rejected proposal at
`docs/ledger/decisions/rejected-structural-enforcement-of-statelessness.md`
identified — correctly — that current tests build `Ergo` fresh per
test. That is a test-coverage gap, not a kernel-doctrine question.
This ledger row tracks closing the coverage gap.

## Start Gate

None. Test addition only; no semantic or kernel change required.

## Delivery Changes

1. Add a workspace-level integration test (in the appropriate
   existing test crate) that:
   - constructs a single `Ergo` handle once,
   - runs the same profile N times in sequence against that handle
     (N ≥ 3),
   - captures each run,
   - replays each capture against the same handle,
   - asserts capture/replay equivalence for every run.
2. Choose N and the test fixture so the test runs in well under the
   workspace test budget; the goal is the reuse axis, not load.
3. Document the test's purpose in its module/file header so its role
   as the handle-reuse determinism net is obvious to future readers.

## Closure Ledger

| ID | Task | Closure Condition | Owner | Status |
|----|------|-------------------|-------|--------|
| DST-1 | Locate the right test crate / module for handle-reuse coverage | Decision recorded inline in the test file header; placement reviewed against existing capture/replay tests | TBD | OPEN |
| DST-2 | Implement the N-run reuse-and-replay test | Test compiles, runs in CI within budget, and passes against current in-repo primitives | TBD | OPEN |
| DST-3 | Negative-case proof (optional but preferred) | A throwaway local fixture with deliberately stateful primitive fields makes the test fail in the expected way; fixture is removed before merge, with the failure mode recorded in the test header | TBD | OPEN |
| DST-4 | Cross-link from kernel trait docstrings | The four `*Primitive` trait docstrings already point at the rejected-enforcement decision; once this test exists, add a sentence noting the runtime net is exercised by the DST-* test | TBD | OPEN |

## Design Constraints

- Do not introduce structural enforcement of statelessness; that path
  is REJECTED. See
  `docs/ledger/decisions/rejected-structural-enforcement-of-statelessness.md`.
- Do not weaken any existing capture/replay test.
- Keep the test focused on the handle-reuse axis. Other determinism
  coverage (ambient I/O, FFI, static atomics) is out of scope here
  and is already covered by capture/replay along its own paths.
