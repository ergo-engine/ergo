---
Authority: PROJECT
Date: 2026-06-07
Decision-Owner: Sebastian
Participants: Claude, Codex
Status: DECIDED
Scope: `0.1.x` release compatibility for the published Ergo crate stack
Supersedes: docs/plans/pre-publish-review-findings.md §4 undecided policy options
---

# Decision: `0.1.x` Stack Release Policy

## Context

The first alpha publishes the Ergo crates as an interdependent stack. Internal
workspace dependencies use normal Cargo version requirements, so already
published dependents can resolve later compatible-looking `0.1.x` releases of
lower crates.

That makes the `0.1.x` compatibility rule load-bearing. If a lower crate
publishes a breaking `0.1.x` change, already-published higher crates can break
on a fresh build even when their own source did not change.

## Decision

Patch releases in the `0.1.x` line must preserve APIs used by
already-published `0.1.x` dependents.

If a lower crate needs a breaking internal-stack change, publish the whole
affected stack at `0.2.0` instead of releasing a compatible-looking `0.1.x`
change.

Exact internal pins are not the first-alpha policy. They remain a future option
only if the project later chooses strict lockstep stack releases.

## Basis

Cargo's normal compatibility requirements allow already-published crates to
resolve newer compatible versions. For the Ergo stack, that is acceptable only
if compatible-looking patch releases preserve the API expectations of already
published dependents.

The policy keeps the first alpha publish simple while avoiding a hidden promise
that every internal-stack crate can break dependents independently under
`0.1.x`.

## Rule

Before publishing a `0.1.x` follow-up release:

1. Identify all already-published Ergo crates that can resolve the changed
   crate.
2. Preserve the APIs those dependents use, or update and publish the affected
   dependent stack together.
3. If preserving those APIs is not intended, bump the affected stack to `0.2.0`.

## Reversibility

The project can adopt stricter lockstep pins later, but doing so is a separate
release-policy decision. This record chooses compatibility-preserving `0.1.x`
patches for the first public alpha line.
