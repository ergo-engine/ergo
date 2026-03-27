---
Authority: PROJECT
Date: 2026-03-23
Author: Sebastian (Architect) + Codex
Status: OPEN
Gap-ID: IMT1-DEFER-1
Unblocks: future post-tranche work that widens in-memory transport beyond graph/cluster prep
---

# IMT1-DEFER-1: In-memory Loader Adjacent Deferred Lanes

## Question

Which adjacent in-memory lanes are explicitly **not** delivered by tranche 1,
and what would have to happen before any of them can be closed honestly?

## Decision Ledger

| ID | Deferred lane | Why deferred in tranche 1 | Closure condition | Owner | Status |
|----|---------------|---------------------------|-------------------|-------|--------|
| IMT1-DEFER-1A | In-memory project/profile product surface | Project/profile semantics remained filesystem-backed product behavior in tranche 1 | A separate product model defines truthful in-memory project/profile resolution, capture semantics, and SDK outputs | Sebastian + Codex | CLOSED |
| IMT1-DEFER-1B | SDK in-memory APIs | Canonical SDK surfaces remained path/project-oriented in tranche 1 | SDK doctrine, lower-level allowlist, and public product shape are explicitly decided and implemented | Sebastian + Codex | CLOSED |
| IMT1-DEFER-1C | CLI in-memory surfaces | CLI remains centered on canonical path-backed run/validate/replay/manual-runner entrypoints | CLI UX, flags, and output contracts for in-memory execution are specified and implemented | Sebastian + Codex | OPEN |
| IMT1-DEFER-1D | Replay in-memory prep | Replay remained its own host lane with path/capture-driven setup in tranche 1 | Replay gets its own asset-loading and prep design without piggybacking on live-prep seams | Sebastian + Codex | CLOSED |
| IMT1-DEFER-1E | DOT/render in-memory prep | DOT kept its own path-backed summary/formatting lane in tranche 1 | DOT gets an explicit in-memory loading/prep design and matching diagnostics story | Sebastian + Codex | CLOSED |
| IMT1-DEFER-1F | Manifest/composition in-memory prep | Manifest/composition stayed path-backed and public in tranche 1 | Manifest/composition gets a separate in-memory contract and compatibility plan | Sebastian + Codex | CLOSED |
| IMT1-DEFER-1G | Adapter object/string transport into live prep | Tranche 1 keeps `adapter_path` path-shaped in `LivePrepOptions` | Adapter transport is redesigned explicitly for lower-level host prep | Sebastian + Codex | CLOSED |
| IMT1-DEFER-1H | Object-based live execution APIs | Tranche 1 is prep-only; no new object-based execution seam is added | A later tranche defines truthful run/capture/output policy for object-based execution | Sebastian + Codex | CLOSED |
| IMT1-DEFER-1I | Render default output naming changes | Render still derives default SVG output from `graph_path`, but no in-memory render lane exists today | An explicit naming/output contract exists for any in-memory render lane | Sebastian + Codex | CLOSED |
| IMT1-DEFER-1J | Demo-fixture capture naming changes | Demo-fixture output still defaults from the fixture file stem, but no in-memory demo-fixture lane exists today | An explicit naming/output contract exists for any in-memory demo-fixture lane | Sebastian + Codex | CLOSED |
| IMT1-DEFER-1K | Fixture-path ingress changes for live execution | Fixture ingress remained a file-path transport concern separate from graph/cluster transport in tranche 1 | Live in-memory execution, if added, defines truthful fixture/object ingress semantics without fake path shims | Sebastian + Codex | CLOSED |
| IMT1-DEFER-1L | Fixture inspect/validate reporting changes | Public fixture reporting still serializes and describes `fixture_path`-shaped behavior, and that path-backed scope is now explicit | Fixture inspect/validate/report surfaces either gain transport-neutral counterparts or explicitly retain path-backed scope | Sebastian + Codex | CLOSED |

## Notes

- These lanes are deferred by design, not forgotten work.
- Tranche 1 may harden shared loader/host internals that later support these
  lanes, but it must not silently expose or partially productize them.
- All rows except `IMT1-DEFER-1C` are now closed by delivered Phase 2 work or
  by an explicit path-backed-scope decision tracked in
  [/Users/sebastian/Projects/ergo/docs/ledger/dev-work/open/in-memory-loader-phase-2.md](/Users/sebastian/Projects/ergo/docs/ledger/dev-work/open/in-memory-loader-phase-2.md).
- `IMT1-DEFER-1C` remains the live defer: CLI still intentionally centers the
  path-backed entrypoints while the SDK and host now carry the truthful
  in-memory transport lanes.
- The closed delivery ledger that carried the tranche-2 implementation work is
  [/Users/sebastian/Projects/ergo/docs/ledger/dev-work/open/in-memory-loader-phase-2.md](/Users/sebastian/Projects/ergo/docs/ledger/dev-work/open/in-memory-loader-phase-2.md).
- This gap record is the explicit evidence backing `IMT1-7` in
  [/Users/sebastian/Projects/ergo/docs/ledger/dev-work/closed/in-memory-loader-phase-1.md](/Users/sebastian/Projects/ergo/docs/ledger/dev-work/closed/in-memory-loader-phase-1.md).
