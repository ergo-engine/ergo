---
Authority: PROJECT
Date: 2026-04-20
Decision-Owner: Sebastian (Architect)
Participants: Codex, Auggie
Status: DECIDED
Scope: v1
Parent-Decision: ../gap-work/closed/sup2-alignment.md
Resolves: none (forward commitment)
---

# Decision: v1 Host-Boundary Migration — Forward Commitment

## Context

This record is the forward companion to a retrospective closure ledger
rather than a child decision of a prior ruling; the `Parent-Decision`
field above names that ledger.

The v0 → v1 host-boundary migration was tracked retrospectively by
[`sup2-alignment.md`](../gap-work/closed/sup2-alignment.md) and closed
on 2026-03-15 against four enumerated gaps (D1–D4). That closure was
valid for what it tracked.

A forensic audit of post-closure state on 2026-04-19 discovered three
residual v0 shapes the closure gate did not catch:

- `DecisionLogEntry.effects: Vec<ActionEffect>` still exists in
  `crates/kernel/supervisor/src/lib.rs`. Every production call site
  writes `vec![]`, so the field is vestigial v0 residue.
- `RunResult` is still publicly importable from the kernel adapter
  crate. Any holder of a `RuntimeHandle` can observe effects directly
  off the return value, so `SUP-2` is preserved by the buffering
  shim's existence rather than enforced at the type level.
- At the time of the Session 1 audit, host-behavior modules
  (`BufferingRuntimeInvoker`, `ContextStore`, `ensure_handler_coverage`,
  the effect-handler module) still lived under
  `crates/kernel/adapter/src/host/` rather than
  `crates/prod/core/host/`. That file path was a v0 migration artifact.
  Session 2 S2.3 relocates them into `crates/prod/core/host/src/host/`.

These are code-shape residuals, not semantic drift. Runtime behavior
at HEAD `7784f46f` matches the v1 boundary described in
[`host-boundary.md`](../../system/host-boundary.md); the types and
module layout encoding that behavior still carry v0 shapes in places.

The audit also surfaced a process finding: the v0 freeze
([`freeze.md`](../../system/freeze.md)) referenced a joint-escalation
convention for v1-only changes that was never defined in any
reachable doc and was not honored in practice. The 046dd4b spec
rewrite landed without escalation, which is part of what motivated
this pass.

---

## Ruling

1. **Invariant authority.** The v1 host boundary specified in
   [`docs/system/host-boundary.md`](../../system/host-boundary.md) is
   the authoritative invariant reference for host / supervisor /
   adapter ownership, the provenance trinity, effect-buffer
   lifecycle, context-merge precedence, and the strict-replay
   contract. Future work referencing any of those surfaces resolves
   against that document.

2. **Symbol-level commitment.** The v1 architecture freeze surface in
   [`docs/system/freeze-v1.md`](../../system/freeze-v1.md) binds the
   symbol-level commitments that encode the invariants. Changes to
   symbols in its §3 follow the freeze-v1.md §6 change protocol
   (commit-body acknowledgment naming which symbol changed and why).

3. **Residual debt schedule.** Session 2 removes the three residual
   v0 shapes via pre-authorized transformations recorded in
   [`freeze-v1.md §4`](../../system/freeze-v1.md):

   - S2.1 removes `DecisionLogEntry.effects`
   - S2.2 redesigns the runtime seam so `RuntimeHandle::run`'s public
     API returns `RunTermination` only (effect-observation mechanism
     chosen during S2.2 planning)
   - S2.3 relocates host-behavior modules to `crates/prod/core/host/`

   Executing these transformations during Session 2 does not require
   re-escalation.

4. **Escalation-protocol lightening.** The v0 freeze's notional
   joint-escalation convention is not carried into v1. The v1 freeze
   uses a single-sentence commit-body-acknowledgment rule. Rationale:
   lighter discipline that will be followed beats heavier discipline
   that won't. This is a solo-dev-plus-AI codebase; protocol weight
   has to be proportionate to enforcement capacity.

