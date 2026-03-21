---
Authority: PROJECT
Date: 2026-03-21
Author: Codex (Implementation)
Status: CLOSED
Branch: feat/sdk-manual-runner-and-reusable-handle
Tier: 3 (Developer Experience — product API)
Depends-On: >-
  docs/ledger/decisions/host-stop-lifecycle.md
  (same-thread reusable SDK surface and host finalization truth);
  docs/ledger/decisions/egress-routing-config.md,
  docs/ledger/decisions/egress-ack-model.md,
  docs/ledger/decisions/egress-timing-lifecycle.md,
  docs/ledger/decisions/egress-failure-taxonomy.md,
  docs/ledger/decisions/egress-provenance.md,
  docs/ledger/decisions/crash-consistency.md
  (manual-runner validation/finalization must preserve canonical live
  egress and capture truth);
  docs/ledger/decisions/custom-implementation-loading.md
  (reusable SDK handles preserve the existing in-process trust model)
---

# SDK Manual Runner + Reusable Handle

## Scope

Deliver the follow-on SDK ergonomics work that makes the built `Ergo`
handle reusable on the same thread and exposes a low-level manual
stepping surface without creating a second execution model.

This branch includes:

- reusable `Ergo` operations (`&self` rather than one-shot ownership)
- a host-owned validation seam for SDK `validate_project()`
- a host-owned manual-runner preparation/finalization seam
- SDK `runner_for_profile(...)` + `ProfileRunner`
- doctrine and boundary-guard updates so the new seams remain canonical

This branch does not change kernel semantics, replay schema, or ingress
model shape.

## Current State

Before this work:

- `Ergo` was documented and implemented as a one-shot handle
- SDK project validation duplicated live host validation logic
- there was no host-owned public path to prepare a ready-to-step
  `HostedRunner` from project/profile assets
- SDK users who wanted step streaming had to reconstruct host setup
  manually or depend on host internals directly
- live doctrine described host ownership too narrowly (run/replay only)

After this work:

- one built `Ergo` handle may be reused for run, replay, validation,
  and manual stepping on the same thread
- SDK validation delegates to canonical host validation
- host exports canonical validation, manual-runner preparation, and
  manual-runner finalization seams
- SDK exposes `runner_for_profile(...)`, `ProfileRunner::finish()`, and
  `ProfileRunner::finish_and_write_capture()`
- live doctrine reflects host-owned validation/manual-runner/finalizer
  semantics and the actual capture finalization order

## Implemented Shape

1. `Ergo` now stores `RuntimeSurfaces` and all public execution methods
   take `&self`.
2. Host live setup is split into validation and construction phases so
   SDK validation can reuse canonical host checks without constructing a
   `HostedRunner` or starting egress.
3. Host exports `validate_graph_from_paths*`,
   `prepare_hosted_runner_from_paths*`, and
   `finalize_hosted_runner_capture(...)` as the canonical client-facing
   seams for live validation/manual stepping.
4. `runner_for_profile(...)` resolves the project profile in the SDK,
   translates it into a host request, and delegates to host-owned
   runner preparation.
5. `ProfileRunner` owns only lifecycle bookkeeping; all execution and
   finalization semantics still flow through `HostedRunner` and host
   finalization.
6. Host finalization now enforces zero-step/non-finalizable rejection,
   pending-ack checks, and egress shutdown before bundle production for
   both canonical runs and manual runners.
7. SDK validation/request-prep duplication was removed, and the
   remaining public runner error name is preserved as a type alias over
   the canonical run error surface.
8. Boundary guards and live doctrine were updated so SDK orchestration
   remains a thin adapter over host-owned setup/finalization.

## Closure Ledger

| ID | Task | Closure Condition | Owner | Status |
|----|------|-------------------|-------|--------|
| SMR-1 | Make `Ergo` reusable | The built SDK handle stores reusable runtime surfaces and `run`, `run_profile`, `replay`, `replay_profile`, and `validate_project` operate on `&self` without changing kernel trait bounds or cross-thread posture. | Codex | CLOSED |
| SMR-2 | Add host-owned validation seam | Host exposes canonical live validation entrypoints that reuse host graph/adapter/egress validation without constructing a hosted session or starting egress channels. | Codex | CLOSED |
| SMR-3 | Remove SDK-owned validation duplication | SDK `validate_project()` delegates to host validation and no longer owns duplicated graph expansion, adapter validation, or egress validation logic. | Codex | CLOSED |
| SMR-4 | Add host-owned manual runner seam | Host exposes canonical runner-preparation and runner-finalization entrypoints for manual stepping, preserving adapter-required preflight, eager egress startup, and host finalization order. | Codex | CLOSED |
| SMR-5 | Expose SDK manual runner | SDK exposes `runner_for_profile(...)` and `ProfileRunner` as a low-level manual stepping surface over resolved profile assets without inventing a second execution model. | Codex | CLOSED |
| SMR-6 | Preserve manual-runner lifecycle truth | Manual stepping rejects zero-step finalization, rejects non-finalizable runner states, allows finalization after dispatch failure, and keeps capture/file-write behavior explicit. | Codex | CLOSED |
| SMR-7 | Tighten boundary guard | `tools/verify_layer_boundaries.sh` blocks SDK-owned canonical run/validation orchestration drift in production code. | Codex | CLOSED |
| SMR-8 | Update live doctrine | Live invariant/system/decision docs reflect host-owned validation/manual-runner/finalization seams and the current capture finalization ordering. | Codex | CLOSED |
| SMR-9 | Prove end-to-end stability | Host and SDK tests cover reusable handles, manual runner setup/finalization, validation delegation, write-failure bundle recovery, and the relevant focused verification commands pass. | Codex | CLOSED |

## Design Constraints

- No kernel API or semantic changes.
- No replay-schema changes.
- No new ingress model; manual stepping still resolves ordinary run
  profiles and ignores ingress operationally.
- Same-thread reuse is supported; cross-thread sharing remains out of
  scope.
- Manual-runner setup, validation, and finalization remain host-owned
  so SDK ergonomics do not become a second orchestration authority.

## What This Branch Enables

After this branch lands:

1. Embedded Rust applications can hold one SDK engine handle and reuse
   it across requests without rebuilding runtime surfaces.
2. SDK callers can stream step-level outcomes from canonical host logic
   through `ProfileRunner` instead of reconstructing host setup
   themselves.
3. SDK project validation stays aligned with host live-path validation
   because both now share the same canonical host seam.
4. Doctrine and boundary guards now describe and defend the actual
   product boundary instead of the pre-manual-runner shape.
