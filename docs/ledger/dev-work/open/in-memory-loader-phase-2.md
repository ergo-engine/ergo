---
Authority: PROJECT
Date: 2026-03-23
Author: Sebastian (Architect) + Codex
Status: CLOSED
Branch: TBD
Tier: 2/3 (Transport Follow-on / Product-Lane Expansion)
Depends-On: >-
  Closed delivery record:
  docs/ledger/dev-work/closed/in-memory-loader-phase-1.md
  Open defer record:
  docs/ledger/gap-work/open/in-memory-loader-adjacent-defers.md
---

# In-memory Loader Phase 2

## Scope

Carry forward every adjacent in-memory lane that Phase 1 explicitly deferred.

This ledger is intentionally broader than a single code slice. It is the
inventory and closure record for the next phase of work after the delivered
loader + lower-level host-prep boundary:

- product-facing in-memory surfaces that remain path-backed today
- separate host lanes that were intentionally left out of live prep
- remaining path-shaped output and reporting contracts

This phase is now complete: every row below is either implemented in the
current codebase or explicitly re-deferred into linked narrower gap work.

Reconciled against the current codebase on 2026-03-26 via three independent
read-only model audits. Status below reflects the rows' actual closure
conditions as implemented today, not only the original planning assumptions.

## Priority Groups

The open work is prioritized into four groups by dependency chain and delivery
value.

### Group A: No files on disk

- `IMT2-7` adapter object/string transport into live prep
- `IMT2-8` object-based live execution APIs

This was the immediate next work target after Phase 1. In the current workspace
state, Group A is the delivered lower-level host base for the remaining Phase 2
rows: lower-level live prep now accepts truthful non-path adapter transport, and
the host now exposes an additive object-based one-shot run API over
`PreparedGraphAssets` with explicit capture policy.

### Group B: SDK product surface

- `IMT2-1` in-memory project/profile product surface
- `IMT2-2` SDK in-memory APIs / wrappers

`IMT2-1` is the prerequisite because a project/profile model must exist before
the SDK can wrap it. Group B builds on Group A's lower-level host surface.

### Group C: Host lane parity

- `IMT2-4` replay in-memory preparation
- `IMT2-5` DOT/render in-memory preparation
- `IMT2-6` manifest/composition in-memory preparation

These are adjacent host lanes. They do not depend on each other and may be
delivered independently once their individual contracts are designed.

### Group D: Edges and CLI

- `IMT2-3` CLI in-memory execution/validation/manual-runner surfaces
- `IMT2-9` render default output naming
- `IMT2-10` demo-fixture capture naming
- `IMT2-11` fixture-path ingress for live execution
- `IMT2-12` fixture inspect/validate reporting

These are lower-priority edge or tooling surfaces. Several are good candidates
for explicit re-deferral if the earlier groups land first.

## Inputs

- [/Users/sebastian/Projects/ergo/docs/ledger/dev-work/closed/in-memory-loader-phase-1.md](/Users/sebastian/Projects/ergo/docs/ledger/dev-work/closed/in-memory-loader-phase-1.md)
- [/Users/sebastian/Projects/ergo/docs/ledger/gap-work/open/in-memory-loader-adjacent-defers.md](/Users/sebastian/Projects/ergo/docs/ledger/gap-work/open/in-memory-loader-adjacent-defers.md)
- supporting design-loop history under [/Users/sebastian/Projects/ergo/docs/plans](/Users/sebastian/Projects/ergo/docs/plans)

## Work Ledger

