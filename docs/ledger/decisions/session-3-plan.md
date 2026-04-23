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

- **§2.3 lines 144-146:** replace the "Supervisor does not receive RunResult" paragraph with v1 framing: *`RuntimeInvoker::run()` returns `RunTermination` only; `RunResult` is private to `ergo-adapter` post-S2.2; effect observation lives off `ReportingRuntimeHandle` in the adapter layer and is consumed only by `BufferingRuntimeInvoker` in `ergo-host`*.
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
- **Frontmatter version-tag and body pre-migration note:** see §5.1. If Option (a) lands, same treatment as supervisor.md — drop the 2026-04-20 note and `Verified Against Tag` field.

### 2.3 `docs/invariants/07-orchestration.md` — **citation folding**

Canonical v1 already. Fold `host-boundary.md §9` source citations into the Notes block for each `SUP-*`/`HST-*`/`RTHANDLE-*` row. Concretely:

- SUP-2 note: add `crates/kernel/adapter/src/lib.rs:182` (private `RunResult`) and the `RuntimeHandle::run` public-seam citation.
- SUP-3 note: add `replay.rs:184` (strict entry).
- SUP-6 note: add `runner.rs:793` (no-rollback comment).
- SUP-7 note: trait-only `log()` citation against `supervisor/src/lib.rs`.
- HST-1 note: add `runner.rs:576` (drain) and `runner.rs:746` (dispatch).
- HST-3 note: add `runner.rs:599-603`.
- HST-4 note: `buffering_invoker.rs:132`.
- HST-5 note: `coverage.rs:50-78`.
- HST-6 note: `runner.rs:721-730`.
- HST-8 note: `runner.rs:556-566`.
- HST-9 note: `runner.rs:542-545`.

### 2.4 `docs/invariants/08-replay.md` — **citation folding**

Canonical v1 already. Fold `host-boundary.md §9` source citations into Notes for each `REP-*` row:

- REP-1: `replay.rs:159-163`, `:165-171`.
- REP-2: `replay.rs:340` (`rehydrate_event`).
- REP-7: `replay.rs:229-255` (`validate_replay_provenance`).
- REP-8: `replay.rs:257-268` (`validate_unique_event_ids`).
- Decision/effect comparison (REP-SCOPE Scope A): `replay.rs:274`, `:284-293`, `:312-329`.

Lines 110-115 freeze-declaration block listing frozen files: verify each path still resolves; no paths moved post-S2.3 for kernel files.

### 2.5 `docs/system/kernel.md` — **drive-by (1-line tense fix)**

Line 174: "Pre-authorizes S2.1/S2.2/S2.3" → "Pre-authorized S2.1/S2.2/S2.3" (past tense now that all three are discharged).

### 2.6 `docs/INDEX.md` — **side-effect of §5.1 Option (a)**

Conditional: only if §5.1 Option (a) lands. The version-tag reviewer notes that `docs/INDEX.md` catalogues documents by authority level; `supervisor.md` and `adapter.md` currently appear in the FROZEN list. Re-tagging them to CANONICAL v1 requires moving them out of that list.

### 2.7 `docs/system/host-boundary.md` §9 — **contingent on §5.2**

Contingent on §5.2 arbitration. If Option (b) (archival framing) is chosen, §9's header prose is rewritten to explicitly mark the table as a dated "v1 freeze-point inventory" and point readers to 07-orchestration.md / 08-replay.md for live citations. The table body is preserved. §11 (claim-verification pass) is not touched.

---

## 3. Ordering

Recommended sequence, with rationale:

1. **`supervisor.md` rewrite** — first. Largest substantive change; explicitly flagged by the closure audit; unblocks citation-folding because the SUP-2 wording changes slightly. Landing it first means 07-orchestration.md's citation fold reflects a stable target.
2. **`adapter.md` minimal pass** — second. Peer to supervisor.md in the v0-FROZEN frontmatter group; easiest to batch with the supervisor.md review context still fresh.
3. **`07-orchestration.md` citation folding** — third. Depends on supervisor.md being final because SUP-2 wording updates may inform the note phrasing.
4. **`08-replay.md` citation folding** — fourth. Independent of 07; deferred by convention to run after its sibling.
5. **`host-boundary.md §9` archival** — fifth (if §5.2 Option b lands). After 07/08 absorb the citations, §9 is re-framed as historical. Conditional on arbitration.
6. **`kernel.md:174` + `INDEX.md` drive-by** — last. Smallest; bundled at the end. INDEX.md touch only if §5.1 Option (a) lands.

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
5. host-boundary.md §9 archival reframe
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

- **supervisor.md:** `git grep -nE "RunResult" docs/orchestration/supervisor.md` → 0 hits. Every rewritten section reads consistently with `host-boundary.md §3` and `freeze-v1.md §3.1 / §4.1`.
- **adapter.md:** every sentence cross-references a `host-boundary.md §§3-4` claim or a `freeze-v1.md §3` symbol without introducing adapter-owned-host-behavior framing.
- **07-orchestration.md / 08-replay.md:** every SUP-*/HST-*/REP-* row in the table has at least one source citation in its Notes entry. Cited line numbers resolve against HEAD.
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