---

## Implementation

The forward commitments are realized through three companion
artifacts. This decision record does not itself specify new behavior,
introduce rule IDs, or enumerate symbols; it records the
forward-commitment decision and cross-references the documents that
carry the content.

- [`host-boundary.md`](../../system/host-boundary.md) — CANONICAL v1
  invariant specification
- [`freeze-v1.md`](../../system/freeze-v1.md) — CANONICAL v1
  symbol-level freeze
- Session task register — S2.1 / S2.2 / S2.3 planning preconditions
  (step-zero audit for S2.2 complete; re-export compatibility question
  for S2.3 recorded)

---

## Methodology

The forensic audit used a four-task pattern. Recording it here so
future boundary-migration closure gates are replayable without
rediscovering the method.

1. **Post-freeze commits against manifest.** Enumerate every commit
   landed after the retrospective closure (`sup2-alignment.md`,
   2026-03-15) and compare each against the post-closure manifest of
   what was supposed to be true. *Catches:* commits that silently
   violated closure without triggering a gate.

2. **Ledger-closure direction classification.** For each closed
   ledger in `gap-work/closed/`, classify the closure as
   *retrospective* (tracked known gaps to closure) vs *forward*
   (committed to state beyond the closure's enumeration). *Catches:*
   retrospective closures that left forward-state commitments
   unwritten and therefore undefended.

3. **Align/reconcile commit-message grep.** Search commit history for
   `align|reconcile|sync` patterns and inspect the diffs. *Catches:*
   invisible prior drift — reconciliation commits are evidence drift
   had occurred even when the original drift wasn't flagged at the
   time.

4. **Symbol-level diff of freeze-state vs HEAD.** For every symbol
   the freeze committed to, diff the symbol's current shape at HEAD
   against its shape at the freeze anchor. *Catches:* shape-only
   follow-through or residual v0 encoding that didn't register as
   semantic drift.

Outputs of this audit are `host-boundary.md §11` (26-row
claim-verification pass at HEAD `7784f46f`), `freeze-v1.md §3`
(symbol list verified via grep against current paths), and
`freeze-v1.md §4` / the §Context of this record (the three residual
shapes and their Session 2 disposition).

---

## What This Does NOT Decide

- **New semantics.** All runtime / ownership / provenance / replay semantics are covered by `host-boundary.md` and inherited from its source material.
- **New rule IDs.** No `SUP-*` / `HST-*` / `REP-*` additions.
- **Session 2 implementation detail.** S2.1 / S2.2 / S2.3 planning artifacts are produced by Codex when each task starts; this record names only the pre-authorization chain.
- **Amendment to `sup2-alignment.md`.** That ledger stays CLOSED and authoritative for the four gaps it closed (D1–D4). This record is its forward companion, not its successor.
- **Primitive ontology.** `freeze.md` (v0 FROZEN) remains authoritative for Source / Compute / Trigger / Action semantics.

---

## Impacted Files

No direct file impact from this decision itself; the companion
documents and the Session 2 work carry the content.

Artifacts produced by the Session 1 pass:

- `docs/system/host-boundary.md` (new — invariant spec)
- `docs/system/freeze-v1.md` (new — freeze surface)
- `docs/system/kernel.md` (edit — workstream log entry C.1; v1 pointer)
- `docs/system/kernel-prod-separation.md` (edit — reference cross-links)
- `docs/orchestration/supervisor.md` (edit — version-tag banner)
- `docs/orchestration/adapter.md` (edit — version-tag banner)
- `docs/invariants/08-replay.md` (edit — architectural-framing prepend)
- `docs/ledger/gap-work/closed/sup2-alignment.md` (CLOSED, unchanged)

Code changes are scheduled through Session 2; see `freeze-v1.md §4`
and the Session task register.
