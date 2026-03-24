---
Authority: PROJECT
Date: 2026-03-22
Author: Sebastian (Architect) + Codex
Status: Active
---

# Plans Convention

`docs/plans/` is the sanctioned home for **working design-loop documents**.

Use this directory when a piece of work still needs an iterative artifact that
holds one or more of the following in a single place:

- blast-radius mapping
- option analysis
- scope shaping
- phased execution planning
- rejected paths and pressure records

This directory exists because some architecture work does not move cleanly
through the ledger lanes while it is still being actively shaped.

## What belongs here

- active architecture/design notes that are not yet final authority
- scope plans that are still being refined before or during implementation
  while the design loop remains active
- broad audit artifacts that would become awkward if split too early across gap,
  decision, and dev-work files

## What does not belong here

- final doctrine authority
- closed delivery evidence
- unresolved gap tracking once the work is clearly a ledger-managed blocker

Those belong in the ledger:

- `docs/ledger/gap-work/` for unresolved contradictions / doctrine blockers
- `docs/ledger/decisions/` for final rulings
- `docs/ledger/dev-work/` for executable implementation scopes with closure rows

## Relationship to the ledger

`docs/plans/` is **pre-ledger or cross-ledger working space**, not a competing
authority system.

The expected lifecycle is:

1. use `docs/plans/` while the design loop is still active
2. graduate final rulings into `docs/ledger/decisions/`
3. graduate executable implementation scope into `docs/ledger/dev-work/`
4. open or update `docs/ledger/gap-work/` if unresolved semantic blockers remain

## Rule

If a `docs/plans/` document starts acting like the source of truth for
implemented behavior, it should either:

- be distilled into ledger records, or
- be explicitly linked from those ledger records as supporting analysis

`docs/plans/` is for clean iteration. The ledger remains the place where work is
closed and authority is recorded.
