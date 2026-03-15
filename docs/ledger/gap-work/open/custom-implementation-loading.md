---
Authority: PROJECT
Date: 2026-03-05
Author: Sebastian (Architect) + Codex (Implementation)
Decision-Owner: Sebastian (Architect)
Status: OPEN
Gap-ID: GW-EI8-1
Unblocks: feat/ergo-init (EI-8)
---

# GW-EI8-1: Custom Implementation Loading Mechanism

## Question

Which v0 mechanism is canonical for workspace custom implementation loading in `ergo-init`?

Candidates:

1. Path-to-crate/in-process build flow
2. Dynamic library loading
3. WASM-based loading

## Impact

This decision gates EI-8 scope, test design, and security/operability constraints.

## Current State

No v0 loading mechanism is selected yet. `feat/ergo-init` may not close
`EI-8` until this gap lands as a decision.

## Decision Ledger

<!-- markdownlint-disable MD013 -->
| ID | Task | Closure Condition | Owner | Status |
| ---- | ---- | ----------------- | ----- | ------ |
| GW-EI8-1 | Select v0 loading mechanism | Decision recorded with rationale, explicit non-goals, and required tests | Sebastian | OPEN |
<!-- markdownlint-restore -->
