---
Authority: PROJECT
Date: 2026-03-15
Author: Sebastian (Architect) + Claude (Structural Auditor)
Status: CLOSED
---

# Extension Surface — Implementation Order

This file is the execution control board for the six
extension-surface branches.

Each branch has a detailed closure ledger in `docs/ledger/dev-work/`
(open for active branches, closed for completed branches). This board
defines start gates, merge gates, and critical path only.

## Branch Board

<!-- markdownlint-disable MD013 -->
| Branch | Detailed Ledger | Start Gate | Merge Gate | Unblocks |
| --- | --- | --- | --- | --- |
| `feat/series-action-types` | `docs/ledger/dev-work/closed/series-action-types.md` | **Blocked on S-0** in `docs/ledger/gap-work/closed/s0-series-action-types-authority.md` | S-0 resolved + S-rows closed | `feat/series-stdlib` |
| `feat/series-stdlib` | `docs/ledger/dev-work/closed/series-stdlib.md` | `feat/series-action-types` merged | SS-rows closed (including `GW-SS3-1` decision application for SS-3) | Series stdlib availability |
| `feat/catalog-builder` | `docs/ledger/dev-work/closed/catalog-builder.md` | none | CB-rows closed | `feat/ergo-init` custom impl loading path |
| `feat/adapter-runtime` | `docs/ledger/dev-work/closed/adapter-ingress-surface.md` | none | AR-rows closed | `feat/ergo-init` adapter ingress + host path |
| `feat/sdk-rust` | `docs/ledger/dev-work/closed/sdk-rust.md` | `feat/catalog-builder` + `feat/adapter-runtime` + `feat/egress-surface` merged | SDK-rows closed | `feat/ergo-init` SDK-first product surface |
| `feat/ergo-init` | `docs/ledger/dev-work/closed/ergo-init.md` | `feat/catalog-builder` + `feat/adapter-runtime` + `feat/egress-surface` + `feat/sdk-rust` merged | EI-rows closed (**EI-8 implements the decided in-process `CatalogBuilder` path**) | Extension-surface completion gate |
<!-- markdownlint-restore -->

## Parallel Start Set

All six extension-surface branches are now closed.

## Critical Path

```text
catalog-builder ----\
adapter-runtime -----> sdk-rust -----> ergo-init
egress-surface ------/
```

## Merge Rule

No branch merged until all of its ledger rows were `CLOSED` in the
branch's detailed ledger file.

## Audit Rule

Per branch, auditor must verify:

1. Closure conditions are objectively satisfied.
2. No invariant was weakened or bypassed.
3. No frozen document changed without explicit escalation.
4. No domain-specific language leaked into kernel/prod core.
5. Ledger sign-off is recorded before merge.
