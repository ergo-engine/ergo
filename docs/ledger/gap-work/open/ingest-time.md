---
Authority: CANONICAL
Date: 2026-03-15
Author: ChatGPT (Build Orchestrator) + Claude (Structural Auditor)
Status: OPEN
Gap-ID: INGEST-TIME-1
Unblocks: future canonical normalization contract beyond Scope A replay
---

# INGEST-TIME-1: Cross-Ingestion Normalization Parity

## Question

What canonical normalization contract, if any, must hold across
different event-ingress paths so that logically equivalent inputs
produce parity beyond self-consistency replay?

## Current Scope

D3 closure is scoped to self-consistency replay (Scope A). Within that
scope, cross-ingestion normalization parity remains explicitly deferred.

## Decision Ledger

<!-- markdownlint-disable MD013 -->
| ID | Gap | Scope | Closure Condition | Owner | Status | Evidence |
| ---- | ---- | ----- | ----------------- | ----- | ------ | -------- |
| INGEST-TIME-1 | Cross-ingestion normalization parity is not guaranteed across all ingress-channel paths | Out of scope for D3 Scope A (self-consistency replay) | Canonical normalization rules are specified and enforced across ingestion modes with invariant-linked tests | ChatGPT + Claude | OPEN | `docs/ledger/gap-work/closed/sup2-alignment.md` |
<!-- markdownlint-restore -->

## Notes

- This remains a deferred non-goal for D3 closure work.
- A future decision must specify whether parity is enforced through host
  normalization, channel contract requirements, capture requirements, or
  another declared boundary.
