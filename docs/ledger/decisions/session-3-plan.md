---
Authority: OPERATIONAL
Version: v1
Last Updated: 2026-04-23
Owner: Sebastian (Architect)
Scope: Session 3 plan — reconcile legacy canonical docs against v1 boundary
Change Rule: Operational log
---

# Session 3 Plan — Legacy Canonical Doc Reconciliation

**Anchor:** HEAD `dbd376fe25eb45ea067285400fc362ef70c97694` (post-rebase v1 high-water mark). Drive-by anchor-hash sync landed at `fdb42ca`.

**Hard bar:** By end of Session 3 every v1-relevant doc is internally consistent with post-S2.x reality. No deferrals within scope.

---

## 1. Full-Tree Sweep Results (97 docs scanned)

Six drift patterns run across `docs/**/*.md`. Historical / closed / planning artifacts (`ledger/decisions/s2.*-plan.md`, `session-2-closure-audit.md`, `v1-host-boundary-migration.md`, `ledger/dev-work/closed/*`, `ledger/gap-work/closed/*`) are excluded from scope — their references are correctly past-tense or authoring-time anchors and rewriting them would break forensic replay.

| # | Pattern | In-scope drift hits | Out-of-scope (historical) |
|---|---|---|---|
| 1 | `RunResult` framed as public | `supervisor.md` ×8 (L144-145, 173, 247-248, 271, 283, 344) | 11 files (planning, closed, audit, correct v1 framing) |
| 2 | `crates/kernel/adapter/src/host/` as current | 0 | 4 files (all past-tense historical) |
| 3 | `DecisionLogEntry.effects` as existent | 0 | 6 files (all past-tense "was removed") |
| 4 | `RuntimeHandle` as effect-capable | 0 | invariants 11/15 cite it as enforcement locus, signature-agnostic, still correct |
| 5 | S2.1/S2.2/S2.3 as pending | `kernel.md:174` (minor tense: "Pre-authorizes" → "Pre-authorized") | rest are past-tense historical notes |
| 6 | Stale line numbers in v1 canonical docs | 0 | spot-checked 40+ citations in `host-boundary.md §9`; all resolve at HEAD |

**Verdict.** The four docs Sebastian named (`supervisor.md`, `adapter.md`, `07-orchestration.md`, `08-replay.md`) cover the structural reconciliation work. One minor drive-by surfaced (`kernel.md:174` tense). No new files enter scope beyond that.

**Scope-completeness reviewer (independent re-sweep):** confirms five-file list is complete and necessary; no scope expansion required. Cold-sweep of authoring/primitives/contracts/system subtrees clean. Cross-check of invariants 11/15 confirms their `ergo_adapter::RuntimeHandle::run` citations are signature-agnostic enforcement-locus references and remain correct post-S2.2 — no edits needed there.

---

## 2. Per-Doc Plan

### 2.1 `docs/orchestration/supervisor.md` — **rewrite (targeted)**

v0 FROZEN frontmatter; body carries pre-S2.2 `RunResult` framing at 8 sites. Rewrite is targeted, not a full-document rewrite:

- **§2.3 lines 144-146:** replace the "Supervisor does not receive RunResult" paragraph with v1 framing: *`RuntimeInvoker::run()` returns `RunTermination` only; `RunResult` is private to `ergo-adapter` post-S2.2; effect observation flows through `ReportingRuntimeHandle::run_reporting(...)` in the adapter layer and is consumed by `BufferingRuntimeInvoker` in `ergo-host`*. **Wording caution (per Codex finding 6):** phrase effect-observation flow as going *through the `run_reporting(...)` seam* rather than "consumed only by `BufferingRuntimeInvoker`" — the latter reads as a type-level constructability claim, which is false (the type can be constructed in tests / preparation code; only the seam is host-boundary-enforced).
- **§2.4 line 173:** replace "belongs in RunResult (as data)" with language that does not cite a private type. Proposal: "belongs in the runtime's effect stream (as data), not in `ErrKind` (as termination)."
- **§3 SUP-2 table rows 247-248:** "Invariant" and "Enforcement" cells rewritten to match v1: `RuntimeInvoker::run()` returns `RunTermination`; `RunResult` is private to `ergo-adapter`.
- **§3 SUP-4 row 271:** "Enforcement" cell replaced: "API: Supervisor's `RuntimeInvoker` seam returns `RunTermination` only; no adapter-private types reach Supervisor."
- **§4.2 line 283 and §4 line 344:** replace "inspection of RunResult" / "no RunResult access" with "inspection of adapter-private effect stream" / "no adapter-private-type access".
- **Frontmatter version-tag and body pre-migration note:** see §5.1 arbitration request. If Option (a) lands, remove the 2026-04-20 pre-migration note (lines 10-15) from the body and drop `Verified Against Tag` from the frontmatter; replace with the CANONICAL-style frontmatter shape used by `07-orchestration.md`.

Non-goals: does **not** rewrite §1, §2.1–2.2, §3 SUP-1/SUP-3/SUP-5/SUP-6/SUP-7, §4.3–4.4. Those sections are v1-correct already.

### 2.2 `docs/orchestration/adapter.md` — **review + minimal v1 framing pass**

Re-reading the current body: it already describes v1 behavior (host-owned ingress/egress, adapters as declarative compatibility contracts). There is **no** residual post-S2.3 drift in the body. The 2026-04-20 version-tag note already points at `freeze-v1.md` and `host-boundary.md`.

The version-tag reviewer independently audited §1–§6 and confirms every section passes the CANONICAL bar without body rewrites.

Planned scope:

- Audit every sentence against `host-boundary.md §§3–4` and `freeze-v1.md §3` for any subtle ergo-adapter-owned-host-behavior framing. Expect a handful of tightening edits, not a rewrite.
- Cross-reference the "host dispatches egress-owned effects through configured egress channels" claim against `runner.rs:837`.
- **Frontmatter version-tag and body pre-migration note:** see §5.1. Same treatment as supervisor.md — drop the 2026-04-20 note and `Verified Against Tag` field; re-tag to `v1 CANONICAL` with the shape used by `07-orchestration.md`.
- **Doc-wide consistency scan (inherited from commit 1 pattern, Sebastian 2026-04-24).** Before drafting, run `grep -nE "FROZEN|v0|freeze-candidate|freeze" docs/orchestration/adapter.md` and enumerate every hit. Categorize each as (a) drift-site-to-update, (b) historical-reference-to-preserve, or (c) ASCII-diagram cross-reference to another doc's authority state. Commit 2 scope inherits from commit 1: H1 title updated to v1; preamble v0 references updated to v1; any "freeze-candidate" / "Treat it as law" / equivalent aspirational-FROZEN preamble rephrased to CANONICAL / Operational-log framing; any `§N Freeze Status`-equivalent section rewritten to `Authority Status` naming the current CANONICAL v1 state, the re-tag transition, and the historical-artifact status of the Revision History and signature block; a new Revision History row added naming the re-tag commit with Session 3 cross-reference. `§N Signatures`-equivalent blocks (if any) are deliberately untouched — preserved as dated historical artifacts; the new Authority Status section makes that preservation explicit. Commit 2 report must include the categorized grep output alongside the body rewrites.

### 2.3 `docs/invariants/07-orchestration.md` — **citation folding (row-by-row)**

Canonical v1 already. The verification bar (per §6, tightened) is: every `SUP-*`/`HST-*`/`CXT-*`/`RTHANDLE-*` row's Notes entry carries at least one filepath-anchored citation, except rows explicitly CLOSED or closed-by-clarification, whose existing Notes are preserved as-is.

Row-by-row audit against the current Notes section and `host-boundary.md §9`:

- **CXT-1** — Notes exists; no filepath. Add `crates/prod/core/host/src/runner.rs:714-744` (host-side `build_external_event` enforcement locus, per §9).
- **SUP-1** — Notes: *"Private `graph_id` field with no setters; set only at construction."* Add filepath anchor `crates/kernel/supervisor/src/lib.rs`.
- **SUP-2** — Notes: *"`RuntimeInvoker::run()` returns `RunTermination` only; no `RunResult` exposure."* Add `crates/kernel/adapter/src/lib.rs` (RuntimeInvoker::run public seam) and `:182` (private `RunResult`). Tighten to match §9 SUP-2 row prose.
- **SUP-3** — add `crates/kernel/supervisor/src/replay.rs:184` (strict replay entry).
- **SUP-4** — Notes carries `should_retry()` logic. Add filepath anchor `crates/kernel/supervisor/src/lib.rs`.
- **SUP-5** — Notes carries enum-variant logic. Add filepath anchor `crates/kernel/supervisor/src/lib.rs`.
- **SUP-6** — add `crates/prod/core/host/src/runner.rs:793` (no-rollback comment).
- **SUP-7** — Notes: *"`DecisionLog` trait has only `fn log()`."* Add filepath anchor `crates/kernel/supervisor/src/lib.rs`.
- **SUP-TICK-1** — Notes already comprehensive (deferred-retry behavior, `Tick`→`Pump` serde alias, `replay_harness.rs` test). Add filepath anchor(s) for the deferred-retry path.
- **RTHANDLE-META-1** — Notes describes `execute_with_metadata(...)` behavior. Add filepath anchor `crates/kernel/adapter/src/lib.rs`.
- **RTHANDLE-ID-1** — Notes describes `FaultRuntimeHandle` behavior. Add filepath anchor `crates/kernel/adapter/src/lib.rs`.
- **RTHANDLE-ERRKIND-1** — CLOSED (2026-02-06). Notes are already extensive with prior-bug analysis. **No citation-fold.** Audit the existing `runtime_validate` / `validate_composition` path descriptions still resolve against HEAD; if they do, leave alone.
- **HST-1** — Notes exists but no filepath. Add `runner.rs:576` (drain), `runner.rs:746` (dispatch).
- **HST-2** — **Notes entry does not currently exist.** §9 cites `crates/prod/core/host/src/host/effects.rs`. Add a Notes entry citing it (set_context validates declared key / writable / type).
- **HST-3** — add `runner.rs:599-603` (non-invoke-decisions-produce-zero-effects enforcement).
- **HST-4** — Notes: *"Enforced by `BufferingRuntimeInvoker` replace semantics."* Add `buffering_invoker.rs:132`.
- **HST-5** — add `crates/prod/core/host/src/host/coverage.rs:50-78` (`ensure_handler_coverage`).
- **HST-6** — add `runner.rs:721-730` (store-first-then-incoming overlay).
- **HST-7** — Notes distributed across multiple bullets in the section. Fold `buffering_invoker.rs:132` (replace) and `:99` (drain via `std::mem::take`) anchors per §9 HST-7 row.
- **HST-8** — add `runner.rs:556-566` (one `on_event` lifecycle per step).
- **HST-9** — add `runner.rs:542-545` (duplicate `event_id` rejection).

### 2.4 `docs/invariants/08-replay.md` — **citation folding (row-by-row)**

Canonical v1 already. Same verification bar: every `REP-*` row's Notes entry carries at least one filepath-anchored citation, except rows explicitly CLOSED or closed-by-clarification, whose existing Notes are preserved as-is.

Row-by-row audit:

