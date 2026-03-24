---
Authority: PROJECT
Date: 2026-03-22
Author: Sebastian (Architect) + Codex (Implementation)
Status: CLOSED
Branch: main
Merge-Evidence: >-
  Local verification on 2026-03-23:
  cargo test -p ergo-loader;
  cargo test -p ergo-host --lib;
  cargo test -p ergo-sdk-rust --lib
Tier: 2 (Loader / Host Transport Boundary)
Depends-On: >-
  Active design-loop sources:
  docs/plans/in-memory-loader-blast-radius.md and
  docs/plans/in-memory-loader-decision-rationale.md
---

# In-memory Loader Tranche 1

## Scope

Deliver the first implementation tranche of in-memory graph/cluster transport
support without widening canonical client surfaces or inventing a second
execution model.

This tranche covers:

- truthful in-memory graph/cluster transport in the loader
- additive parallel transport carriers at the loader boundary
- a prep-only host object seam for canonical validation and manual-runner prep
- internal/shared host prep reuse behind existing canonical `*_from_paths`
  run entrypoints
- decode/discovery parity work and the required docs/test updates

This tranche does not include:

- in-memory project/profile product surface
- new SDK or CLI in-memory APIs
- replay lower-level in-memory prep
- DOT/render lower-level in-memory prep
- manifest/composition lower-level in-memory prep
- adapter object/string transport
- new lower-level object-based live execution APIs

## Working Design Sources

- [in-memory-loader-blast-radius.md](/Users/sebastian/Projects/ergo/docs/plans/in-memory-loader-blast-radius.md)
- [in-memory-loader-decision-rationale.md](/Users/sebastian/Projects/ergo/docs/plans/in-memory-loader-decision-rationale.md)

The plan docs remain the design-loop artifacts that shaped the implementation.
This ledger is the execution/closure record.

Deferred adjacent lanes are recorded explicitly in
[/Users/sebastian/Projects/ergo/docs/ledger/gap-work/open/in-memory-loader-adjacent-defers.md](/Users/sebastian/Projects/ergo/docs/ledger/gap-work/open/in-memory-loader-adjacent-defers.md).

## Closure Ledger

| ID | Task | Closure Condition | Owner | Status |
|----|------|-------------------|-------|--------|
| IMT1-0 | Freeze Phase 0 boundary | The naming freeze, split-carrier shape (`PreparedGraphAssets` + `LivePrepOptions`), API delta/no-change list, and Phase 1 start gate are recorded in the tranche plan and reflected in implementation kickoff choices. | Sebastian + Codex | CLOSED |
| IMT1-1 | Fix decode seam truthfully | Loader exposes a truthful label-aware public YAML decode path, and `decode_graph_json(...)` is either implemented honestly or explicitly deferred with code/docs/tests aligned. | Codex | CLOSED |
| IMT1-2 | Preserve discovery parity internals | Resolver/carrier internals preserve deterministic ordering, referrer-sensitive scope, opened-label/search-trace diagnostics, pre-filter conflict recording, and root-reachable semantics for filesystem parity. | Codex | CLOSED |
| IMT1-3 | Add parallel loader carriers | `LoadedGraphBundle` is renamed truthfully, an in-memory peer is added, the filesystem discovery/load helper pair is tightened to parse and return the root internally, and affected public loader surfaces/docs/tests are updated without flattening filesystem-only contracts. | Codex | CLOSED |
| IMT1-4 | Add prep-only host object seam | Host adds the additive lower-level asset/prep seam around loader-owned sealed `PreparedGraphAssets` plus host-owned `LivePrepOptions`, preserves canonical `*_from_paths` entrypoints, and does not create a supported bridge into object-based `run_graph(...)` execution. | Codex | CLOSED |
| IMT1-5 | Preserve host lifecycle and error truth | Validation remains non-session/non-egress-starting, lower-level asset validation stops before runner construction, manual-runner prep preserves adapter-required preflight and eager egress startup ordering, and current public error buckets remain truthful. | Codex | CLOSED |
| IMT1-6 | Update docs and compatibility coverage | Canonical docs, loader docs, and tests cover the renamed loader surface, decode/discovery parity, and the actual affected public surface including `discover_cluster_tree(...)`, `ClusterDiscovery`, and `load_cluster_tree(...)`. | Codex | CLOSED |
| IMT1-7 | Keep defers explicit | Deferred lanes remain explicitly deferred in code/docs/guardrails and are not silently half-implemented during the tranche. | Sebastian + Codex | CLOSED |

## Design Constraints

- Canonical client-facing host path APIs remain the source of truth for product
  run/replay/validation/manual stepping.
- Any new host object seam is additive, lower-level, and prep-only.
- Canonical run preparation support in this tranche may be internal/shared prep
  reused behind existing path entrypoints, not a new public prep API.
- Loader transport carriers remain honest about transport; public path-shaped
  APIs are not flattened into fake-neutral types.
- First-class DX is a destination, not an excuse to erase real transport
  differences at the wrong layer.

## Closure Gate

This work was ready to close once:

1. Every closure row above is `CLOSED`.
2. The delivered code matches the frozen tranche scope in
   [in-memory-loader-decision-rationale.md](/Users/sebastian/Projects/ergo/docs/plans/in-memory-loader-decision-rationale.md).
3. Any remaining semantic blockers are either resolved or moved explicitly into
   gap-work / decision records before merge.
