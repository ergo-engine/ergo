---
Authority: PROJECT
Date: 2026-05-11
Decision-Owner: Sebastian
Participants: Claude, Codex, Auggie
Status: DECIDED
Scope: First crates.io publish set, package naming, versioning, and publish gate
Resolves: PUB-3 from docs/plans/crates-io-publish.md
---

# Decision: crates.io Publish Set

## Context

Publishing is the first action that turns internal `pub` symbols into
external semver commitments. The first publish is valuable for two
reasons:

- Sebastian wants Ergo usable as a versioned crates.io dependency in his
  own application crate (`my-new-app`) without requiring `--sdk-path`.
- The codebase benefits from publish-grade discipline as a forcing
  function on public-surface coherence.

There is no other known external user. The publish surface is still
audited as if it could become external, but the audit user for PUB-1 is
Sebastian as the concrete consumer.

## Inputs

This record consumes Q-SURFACE, Q-USER, the publish-plan questions that
this record resolves, and the PUB-1 plan:

- Q-SURFACE:
  [`sdk-error-surface-wrapping.md`](sdk-error-surface-wrapping.md)
  decides thick SDK wrapping, collapsed `Ergo*` error categories,
  opaque source-chain preservation through `ErgoErrorSource`, and applies
  that rule to lower-crate diagnostic error sources.
- Q-USER:
  [`crates-io-publish.md`](../../plans/crates-io-publish.md)
  names Sebastian as the concrete audit user and calibrates PUB-1 to
  publish-grade rigor without committing to publish before the final
  go/no-go gate.
- Q-NAMING:
  [`crates-io-publish.md`](../../plans/crates-io-publish.md)
  posed the SDK package naming question. This record resolves it as
  `ergo-sdk`.