- **REP-1** — Notes comprehensive with anchor tests. Add `crates/kernel/supervisor/src/replay.rs:159-163` and `:165-171`.
- **REP-2** — Notes: *"`rehydrate()` uses only record fields."* Add filepath anchor `crates/kernel/supervisor/src/replay.rs:340` (`rehydrate_event`).
- **REP-3** — Notes describes `FaultRuntimeHandle` discard behavior. Add filepath anchor `crates/kernel/adapter/src/lib.rs`.
- **REP-4** — Notes describes capture/runtime type separation. Add filepath anchors for both families: `crates/kernel/supervisor/src/capture.rs` (`ExternalEventRecord`, `EpisodeInvocationRecord`) and `crates/kernel/adapter/src/lib.rs` (`ExternalEvent`) / `crates/kernel/supervisor/src/lib.rs` (`DecisionLogEntry`).
- **REP-5** — Notes cites `replay_harness::no_wall_clock_usage`. Tighten to full filepath anchor (`crates/kernel/supervisor/tests/replay_harness.rs`).
- **REP-6** — CLOSED BY CLARIFICATION (2025-12-28). **Do not touch.**
- **REP-7** — Notes comprehensive with anchor tests. Add `crates/kernel/supervisor/src/replay.rs:229-255` (`validate_replay_provenance`).
- **REP-8** — Notes comprehensive. Add `crates/kernel/supervisor/src/replay.rs:257-268` (`validate_unique_event_ids`).
- **REP-SCOPE** — Notes narrative paragraph on Scope A. No additional fold needed; already narrative / non-rule shape.
- **SOURCE-TRUST** — Notes cites `source/registry.rs::validate_manifest()`. Citation present; optional tightening to line range if the function's lines are stable.

Lines 110-115 freeze-declaration block listing frozen files: verify each path still resolves; no paths moved post-S2.3 for kernel files.

### 2.5 `docs/system/kernel.md` — **drive-by (1-line tense fix)**

Line 174: "Pre-authorizes S2.1/S2.2/S2.3" → "Pre-authorized S2.1/S2.2/S2.3" (past tense now that all three are discharged).

### 2.6 `docs/INDEX.md` — **side-effect of §5.1 Option (a)**

Conditional: only if §5.1 Option (a) lands. The version-tag reviewer notes that `docs/INDEX.md` catalogues documents by authority level; `supervisor.md` and `adapter.md` currently appear in the FROZEN list. Re-tagging them to CANONICAL v1 requires moving them out of that list.

### 2.7 `docs/system/host-boundary.md` §9 — **archival reframe + drive-by past-tensing**

Per §5.2 Option (b) arbitration: §9's header prose is rewritten to explicitly mark the table as a dated "v1 freeze-point inventory" and point readers to `07-orchestration.md` / `08-replay.md` for live citations. The table body is preserved. §11 (claim-verification pass) is not touched.

**Drive-by past-tensing (bundled into the same commit as the §9 archival reframe; not split):** the document currently refers to Session 3 as pending at several sites. Since the commit containing the archival reframe *is* the landing moment of the Session 3 rewrites, past-tense the following sites in the same commit:

- **line 23** (§0 header prose) — "`adapter.md` are deferred to Session 3 and must reconcile against §9" → past-tense, naming the Session 3 plan artifact / commits as the reconciliation moment.
- **line 72** — "Session 3 rewrite of `supervisor.md`, `adapter.md`, `07-orchestration.md`, or `08-replay.md`. Those are deferred; §9 is the working table for that rewrite." → past-tense; §9 was the working table.
- **line 380** — "This table is the working document for the deferred Session 3 rewrite" → past-tense; frame as the inventory that drove the rewrite.
- **line 388** — "**clarified** — rule holds but prose needs tightening in the Session 3 rewrite…" → past-tense ("…was tightened in the Session 3 rewrite…").
- **line 511** — "canonical v1 reference that the Session 3 rewrite will reconcile" → past-tense.
- **line 517** — supervisor.md line: remove "Re-anchoring the authority line is the subject of Artifact C (v1 freeze declaration), not this doc." The re-tag landed in Session 3 commit 1; replace with a past-tense note pointing at the Session 3 plan.
- **line 518** — adapter.md line: same treatment as 517; re-tag landed in Session 3 commit 2.

