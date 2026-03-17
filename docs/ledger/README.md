---
Authority: PROJECT
Date: 2026-03-04
Author: Sebastian (Architect) + Codex (Implementation)
Status: Active
---

# Ledger Convention

The ledger is split into three lanes to keep execution work distinct from uncertainty work.

## Lane 1: Dev Work (Delivery)

Use this lane for implementation branches and mergeable engineering work.

- Open items: `docs/ledger/dev-work/open/`
- Closed items: `docs/ledger/dev-work/closed/`
- Each file represents one delivery scope (usually one branch)
- Every file must include a closure table with objective closure conditions

Status flow:

`OPEN -> IN_PROGRESS -> READY_FOR_REVIEW -> CLOSED`

## Lane 2: Gap Work (Risk / Doctrine / Unknowns)

Use this lane for unresolved contradictions, doctrine ambiguities, and blocked semantics.

- Open items: `docs/ledger/gap-work/open/`
- Closed items: `docs/ledger/gap-work/closed/`
- Each file must name the decision owner and exact unblock condition

Status flow:

`OPEN -> DECISION_PENDING -> DECIDED -> CLOSED`

## Lane 3: Decisions (Authority Outcomes)

Use this lane for final rulings that unblock dev work or resolve gaps.

- Decision records: `docs/ledger/decisions/`
- Decision docs are append-only records of authority calls
- Every decision should link to affected dev/gap ledger files

## Non-Negotiable Rules

1. Do not mix dev work and gap work in the same ledger file.
2. A dev ledger row is not CLOSED without code + tests + docs evidence.
3. A gap ledger row is not CLOSED without a recorded decision or implemented resolution.
4. Move files between `open/` and `closed/` only when all row closure conditions are satisfied.
5. Keep `docs/ledger/closure-register.md` as the semantic cross-check index, and link lane files back to it when relevant.
6. If a delivery branch is blocked by unresolved semantics, create a separate gap-work decision file and reference it as a start or merge gate in the dev-work ledger.

## Execution Board

The extension-surface dependency board lives at:

- `docs/ledger/dev-work/closed/extension-surface-order.md`
