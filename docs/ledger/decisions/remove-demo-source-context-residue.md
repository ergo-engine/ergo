---
Authority: PROJECT
Date: 2026-06-07
Decision-Owner: Sebastian
Participants: Claude, Codex
Status: DECIDED
Scope: Pre-publish dead-code cleanup for demo-source-context residue
---

# Decision: Remove Demo Source Context Residue

## Context

`ergo-adapter` still exposed two demo-source-context names:

- `DemoSourceContextError`
- `ensure_demo_sources_have_no_required_context(...)`

`ergo-host` still carried matching setup-error plumbing through
`HostAdapterSetupError::DemoSourceContext`, even though the variant no
longer had a construction site.

This record has no PHASE_INVARIANTS ID. It is dead-code removal, not an
invariant fix.

## Decision

Remove `DemoSourceContextError` and
`ensure_demo_sources_have_no_required_context(...)` from `ergo-adapter`.

Remove the dead `HostAdapterSetupError::DemoSourceContext` variant, its
`Display` arm, its `Error::source` arm, and the corresponding
`DemoSourceContextError` import from `ergo-host`.

`FaultRuntimeHandle` is explicitly out of scope. It remains live
supervisor replay/integration test-harness machinery and is kept for
the first alpha.

## Basis

Workspace grep found zero callers for
`ensure_demo_sources_have_no_required_context(...)`.

The host variant had no construction site after `d32a60f7`
(2026-05-12). It remained only as a declaration plus trait arms.

History shows this was orphaned-by-cleanup, not designed-as-dead:

- Introduced on 2026-02-15 with two callers.
- Gained a third caller on 2026-03-04.
- The callers were removed across `e383d8f3`, `d32a60f7`, and
  `84e277c1`.
- The kernel-side helper survived those cleanups because it lived in
  `crates/kernel/adapter/src/lib.rs`, separate from each deleted caller.

## Safety

No invariant is weakened.

The safety rule duplicated by the removed helper — refuse a run when a
source needs adapter-provided context and no adapter is bound — remains
enforced on the live run path by:

- `scan_adapter_dependencies(...)`
- `ensure_adapter_requirement_satisfied(...)`

Those host checks are the live enforcement path. Removing the orphaned
adapter helper removes redundant residue, not the protection.

## Correction Recorded

`docs/plans/pre-publish-review-findings.md` commit `c8abc84` asserted
that these names were "used through host setup" and "not accidental dead
code." Both claims were false. The assertion came from a stale
understanding and was not caught until a disposition-drift concern
prompted git-history verification.

This correction is recorded here so the error is visible rather than
silently overwritten.