All past-tense rewrites are editorial; no `§9` row content, no §11 claims, and no cited line numbers change.

**Doc-wide consistency scan (inherited from commit 1 pattern, Sebastian 2026-04-24).** Before drafting, run `grep -nE "Session 3|Artifact C|deferred|FROZEN|v0|freeze-candidate|freeze" docs/system/host-boundary.md` and enumerate every hit. The seven sites listed above are the known anchors; the grep catches any additional authority-state or forward-reference sites that surfaced since the plan was drafted. Categorize each as (a) past-tense-site (Session 3 forward-reference now realized), (b) historical-reference-to-preserve (legitimately dated mention), or (c) cross-reference to another doc's authority state. host-boundary.md is already `v1 CANONICAL`, so no frontmatter re-tag is in scope; the consistency bar is "no residual forward-reference to Session 3 as pending, to `Artifact C` as the subject of the re-tag, or to supervisor.md / adapter.md as FROZEN." Commit 5 report must include the categorized grep output alongside the seven site rewrites.

---

## 3. Ordering

Recommended sequence, with rationale:

1. **`supervisor.md` rewrite** — first. Largest substantive change; explicitly flagged by the closure audit; unblocks citation-folding because the SUP-2 wording changes slightly. Landing it first means 07-orchestration.md's citation fold reflects a stable target.
2. **`adapter.md` minimal pass** — second. Peer to supervisor.md in the v0-FROZEN frontmatter group; easiest to batch with the supervisor.md review context still fresh.
3. **`07-orchestration.md` citation folding** — third. Depends on supervisor.md being final because SUP-2 wording updates may inform the note phrasing.
4. **`08-replay.md` citation folding** — fourth. Independent of 07; deferred by convention to run after its sibling.
5. **`host-boundary.md §9` archival + past-tensing drive-by** — fifth. After 07/08 absorb the citations, §9 is re-framed as historical and the seven past-tense sites (per §2.7) land in the same commit.
6. **`kernel.md:174` + `INDEX.md` drive-by** — last. Smallest; bundled at the end.

No file ordering constraint is load-bearing aside from 1→3 and 3,4→5.

---

## 4. Commit Structure — Recommendation

**Recommend: one commit per doc, five-to-seven commits total** (exact count depends on §5.1 and §5.2 arbitration). Rationale:

- Matches Session 2's "one commit per substantive unit" pattern.
- Each commit stands alone in the log; `git blame` against any line points to the commit that made *that* decision, not a rollup.
- The commits have distinct review profiles (rewrite vs. minimal pass vs. citation fold vs. archival reframe vs. drive-by); bundling obscures that.
- Per-commit verification is cheap (`cargo test` not required for doc-only commits).

Resolved commit slate (post-arbitration):

1. supervisor.md rewrite — **atomic**: frontmatter re-tag to `v1 CANONICAL`, `Verified Against Tag` removal, in-body pre-migration note removal (lines 10-15), and §2.3 / §2.4 / §3 / §4 RunResult rewrites all land in one commit. No frontmatter/body splits.
2. adapter.md v1 framing pass — **atomic**: same rule as supervisor.md. Frontmatter re-tag, `Verified Against Tag` removal, in-body pre-migration note removal, and any body tightening land together.
3. 07-orchestration.md citation fold
4. 08-replay.md citation fold
5. host-boundary.md §9 archival reframe **+ bundled past-tensing drive-by** (per §2.7 — lines 23, 72, 380, 388, 511, 517, 518 past-tensed in the same commit; no split).
6. kernel.md tense fix + INDEX.md authority-list update (bundled as one drive-by commit covering the small cross-doc cleanup)

Non-recommendation: grouping by edit-type (rewrites vs. citation folds) loses the per-doc commit-body review surface and makes `§9` reconciliation harder to trace.

---