| Group | ID | Lane | Why still open after Phase 1 | Closure condition | Owner | Status |
|-------|----|------|------------------------------|-------------------|-------|--------|
| A | IMT2-7 | Adapter object/string transport into live prep | `LivePrepOptions` still carried `adapter_path`, so in-memory graph assets still depended on a path-backed adapter ingress | Host lower-level prep accepts a truthful non-path adapter input contract or an explicitly chosen object/string adapter transport surface | Sebastian + Codex | CLOSED |
| A | IMT2-8 | Object-based live execution APIs | Phase 1 added prep-only object seams, not a truthful one-shot object-based run surface | A later object-based run API defines its own capture/output policy, lifecycle/error truth, and no fake-path bridge into canonical path execution | Sebastian + Codex | CLOSED |
| B | IMT2-1 | In-memory project/profile product surface | SDK/host now implement an in-memory project/profile surface and the canonical project docs describe the two truthful project sources | A truthful in-memory project/profile model is specified and implemented across loader, SDK, and host output/capture semantics | Sebastian + Codex | CLOSED |
| B | IMT2-2 | SDK in-memory APIs / wrappers | SDK in-memory APIs now exist in code and are explicitly documented as the canonical SDK in-memory lane instead of an unresolved advanced side path | Any SDK in-memory surface is explicitly designed, documented as canonical or advanced, and implemented without inventing a second orchestration model | Sebastian + Codex | CLOSED |
| C | IMT2-4 | Replay in-memory preparation | Replay now has a dedicated assets-based prep path and bundle replay surface | Replay gets its own asset-loading and prep design without piggybacking on the live-prep seam or weakening replay error/lifecycle guarantees | Sebastian + Codex | CLOSED |
| C | IMT2-5 | DOT/render in-memory preparation | DOT now has an explicit assets-based host seam and source-label diagnostics; CLI render remains path-only by deliberate output-policy scope | DOT/render receives an explicit in-memory loading/prep contract, diagnostics story, and output policy instead of relying on the live-prep seam by implication | Sebastian + Codex | CLOSED |
| C | IMT2-6 | Manifest/composition in-memory preparation | Manifest/composition now exposes labeled text/value host seams instead of only path-backed helpers | Manifest/composition gets a dedicated in-memory contract and compatibility plan without collapsing it into unrelated graph prep surfaces | Sebastian + Codex | CLOSED |
| D | IMT2-3 | CLI in-memory execution/validation/manual-runner surfaces | CLI commands remain path-backed, but this phase now closes the row by explicitly re-deferring fileless CLI UX into linked gap work | CLI gains explicit UX and request contracts for in-memory execution/validation/manual-runner flows, or this lane is explicitly re-deferred into narrower child work | Sebastian + Codex | CLOSED |
| D | IMT2-9 | Render default output naming | Render remains path-only; no in-memory render lane exists today whose default naming contract still needs resolution | Any in-memory render lane gets an explicit naming/output contract instead of path-derived implicit naming | Sebastian + Codex | CLOSED |
| D | IMT2-10 | Demo-fixture capture naming | Demo-fixture remains path-only; no in-memory demo-fixture lane exists today whose capture naming contract still needs resolution | Any in-memory demo-fixture lane gets an explicit capture naming/output contract instead of a fake file-stem substitute | Sebastian + Codex | CLOSED |
| D | IMT2-11 | Fixture-path ingress for live execution | Live in-memory execution now supports explicit fixture-items ingress distinct from graph transport and file-backed fixture ingress | If live in-memory execution expands to fixture/object ingress, that ingress contract is designed explicitly and stays distinct from graph/cluster transport semantics | Sebastian + Codex | CLOSED |
| D | IMT2-12 | Fixture inspect/validate reporting | Fixture inspect/validate/report surfaces remain explicitly path-backed in public CLI usage and serialized schema | Fixture inspect/validate/report surfaces either gain transport-neutral counterparts or explicitly remain path-backed with matching docs | Sebastian + Codex | CLOSED |

## Evidence Anchors

- `IMT2-1`: SDK in-memory project/profile support is implemented in
  [/Users/sebastian/Projects/ergo/crates/prod/clients/sdk-rust/src/lib.rs](/Users/sebastian/Projects/ergo/crates/prod/clients/sdk-rust/src/lib.rs),
  while filesystem loader project/profile resolution remains loader-owned in
  [/Users/sebastian/Projects/ergo/crates/prod/core/loader/src/project.rs](/Users/sebastian/Projects/ergo/crates/prod/core/loader/src/project.rs)
  and canonical project docs now describe both truthful project sources in
  [/Users/sebastian/Projects/ergo/docs/authoring/project-convention.md](/Users/sebastian/Projects/ergo/docs/authoring/project-convention.md).