- Q-VERSION:
  [`crates-io-publish.md`](../../plans/crates-io-publish.md)
  posed `0.1.0` vs `0.1.0-alpha.1`; this record resolves it as
  `0.1.0-alpha.1`. The version posture is consistent with
  [`freeze-v1.md` §5](../../system/freeze-v1.md#5-explicit-non-scope),
  which explicitly excludes SDK composition from the v1 architecture
  freeze.
- PUB-1 classification:
  [`sdk-error-surface-wrapping-pub1.md`](../../plans/sdk-error-surface-wrapping-pub1.md)
  classifies all 39 audited `ergo-host` re-exports and adjacent SDK
  transparent surfaces.
- Loader pre-loaded inventory:
  [`in-memory-loader-decision-rationale.md`](../../plans/in-memory-loader-decision-rationale.md)
  §0A, §0A.1, §0B, and §0D carry forward the March loader
  public-surface inventory, including loader public names, in-memory
  carrier types, `PreparedGraphAssets`, and the asset-loading helpers.

## Ruling

The first crates.io publish targets ten crates, uses the `ergo-sdk`
package name for the Rust SDK, and publishes all ten crates at
`0.1.0-alpha.1`.

The publish does not happen until PUB-1, PUB-2, PUB-4, PUB-5, and PUB-6
have landed and a final go/no-go decision explicitly says to execute
PUB-7.

## Publish Set

| Publish order | Published crate | Source crate today | Rationale |
|---:|---|---|---|
| 1 | `ergo-runtime` | `crates/kernel/runtime` | Kernel runtime semantics and public runtime/catalog types used by host and SDK. |
| 2 | `ergo-prod-duration` | `crates/prod/shared/duration` | Shared duration parsing used by loader and host. |
| 3 | `ergo-adapter` | `crates/kernel/adapter` | Adapter contract, fixture types, and host/SDK boundary inputs. |
| 4 | `ergo-supervisor` | `crates/kernel/supervisor` | Supervisor/capture/replay support required by host. |
| 5 | `ergo-loader` | `crates/prod/core/loader` | Project, graph, and cluster loading used directly by SDK and host; depends on `ergo-prod-duration`. |
| 6 | `ergo-host` | `crates/prod/core/host` | Canonical orchestration layer used by SDK and CLI. |
| 7 | `ergo-sdk-types` | `crates/prod/clients/sdk-types` | Standalone SDK-adjacent type crate. |
| 8 | `ergo-fixtures` | `crates/shared/fixtures` | Non-optional dependency of `ergo-cli` fixture commands. |
| 9 | `ergo-sdk` | `crates/prod/clients/sdk-rust` | Primary user-facing Rust SDK. Package rename from `ergo-sdk-rust`. |
| 10 | `ergo-cli` | `crates/prod/clients/cli` | User-installable `ergo` binary. |

`ergo-test-support` is not published and should be marked
`publish = false`.

## Naming

The SDK package name is `ergo-sdk`.

`ergo-sdk-rust` is rejected as residue from the in-repo placeholder
period. The `-rust` suffix is redundant once the package is published to
Cargo. The bare `ergo` package name is intentionally left available for
a possible future umbrella crate.

The CLI binary remains `ergo`; it is already declared through `[[bin]]`
in `ergo-cli`.

No package rename is decided here for the other nine published crates.

## Versioning

All ten published crates use `0.1.0-alpha.1` for the first publish.
The version is workspace-uniform.

Rationale:

- `freeze-v1.md` freezes the host-boundary architecture, not SDK type
  composition.
- `kernel-prod-separation.md` freezes SDK behavior as delegation to host
  (`SDK-CANON-*`), not the SDK's exported error/type surface.
- PUB-1 has not yet executed the SDK type-surface refactor.
- No prior decision claims SDK type stability.

The alpha tag is therefore honest: users can try the SDK as a versioned
dependency, but the public type surface remains in active hardening.

## Host-Crate Semver Propagation

Under thick-wrapper Q-SURFACE, `ergo-host`, `ergo-loader`,
`ergo-runtime`, and the other non-SDK published crates are publicly
published crates with their own semver. They are not unpublished
internals and should not be treated as "no independent public contract."

They also are not the primary user-facing product surface. The
Sebastian-as-user PUB-1 filter applies to `ergo-sdk`, because that is
the crate an Ergo application author is expected to depend on first.
The other nine crates' public surfaces are governed by their internal
role: kernel semantics, loader transport/discovery, host orchestration,
CLI support, fixtures, or shared support.

Semver rule:

- A breaking change in a non-SDK crate requires the appropriate semver
  bump for that crate.
- `ergo-sdk` requires a semver bump when its own public API changes,
  when its dependency constraints require users to resolve a different
  incompatible lower crate, or when its public surface exposes a non-SDK
  type whose shape changes. This includes method signatures, struct
  fields, enum variants, and transparent root re-exports classified as
  intentional SDK authoring/config/outcome surface.
- The SDK should avoid root re-exporting lower-layer taxonomies unless
  PUB-1 classifies them as intentional SDK authoring/config/outcome
  surface.

This posture preserves honest semver for every published crate without
turning every lower-layer public symbol into SDK vocabulary.

### Reachable Lower-Crate Public Error Enum Stability

The typed parent-accessor model is superseded before first publish. The
SDK no longer makes lower-crate error enums part of its public method
signatures through helpers such as host-run or hosted-step accessors.
Lower-crate diagnostic detail is instead reachable through
`std::error::Error::source`, `ErgoErrorSource::as_dyn_error()`, direct SDK
reexports classified as authoring/configuration surface, or direct SDK
exception fields.

The old accessor-reachable host enum gate is therefore replaced, not
deleted. Before publishing, PUB-1/PUB-6 must inventory public lower-crate
error enums reachable from the SDK by any of these routes:

- source-chain traversal from `Ergo*` errors;
- `ErgoErrorSource::as_dyn_error()` downcasting;
- direct SDK root reexports classified as intentional SDK surface;
- direct SDK exception fields such as `EgressConfigError` or
  `EgressConfigParseError`.

Required posture:

- The inventory covers host, adapter, supervisor, loader, and runtime error
  enums, including nested public enums matchable after downcasting a parent
  source.
- Extensible public lower-crate error enums are marked `#[non_exhaustive]`
  before first publish.
- Any enum deliberately left exhaustive is recorded as frozen/exhaustive for
  the first published contract, with a reason.
- Variant additions to lower-crate enums are lower-crate semver events
  unless the SDK directly names, reexports, or documents that enum as SDK
  API. SDK semver still changes if SDK-owned categories, source-chain
  behavior, direct exception fields, or dependency constraints change.

PUB-6 dry-runs are not sufficient evidence by themselves. The final
go/no-go must confirm that PUB-1 recorded and resolved the reachable
lower-crate public error enum stability inventory.

## Publish Gate

PUB-7 cannot execute until all gates below are satisfied:

1. PUB-1 SDK surface refactor lands against
   `sdk-error-surface-wrapping-pub1.md`.
2. PUB-2 mechanical publish blockers land for every published crate.
3. PUB-4 rustdoc lands for every kept SDK public item from PUB-1.
4. PUB-5 user-facing READMEs land for the published crates.
5. PUB-6 dry-runs complete cleanly in dependency order.
6. Final go/no-go looks at the classified and implemented surface.

The final go/no-go requires explicit evidence that:

- PUB-1's implemented SDK surface matches its classification table;
- PUB-1's reachable lower-crate public error enum stability inventory was
  completed, and required `#[non_exhaustive]` annotations or documented
  frozen/exhaustive exceptions landed;
- PUB-2's manifest/license/path-dependency blockers are complete;
- PUB-4/PUB-5 documentation reflects the post-PUB-1 surface; and
- PUB-6 produced clean dry-runs for all ten crates in the order below.

If the final gate is no-go, stop at PUB-6. The audit and dry-runs still
produce the publish spine, but nothing is pushed to crates.io.

If the final gate is go, PUB-7 publishes in this order:

1. `ergo-runtime`
2. `ergo-prod-duration`
3. `ergo-adapter`
4. `ergo-supervisor`
5. `ergo-loader`
6. `ergo-host`
7. `ergo-sdk-types`
8. `ergo-fixtures`
9. `ergo-sdk`
10. `ergo-cli`

## Follow-Ups

### Graph Builder Future Surface

The programmatic graph builder remains named but unsolved. Once
`0.1.0-alpha.1` ships, adding a builder is additive at semver level, but
the surface decision recurs when the work lands: whether the builder is
an `ergo-sdk` module, a sibling crate such as `ergo-graph-builder`, or
loader-side API, and whether its public surface follows the same
Q-SURFACE philosophy.

### Documentation Source Split

`/docs/` is internal authoring documentation. crates.io users see
READMEs and rustdoc. PUB-4/PUB-5 decide whether any `/docs/` content is
mirrored into rustdoc or crate READMEs, such as a condensed SDK getting
started guide.

### `ergo init` SDK Path Injection

After publish, the scaffold should default to a versioned crates.io
dependency on `ergo-sdk`. `--sdk-path` becomes opt-in for repo-internal
or local checkout development only.

### Publish Execution Hygiene

The first real publish should run from a tagged commit after PUB-6 is
clean and the final go/no-go is affirmative.

## Non-Goals

This decision does not:

- edit any `Cargo.toml`;
- rename the package in code;
- bump versions from the current workspace state;
- execute the PUB-1 SDK error refactor;
- publish anything to crates.io.