## 5. Known Arbitration Points

### 5.1 Version-tag doctrine on `supervisor.md` / `adapter.md` — **RESOLVED: Option (a)**

Both were `Version: v0 FROZEN` with a 2026-04-20 "pre-migration tag" note deferring re-tag to Session 3. Options considered:

- **(a) Re-tag to `Version: v1 Authority: CANONICAL`.** Commits both documents to the operational-log change rule. Aligns frontmatter with v1 body. Also removes the obsolete `Verified Against Tag` field and the in-body pre-migration note; moves both files out of INDEX.md's FROZEN list.
- **(b) Keep `Version: v0` with an expanded v1-compat note.**
- **(c) Split frontmatter** with a new `v1 Framing` field.

**Arbitration (Sebastian, 2026-04-23):** Option (a). Decisive point is the inverted-authority problem — `07-orchestration.md` and `08-replay.md` are already `CANONICAL v1` and cite supervisor.md and adapter.md as sources; a `CANONICAL` doc citing a `FROZEN` doc as its source is a taxonomy error. Fix the source tags.

Both the primary plan and the independent version-tag-doctrine reviewer converged on (a). Reviewer's supporting rationale:
- No doc currently uses `Version: v1 Authority: FROZEN`; FROZEN is reserved for ontology/execution/primitives.
- Real change gates are at the symbol level (freeze-v1.md §3, §6), not the doc level.
- Body audit (§1–§4 supervisor, §1–§6 adapter) confirms both pass the CANONICAL bar without body rewrites.

### 5.2 `host-boundary.md §9` disposition post-Session 3 — **RESOLVED: Option (b)**

§9 is the working table that Session 3 citation-folds from. Options considered:

- **(a) Fold away.** Delete §9; citations now live in the invariant docs directly.
- **(b) Archive as historical note.** Keep §9 but reframe as "v1 freeze-point inventory at 2026-04-20 re-anchor."
- **(c) Leave as dual-source.** Canonical citations in both places.

**Arbitration (Sebastian, 2026-04-23):** Option (b). Matches the established repo pattern (freeze-v1.md §4 and host-boundary.md §10 both archive as historical rather than delete); preserves the drift-detection anchor; avoids the second-source-of-truth risk that (c) carries.

Both the primary plan and the §9-disposition reviewer converged on (b). Reviewer's supporting rationale:
- (a) loses the "source inventory at the freeze point" value without explicit archival framing.
- (c) violates AGENTS.md §3 hardening posture ("do not invent a second source of truth"); guaranteed to drift.
- §11 (claim-verification pass) stays untouched — separate verification role, not part of the §9 working-table.

### 5.3 Re-anchor trigger after Session 3 — **RESOLVED: no re-anchor**

Session 3 lands new prose and citation folding. Does `host-boundary.md §0` / `freeze-v1.md §0` need another anchor refresh?

- No §3 symbols change. No §6 invocation.
- The anchor hash cites the architectural high-water mark, not the last-edit SHA. By that doctrine, the anchor stays at `dbd376f` unless a §3 symbol moves.
- Session 3 does update `Last Updated` dates in the frontmatter of the four invariant/orchestration docs.

**Arbitration (Sebastian, 2026-04-23):** no re-anchor. Session 3 does not cross the §3-symbol threshold; anchor stays at `dbd376f`. The anchor-hash commit (`fdb42ca`) remains the post-rebase sync point.

### 5.4 Subagent dispatch (complete)

Three parallel reviewers ran per the Session-2 pattern:

1. **Scope-completeness reviewer.** CONFIRMED — five-file list is complete; no scope expansion needed. See §1 above for the folded verdict.
2. **Version-tag doctrine reviewer.** Recommends Option (a). See §5.1.
3. **§9 disposition reviewer.** Recommends Option (b). See §5.2.

All three reports returned clean; no new findings change scope. Awaiting your arbitration on §5.1 and §5.2 before any doc edits begin.