- `IMT2-2`: SDK in-memory wrappers now exist in
  [/Users/sebastian/Projects/ergo/crates/prod/clients/sdk-rust/src/lib.rs](/Users/sebastian/Projects/ergo/crates/prod/clients/sdk-rust/src/lib.rs),
  and the public SDK README plus orchestration doctrine now classify them as the
  canonical SDK in-memory lane in
  [/Users/sebastian/Projects/ergo/crates/prod/clients/sdk-rust/README.md](/Users/sebastian/Projects/ergo/crates/prod/clients/sdk-rust/README.md)
  and [/Users/sebastian/Projects/ergo/docs/invariants/07-orchestration.md](/Users/sebastian/Projects/ergo/docs/invariants/07-orchestration.md).
- `IMT2-3`: CLI path-backed execution/reporting is still visible in
  [/Users/sebastian/Projects/ergo/crates/prod/clients/cli/src/graph_yaml.rs](/Users/sebastian/Projects/ergo/crates/prod/clients/cli/src/graph_yaml.rs),
  [/Users/sebastian/Projects/ergo/crates/prod/clients/cli/src/graph_to_dot.rs](/Users/sebastian/Projects/ergo/crates/prod/clients/cli/src/graph_to_dot.rs),
  and [/Users/sebastian/Projects/ergo/crates/prod/clients/cli/src/validate.rs](/Users/sebastian/Projects/ergo/crates/prod/clients/cli/src/validate.rs);
  the explicitly carried-forward defer record is
  [/Users/sebastian/Projects/ergo/docs/ledger/gap-work/open/in-memory-loader-adjacent-defers.md](/Users/sebastian/Projects/ergo/docs/ledger/gap-work/open/in-memory-loader-adjacent-defers.md).
- `IMT2-4`: replay now has its own assets-based prep/request path in
  [/Users/sebastian/Projects/ergo/crates/prod/core/host/src/usecases.rs](/Users/sebastian/Projects/ergo/crates/prod/core/host/src/usecases.rs)
  and [/Users/sebastian/Projects/ergo/crates/prod/core/host/src/usecases/live_prep.rs](/Users/sebastian/Projects/ergo/crates/prod/core/host/src/usecases/live_prep.rs),
  with SDK bundle replay support in
  [/Users/sebastian/Projects/ergo/crates/prod/clients/sdk-rust/src/lib.rs](/Users/sebastian/Projects/ergo/crates/prod/clients/sdk-rust/src/lib.rs).
- `IMT2-5`: DOT/render now has an explicit in-memory host seam in
  [/Users/sebastian/Projects/ergo/crates/prod/core/host/src/graph_dot_usecase.rs](/Users/sebastian/Projects/ergo/crates/prod/core/host/src/graph_dot_usecase.rs)
  with public exports in
  [/Users/sebastian/Projects/ergo/crates/prod/core/host/src/lib.rs](/Users/sebastian/Projects/ergo/crates/prod/core/host/src/lib.rs).
- `IMT2-6`: manifest/composition now has labeled text/value seams in
  [/Users/sebastian/Projects/ergo/crates/prod/core/host/src/manifest_usecases.rs](/Users/sebastian/Projects/ergo/crates/prod/core/host/src/manifest_usecases.rs)
  and public exports in
  [/Users/sebastian/Projects/ergo/crates/prod/core/host/src/lib.rs](/Users/sebastian/Projects/ergo/crates/prod/core/host/src/lib.rs).
- `IMT2-7`: lower-level live prep now accepts truthful non-path adapter transport
  in [/Users/sebastian/Projects/ergo/crates/prod/core/host/src/usecases.rs](/Users/sebastian/Projects/ergo/crates/prod/core/host/src/usecases.rs).
