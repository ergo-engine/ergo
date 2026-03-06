---
Authority: PROJECT
Date: 2026-03-05
Author: Sebastian (Architect) + Codex (Implementation)
Status: DECISION_PENDING
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

## Decision Ledger

| ID | Task | Closure Condition | Owner | Status |
|----|------|-------------------|-------|--------|
| GW-EI8-1 | Select v0 loading mechanism | Decision recorded with rationale, explicit non-goals, and required tests | Sebastian | DECISION_PENDING |
