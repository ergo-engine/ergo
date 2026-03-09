---
Authority: PROJECT
Date: 2026-03-05
Author: Sebastian (Architect) + Codex (Implementation)
Status: CLOSED
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
| S-0 | Decide parenthetical semantics (exhaustive vs descriptive) | Decision text recorded with rationale and impacted docs list | Sebastian | CLOSED |
| S-0A | If exhaustive: amend frozen wording via explicit authority path | Approved amendment record exists before code implementation | Sebastian | N/A |
| S-0B | If descriptive: confirm no frozen amendment required | Written confirmation recorded in this file | Sebastian | CLOSED |

## Required Decision Record Fields

- Date
- Decision owner
- Selected option
- Rationale
- Affected docs
- Affected branch ledgers

## Decision Record

- Date: 2026-03-05
- Decision owner: Sebastian
- Selected option: Descriptive
- Rationale: The ontology §3 parenthetical is interpreted as descriptive freeze-time inventory, not an immutable cap on future scalar payload types. Adding `Series` to Action payloads does not alter trigger gating, DAG semantics, or action terminality.
- Affected docs:
  - `docs/primitives/action.md`
  - `docs/invariants/14-action-registration.md`
  - `docs/contracts/extension-roadmap.md`
- Affected branch ledgers:
  - `docs/ledger/dev-work/closed/series-action-types.md`
  - `docs/ledger/dev-work/closed/series-stdlib.md`
