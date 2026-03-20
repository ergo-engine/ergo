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

## Why This Matters Concretely

The supervisor has a deterministic clock that advances from each
event's `at` field (`event.at()` → `clock.advance_to()`). That clock
drives rate-window deferral and deferred-work scheduling. So `at` is
not just metadata — it changes whether the supervisor decides Invoke
or Defer.

Today's two ingress paths handle `at` differently:

- **Fixture ingress** does not carry `at`. The host fills in
  `EventTime::default()` (time zero) for every event. The supervisor's
  clock never advances. Rate limiting based on time gaps between events
  has no effect.
- **Process ingress** carries real timestamps in `at`. The supervisor's
  clock advances normally. Rate limiting works as designed.

This means the same logical data fed through fixture vs process
ingress can produce different `Decision` sequences when rate
constraints are active. Strict replay also compares `schedule_at`,
so captures from the two paths will differ in decision records, not
just in event timestamps.

Additionally, fixture auto-generates `event_id` when omitted, and
`event_id` feeds deterministic `intent_id` derivation — another axis
of potential divergence.

Self-consistency replay (Scope A) is unaffected: each ingress path is
internally deterministic and replayable. The gap is only about
cross-ingress parity.

## Notes

- This remains a deferred non-goal for D3 closure work.
- A future decision must specify whether parity is enforced through host
  normalization, channel contract requirements, capture requirements, or
  another declared boundary.