---

## 6. Verification Approach

Per-doc "correct" criterion:

- **supervisor.md:** residual `RunResult` mentions are permitted **only** when they establish the adapter-private-type privacy boundary — i.e., the type is named to state that it is private to `ergo-adapter`, does not cross the Supervisor seam, is not Supervisor-observable, or is the subject of a historical framing-update record. No residual mention frames `RunResult` as public, adapter-boundary-crossing, Supervisor-observable, or as a type the Supervisor receives/inspects. (Tightened post-commit-1 per Sebastian's Flag 1 arbitration, 2026-04-24; supersedes the prior literal "0 hits" bar, which conflicted with the plan's own §2.1 rewrite wording.) Every rewritten section reads consistently with `host-boundary.md §3` and `freeze-v1.md §3.1 / §4.1`. The `ReportingRuntimeHandle` wording caution (per §2.1) holds: effect observation phrased as flowing *through the `run_reporting(...)` seam*, not as type-level constructability.
- **adapter.md:** every sentence cross-references a `host-boundary.md §§3-4` claim or a `freeze-v1.md §3` symbol without introducing adapter-owned-host-behavior framing. Doc-wide consistency scan (per §2.2 pattern) clean: H1 title, preamble v0 references, any `§N Freeze Status`-equivalent section, and Revision History all reconciled to `v1 CANONICAL`; `§N Signatures`-equivalent blocks preserved as dated historical artifacts.
- **07-orchestration.md / 08-replay.md:** every `SUP-*`/`HST-*`/`CXT-*`/`RTHANDLE-*`/`REP-*` row's Notes entry carries at least one filepath-anchored source citation, **except** rows explicitly marked CLOSED or closed-by-clarification (`RTHANDLE-ERRKIND-1`, `REP-6`), whose existing Notes content is preserved as-is. Cited line numbers resolve against HEAD.
- **host-boundary.md:** no residual "deferred to Session 3" / "Session 3 rewrite will" / "subject of Artifact C" prose at the seven sites listed in §2.7. `§9` header prose explicitly frames the table as a dated v1 freeze-point inventory. `§11` untouched.
- **kernel.md:** grep confirms no "Pre-authorizes" residual.

Tooling:

- `git grep` verification at each commit boundary.
- Spot-check 5 random cited line numbers per citation-folded doc against `sed -n`.
- `cargo test` is not required (docs only) but run anyway at the end of the session for belt-and-suspenders.

---

## 7. Workflow Discipline

- Arbitration on §5.1 / §5.2 / §5.3 returned (Sebastian, 2026-04-23); subagents dispatched and folded.
- Per-commit review chain: draft & commit locally on `main` → diff + message reviewed by Codex for code-reality verification (cited line numbers resolve, prose matches code, adversarial read) → any fixes amended on the local commit → Sebastian arbitrates → push directly to `main` after approval. Each commit is a discrete review unit with its own SHA; commits are not batched at push.
- `git reset --hard HEAD~1` is the recovery path for commits that need structural rework before re-drafting.
- Frontmatter and body changes in the same doc land in the **same commit** for commits 1 and 2 (supervisor.md, adapter.md). No frontmatter/body splits.
- No §6 invocation (pure doc work; no §3 symbol changes).
- Plan-artifact updates folded back into this file if subagent findings change scope.

---

## 8. Status

- [x] Full-tree sweep complete (6 patterns × 97 docs)
- [x] Per-doc scope draft
- [x] Commit-structure recommendation
- [x] Arbitration points enumerated
- [x] Subagent dispatch (scope completeness, version-tag doctrine, §9 disposition — all three returned clean)
- [x] Subagent findings folded into the plan (§1, §5.1, §5.2)
- [x] Sebastian arbitration on §5.1 (Option a), §5.2 (Option b), §5.3 (no re-anchor)
- [ ] Per-doc edits (6 commits per §4)
