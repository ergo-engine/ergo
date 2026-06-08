---
Authority: PROJECT
Date: 2026-06-07
Decision-Owner: Sebastian
Participants: Claude, Codex
Status: DECIDED
Scope: First-alpha publish set and workspace membership
Supersedes: docs/plans/pre-publish-review-findings.md §8A accepted-for-alpha reservation disposition
---

# Decision: Remove `ergo-sdk-types`

## Context

`ergo-sdk-types` was a deliberate forward reservation for future
cross-language SDK bindings and cross-client DTOs. It was not dead or
vestigial code.

The crate's current public surface was a single serializable DTO:

- `SdkVersion { value: String }`

No workspace crate consumed that DTO.

## Decision

Remove the `ergo-sdk-types` crate from the workspace and from the first
crates.io publish set.

The first alpha now ships nine crates:

1. `ergo-runtime`
2. `ergo-prod-duration`
3. `ergo-adapter`
4. `ergo-supervisor`
5. `ergo-loader`
6. `ergo-host`
7. `ergo-fixtures`
8. `ergo-sdk`
9. `ergo-cli`

## Basis

The pre-removal gate confirmed the crate was idle:

- `rg -n 'ergo-sdk-types' --glob '**/Cargo.toml'` returned only the
  crate's own manifest: `crates/prod/clients/sdk-types/Cargo.toml`.
- `rg -n 'ergo_sdk_types' crates --glob '!target/**'` returned no
  source imports.

Because nothing in the workspace depends on the crate, removing it
breaks no internal dependency edge and removes no currently used public
contract.

## Framing

This is not rot cleanup. The crate existed to reserve a future
cross-language or cross-client SDK type surface.

The reserved use has not materialized, and publishing an unused public
crate would create a public package to maintain through `0.1.x` without
a real consumer proving the shape. That maintenance burden is not worth
carrying for the first alpha.

## Reversibility

The removal is reversible. Since no crate depends on `ergo-sdk-types`,
the package can be introduced later as an additive public crate when a
real binding or cross-client consumer exists.

## Correction Recorded

This supersedes the `pre-publish-review-findings.md` §8A disposition
that accepted the namespace reservation for alpha. The corrected
disposition is "removed pre-publish": the reservation was intentional,
but the first-alpha publish set should not ship an idle crate.