- `IMT2-8`: object-based live execution now lands as additive lower-level
  assets-based run APIs with explicit capture policy in
  [/Users/sebastian/Projects/ergo/crates/prod/core/host/src/usecases.rs](/Users/sebastian/Projects/ergo/crates/prod/core/host/src/usecases.rs),
  while canonical path entrypoints remain distinct in
  [/Users/sebastian/Projects/ergo/crates/prod/core/host/src/lib.rs](/Users/sebastian/Projects/ergo/crates/prod/core/host/src/lib.rs).
- `IMT2-9`: render remains path-only in
  [/Users/sebastian/Projects/ergo/crates/prod/clients/cli/src/render.rs](/Users/sebastian/Projects/ergo/crates/prod/clients/cli/src/render.rs)
  and [/Users/sebastian/Projects/ergo/crates/prod/core/host/src/graph_dot_usecase.rs](/Users/sebastian/Projects/ergo/crates/prod/core/host/src/graph_dot_usecase.rs),
  so there is no in-memory render lane whose default naming contract remains unresolved.
- `IMT2-10`: demo-fixture remains path-only in
  [/Users/sebastian/Projects/ergo/crates/prod/core/host/src/demo_fixture_usecase.rs](/Users/sebastian/Projects/ergo/crates/prod/core/host/src/demo_fixture_usecase.rs)
  and [/Users/sebastian/Projects/ergo/crates/kernel/adapter/src/fixture.rs](/Users/sebastian/Projects/ergo/crates/kernel/adapter/src/fixture.rs),
  so there is no in-memory demo-fixture lane whose capture naming contract remains unresolved.
- `IMT2-11`: explicit fixture-items ingress now exists in
  [/Users/sebastian/Projects/ergo/crates/prod/clients/sdk-rust/src/lib.rs](/Users/sebastian/Projects/ergo/crates/prod/clients/sdk-rust/src/lib.rs)
  and [/Users/sebastian/Projects/ergo/crates/prod/core/host/src/usecases.rs](/Users/sebastian/Projects/ergo/crates/prod/core/host/src/usecases.rs),
  with shared fixture-items execution in
  [/Users/sebastian/Projects/ergo/crates/prod/core/host/src/usecases/live_run.rs](/Users/sebastian/Projects/ergo/crates/prod/core/host/src/usecases/live_run.rs).
- `IMT2-12`: fixture reporting remains explicitly path-backed in
  [/Users/sebastian/Projects/ergo/crates/shared/fixtures/src/report.rs](/Users/sebastian/Projects/ergo/crates/shared/fixtures/src/report.rs)
  and [/Users/sebastian/Projects/ergo/crates/prod/clients/cli/src/fixture_ops.rs](/Users/sebastian/Projects/ergo/crates/prod/clients/cli/src/fixture_ops.rs),
  which satisfies the row's explicit path-backed branch.

## Design Constraints

- Phase 1's shipped boundary remains truthful:
  - loader transport + lower-level host prep are delivered
  - canonical client entrypoints remain path-backed
  - replay, DOT, manifest/composition, and fixture/reporting lanes are separate
- Group A is the immediate next work target because it is the shortest path to
  a truthful “no files on disk” story for graph, cluster, and adapter transport.
- No row above may close by smuggling fake `mem://` or synthetic path shims
  through path-backed APIs.
- If SDK or CLI surfaces widen, doctrine and boundary/guardrail checks must
  change with them rather than lag behind implementation.
- If this phase splits into child ledgers, this file must link to them and keep
  each row's status accurate.

## Closure Gate

This phase closes because:

1. Every row above is either `CLOSED` here by implementation or explicitly
   delegated to a linked open gap-work record.
2. Public docs and invariants now match the delivered widened surface.
3. The remaining path-backed CLI lane is still explicitly deferred in open
   gap-work records instead of being left ambiguous.
