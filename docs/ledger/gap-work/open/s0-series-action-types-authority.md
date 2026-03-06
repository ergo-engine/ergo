---
Authority: PROJECT
Date: 2026-03-05
Author: Sebastian (Architect) + Codex (Implementation)
Status: DECISION_PENDING
Gap-ID: S-0
Unblocks: feat/series-action-types, feat/series-stdlib
---

# S-0 Decision: Ontology §3 Parenthetical Interpretation

## Question

In `ontology.md` §3, does the parenthetical
`(v0 non-Event payload types: Number/Bool/String)`
mean:

1. **Exhaustive**: only these three payload types are permitted in v0 forever unless freeze amendment.
2. **Descriptive**: these were the existing v0 payload types at freeze time, but additional non-Event payload types may be added without changing the causal rule.

## Impact

This decision gates whether `Series` can be admitted as an Action scalar payload type in `feat/series-action-types`.

## Decision Ledger

| ID | Task | Closure Condition | Owner | Status |
|----|------|-------------------|-------|--------|
| S-0 | Decide parenthetical semantics (exhaustive vs descriptive) | Decision text recorded with rationale and impacted docs list | Sebastian | DECISION_PENDING |
| S-0A | If exhaustive: amend frozen wording via explicit authority path | Approved amendment record exists before code implementation | Sebastian | OPEN |
| S-0B | If descriptive: confirm no frozen amendment required | Written confirmation recorded in this file | Sebastian | OPEN |

## Required Decision Record Fields

- Date
- Decision owner
- Selected option
- Rationale
- Affected docs
- Affected branch ledgers
