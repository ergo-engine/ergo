---
Authority: PROJECT
Date: 2026-03-04
Author: Sebastian (Architect) + Claude (Structural Auditor)
Status: Active
---

# Extension Surface — Implementation Order

This file is the execution control board for the six extension-surface branches.

Each branch has a detailed closure ledger in `docs/ledger/dev-work/` (open for active branches, closed for completed branches). This board defines start gates, merge gates, and critical path only.

## Branch Board

| Branch | Detailed Ledger | Start Gate | Merge Gate | Unblocks |
|---|---|---|---|---|
| `feat/series-action-types` | `docs/ledger/dev-work/closed/series-action-types.md` | **Blocked on S-0** in `docs/ledger/gap-work/closed/s0-series-action-types-authority.md` | S-0 resolved + S-rows closed | `feat/series-stdlib` |
| `feat/series-stdlib` | `docs/ledger/dev-work/closed/series-stdlib.md` | `feat/series-action-types` merged | SS-rows closed (including `GW-SS3-1` decision application for SS-3) | Series stdlib availability |
| `feat/catalog-builder` | `docs/ledger/dev-work/open/catalog-builder.md` | none | CB-rows closed | `feat/ergo-init` custom impl loading path |
| `feat/adapter-runtime` | `docs/ledger/dev-work/open/adapter-runtime.md` | none | AR-rows closed | `feat/ingestion-driver`, `feat/ergo-init` runtime adapter contract |
| `feat/ingestion-driver` | `docs/ledger/dev-work/open/ingestion-driver.md` | `feat/adapter-runtime` merged | ID-rows closed | `feat/ergo-init` run-path completion |
| `feat/ergo-init` | `docs/ledger/dev-work/open/ergo-init.md` | `feat/catalog-builder` + `feat/adapter-runtime` merged | EI-rows closed (**EI-5/EI-9 require `feat/ingestion-driver`; EI-8 requires `GW-EI8-1` decision application**) | Extension-surface completion gate |

## Parallel Start Set

Start immediately:

- `feat/catalog-builder`
- `feat/adapter-runtime`

After S-0 decision:

- `feat/series-action-types`

## Critical Path

```text
S-0 decision -> series-action-types -> series-stdlib
adapter-runtime -> ingestion-driver -> ergo-init
catalog-builder ---------------------> ergo-init
```

## Merge Rule

No branch merges until all of its ledger rows are `CLOSED` in the branch's detailed ledger file.

## Audit Rule

Per branch, auditor must verify:

1. Closure conditions are objectively satisfied.
2. No invariant was weakened or bypassed.
3. No frozen document changed without explicit escalation.
4. No domain-specific language leaked into kernel/prod core.
5. Ledger sign-off is recorded before merge.
