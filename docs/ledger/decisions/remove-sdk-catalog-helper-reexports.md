---
Authority: PROJECT
Date: 2026-06-07
Decision-Owner: Sebastian
Participants: Claude, Codex
Status: DECIDED
Scope: First-alpha SDK public facade
Supersedes: docs/plans/pre-publish-review-findings.md §8C classification of `build_core`, `build_core_catalog`, and `core_registries`
Implemented-By: e224fac
---

# Decision: Remove SDK Catalog Helper Re-Exports

## Context

The `ergo-sdk` root re-exported three runtime catalog helpers:

- `build_core`
- `build_core_catalog`
- `core_registries`

The SDK doc comment described them as "advanced primitive registration"
helpers. That was too broad. These helpers build the default runtime core;
they are not the SDK facade's custom-registration path.

The functions remain public in `ergo-runtime`.

## Decision

Remove the SDK-root re-export:

```rust
pub use ergo_runtime::catalog::{build_core, build_core_catalog, core_registries};
```

Do not change the functions' definitions or visibility in `ergo-runtime`.

Keep `ExecutionContext` and the five runtime primitive modules (`action`,
`common`, `compute`, `source`, `trigger`) on the SDK root. Those are genuine
custom-primitive authoring surface.

## Basis

The pre-removal audit found no SDK-facade consumer for the catalog helper trio:

- No SDK-internal logic used them through `ergo_sdk`.
- No public SDK signature depends on them.
- No example, test, scaffold template, or workspace crate reached them through
  the SDK root.
- `Ergo` and `ErgoBuilder` do not accept externally built `CoreRegistries` or
  `CorePrimitiveCatalog` values.
- `RuntimeSurfaces` remains private to the SDK implementation.

The gating search returned no matches:

```sh
rg -n 'ergo_sdk[^\n]*(build_core|build_core_catalog|core_registries)|(build_core|build_core_catalog|core_registries)[^\n]*ergo_sdk' . --glob '!target/**'
```

The existing catalog-builder ledger classifies the three helpers as core-only:

- `docs/ledger/dev-work/closed/catalog-builder.md`

The custom-implementation loading decision records the real extension path:
custom implementations enter through `CatalogBuilder`, which is wrapped by the
SDK's `ErgoBuilder::add_*` methods.

- `docs/ledger/decisions/custom-implementation-loading.md`

## Correction Recorded

The SDK-root doc comment called these helpers "advanced primitive
registration." That label conflated default-core construction with custom
implementation registration.

Registration is `CatalogBuilder`'s job. The SDK facade already wraps that
registration path through `ErgoBuilder::add_*`; it does not expose or consume
externally assembled core registries/catalogs.

This record preserves the correction instead of silently dropping the
over-broad comment.

## Kept In Scope

The SDK root keeps:

- `ExecutionContext`
- `action`
- `common`
- `compute`
- `source`
- `trigger`

Those names are needed by users writing custom primitives. Implementations name
`ExecutionContext`, commonly name `common::Value`, and satisfy trait bounds from
the primitive modules used by `ErgoBuilder::add_*`.

## Reversibility

Nothing has been published yet, so removing the SDK re-export breaks no public
crate contract.

If a real SDK-facade catalog assembly use emerges later, re-exporting the
helpers from the SDK root would be additive and non-breaking.

## Supersedes

This supersedes the `pre-publish-review-findings.md` §8C classification that
kept `build_core`, `build_core_catalog`, and `core_registries` as advanced SDK
registration surface.
