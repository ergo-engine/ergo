---
Authority: OPERATIONAL
Version: v1
Last Updated: 2026-04-23
Owner: Codex
Scope: Session 3 plan-level verification review
Change Rule: Operational log
---

# Session 3 Codex Review

## Scope reviewed

Read in full:

- `docs/ledger/decisions/session-3-plan.md`
- `docs/orchestration/supervisor.md`
- `docs/orchestration/adapter.md`
- `docs/invariants/07-orchestration.md`
- `docs/invariants/08-replay.md`
- `docs/system/host-boundary.md §9`

Independent checks run:

- Full-tree sweep across `docs/**/*.md` for the six Session-3 drift
  patterns named in the plan
- `RunResult` hit mapping in `docs/orchestration/supervisor.md`
- Cross-check of `session-3-plan.md §§2.3–2.4` against
  `host-boundary.md §9`
- Spot verification against current code at HEAD for the S2.2 seam
  (`RuntimeHandle::run`, private `RunResult`, `ReportingRuntimeHandle`)

## Findings

### 1. `supervisor.md` rewrite scope is correctly bounded

`git grep` / `rg` confirms the live `RunResult` framing in
`docs/orchestration/supervisor.md` is exactly the eight-hit set already
enumerated in `session-3-plan.md §2.1`:

- lines 144-145
- line 173
- lines 247-248
- line 271
- line 283
- line 344

I did not find an additional ninth in-scope `RunResult` drift site in
that file. The targeted rewrite shape in `§2.1` is therefore complete
for the `RunResult` problem it is trying to solve.

### 2. `adapter.md` scope looks right

I did not find a missed post-S2.x body drift in
`docs/orchestration/adapter.md`. The body already reads as v1-consistent
host-boundary prose. The remaining work there is correctly framed as:

- frontmatter/tag reconciliation
- removal of the pre-migration note
- a minimal tightening pass rather than a rewrite

### 3. `07-orchestration.md` citation-fold list is incomplete relative to the plan’s own verification bar

This is the main plan-level gap.

`session-3-plan.md §6` says the correctness bar for
`07-orchestration.md` is:

> every SUP-*/HST-*/RTHANDLE-* row in the table has at least one source citation in its Notes entry

But `§2.3` only enumerates citations for:

- `SUP-2`
- `SUP-3`
- `SUP-6`
- `SUP-7`
- `HST-1`
- `HST-3`
- `HST-4`
- `HST-5`
- `HST-6`
- `HST-8`
- `HST-9`

Compared against `host-boundary.md §9`, the following rows still have
current code evidence there but are **not** called out in the plan’s
folding list:

- `CXT-1`
- `SUP-1`
- `SUP-4`
- `SUP-5`
- `SUP-TICK-1`
- `RTHANDLE-META-1`
- `RTHANDLE-ID-1`
- `RTHANDLE-ERRKIND-1`
- `HST-2`

If the intended Session-3 bar is truly "every SUP/HST/RTHANDLE row gets
at least one code citation," `§2.3` needs to expand before drafting.

### 4. `08-replay.md` citation-fold list is also incomplete

Same issue as above.

`session-3-plan.md §6` sets the bar that every `REP-*` row should end
the session with at least one source citation in Notes, but `§2.4`
currently enumerates only:

- `REP-1`
- `REP-2`
- `REP-7`
- `REP-8`
- `REP-SCOPE`

Compared against `host-boundary.md §9`, the following replay rows still
have live evidence but are not named in the plan’s folding list:

- `REP-3`
- `REP-4`
- `REP-5`
- `SOURCE-TRUST`

`REP-6` is a closed clarification note, so I would not force new code
citations there unless Sebastian wants historical anchoring. The four
rows above are the real omission.

### 5. The added `host-boundary.md` self-reference cleanup is real and should be explicit in the plan

Sebastian’s kickoff note called out the need to past-tense
`host-boundary.md`’s self-referential Session-3 language. I independently
confirmed that drift is still present, including but not limited to:

- `docs/system/host-boundary.md:23`
- `docs/system/host-boundary.md:72`
- `docs/system/host-boundary.md:380`
- `docs/system/host-boundary.md:388`
- `docs/system/host-boundary.md:511`
- `docs/system/host-boundary.md:517-518`

Some of these are exactly the user-mentioned `§0` / `§12` category; a
few also live in the `§9` framing that will be archived under the
already-arbitrated Option (b).

This does **not** require scope expansion beyond the existing five-file
Session-3 set, but it should be named in the drive-by slate rather than
left implicit.

### 6. One wording nuance to keep in mind during rewrite review

The plan’s proposed supervisor.md replacement sentence says effect
observation on `ReportingRuntimeHandle` is "consumed only by
`BufferingRuntimeInvoker` in `ergo-host`."

That is accurate for the actual **`run_reporting(...)` call path** in
live code, but it can be misread as "the type is only used there."
Current host prep/demo/test code constructs `ReportingRuntimeHandle`
directly before wrapping it, so when Auggie drafts that prose, I’d
verify it stays scoped to the seam/method, not the type’s total
construction footprint.

This is a review caution, not a plan blocker.

## Verdict

Plan is structurally sound and ready to execute **after one small
expansion pass in the plan itself**:

- expand `§2.3` so its citation-fold inventory matches the "every
  SUP/HST/RTHANDLE row gets at least one source citation" bar in `§6`
- expand `§2.4` so the replay citation-fold inventory includes the
  omitted `REP-3`, `REP-4`, `REP-5`, and `SOURCE-TRUST` rows
- explicitly name the `host-boundary.md` self-reference past-tensing in
  the drive-by scope

Everything else in the plan-level shape checks out against current HEAD.
